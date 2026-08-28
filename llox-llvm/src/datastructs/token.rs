use super::literal::Literal;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Single-character tokens.
    LeftParen, RightParen, LeftBrace, RightBrace,
    Comma, Dot, Minus, Plus, Semicolon, Slash, Star, Question, Colon,

    // One or two character tokens.
    Bang, BangEqual,
    Equal, EqualEqual,
    Greater, GreaterEqual,
    Less, LessEqual,

    // Literals.
    Identifier, String, Number,

    // Keywords.
    And, Class, Else, False, Fun, For, If, Nil, Or,
    Print, Return, Super, This, True, Var, While,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    token_type: TokenType,
    lexeme: String,
    literal: Option<Literal>,
    line: usize,
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.token_type == other.token_type && self.lexeme == other.lexeme && self.line == other.line
    }
}

impl Eq for Token {}

impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token_type.hash(state);
        self.lexeme.hash(state);
        self.line.hash(state);
    }
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, literal: Option<Literal>, line: usize) -> Self {
        Token {
            token_type,
            lexeme,
            literal,
            line,
        }
    }

    pub fn identifier(name: &str) -> Self {
        Token::new(TokenType::Identifier, name.to_string(), None, 0)
    }

    pub fn token_type(&self) -> TokenType {
        self.token_type
    }

    pub fn lexeme(&self) -> &str {
        &self.lexeme
    }

    pub fn literal(&self) -> Option<&Literal> {
        self.literal.as_ref()
    }

    pub fn line(&self) -> usize {
        self.line
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.literal {
            Some(literal) => write!(f, "{:?} {} {:?}", self.token_type, self.lexeme, literal),
            None => write!(f, "{:?} {}", self.token_type, self.lexeme),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_token() {
        let t = Token::new(TokenType::Number, "42".into(), Some(Literal::Number(42.0)), 1);
        assert_eq!(t.token_type(), TokenType::Number);
        assert_eq!(t.lexeme(), "42");
        assert_eq!(t.line(), 1);
        assert!(t.literal().is_some());
    }

    #[test]
    fn test_identifier_token() {
        let t = Token::identifier("foo");
        assert_eq!(t.token_type(), TokenType::Identifier);
        assert_eq!(t.lexeme(), "foo");
        assert_eq!(t.line(), 0);
    }

    #[test]
    fn test_token_eq_same_line() {
        let a = Token::new(TokenType::Identifier, "x".into(), None, 3);
        let b = Token::new(TokenType::Identifier, "x".into(), None, 3);
        assert_eq!(a, b);
    }

    #[test]
    fn test_token_eq_different_line() {
        let a = Token::new(TokenType::Identifier, "x".into(), None, 3);
        let b = Token::new(TokenType::Identifier, "x".into(), None, 5);
        assert_ne!(a, b);
    }

    #[test]
    fn test_token_eq_different_type() {
        let a = Token::new(TokenType::Identifier, "x".into(), None, 1);
        let b = Token::new(TokenType::String, "x".into(), None, 1);
        assert_ne!(a, b);
    }

    #[test]
    fn test_token_hash() {
        use std::collections::HashSet;
        let a = Token::new(TokenType::Identifier, "x".into(), None, 1);
        let b = Token::new(TokenType::Identifier, "x".into(), None, 1);
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
    }
}