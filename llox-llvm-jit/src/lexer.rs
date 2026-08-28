use crate::datastructs::token::{Token, TokenType};
use crate::datastructs::literal::Literal;
use crate::datastructs::exceptions::LexError;

pub struct Lexer {
    source: Vec<char>,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
    start: usize,
    current: usize,
    line: usize,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Lexer {
            source: source.chars().collect(),
            tokens: Vec::new(),
            errors: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_tokens(&mut self) -> (Vec<Token>, Vec<LexError>) {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        self.tokens.push(Token::new(TokenType::Eof, "".to_string(), None, self.line));
        (std::mem::take(&mut self.tokens), std::mem::take(&mut self.errors))
    }

    // Scanning routines
    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            '(' => self.add_token(TokenType::LeftParen, None),
            ')' => self.add_token(TokenType::RightParen, None),
            '{' => self.add_token(TokenType::LeftBrace, None),
            '}' => self.add_token(TokenType::RightBrace, None),
            ',' => self.add_token(TokenType::Comma, None),
            '.' => self.add_token(TokenType::Dot, None),
            '-' => self.add_token(TokenType::Minus, None),
            '+' => self.add_token(TokenType::Plus, None),
            ';' => self.add_token(TokenType::Semicolon, None),
            '*' => self.add_token(TokenType::Star, None),
            '?' => self.add_token(TokenType::Question, None),
            ':' => self.add_token(TokenType::Colon, None),
            '!' => {
                if self.match_char('=') {
                    self.add_token(TokenType::BangEqual, None);
                } else {
                    self.add_token(TokenType::Bang, None);
                }
            }
            '=' => {
                if self.match_char('=') {
                    self.add_token(TokenType::EqualEqual, None);
                } else {
                    self.add_token(TokenType::Equal, None);
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.add_token(TokenType::LessEqual, None);
                } else {
                    self.add_token(TokenType::Less, None);
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.add_token(TokenType::GreaterEqual, None);
                } else {
                    self.add_token(TokenType::Greater, None);
                }
            }
            '/' => {
                if self.match_char('/') {
                    // Handle single-line comments
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else if self.match_char('*') {
                    // Handle block comments
                    while !(self.peek() == '*' && self.peek_next() == '/') && !self.is_at_end() {
                        if self.peek() == '\n' {
                            self.line += 1;
                        }
                        self.advance();
                    }
                    if self.is_at_end() {
                        self.error("Unterminated block comment.");
                    } else {
                        // Consume the closing */
                        self.advance(); // consume '*'
                        self.advance(); // consume '/'
                    }
                } else {
                    self.add_token(TokenType::Slash, None);
                }
            }
            ' ' | 
            '\r' | 
            '\t' => { /* Ignore whitespace */ }
            '\n' => self.line += 1,
            '"' => self.string(),
            c if Self::is_digit(c) => self.number(),
            c if Self::is_alpha(c) => self.identifier(),
            _ => {
                self.error("Unexpected character.");
            }
        }
    }

    fn string(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            self.error("Unterminated string.");
            return;
        }

        // The closing ".
        self.advance();

        // Trim the surrounding quotes.
        let value: String = self.source[self.start + 1..self.current - 1].iter().collect();
        self.add_token(TokenType::String, Some(Literal::String(value)));
    }

    fn error(&mut self, message: &str) {
        self.errors.push(LexError {
            line: self.line,
            message: message.to_string(),
        });
    }

    fn number(&mut self) {
        while Self::is_digit(self.peek()) {
            self.advance();
        }

        // Look for a fractional part.
        if self.peek() == '.' && Self::is_digit(self.peek_next()) {
            // Consume the "."
            self.advance();

            while Self::is_digit(self.peek()) {
                self.advance();
            }
        }

        let value: String = self.source[self.start..self.current].iter().collect();
        let number_value: f64 = value.parse().unwrap();
        self.add_token(TokenType::Number, Some(Literal::Number(number_value)));
    }

    // Token stream helpers
    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn add_token(&mut self, token_type: TokenType, literal: Option<Literal>) {
        let text: String = self.source[self.start..self.current].iter().collect();
        self.tokens.push(Token::new(token_type, text, literal, self.line));
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source[self.current + 1]
        }
    }

    fn identifier(&mut self) {
        while Self::is_alphanumeric(self.peek()) {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].iter().collect();
        let token_type = Self::keyword_type(&text).unwrap_or(TokenType::Identifier);
        self.add_token(token_type, None);
    }

    // Helper functions

    fn is_digit(c: char) -> bool {
        c >= '0' && c <= '9'
    }

    fn is_alpha(c: char) -> bool {
        (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
    }

    fn is_alphanumeric(c: char) -> bool {
        Self::is_alpha(c) || Self::is_digit(c)
    }

    fn keyword_type(text: &str) -> Option<TokenType> {
        match text {
            "and" => Some(TokenType::And),
            "class" => Some(TokenType::Class),
            "else" => Some(TokenType::Else),
            "false" => Some(TokenType::False),
            "for" => Some(TokenType::For),
            "fun" => Some(TokenType::Fun),
            "if" => Some(TokenType::If),
            "nil" => Some(TokenType::Nil),
            "or" => Some(TokenType::Or),
            "print" => Some(TokenType::Print),
            "return" => Some(TokenType::Return),
            "super" => Some(TokenType::Super),
            "this" => Some(TokenType::This),
            "true" => Some(TokenType::True),
            "var" => Some(TokenType::Var),
            "while" => Some(TokenType::While),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source.to_string());
        lexer.scan_tokens().0
    }

    fn errors(source: &str) -> Vec<LexError> {
        let mut lexer = Lexer::new(source.to_string());
        lexer.scan_tokens().1
    }

    fn assert_token(token: &Token, expected_type: TokenType, expected_lexeme: &str, expected_line: usize) {
        assert_eq!(token.token_type(), expected_type, "type mismatch for '{}'", expected_lexeme);
        assert_eq!(token.lexeme(), expected_lexeme, "lexeme mismatch");
        assert_eq!(token.line(), expected_line, "line mismatch for '{}'", expected_lexeme);
    }

    // --- Single-character tokens ---

    #[test]
    fn test_left_paren() {
        let toks = tokenize("(");
        assert_token(&toks[0], TokenType::LeftParen, "(", 1);
    }

    #[test]
    fn test_right_paren() {
        let toks = tokenize(")");
        assert_token(&toks[0], TokenType::RightParen, ")", 1);
    }

    #[test]
    fn test_braces() {
        let toks = tokenize("{}");
        assert_token(&toks[0], TokenType::LeftBrace, "{", 1);
        assert_token(&toks[1], TokenType::RightBrace, "}", 1);
    }

    #[test]
    fn test_punctuation() {
        let toks = tokenize(",.;");
        assert_token(&toks[0], TokenType::Comma, ",", 1);
        assert_token(&toks[1], TokenType::Dot, ".", 1);
        assert_token(&toks[2], TokenType::Semicolon, ";", 1);
    }

    #[test]
    fn test_operators() {
        let toks = tokenize("+-*/");
        assert_token(&toks[0], TokenType::Plus, "+", 1);
        assert_token(&toks[1], TokenType::Minus, "-", 1);
        assert_token(&toks[2], TokenType::Star, "*", 1);
        assert_token(&toks[3], TokenType::Slash, "/", 1);
    }

    // --- Two-character tokens ---

    #[test]
    fn test_bang() {
        let toks = tokenize("!!=");
        assert_token(&toks[0], TokenType::Bang, "!", 1);
        assert_token(&toks[1], TokenType::BangEqual, "!=", 1);
    }

    #[test]
    fn test_equality() {
        let toks = tokenize("===");
        assert_token(&toks[0], TokenType::EqualEqual, "==", 1);
        assert_token(&toks[1], TokenType::Equal, "=", 1);
    }

    #[test]
    fn test_comparison() {
        let toks = tokenize("<=<>=>");
        assert_token(&toks[0], TokenType::LessEqual, "<=", 1);
        assert_token(&toks[1], TokenType::Less, "<", 1);
        assert_token(&toks[2], TokenType::GreaterEqual, ">=", 1);
        assert_token(&toks[3], TokenType::Greater, ">", 1);
    }

    #[test]
    fn test_question_colon() {
        let toks = tokenize("?:");
        assert_token(&toks[0], TokenType::Question, "?", 1);
        assert_token(&toks[1], TokenType::Colon, ":", 1);
    }

    // --- Literals ---

    #[test]
    fn test_integer() {
        let toks = tokenize("42");
        assert_token(&toks[0], TokenType::Number, "42", 1);
        if let Some(Literal::Number(n)) = toks[0].literal() {
            assert!((*n - 42.0).abs() < 1e-10);
        } else {
            panic!("expected Number literal");
        }
    }

    #[test]
    fn test_float() {
        let toks = tokenize("3.14");
        assert_token(&toks[0], TokenType::Number, "3.14", 1);
        if let Some(Literal::Number(n)) = toks[0].literal() {
            assert!((*n - 3.14).abs() < 1e-10);
        } else {
            panic!("expected Number literal");
        }
    }

    #[test]
    fn test_string() {
        let toks = tokenize("\"hello\"");
        assert_token(&toks[0], TokenType::String, "\"hello\"", 1);
        if let Some(Literal::String(s)) = toks[0].literal() {
            assert_eq!(s, "hello");
        } else {
            panic!("expected String literal");
        }
    }

    #[test]
    fn test_string_with_escape() {
        let toks = tokenize("\"a\\b\"");
        assert_token(&toks[0], TokenType::String, "\"a\\b\"", 1);
    }

    #[test]
    fn test_multiline_string() {
        let toks = tokenize("\"line1\nline2\"");
        assert_token(&toks[0], TokenType::String, "\"line1\nline2\"", 2);
    }

    // --- Identifiers and keywords ---

    #[test]
    fn test_identifier() {
        let toks = tokenize("myVar");
        assert_token(&toks[0], TokenType::Identifier, "myVar", 1);
    }

    #[test]
    fn test_identifier_with_underscore() {
        let toks = tokenize("_private");
        assert_token(&toks[0], TokenType::Identifier, "_private", 1);
    }

    #[test]
    fn test_keyword_var() {
        let toks = tokenize("var");
        assert_token(&toks[0], TokenType::Var, "var", 1);
    }

    #[test]
    fn test_keyword_fun() {
        let toks = tokenize("fun");
        assert_token(&toks[0], TokenType::Fun, "fun", 1);
    }

    #[test]
    fn test_keyword_class() {
        let toks = tokenize("class");
        assert_token(&toks[0], TokenType::Class, "class", 1);
    }

    #[test]
    fn test_keyword_print() {
        let toks = tokenize("print");
        assert_token(&toks[0], TokenType::Print, "print", 1);
    }

    #[test]
    fn test_keyword_if_else() {
        let toks = tokenize("if else");
        assert_token(&toks[0], TokenType::If, "if", 1);
        assert_token(&toks[1], TokenType::Else, "else", 1);
    }

    #[test]
    fn test_keyword_while() {
        let toks = tokenize("while");
        assert_token(&toks[0], TokenType::While, "while", 1);
    }

    #[test]
    fn test_keyword_for() {
        let toks = tokenize("for");
        assert_token(&toks[0], TokenType::For, "for", 1);
    }

    #[test]
    fn test_keyword_return() {
        let toks = tokenize("return");
        assert_token(&toks[0], TokenType::Return, "return", 1);
    }

    #[test]
    fn test_keyword_nil() {
        let toks = tokenize("nil");
        assert_token(&toks[0], TokenType::Nil, "nil", 1);
    }

    #[test]
    fn test_keyword_bool() {
        let toks = tokenize("true false");
        assert_token(&toks[0], TokenType::True, "true", 1);
        assert_token(&toks[1], TokenType::False, "false", 1);
    }

    #[test]
    fn test_keyword_this_super() {
        let toks = tokenize("this super and or");
        assert_token(&toks[0], TokenType::This, "this", 1);
        assert_token(&toks[1], TokenType::Super, "super", 1);
        assert_token(&toks[2], TokenType::And, "and", 1);
        assert_token(&toks[3], TokenType::Or, "or", 1);
    }

    // --- Comments ---

    #[test]
    fn test_line_comment() {
        let toks = tokenize("// this is a comment\nvar");
        assert_eq!(toks.len(), 2); // Var + Eof
        assert_token(&toks[0], TokenType::Var, "var", 2);
    }

    #[test]
    fn test_block_comment() {
        let toks = tokenize("/* comment */print");
        assert_token(&toks[0], TokenType::Print, "print", 1);
    }

    #[test]
    fn test_block_comment_multiline() {
        let toks = tokenize("/*\nmulti\nline\n*/print");
        assert_token(&toks[0], TokenType::Print, "print", 4);
    }

    // --- Whitespace and lines ---

    #[test]
    fn test_line_counting() {
        let toks = tokenize("a\nb\n\nc");
        assert_token(&toks[0], TokenType::Identifier, "a", 1);
        assert_token(&toks[1], TokenType::Identifier, "b", 2);
        assert_token(&toks[2], TokenType::Identifier, "c", 4);
    }

    // --- Errors ---

    #[test]
    fn test_unexpected_character() {
        let errs = errors("@");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].message, "Unexpected character.");
    }

    #[test]
    fn test_unterminated_string() {
        let errs = errors("\"unclosed");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].message, "Unterminated string.");
    }

    #[test]
    fn test_unterminated_block_comment() {
        let errs = errors("/* oops");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].message, "Unterminated block comment.");
    }

    // --- Compound expressions ---

    #[test]
    fn test_arithmetic_expression() {
        let toks = tokenize("a + b * 3");
        assert_token(&toks[0], TokenType::Identifier, "a", 1);
        assert_token(&toks[1], TokenType::Plus, "+", 1);
        assert_token(&toks[2], TokenType::Identifier, "b", 1);
        assert_token(&toks[3], TokenType::Star, "*", 1);
        assert_token(&toks[4], TokenType::Number, "3", 1);
    }

    #[test]
    fn test_grouping() {
        let toks = tokenize("(a)");
        assert_token(&toks[0], TokenType::LeftParen, "(", 1);
        assert_token(&toks[1], TokenType::Identifier, "a", 1);
        assert_token(&toks[2], TokenType::RightParen, ")", 1);
    }

    #[test]
    fn test_ends_with_eof() {
        let toks = tokenize("1");
        assert_eq!(toks.last().unwrap().token_type(), TokenType::Eof);
    }
}