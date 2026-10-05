use inkwell::IntPredicate;
use inkwell::module::Linkage;
use inkwell::values::{GlobalValue, StructValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::runtime::TAG_UNDEFINED;

impl<'ctx> CodeGen<'ctx> {
    fn get_or_create_global(&self, name: &str) -> GlobalValue<'ctx> {
        if let Some(global) = self.globals.borrow().get(name) {
            return *global;
        }
        let global = self
            .module
            .add_global(self.lox_value_type(), None, &format!("g_{name}"));
        global.set_linkage(Linkage::Internal);
        let init = self.lox_value_type().const_named_struct(&[
            self.context.i8_type().const_int(TAG_UNDEFINED as u64, false).into(),
            self.context.i64_type().const_int(0, false).into(),
        ]);
        global.set_initializer(&init);
        self.globals.borrow_mut().insert(name.to_string(), global);
        global
    }

    fn check_defined(
        &self,
        global: GlobalValue<'ctx>,
        name: &str,
        line: usize,
    ) -> Result<(), CodeGenError> {
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

        let error_block = self.context.append_basic_block(function, "undef_var");
        let continue_block = self.context.append_basic_block(function, "defined_var");

        let value = self
            .builder
            .build_load(self.lox_value_type(), global.as_pointer_value(), name)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_struct_value();
        let tag = self
            .builder
            .build_extract_value(value, 0, "tag")
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_int_value();
        let is_undefined = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                self.context.i8_type().const_int(TAG_UNDEFINED as u64, false),
                "is_undefined",
            )
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder
            .build_conditional_branch(is_undefined, error_block, continue_block)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(error_block);
        let message = format!("Undefined variable '{name}'.");
        self.build_runtime_error(line, &message)?;
        self.builder
            .build_return(Some(&self.context.i32_type().const_int(70, false)))
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;

        self.builder.position_at_end(continue_block);
        Ok(())
    }

    pub(super) fn build_global_define(
        &self,
        name: &str,
        value: StructValue<'ctx>,
    ) -> Result<(), CodeGenError> {
        let global = self.get_or_create_global(name);
        self.builder
            .build_store(global.as_pointer_value(), value)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(())
    }

    pub(super) fn build_global_load(
        &self,
        name: &str,
        line: usize,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let global = self.get_or_create_global(name);
        self.check_defined(global, name, line)?;
        let value = self
            .builder
            .build_load(self.lox_value_type(), global.as_pointer_value(), name)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?
            .into_struct_value();
        Ok(value)
    }

    pub(super) fn build_global_assign(
        &self,
        name: &str,
        value: StructValue<'ctx>,
        line: usize,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let global = self.get_or_create_global(name);
        self.check_defined(global, name, line)?;
        self.builder
            .build_store(global.as_pointer_value(), value)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(value)
    }
}
