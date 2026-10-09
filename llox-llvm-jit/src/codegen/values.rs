use inkwell::types::{IntType, StructType};
use inkwell::values::{FloatValue, IntValue, PointerValue, StructValue};
use inkwell::{AddressSpace, IntPredicate};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::runtime::{TAG_BOOL, TAG_NIL};

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn usize_type(&self) -> IntType<'ctx> {
        self.context
            .ptr_sized_int_type(&self.machine.get_target_data(), None)
    }

    pub(super) fn lox_string_type(&self) -> StructType<'ctx> {
        self.context.struct_type(
            &[
                self.usize_type().into(),
                self.context.ptr_type(AddressSpace::default()).into(),
            ],
            false,
        )
    }

    pub(super) fn value_tag(
        &self,
        value: StructValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        Ok(self
            .builder
            .build_extract_value(value, 0, "tag")?
            .into_int_value())
    }

    pub(super) fn value_bits(
        &self,
        value: StructValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        Ok(self
            .builder
            .build_extract_value(value, 1, "bits")?
            .into_int_value())
    }

    pub(super) fn object_pointer(
        &self,
        value: StructValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        Ok(self.builder.build_int_to_ptr(
            self.value_bits(value)?,
            self.context.ptr_type(AddressSpace::default()),
            "object",
        )?)
    }

    pub(super) fn string_length(
        &self,
        object: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        let field =
            self.builder
                .build_struct_gep(self.lox_string_type(), object, 0, "length_ptr")?;
        Ok(self
            .builder
            .build_load(self.usize_type(), field, "length")?
            .into_int_value())
    }

    pub(super) fn string_chars(
        &self,
        object: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let field =
            self.builder
                .build_struct_gep(self.lox_string_type(), object, 1, "chars_ptr")?;
        Ok(self
            .builder
            .build_load(
                self.context.ptr_type(AddressSpace::default()),
                field,
                "chars",
            )?
            .into_pointer_value())
    }

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

    pub(super) fn build_check_type(
        &self,
        val: StructValue<'ctx>,
        expected_tag: u8,
        line: usize,
        message: &str,
    ) -> Result<(), CodeGenError> {
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
        self.build_guard(matches, line, message)
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
