use std::ffi::{c_char, c_int, c_void};

use inkwell::OptimizationLevel;

#[cfg(any(feature = "debug_dump_ir", feature = "debug_dump_assembly"))]
use crate::debug::llvm;
use crate::runtime::{
    LoxString, LoxValue, StringHeapGuard, lox_print_value, lox_runtime_error, lox_string_alloc,
    lox_string_concat,
};

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

unsafe extern "C" {
    fn memcmp(a: *const c_void, b: *const c_void, len: usize) -> c_int;
}

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
            let lox_print_ptr: unsafe extern "C" fn(LoxValue) = lox_print_value;
            execution_engine.add_global_mapping(&lox_print, lox_print_ptr as usize);
        }
        if let Some(runtime_error) = self.module.get_function("lox_runtime_error") {
            let runtime_error_ptr: unsafe extern "C" fn(u32, *const c_char) -> () =
                lox_runtime_error;
            execution_engine.add_global_mapping(&runtime_error, runtime_error_ptr as usize);
        }
        // LLVM may replace a zero-only memcmp comparison with bcmp.
        // memcmp also satisfies bcmp's zero/nonzero equality contract.
        for name in ["memcmp", "bcmp"] {
            if let Some(function) = self.module.get_function(name) {
                let ptr: unsafe extern "C" fn(*const c_void, *const c_void, usize) -> c_int =
                    memcmp;
                execution_engine.add_global_mapping(&function, ptr as usize);
            }
        }
        if let Some(function) = self.module.get_function("lox_string_alloc") {
            let ptr: unsafe extern "C" fn(*const u8, usize) -> *mut LoxString = lox_string_alloc;
            execution_engine.add_global_mapping(&function, ptr as usize);
        }
        if let Some(function) = self.module.get_function("lox_string_concat") {
            let ptr: unsafe extern "C" fn(LoxValue, LoxValue) -> LoxValue = lox_string_concat;
            execution_engine.add_global_mapping(&function, ptr as usize);
        }
        let function =
            unsafe { execution_engine.get_function::<unsafe extern "C" fn() -> i32>("llox_main") }
                .map_err(|error| CodeGenError::Llvm {
                    message: error.to_string(),
                })?;
        let _heap = StringHeapGuard::new().map_err(|message| CodeGenError::Llvm {
            message: message.to_string(),
        })?;
        Ok(unsafe { function.call() })
    }
}
