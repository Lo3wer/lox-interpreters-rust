use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::FloatValue;
use inkwell::OptimizationLevel;
use inkwell::targets::{Target, InitializationConfig};
use crate::datastructs::exceptions::{CodeGenError, RuntimeException};
use crate::datastructs::expr::Expr;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::literal::Literal;
use crate::datastructs::token::TokenType;

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub builder: Builder<'ctx>,
    pub module: Module<'ctx>,
}

impl<'ctx> CodeGen<'ctx> {

    pub fn new(context: &'ctx Context) -> Self {
        CodeGen {
            context,
            builder: context.create_builder(),
            module: context.create_module("llox_module"),
        }
    }

    pub fn compile_main(&self, expr: &Expr) -> Result<(), CodeGenError> {
        let f64_type = self.context.f64_type();
        let function_type = f64_type.fn_type(&[], false);
        let function = self.module.add_function("llox_main", function_type, None);
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let result = self.compile_expr(expr)?;
        self.builder.build_return(Some(&result)).map_err(|error| CodeGenError::Llvm { message: error.to_string() })?;
        Ok(())
    }

    pub fn compile_stmt(&self, statement: &Stmt) -> Result<(), CodeGenError> {
        match statement {
            Stmt::Print { expression } => {
                self.compile_main(expression)
            }
            _ => Err(CodeGenError::Llvm { message: "unsupported statement".to_string() }),
        }
    }

    fn compile_expr(&self, expr: &Expr) -> Result<FloatValue<'ctx>, CodeGenError> {
        // ignore id with ..
        match expr {
            Expr::Literal {
                value: Literal::Number(number),
                ..
            } => Ok(self.context.f64_type().const_float(*number)),

            Expr::Grouping { expression, .. } => {
                self.compile_expr(expression)
            }

            Expr::Unary { operator, right, .. } => {
                let value = self.compile_expr(right)?;

                match operator.token_type() {
                    TokenType::Minus => self
                        .builder
                        .build_float_neg(value, "negtmp")
                        .map_err(|error| CodeGenError::Llvm { message: error.to_string() }),

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

                match operator.token_type() {
                    TokenType::Plus => self
                        .builder
                        .build_float_add(lhs, rhs, "addtmp")
                        .map_err(|error| CodeGenError::Llvm { message: error.to_string() }),

                    TokenType::Minus => self
                        .builder
                        .build_float_sub(lhs, rhs, "subtmp")
                        .map_err(|error| CodeGenError::Llvm { message: error.to_string() }),

                    TokenType::Star => self
                        .builder
                        .build_float_mul(lhs, rhs, "multmp")
                        .map_err(|error| CodeGenError::Llvm { message: error.to_string() }),

                    TokenType::Slash => self
                        .builder
                        .build_float_div(lhs, rhs, "divtmp")
                        .map_err(|error| CodeGenError::Llvm { message: error.to_string() }),

                    _ => Err(CodeGenError::Unsupported { token: None, message: "unsupported binary operator".to_string() }),
                }
            }

            _ => Err(CodeGenError::Unsupported { token: None, message: "unsupported expression".to_string() }),
        }
    }

    pub unsafe fn run(&self) -> Result<f64, RuntimeException> {
        Target::initialize_native(&InitializationConfig::default());
        let execution_engine = self.module.create_jit_execution_engine(OptimizationLevel::None).map_err(|error| RuntimeException::Llvm { message: error.to_string()})?;
        let function = execution_engine.get_function::<unsafe extern "C" fn() -> f64>("llox_main").map_err(|error| RuntimeException::Llvm { message: error.to_string()})?;
        Ok(function.call())
    }
}