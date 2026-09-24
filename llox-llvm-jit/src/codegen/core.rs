use inkwell::OptimizationLevel;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};

use crate::datastructs::exceptions::CodeGenError;
use crate::datastructs::stmt::Stmt;

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub builder: Builder<'ctx>,
    pub module: Module<'ctx>,
    pub(super) machine: TargetMachine,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context) -> Result<Self, CodeGenError> {
        Target::initialize_native(&InitializationConfig::default()).map_err(|error| {
            CodeGenError::Llvm {
                message: error.to_string(),
            }
        })?;
        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|error| CodeGenError::Llvm {
            message: error.to_string(),
        })?;
        let machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::None,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| CodeGenError::Llvm {
                message: "failed to create native target machine".to_string(),
            })?;
        Ok(CodeGen {
            context,
            builder: context.create_builder(),
            module: context.create_module("llox_module"),
            machine,
        })
    }

    pub fn compile_main(&self, statements: &[Stmt]) -> Result<(), CodeGenError> {
        let i32_type = self.context.i32_type();
        let function_type = i32_type.fn_type(&[], false);
        let function = self.module.add_function("llox_main", function_type, None);
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        for statement in statements {
            self.compile_stmt(statement)?;
        }
        let zero = i32_type.const_int(0, false);
        self.builder
            .build_return(Some(&zero))
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        Ok(())
    }
}
