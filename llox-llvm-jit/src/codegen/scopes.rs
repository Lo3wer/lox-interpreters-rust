use std::collections::HashMap;

use inkwell::values::PointerValue;

use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;

/// Lexical scopes exist only during compilation; each binding names a stack slot.
#[derive(Default)]
pub(super) struct ScopeTable<'ctx> {
    scopes: Vec<HashMap<String, PointerValue<'ctx>>>,
}

impl<'ctx> ScopeTable<'ctx> {
    pub(super) fn is_global(&self) -> bool {
        self.scopes.is_empty()
    }

    pub(super) fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(super) fn end_scope(&mut self) {
        self.scopes.pop();
    }

    pub(super) fn insert(
        &mut self,
        name: &str,
        slot: PointerValue<'ctx>,
    ) -> Result<(), CodeGenError> {
        let scope = self.scopes.last_mut().ok_or_else(|| CodeGenError::Llvm {
            message: "cannot insert a local binding without a local scope".to_string(),
        })?;
        scope.insert(name.to_string(), slot);
        Ok(())
    }

    pub(super) fn get_at(
        &self,
        depth: usize,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        self.scopes
            .len()
            .checked_sub(depth)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.scopes[index].get(name))
            .copied()
            .ok_or_else(|| CodeGenError::Llvm {
                message: format!("missing resolved local '{name}' at scope depth {depth}"),
            })
    }
}

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn create_entry_alloca(
        &self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let entry = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .and_then(|function| function.get_first_basic_block())
            .ok_or_else(|| CodeGenError::Llvm {
                message: "cannot allocate a local without a function entry block".to_string(),
            })?;

        // Preserve the expression builder's insertion point, even when the
        // declaration's initializer introduces additional basic blocks.
        let builder = self.context.create_builder();
        if let Some(instruction) = entry.get_first_instruction() {
            builder.position_before(&instruction);
        } else {
            builder.position_at_end(entry);
        }
        builder
            .build_alloca(self.lox_value_type(), name)
            .map_err(|error| CodeGenError::Llvm {
                message: error.to_string(),
            })
    }

    pub(super) fn resolved_local(
        &self,
        expression_id: usize,
        name: &str,
    ) -> Result<Option<PointerValue<'ctx>>, CodeGenError> {
        match self.locals.get(&expression_id) {
            Some(depth) => self.scopes.borrow().get_at(*depth, name).map(Some),
            None => Ok(None),
        }
    }
}
