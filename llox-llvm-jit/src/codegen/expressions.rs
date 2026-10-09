use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::module::Linkage;
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
                    // Byte constants preserve interior NULs. The extra terminator
                    // gives empty literals backing storage but is not copied.
                    let bytes = self.context.const_string(string.as_bytes(), true);
                    let global = self.module.add_global(bytes.get_type(), None, "str_bytes");
                    global.set_linkage(Linkage::Private);
                    global.set_constant(true);
                    global.set_initializer(&bytes);
                    let length = self.usize_type().const_int(string.len() as u64, false);
                    let object = self
                        .builder
                        .build_call(
                            self.declare_lox_string_alloc(),
                            &[global.as_pointer_value().into(), length.into()],
                            "str_alloc",
                        )?
                        .try_as_basic_value()
                        .basic()
                        .ok_or_else(|| CodeGenError::Llvm {
                            message: "string allocation returned no value".to_string(),
                        })?
                        .into_pointer_value();
                    let bits =
                        self.builder
                            .build_ptr_to_int(object, self.context.i64_type(), "strptr")?;
                    Ok(self.make_lox_value(TAG_STRING, bits))
                }
            },

            Expr::Grouping { expression, .. } => self.compile_expr(expression),

            Expr::Logical {
                left,
                operator,
                right,
                ..
            } => self.compile_logical(left, operator, right),

            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.compile_ternary(condition, then_branch, else_branch),

            Expr::Variable { name, id } => {
                if let Some(slot) = self.resolved_local(*id, name.lexeme())? {
                    Ok(self
                        .builder
                        .build_load(self.lox_value_type(), slot, name.lexeme())
                        .map_err(|error| CodeGenError::Llvm {
                            message: error.to_string(),
                        })?
                        .into_struct_value())
                } else {
                    self.build_global_load(name.lexeme(), name.line())
                }
            }

            Expr::Assign { name, value, id } => {
                let rhs = self.compile_expr(value)?;
                if let Some(slot) = self.resolved_local(*id, name.lexeme())? {
                    self.builder
                        .build_store(slot, rhs)
                        .map_err(|error| CodeGenError::Llvm {
                            message: error.to_string(),
                        })?;
                    Ok(rhs)
                } else {
                    self.build_global_assign(name.lexeme(), rhs, name.line())
                }
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

    fn compile_logical(
        &self,
        left: &Expr,
        operator: &Token,
        right: &Expr,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let left_value = self.compile_expr(left)?;
        let is_truthy = self.build_is_truthy(left_value)?;
        let branch_block = self.current_block()?;
        let right_block = self.append_block("logical_rhs")?;
        let merge_block = self.append_block("logical_merge")?;

        match operator.token_type() {
            TokenType::And => self.conditional_branch_if_open(is_truthy, right_block, merge_block),
            TokenType::Or => self.conditional_branch_if_open(is_truthy, merge_block, right_block),
            _ => {
                return Err(CodeGenError::Unsupported {
                    token: Some(operator.clone()),
                    message: "unsupported logical operator".to_string(),
                });
            }
        }?;

        self.builder.position_at_end(right_block);
        let right_value = self.compile_expr(right)?;
        let right_end = self.current_block()?;
        if !self.branch_if_open(merge_block)? {
            return Err(CodeGenError::Llvm {
                message: "logical right operand has no fallthrough value".to_string(),
            });
        }

        self.builder.position_at_end(merge_block);
        let result = self
            .builder
            .build_phi(self.lox_value_type(), "logical_result")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        result.add_incoming(&[(&left_value, branch_block), (&right_value, right_end)]);
        Ok(result.as_basic_value().into_struct_value())
    }

    fn compile_ternary(
        &self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Expr,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let condition_value = self.compile_expr(condition)?;
        let condition_truthy = self.build_is_truthy(condition_value)?;
        let then_block = self.append_block("ternary_then")?;
        let else_block = self.append_block("ternary_else")?;
        let merge_block = self.append_block("ternary_merge")?;
        self.conditional_branch_if_open(condition_truthy, then_block, else_block)?;

        self.builder.position_at_end(then_block);
        let then_value = self.compile_expr(then_branch)?;
        let then_end = self.current_block()?;
        if !self.branch_if_open(merge_block)? {
            return Err(CodeGenError::Llvm {
                message: "ternary then expression has no fallthrough value".to_string(),
            });
        }

        self.builder.position_at_end(else_block);
        let else_value = self.compile_expr(else_branch)?;
        let else_end = self.current_block()?;
        if !self.branch_if_open(merge_block)? {
            return Err(CodeGenError::Llvm {
                message: "ternary else expression has no fallthrough value".to_string(),
            });
        }

        self.builder.position_at_end(merge_block);
        let result = self
            .builder
            .build_phi(self.lox_value_type(), "ternary_result")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        result.add_incoming(&[(&then_value, then_end), (&else_value, else_end)]);
        Ok(result.as_basic_value().into_struct_value())
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
        let tag = self.value_tag(lhs)?;
        let same_tag = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            self.value_tag(rhs)?,
            "add_same_tag",
        )?;
        let dispatch = self.append_block("add_dispatch")?;
        let num_bb = self.append_block("add_num")?;
        let str_bb = self.append_block("add_str")?;
        let err_bb = self.append_block("add_err")?;
        let done_bb = self.append_block("add_done")?;

        self.conditional_branch_if_open(same_tag, dispatch, err_bb)?;
        self.builder.position_at_end(dispatch);
        self.builder.build_switch(
            tag,
            err_bb,
            &[
                (
                    self.context.i8_type().const_int(TAG_NUMBER as u64, false),
                    num_bb,
                ),
                (
                    self.context.i8_type().const_int(TAG_STRING as u64, false),
                    str_bb,
                ),
            ],
        )?;

        self.builder.position_at_end(num_bb);
        let num_result = self.num_add_unchecked(lhs, rhs)?;
        self.branch_if_open(done_bb)?;

        self.builder.position_at_end(str_bb);
        let str_result = self.string_concat_unchecked(lhs, rhs)?;
        self.branch_if_open(done_bb)?;

        self.builder.position_at_end(err_bb);
        self.emit_runtime_failure(
            operator.line(),
            "Operands must be two numbers or two strings.",
        )?;

        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.lox_value_type(), "add_result")?;
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
        let result = self.builder.build_float_add(left, right, "addtmp")?;
        let bits = self
            .builder
            .build_bit_cast(result, self.context.i64_type(), "numbits")?
            .into_int_value();
        Ok(self.make_lox_value(TAG_NUMBER, bits))
    }

    fn checked_numeric_operands(
        &self,
        lhs: StructValue<'ctx>,
        operator: &Token,
        rhs: StructValue<'ctx>,
    ) -> Result<(FloatValue<'ctx>, FloatValue<'ctx>), CodeGenError> {
        let lhs_tag = self.value_tag(lhs)?;
        let rhs_tag = self.value_tag(rhs)?;
        let number_tag = self.context.i8_type().const_int(TAG_NUMBER as u64, false);
        let lhs_is_number = self
            .builder
            .build_int_compare(IntPredicate::EQ, lhs_tag, number_tag, "lhs_is_number")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let rhs_is_number = self
            .builder
            .build_int_compare(IntPredicate::EQ, rhs_tag, number_tag, "rhs_is_number")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        let both_are_numbers = self
            .builder
            .build_and(lhs_is_number, rhs_is_number, "both_are_numbers")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        self.build_guard(
            both_are_numbers,
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
        let result = self
            .builder
            .build_call(
                self.declare_lox_string_concat(),
                &[lhs.into(), rhs.into()],
                "concat",
            )?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "string concatenation returned no value".to_string(),
            })?
            .into_struct_value();
        Ok(result)
    }
}
