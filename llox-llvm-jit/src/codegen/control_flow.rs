use inkwell::basic_block::BasicBlock;
use inkwell::values::{FunctionValue, IntValue};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn current_block(&self) -> Result<BasicBlock<'ctx>, CodeGenError> {
        self.builder
            .get_insert_block()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "no current LLVM insertion block".to_string(),
            })
    }

    pub(super) fn current_function(&self) -> Result<FunctionValue<'ctx>, CodeGenError> {
        self.current_block()?
            .get_parent()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "current LLVM block has no parent function".to_string(),
            })
    }

    pub(super) fn append_block(&self, name: &str) -> Result<BasicBlock<'ctx>, CodeGenError> {
        Ok(self
            .context
            .append_basic_block(self.current_function()?, name))
    }

    pub(super) fn branch_if_open(
        &self,
        destination: BasicBlock<'ctx>,
    ) -> Result<bool, CodeGenError> {
        if self.current_block()?.get_terminator().is_some() {
            return Ok(false);
        }
        self.builder
            .build_unconditional_branch(destination)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(true)
    }

    pub(super) fn conditional_branch_if_open(
        &self,
        condition: IntValue<'ctx>,
        then_block: BasicBlock<'ctx>,
        else_block: BasicBlock<'ctx>,
    ) -> Result<(), CodeGenError> {
        if self.current_block()?.get_terminator().is_some() {
            return Err(CodeGenError::Llvm {
                message: "cannot add a conditional branch to a terminated block".to_string(),
            });
        }
        self.builder
            .build_conditional_branch(condition, then_block, else_block)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })
            .map(|_| ())
    }

    pub(super) fn build_guard(
        &self,
        condition: IntValue<'ctx>,
        line: usize,
        message: &str,
    ) -> Result<(), CodeGenError> {
        let error_block = self.append_block("runtime_error")?;
        let continue_block = self.append_block("runtime_continue")?;
        self.conditional_branch_if_open(condition, continue_block, error_block)?;

        self.builder.position_at_end(error_block);
        self.emit_runtime_failure(line, message)?;
        self.builder.position_at_end(continue_block);
        Ok(())
    }

    /// Emit the runtime diagnostic, then route control to the shared main exit.
    pub(super) fn emit_runtime_failure(
        &self,
        line: usize,
        message: &str,
    ) -> Result<(), CodeGenError> {
        let exit = self.runtime_error_exit_block()?;
        self.build_runtime_error(line, message)?;
        self.branch_if_open(exit)?;
        Ok(())
    }

    fn runtime_error_exit_block(&self) -> Result<BasicBlock<'ctx>, CodeGenError> {
        if let Some(block) = *self.runtime_error_exit.borrow() {
            return Ok(block);
        }

        let insertion_block = self.current_block()?;
        let function = insertion_block
            .get_parent()
            .ok_or_else(|| CodeGenError::Llvm {
                message: "current LLVM block has no parent function".to_string(),
            })?;
        let exit = self
            .context
            .append_basic_block(function, "runtime_error_exit");
        self.builder.position_at_end(exit);
        let status = self.context.i32_type().const_int(70, false);
        self.builder
            .build_return(Some(&status))
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        self.builder.position_at_end(insertion_block);
        *self.runtime_error_exit.borrow_mut() = Some(exit);
        Ok(exit)
    }
}
