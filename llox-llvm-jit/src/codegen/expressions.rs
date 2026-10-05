use inkwell::AddressSpace;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::values::{FloatValue, StructValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::datastructs::expr::Expr;
use crate::datastructs::literal::Literal;
use crate::datastructs::token::{Token, TokenType};
use crate::runtime::{TAG_BOOL, TAG_NIL, TAG_NUMBER, TAG_STRING};

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn compile_expr(&self, expr: &Expr) -> Result<StructValue<'ctx>, CodeGenError> {
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
                    let ptr = self
                        .builder
                        .build_global_string_ptr(string, "str")
                        .map_err(|error| CodeGenError::Llvm {
                            message: error.to_string(),
                        })?
                        .as_pointer_value();
                    let bits = self
                        .builder
                        .build_ptr_to_int(ptr, self.context.i64_type(), "strptr")
                        .map_err(|error| CodeGenError::Llvm {
                            message: error.to_string(),
                        })?;
                    Ok(self.make_lox_value(TAG_STRING, bits))
                }
            },

            Expr::Grouping { expression, .. } => self.compile_expr(expression),

            Expr::Variable { name, .. } => self.build_global_load(name.lexeme(), name.line()),

            Expr::Assign { name, value, .. } => {
                let rhs = self.compile_expr(value)?;
                self.build_global_assign(name.lexeme(), rhs, name.line())
            }

            Expr::Unary {
                operator, right, ..
            } => {
                let value = self.compile_expr(right)?;
                match operator.token_type() {
                    TokenType::Bang => {
                        let is_truthy = self.build_is_truthy(value)?;
                        let not = self.builder.build_not(is_truthy, "not").map_err(|error| {
                            CodeGenError::Llvm {
                                message: error.to_string(),
                            }
                        })?;
                        let bits = self
                            .builder
                            .build_int_z_extend(not, self.context.i64_type(), "boolbits")
                            .map_err(|error| CodeGenError::Llvm {
                                message: error.to_string(),
                            })?;
                        Ok(self.make_lox_value(TAG_BOOL, bits))
                    }
                    TokenType::Minus => {
                        self.build_check_type(
                            value,
                            TAG_NUMBER,
                            operator.line(),
                            "Operand must be a number.",
                        )?;
                        let num = self.as_f64(value);
                        let neg = self.builder.build_float_neg(num, "neg").map_err(|error| {
                            CodeGenError::Llvm {
                                message: error.to_string(),
                            }
                        })?;
                        let bits = self
                            .builder
                            .build_bit_cast(neg, self.context.i64_type(), "numbits")
                            .map_err(|error| CodeGenError::Llvm {
                                message: error.to_string(),
                            })?
                            .into_int_value();
                        Ok(self.make_lox_value(TAG_NUMBER, bits))
                    }
                    _ => Err(CodeGenError::Unsupported {
                        token: None,
                        message: "unsupported unary operator".to_string(),
                    }),
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
                    TokenType::Plus => self.compile_addition(lhs, operator, rhs),
                    TokenType::Minus | TokenType::Star | TokenType::Slash => {
                        self.num_binary_op(lhs, operator, rhs)
                    }
                    TokenType::Less
                    | TokenType::LessEqual
                    | TokenType::Greater
                    | TokenType::GreaterEqual => self.num_compare_op(lhs, operator, rhs),
                    TokenType::EqualEqual | TokenType::BangEqual => {
                        let equal = self.build_values_equal(lhs, rhs)?;
                        let result = if operator.token_type() == TokenType::BangEqual {
                            self.builder.build_not(equal, "neq").map_err(|error| {
                                CodeGenError::Llvm {
                                    message: error.to_string(),
                                }
                            })?
                        } else {
                            equal
                        };
                        let bits = self
                            .builder
                            .build_int_z_extend(result, self.context.i64_type(), "eqbits")
                            .map_err(|error| CodeGenError::Llvm {
                                message: error.to_string(),
                            })?;
                        Ok(self.make_lox_value(TAG_BOOL, bits))
                    }
                    _ => Err(CodeGenError::Unsupported {
                        token: Some(operator.clone()),
                        message: "unsupported binary operator".to_string(),
                    }),
                }
            }

            _ => Err(CodeGenError::Unsupported {
                token: None,
                message: "unsupported expression".to_string(),
            }),
        }
    }

    pub(super) fn num_binary_op(
        &self,
        lhs: StructValue<'ctx>,
        operator: &Token,
        rhs: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let (left, right) = self.checked_numeric_operands(lhs, operator, rhs)?;
        let result = match operator.token_type() {
            TokenType::Minus => self.builder.build_float_sub(left, right, "subtmp"),
            TokenType::Star => self.builder.build_float_mul(left, right, "multmp"),
            TokenType::Slash => self.builder.build_float_div(left, right, "divtmp"),
            _ => {
                return Err(CodeGenError::Unsupported {
                    token: Some(operator.clone()),
                    message: "unsupported numeric binary operator".to_string(),
                });
            }
        }
        .map_err(|error| CodeGenError::Llvm {
            message: error.to_string(),
        })?;

        let bits = self
            .builder
            .build_bit_cast(result, self.context.i64_type(), "numbits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        Ok(self.make_lox_value(TAG_NUMBER, bits))
    }

    fn compile_addition(
        &self,
        lhs: StructValue<'ctx>,
        operator: &Token,
        rhs: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let tag_l = self
            .builder
            .build_extract_value(lhs, 0, "lhs_tag")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        let tag_r = self
            .builder
            .build_extract_value(rhs, 0, "rhs_tag")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();

        let num_tag = self.context.i8_type().const_int(TAG_NUMBER as u64, false);
        let str_tag = self.context.i8_type().const_int(TAG_STRING as u64, false);

        let lhs_num = self
            .builder
            .build_int_compare(IntPredicate::EQ, tag_l, num_tag, "lhs_num")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let rhs_num = self
            .builder
            .build_int_compare(IntPredicate::EQ, tag_r, num_tag, "rhs_num")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let both_num = self
            .builder
            .build_and(lhs_num, rhs_num, "both_num")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        let lhs_str = self
            .builder
            .build_int_compare(IntPredicate::EQ, tag_l, str_tag, "lhs_str")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let rhs_str = self
            .builder
            .build_int_compare(IntPredicate::EQ, tag_r, str_tag, "rhs_str")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let both_str = self
            .builder
            .build_and(lhs_str, rhs_str, "both_str")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "no current LLVM insertion block".to_string(),
            })?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "current block has no parent function".to_string(),
            })?;

        let num_bb = self.context.append_basic_block(function, "add_num");
        let str_check_bb = self.context.append_basic_block(function, "add_str_check");
        let str_bb = self.context.append_basic_block(function, "add_str");
        let err_bb = self.context.append_basic_block(function, "add_err");
        let done_bb = self.context.append_basic_block(function, "add_done");

        self.builder
            .build_conditional_branch(both_num, num_bb, str_check_bb)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(str_check_bb);
        self.builder
            .build_conditional_branch(both_str, str_bb, err_bb)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(num_bb);
        let num_result = self.num_add_unchecked(lhs, rhs)?;
        self.builder
            .build_unconditional_branch(done_bb)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(str_bb);
        let str_result = self.string_concat_unchecked(lhs, rhs)?;
        self.builder
            .build_unconditional_branch(done_bb)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(err_bb);
        self.build_runtime_error(operator.line(), "Operands must be two numbers or two strings.")?;
        self.builder
            .build_return(Some(&self.context.i32_type().const_int(70, false)))
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.lox_value_type(), "add_result")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        phi.add_incoming(&[(&num_result, num_bb), (&str_result, str_bb)]);
        Ok(phi.as_basic_value().into_struct_value())
    }

    fn num_add_unchecked(
        &self,
        lhs: StructValue<'ctx>,
        rhs: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let left = self.as_f64(lhs);
        let right = self.as_f64(rhs);
        let result = self.builder.build_float_add(left, right, "addtmp").map_err(|error| {
            CodeGenError::Llvm {
                message: error.to_string(),
            }
        })?;
        let bits = self
            .builder
            .build_bit_cast(result, self.context.i64_type(), "numbits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        Ok(self.make_lox_value(TAG_NUMBER, bits))
    }

    fn checked_numeric_operands(
        &self,
        lhs: StructValue<'ctx>,
        operator: &Token,
        rhs: StructValue<'ctx>,
    ) -> Result<(FloatValue<'ctx>, FloatValue<'ctx>), CodeGenError> {
        self.build_check_type(
            lhs,
            TAG_NUMBER,
            operator.line(),
            "Operands must be numbers.",
        )?;
        self.build_check_type(
            rhs,
            TAG_NUMBER,
            operator.line(),
            "Operands must be numbers.",
        )?;

        Ok((self.as_f64(lhs), self.as_f64(rhs)))
    }

    fn num_compare_op(
        &self,
        lhs: StructValue<'ctx>,
        operator: &Token,
        rhs: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let (left, right) = self.checked_numeric_operands(lhs, operator, rhs)?;
        let (predicate, name) = match operator.token_type() {
            TokenType::Less => (FloatPredicate::OLT, "cmplt"),
            TokenType::LessEqual => (FloatPredicate::OLE, "cmplte"),
            TokenType::Greater => (FloatPredicate::OGT, "cmpgt"),
            TokenType::GreaterEqual => (FloatPredicate::OGE, "cmpgte"),
            _ => {
                return Err(CodeGenError::Unsupported {
                    token: Some(operator.clone()),
                    message: "unsupported comparison operator".to_string(),
                });
            }
        };

        let result = self
            .builder
            .build_float_compare(predicate, left, right, name)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let bits = self
            .builder
            .build_int_z_extend(result, self.context.i64_type(), "boolbits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        Ok(self.make_lox_value(TAG_BOOL, bits))
    }

    fn string_concat_unchecked(
        &self,
        lhs: StructValue<'ctx>,
        rhs: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let left_bits = self
            .builder
            .build_extract_value(lhs, 1, "left_string_bits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        let right_bits = self
            .builder
            .build_extract_value(rhs, 1, "right_string_bits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        let left = self
            .builder
            .build_int_to_ptr(left_bits, ptr_type, "left_string")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let right = self
            .builder
            .build_int_to_ptr(right_bits, ptr_type, "right_string")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let result = self
            .builder
            .build_call(
                self.declare_lox_concat_strings(),
                &[left.into(), right.into()],
                "concat",
            )
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "string concatenation returned no value".to_string(),
            })?
            .into_pointer_value();
        let bits = self
            .builder
            .build_ptr_to_int(result, self.context.i64_type(), "string_bits")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(self.make_lox_value(TAG_STRING, bits))
    }
}
