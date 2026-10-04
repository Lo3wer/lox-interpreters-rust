use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::values::{FunctionValue, IntValue, StructValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

#[derive(Clone, Copy)]
pub(super) enum RuntimeErrorKind {
    OperandMustBeNumber,
    OperandsMustBeNumbers,
    OperandsMustBeStrings,
    OperandsMustBeNumbersOrStrings,
}

impl RuntimeErrorKind {
    fn message(self) -> &'static str {
        match self {
            Self::OperandMustBeNumber => "Operand must be a number.",
            Self::OperandsMustBeNumbers => "Operands must be numbers.",
            Self::OperandsMustBeStrings => "Operands must be two strings.",
            Self::OperandsMustBeNumbersOrStrings => "Operands must be two numbers or two strings.",
        }
    }

    fn global_name(self) -> &'static str {
        match self {
            Self::OperandMustBeNumber => "err_msg_operand_number",
            Self::OperandsMustBeNumbers => "err_msg_operands_numbers",
            Self::OperandsMustBeStrings => "err_msg_operands_strings",
            Self::OperandsMustBeNumbersOrStrings => "err_msg_operands_strings_numbers",
        }
    }
}

impl<'ctx> CodeGen<'ctx> {
    fn declare_lox_print(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("lox_print_value") {
            return function;
        }
        let void_type = self.context.void_type();
        let function_type = void_type.fn_type(&[self.lox_value_type().into()], false);
        self.module
            .add_function("lox_print_value", function_type, Some(Linkage::External))
    }

    pub(super) fn build_print(&self, value: StructValue<'ctx>) -> Result<(), CodeGenError> {
        let lox_print = self.declare_lox_print();
        self.builder
            .build_call(lox_print, &[value.into()], "")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
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
        self.module
            .add_function("lox_runtime_error", function_type, Some(Linkage::External))
    }

    pub(super) fn declare_lox_values_equal(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("lox_values_equal") {
            return function;
        }
        let bool_type = self.context.bool_type();
        let function_type = bool_type.fn_type(
            &[self.lox_value_type().into(), self.lox_value_type().into()],
            false,
        );
        self.module
            .add_function("lox_values_equal", function_type, Some(Linkage::External))
    }

    pub(super) fn build_values_equal(
        &self,
        lhs: StructValue<'ctx>,
        rhs: StructValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        let lox_values_equal = self.declare_lox_values_equal();
        Ok(self
            .builder
            .build_call(lox_values_equal, &[lhs.into(), rhs.into()], "eq")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "equality comparison returned no value".to_string(),
            })?
            .into_int_value())
    }

    pub(super) fn declare_lox_concat_strings(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("lox_concat_strings") {
            return function;
        }
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let function_type = ptr_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        self.module
            .add_function("lox_concat_strings", function_type, Some(Linkage::External))
    }

    pub(super) fn build_runtime_error(
        &self,
        line: usize,
        error_kind: RuntimeErrorKind,
    ) -> Result<(), CodeGenError> {
        let lox_runtime_error = self.declare_lox_runtime_error();
        let line_val = self.context.i32_type().const_int(line as u64, false);
        let message_name = error_kind.global_name();
        let msg_ptr = match self.module.get_global(message_name) {
            Some(global) => global.as_pointer_value(),
            None => self
                .builder
                .build_global_string_ptr(error_kind.message(), message_name)
                .map_err(|error| CodeGenError::Llvm {
                    message: error.to_string(),
                })?
                .as_pointer_value(),
        };

        self.builder
            .build_call(lox_runtime_error, &[line_val.into(), msg_ptr.into()], "")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(())
    }
}
