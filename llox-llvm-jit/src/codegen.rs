use std::ffi::c_char;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::StructType;
use inkwell::values::{FloatValue, FunctionValue, IntValue, StructValue};
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::OptimizationLevel;

use crate::datastructs::exceptions::CodeGenError;
#[cfg(any(feature = "debug_dump_ir", feature = "debug_dump_assembly"))]
use crate::debug::llvm;
use crate::datastructs::expr::Expr;
use crate::datastructs::literal::Literal;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::token::TokenType;
use crate::runtime::{
    lox_print_value, lox_runtime_error, LoxValue, TAG_BOOL, TAG_NIL, TAG_NUMBER, TAG_STRING,
};

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

    fn declare_lox_runtime_error(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("lox_runtime_error") {
            return function;
        }
        let void_type = self.context.void_type();
        let i32_type = self.context.i32_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let function_type = void_type.fn_type(&[i32_type.into(), ptr_type.into()], false);
        self.module.add_function("lox_runtime_error", function_type, Some(Linkage::External))
    }

    fn build_runtime_error(&self, line: usize, message: &str) -> Result<(), CodeGenError> {
        let lox_runtime_error = self.declare_lox_runtime_error();
        let line_val = self.context.i32_type().const_int(line as u64, false);
        let msg_ptr = self.builder
            .build_global_string_ptr(message, "err_msg")
            .map_err(|error| CodeGenError::Llvm { message: error.to_string() })?
            .as_pointer_value();

        self.builder
            .build_call(lox_runtime_error, &[line_val.into(), msg_ptr.into()], "")
            .map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        Ok(())
    }

    fn lox_value_type(&self) -> StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), self.context.i64_type().into()],
            false,
        )
    }

    fn make_lox_value(&self, tag: u8, bits: IntValue<'ctx>) -> StructValue<'ctx> {
        let ty = self.lox_value_type();
        let tag_value = self.context.i8_type().const_int(tag as u64, false);
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
                    TokenType::Bang => {
                        let is_truthy = self.build_is_truthy(value)?;
                        let not = self.builder.build_not(is_truthy, "not").map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
                        let bits = self.builder.build_int_z_extend(not, self.context.i64_type(), "boolbits").unwrap();
                        Ok(self.make_lox_value(TAG_BOOL, bits))
                    }
                    TokenType::Minus => {
                        self.build_check_type(value, TAG_NUMBER, operator.line(), "Operand must be a number.")?;
                        let num = self.as_f64(value);
                        let neg = self.builder.build_float_neg(num, "neg").map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
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

    fn build_check_type(
        &self,
        val: StructValue<'ctx>,
        expected_tag: u8,
        line: usize,
        error_msg: &str,
    ) -> Result<(), CodeGenError> {
        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "no current LLVM insertion block".into(),
            })?;

        let function = current_block
            .get_parent()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "current block has no parent function".into(),
            })?;

        let error_block = self.context.append_basic_block(function, "type_error");
        let continue_block = self.context.append_basic_block(function, "type_check_continue");

        let tag = self
            .builder
            .build_extract_value(val, 0, "tag")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?
            .into_int_value();

        let expected = self.context.i8_type().const_int(expected_tag as u64, false);

        let matches = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                expected,
                "tag_matches",
            )
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        self.builder
            .build_conditional_branch(matches, continue_block, error_block)
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        self.builder.position_at_end(error_block);

        self.build_runtime_error(line, error_msg)?;

        self.builder
            .build_return(Some(&self.context.i32_type().const_int(70, false)))
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        self.builder.position_at_end(continue_block);
        Ok(())
    }

    fn build_is_truthy(&self, val: StructValue<'ctx>) -> Result<IntValue<'ctx>, CodeGenError> {
        let tag = self
            .builder
            .build_extract_value(val, 0, "tag")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?
            .into_int_value();

        let bits = self
            .builder
            .build_extract_value(val, 1, "bits")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?
            .into_int_value();

        let is_nil = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            self.context.i8_type().const_int(TAG_NIL as u64, false),
            "is_nil",
        ).map_err(|e| CodeGenError::Llvm {
            message: e.to_string(),
        })?;

        let is_false = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            self.context.i8_type().const_int(TAG_BOOL as u64, false),
            "is_bool",
        ).map_err(|e| CodeGenError::Llvm {
            message: e.to_string(),
        })?;

        let is_false_val = self.builder.build_and(
            is_false,
            self.builder.build_int_compare(
                IntPredicate::EQ,
                bits,
                self.context.i64_type().const_int(0, false),
                "is_false_val",
            ).map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?,
            "is_false_and_val",
        ).map_err(|e| CodeGenError::Llvm {
            message: e.to_string(),
        })?;

        let is_falsy = self.builder.build_or(is_nil, is_false_val, "is_falsy").map_err(|e| CodeGenError::Llvm {
            message: e.to_string(),
        })?;

        let is_truthy = self.builder.build_not(is_falsy, "is_truthy").map_err(|e| CodeGenError::Llvm {
            message: e.to_string(),
        })?;

        Ok(is_truthy)
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
            let lox_print_ptr: extern "C" fn(LoxValue) = lox_print_value;
            execution_engine.add_global_mapping(&lox_print, lox_print_ptr as usize);
        }
        if let Some(runtime_error) = self.module.get_function("lox_runtime_error") {
            let runtime_error_ptr: unsafe extern "C" fn(u32, *const c_char) -> () = lox_runtime_error;
            execution_engine.add_global_mapping(
                &runtime_error,
                runtime_error_ptr as usize,
            );
        }
        let function = unsafe { execution_engine.get_function::<unsafe extern "C" fn() -> i32>("llox_main") }.map_err(|error| CodeGenError::Llvm { message: error.to_string()})?;
        Ok(unsafe { function.call() })
    }
}
