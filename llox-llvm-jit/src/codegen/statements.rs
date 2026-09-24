use super::CodeGen;
use crate::datastructs::exceptions::CodeGenError;
use crate::datastructs::stmt::Stmt;

impl<'ctx> CodeGen<'ctx> {
    pub(super) fn compile_stmt(&self, statement: &Stmt) -> Result<(), CodeGenError> {
        match statement {
            Stmt::Print { expression } => {
                let value = self.compile_expr(expression)?;
                self.build_print(value)
            }
            _ => Err(CodeGenError::Unsupported {
                token: None,
                message: "unsupported statement".to_string(),
            }),
        }
    }
}
