use inkwell::values::{IntValue, StructValue};
use inkwell::{FloatPredicate, IntPredicate};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::runtime::{TAG_NIL, TAG_NUMBER, TAG_STRING};

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn build_values_equal(
        &self,
        lhs: StructValue<'ctx>,
        rhs: StructValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        let left_tag = self.value_tag(lhs)?;
        let same_tag = self.builder.build_int_compare(
            IntPredicate::EQ,
            left_tag,
            self.value_tag(rhs)?,
            "same_tag",
        )?;
        let dispatch = self.append_block("eq_dispatch")?;
        let yes = self.append_block("eq_true")?;
        let no = self.append_block("eq_false")?;
        let number = self.append_block("eq_number")?;
        let string = self.append_block("eq_string")?;
        let identity = self.append_block("eq_identity")?;
        let done = self.append_block("eq_done")?;
        self.conditional_branch_if_open(same_tag, dispatch, no)?;

        self.builder.position_at_end(dispatch);
        let tag = |tag| self.context.i8_type().const_int(tag as u64, false);
        self.builder.build_switch(
            left_tag,
            identity,
            &[
                (tag(TAG_NIL), yes),
                (tag(TAG_NUMBER), number),
                (tag(TAG_STRING), string),
            ],
        )?;

        self.builder.position_at_end(number);
        let numeric_equal = self.builder.build_float_compare(
            FloatPredicate::OEQ,
            self.as_f64(lhs),
            self.as_f64(rhs),
            "numeric_equal",
        )?;
        self.conditional_branch_if_open(numeric_equal, yes, no)?;

        // Booleans and other object tags use payload equality.
        self.builder.position_at_end(identity);
        let identical = self.builder.build_int_compare(
            IntPredicate::EQ,
            self.value_bits(lhs)?,
            self.value_bits(rhs)?,
            "identical",
        )?;
        self.conditional_branch_if_open(identical, yes, no)?;

        self.builder.position_at_end(string);
        let left = self.object_pointer(lhs)?;
        let right = self.object_pointer(rhs)?;
        let same_object =
            self.builder
                .build_int_compare(IntPredicate::EQ, left, right, "same_object")?;
        let lengths = self.append_block("eq_lengths")?;
        let empty = self.append_block("eq_empty")?;
        let bytes = self.append_block("eq_bytes")?;
        self.conditional_branch_if_open(same_object, yes, lengths)?;

        self.builder.position_at_end(lengths);
        let length = self.string_length(left)?;
        let same_length = self.builder.build_int_compare(
            IntPredicate::EQ,
            length,
            self.string_length(right)?,
            "same_length",
        )?;
        self.conditional_branch_if_open(same_length, empty, no)?;

        self.builder.position_at_end(empty);
        let is_empty = self.builder.build_int_compare(
            IntPredicate::EQ,
            length,
            self.usize_type().const_zero(),
            "is_empty",
        )?;
        self.conditional_branch_if_open(is_empty, yes, bytes)?;

        self.builder.position_at_end(bytes);
        let comparison = self
            .builder
            .build_call(
                self.declare_memcmp(),
                &[
                    self.string_chars(left)?.into(),
                    self.string_chars(right)?.into(),
                    length.into(),
                ],
                "memcmp",
            )?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "memcmp returned no value".to_string(),
            })?
            .into_int_value();
        let same_bytes = self.builder.build_int_compare(
            IntPredicate::EQ,
            comparison,
            self.context.i32_type().const_zero(),
            "same_bytes",
        )?;
        self.conditional_branch_if_open(same_bytes, yes, no)?;

        self.builder.position_at_end(yes);
        self.branch_if_open(done)?;
        self.builder.position_at_end(no);
        self.branch_if_open(done)?;
        self.builder.position_at_end(done);
        let result = self.builder.build_phi(self.context.bool_type(), "equal")?;
        result.add_incoming(&[
            (&self.context.bool_type().const_int(1, false), yes),
            (&self.context.bool_type().const_zero(), no),
        ]);
        Ok(result.as_basic_value().into_int_value())
    }
}
