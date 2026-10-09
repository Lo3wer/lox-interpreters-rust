use super::literal::Literal;
use super::token::Token;

#[derive(Debug, Clone)]
pub struct LexError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub token: Token,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ResolveError {
    pub token: Token,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum CodeGenError {
    Unsupported {
        token: Option<Token>,
        message: String,
    },
    Llvm {
        message: String,
    },
}

impl From<inkwell::builder::BuilderError> for CodeGenError {
    fn from(error: inkwell::builder::BuilderError) -> Self {
        Self::Llvm {
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RuntimeException {
    Llvm { message: String },
    Error { token: Token, message: String },
    Return { value: Literal },
}
