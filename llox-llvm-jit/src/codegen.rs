use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::StructType;
use inkwell::values::{FloatValue, FunctionValue, IntValue, StructValue};
use inkwell::OptimizationLevel;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use crate::datastructs::exceptions::CodeGenError;
#[cfg(any(feature = "debug_dump_ir", feature = "debug_dump_assembly"))]
use crate::debug::llvm;
use crate::datastructs::expr::Expr;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::literal::Literal;
use crate::datastructs::token::TokenType;

const TAG_NUMBER: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_NIL: u64 = 2;
const TAG_STRING: u64 = 3;

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub builder: Builder<'ctx>,
    pub module: Module<'ctx>,
    machine: TargetMachine,
}

impl<'ctx> CodeGen<'ctx> {

    pub fn new(context: &'ctx Context) -> Result<Self, CodeGenError> {
        Target::initialize_native(&InitializationConfig::default()).map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        let machine = target.create_target_machine(&triple, "generic", "", OptimizationLevel::None, RelocMode::Default, CodeModel::Default)
            .ok_or_else(|| CodeGenError::Llvm { message: "failed to create native target machine".to_string() })?;
        Ok(CodeGen {
            context,
            builder: context.create_builder(),
            module: context.create_module("llox_module"),
            machine,
        })
    }

    pub fn compile_main(&self, statements: &[Stmt]) -> Result<(), CodeGenError> {
        let i32_type = self.context.i32_type();
        let function_type = i32_type.fn_type(&[], false);
        let function = self.module.add_function("llox_main", function_type, None);
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        for statement in statements {
            self.compile_stmt(statement)?;
        }
        let zero = i32_type.const_int(0, false);
        self.builder.build_return(Some(&zero)).map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        Ok(())
    }

    pub fn compile_stmt(&self, statement: &Stmt) -> Result<(), CodeGenError> {
        match statement {
            Stmt::Print { expression } => {
                let value = self.compile_expr(expression)?;
                self.build_print(value)
            }
            _ => Err(CodeGenError::Unsupported { token: None, message: "unsupported statement".to_string() }),
        }
    }

    fn declare_lox_print(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("lox_print_value") {
            return function;
        }
        let void_type = self.context.void_type();
        let function_type = void_type.fn_type(&[self.lox_value_type().into()], false);
        self.module.add_function("lox_print_value", function_type, Some(Linkage::External))
    }

    fn build_print(&self, value: StructValue<'ctx>) -> Result<(), CodeGenError> {
        let lox_print = self.declare_lox_print();
        self.builder
            .build_call(lox_print, &[value.into()], "")
            .map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        Ok(())
    }

    fn lox_value_type(&self) -> StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), self.context.i64_type().into()],
            false,
        )
    }

    fn make_lox_value(&self, tag: u64, bits: IntValue<'ctx>) -> StructValue<'ctx> {
        let ty = self.lox_value_type();
        let tag_value = self.context.i8_type().const_int(tag, false);
        let mut value = ty.get_undef();
        value = self.builder.build_insert_value(value, tag_value, 0, "tag").unwrap().into_struct_value();
        value = self.builder.build_insert_value(value, bits, 1, "bits").unwrap().into_struct_value();
        value
    }

    fn as_f64(&self, value: StructValue<'ctx>) -> FloatValue<'ctx> {
        let bits = self.builder.build_extract_value(value, 1, "bits").unwrap().into_int_value();
        self.builder.build_bit_cast(bits, self.context.f64_type(), "num").unwrap().into_float_value()
    }

    fn compile_expr(&self, expr: &Expr) -> Result<StructValue<'ctx>, CodeGenError> {
        // ignore id with ..
        match expr {
            Expr::Literal { value, .. } => match value {
                Literal::Number(number) => {
                    let bits = self.context.i64_type().const_int(number.to_bits(), false);
                    Ok(self.make_lox_value(TAG_NUMBER, bits))
                }
                Literal::Bool(boolean) => {
                    let bits = self.context.i64_type().const_int(*boolean as u64, false);
                    Ok(self.make_lox_value(TAG_BOOL, bits))
                }
                Literal::Nil => {
                    let bits = self.context.i64_type().const_int(0, false);
                    Ok(self.make_lox_value(TAG_NIL, bits))
                }
                Literal::String(string) => {
                    let ptr = self.builder.build_global_string_ptr(string, "str").unwrap().as_pointer_value();
                    let bits = self.builder.build_ptr_to_int(ptr, self.context.i64_type(), "strptr").unwrap();
                    Ok(self.make_lox_value(TAG_STRING, bits))
                }
            },

            Expr::Grouping { expression, .. } => {
                self.compile_expr(expression)
            }

            Expr::Unary { operator, right, .. } => {
                let value = self.compile_expr(right)?;

                match operator.token_type() {
                    TokenType::Minus => {
                        let num = self.as_f64(value);
                        let neg = self.builder.build_float_neg(num, "negtmp").map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
                        let bits = self.builder.build_bit_cast(neg, self.context.i64_type(), "numbits").unwrap().into_int_value();
                        Ok(self.make_lox_value(TAG_NUMBER, bits))
                    }

                    _ => Err(CodeGenError::Unsupported { token: None, message: "unsupported unary operator".to_string()}),
                }
            }

            Expr::Binary {
                left,
                operator,
                right,
                ..
            } => {
                let lhs = self.compile_expr(left)?;
                let rhs = self.compile_expr(right)?;
                let l = self.as_f64(lhs);
                let r = self.as_f64(rhs);

                let result = match operator.token_type() {
                    TokenType::Plus => self.builder.build_float_add(l, r, "addtmp"),
                    TokenType::Minus => self.builder.build_float_sub(l, r, "subtmp"),
                    TokenType::Star => self.builder.build_float_mul(l, r, "multmp"),
                    TokenType::Slash => self.builder.build_float_div(l, r, "divtmp"),
                    _ => return Err(CodeGenError::Unsupported { token: None, message: "unsupported binary operator".to_string() }),
                }.map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;

                let bits = self.builder.build_bit_cast(result, self.context.i64_type(), "numbits").unwrap().into_int_value();
                Ok(self.make_lox_value(TAG_NUMBER, bits))
            }

            _ => Err(CodeGenError::Unsupported { token: None, message: "unsupported expression".to_string() }),
        }
    }

    #[cfg(feature = "debug_dump_ir")]
    pub fn dump_ir(&self) {
        llvm::dump_ir(&self.module);
    }

    #[cfg(feature = "debug_dump_assembly")]
    pub fn dump_assembly(&self) {
        llvm::dump_assembly(&self.module, &self.machine);
    }

    pub unsafe fn run(&self) -> Result<i32, CodeGenError> {
        let execution_engine = self.module.create_jit_execution_engine(OptimizationLevel::None).map_err(|error| CodeGenError::Llvm { message: error.to_string()})?;
        if let Some(lox_print) = self.module.get_function("lox_print_value") {
            let lox_print_ptr: extern "C" fn(crate::runtime::LoxValue) = crate::runtime::lox_print_value;
            execution_engine.add_global_mapping(&lox_print, lox_print_ptr as usize);
        }
        let function = unsafe { execution_engine.get_function::<unsafe extern "C" fn() -> i32>("llox_main") }.map_err(|error| CodeGenError::Llvm { message: error.to_string()})?;
        Ok(unsafe { function.call() })
    }
}
