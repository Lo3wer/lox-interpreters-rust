use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::values::{FunctionValue, StructValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

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
        message: &str,
    ) -> Result<(), CodeGenError> {
        let lox_runtime_error = self.declare_lox_runtime_error();
        let line_val = self.context.i32_type().const_int(line as u64, false);
        let msg_ptr = self
            .builder
            .build_global_string_ptr(message, "err_msg")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .as_pointer_value();

        self.builder
            .build_call(lox_runtime_error, &[line_val.into(), msg_ptr.into()], "")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(())
    }
}
