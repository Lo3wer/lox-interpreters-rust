use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::datastructs::stmt::Stmt;

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn compile_stmt(&self, statement: &Stmt) -> Result<(), CodeGenError> {
        match statement {
            Stmt::Block { statements } => {
                self.scopes.borrow_mut().begin_scope();
                let result = statements
                    .iter()
                    .try_for_each(|statement| self.compile_stmt(statement));
                self.scopes.borrow_mut().end_scope();
                result
            }
            Stmt::Expression { expression } => {
                self.compile_expr(expression)?;
                Ok(())
            }
            Stmt::Print { expression } => {
                let value = self.compile_expr(expression)?;
                self.build_print(value)
            }
            Stmt::Var { name, initializer } => {
                if self.scopes.borrow().is_global() {
                    let value = self.compile_expr(initializer)?;
                    self.build_global_define(name.lexeme(), value)
                } else {
                    let slot = self.create_entry_alloca(name.lexeme())?;
                    // Assignments in the initializer can target this binding;
                    // reads in its own initializer are rejected by the resolver.
                    self.scopes.borrow_mut().insert(name.lexeme(), slot)?;
                    let value = self.compile_expr(initializer)?;
                    self.builder
                        .build_store(slot, value)
                        .map_err(|error| CodeGenError::Llvm {
                            message: error.to_string(),
                        })?;
                    Ok(())
                }
            }
            _ => Err(CodeGenError::Unsupported {
                token: None,
                message: "unsupported statement".to_string(),
            }),
        }
    }
}
