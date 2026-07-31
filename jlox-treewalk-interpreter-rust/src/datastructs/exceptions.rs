use super::token::Token;
use super::literal::Literal;

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
pub enum RuntimeException {
    Error { token: Token, message: String },
    Return { value: Literal },
}