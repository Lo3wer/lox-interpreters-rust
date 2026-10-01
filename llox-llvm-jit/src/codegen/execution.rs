use std::ffi::c_char;

use inkwell::OptimizationLevel;

#[cfg(any(feature = "debug_dump_ir", feature = "debug_dump_assembly"))]
use crate::debug::llvm;
use crate::runtime::{LoxValue, lox_concat_strings, lox_print_value, lox_runtime_error};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

impl<'ctx> CodeGen<'ctx> {
    #[cfg(feature = "debug_dump_ir")]
    pub fn dump_ir(&self) {
        llvm::dump_ir(&self.module);
    }

    #[cfg(feature = "debug_dump_assembly")]
    pub fn dump_assembly(&self) {
        llvm::dump_assembly(&self.module, &self.machine);
    }

    pub unsafe fn run(&self) -> Result<i32, CodeGenError> {
        let execution_engine = self
            .module
            .create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        if let Some(lox_print) = self.module.get_function("lox_print_value") {
            let lox_print_ptr: extern "C" fn(LoxValue) = lox_print_value;
            execution_engine.add_global_mapping(&lox_print, lox_print_ptr as usize);
        }
        if let Some(runtime_error) = self.module.get_function("lox_runtime_error") {
            let runtime_error_ptr: unsafe extern "C" fn(u32, *const c_char) -> () =
                lox_runtime_error;
            execution_engine.add_global_mapping(&runtime_error, runtime_error_ptr as usize);
        }
        if let Some(concat_strings) = self.module.get_function("lox_concat_strings") {
            let concat_strings_ptr: extern "C" fn(*const c_char, *const c_char) -> *mut c_char =
                lox_concat_strings;
            execution_engine.add_global_mapping(&concat_strings, concat_strings_ptr as usize);
        }
        let function =
            unsafe { execution_engine.get_function::<unsafe extern "C" fn() -> i32>("llox_main") }
                .map_err(|error| CodeGenError::Llvm {
                    message: error.to_string(),
                })?;
        Ok(unsafe { function.call() })
    }
}
