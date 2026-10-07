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
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.compile_expr(condition)?;
                let condition_truthy = self.build_is_truthy(condition_value)?;
                let then_block = self.append_block("if_then")?;
                let merge_block = self.append_block("if_merge")?;
                let else_block = match else_branch {
                    Some(_) => self.append_block("if_else")?,
                    None => merge_block,
                };
                self.conditional_branch_if_open(condition_truthy, then_block, else_block)?;

                self.builder.position_at_end(then_block);
                self.compile_stmt(then_branch)?;
                self.branch_if_open(merge_block)?;

                if let Some(else_branch) = else_branch {
                    self.builder.position_at_end(else_block);
                    self.compile_stmt(else_branch)?;
                    self.branch_if_open(merge_block)?;
                }

                self.builder.position_at_end(merge_block);
                Ok(())
            }
            Stmt::While { condition, body } => {
                let condition_block = self.append_block("while_condition")?;
                let body_block = self.append_block("while_body")?;
                let exit_block = self.append_block("while_exit")?;
                self.branch_if_open(condition_block)?;

                self.builder.position_at_end(condition_block);
                let condition_value = self.compile_expr(condition)?;
                let condition_truthy = self.build_is_truthy(condition_value)?;
                self.conditional_branch_if_open(condition_truthy, body_block, exit_block)?;

                self.builder.position_at_end(body_block);
                self.compile_stmt(body)?;
                self.branch_if_open(condition_block)?;

                self.builder.position_at_end(exit_block);
                Ok(())
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
