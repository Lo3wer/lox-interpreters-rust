use std::cell::RefCell;
use std::collections::HashMap;

use inkwell::OptimizationLevel;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::values::GlobalValue;

use super::scopes::ScopeTable;
use crate::datastructs::exceptions::CodeGenError;
use crate::datastructs::stmt::Stmt;

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub builder: Builder<'ctx>,
    pub module: Module<'ctx>,
    pub(super) machine: TargetMachine,
    pub(super) globals: RefCell<HashMap<String, GlobalValue<'ctx>>>,
    pub(super) locals: HashMap<usize, usize>,
    pub(super) scopes: RefCell<ScopeTable<'ctx>>,
    pub(super) runtime_error_exit: RefCell<Option<BasicBlock<'ctx>>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(
        context: &'ctx Context,
        locals: HashMap<usize, usize>,
    ) -> Result<Self, CodeGenError> {
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
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| CodeGenError::Llvm {
                message: "failed to create native target machine".to_string(),
            })?;
        let module = context.create_module("llox_module");
        module.set_triple(&triple);
        let data_layout = machine.get_target_data().get_data_layout();
        module.set_data_layout(&data_layout);

        Ok(CodeGen {
            context,
            builder: context.create_builder(),
            module,
            machine,
            globals: RefCell::new(HashMap::new()),
            locals,
            scopes: RefCell::new(ScopeTable::default()),
            runtime_error_exit: RefCell::new(None),
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
        if let Some(block) = self.builder.get_insert_block()
            && block.get_terminator().is_none()
        {
            let zero = i32_type.const_int(0, false);
            self.builder
                .build_return(Some(&zero))
                .map_err(|error| CodeGenError::Llvm {
                    message: error.to_string(),
                })?;
        }
        Ok(())
    }

    pub fn optimize(&self) -> Result<(), CodeGenError> {
        self.verify()?;
        self.module
            .run_passes("default<O1>", &self.machine, PassBuilderOptions::create())
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })?;
        self.verify()
    }

    pub fn verify(&self) -> Result<(), CodeGenError> {
        self.module.verify().map_err(|error| CodeGenError::Llvm {
            message: format!("LLVM module verification failed: {error}"),
        })
    }
}
