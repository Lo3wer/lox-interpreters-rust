use inkwell::IntPredicate;
use inkwell::types::StructType;
use inkwell::values::{FloatValue, IntValue, StructValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::runtime::{TAG_BOOL, TAG_NIL};

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn lox_value_type(&self) -> StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i8_type().into(),
                self.context.i64_type().into(),
            ],
            false,
        )
    }

    pub(super) fn make_lox_value(&self, tag: u8, bits: IntValue<'ctx>) -> StructValue<'ctx> {
        let ty = self.lox_value_type();
        let tag_value = self.context.i8_type().const_int(tag as u64, false);
        let mut value = ty.get_undef();
        value = self
            .builder
            .build_insert_value(value, tag_value, 0, "tag")
            .unwrap()
            .into_struct_value();
        value = self
            .builder
            .build_insert_value(value, bits, 1, "bits")
            .unwrap()
            .into_struct_value();
        value
    }

    pub(super) fn as_f64(&self, value: StructValue<'ctx>) -> FloatValue<'ctx> {
        let bits = self
            .builder
            .build_extract_value(value, 1, "bits")
            .unwrap()
            .into_int_value();
        self.builder
            .build_bit_cast(bits, self.context.f64_type(), "num")
            .unwrap()
            .into_float_value()
    }

    pub(super) fn type_of(&self, val: StructValue<'ctx>) -> Result<u8, CodeGenError> {
        let tag = self
            .builder
            .build_extract_value(val, 0, "tag")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?
            .into_int_value();

        let tag_const = self
            .builder
            .build_int_cast(tag, self.context.i8_type(), "tag_const")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        Ok(tag_const
            .get_zero_extended_constant()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "failed to get constant value of tag".into(),
            })? as u8)
    }

    pub(super) fn build_check_type(
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
        let continue_block = self
            .context
            .append_basic_block(function, "type_check_continue");

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
            .build_int_compare(IntPredicate::EQ, tag, expected, "tag_matches")
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

    pub(super) fn build_is_truthy(
        &self,
        val: StructValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
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

        let is_nil = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                self.context.i8_type().const_int(TAG_NIL as u64, false),
                "is_nil",
            )
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        let is_false = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                self.context.i8_type().const_int(TAG_BOOL as u64, false),
                "is_bool",
            )
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        let is_false_val = self
            .builder
            .build_and(
                is_false,
                self.builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        bits,
                        self.context.i64_type().const_int(0, false),
                        "is_false_val",
                    )
                    .map_err(|e| CodeGenError::Llvm {
                        message: e.to_string(),
                    })?,
                "is_false_and_val",
            )
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        let is_falsy = self
            .builder
            .build_or(is_nil, is_false_val, "is_falsy")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })?;

        self.builder
            .build_not(is_falsy, "is_truthy")
            .map_err(|e| CodeGenError::Llvm {
                message: e.to_string(),
            })
    }
}
