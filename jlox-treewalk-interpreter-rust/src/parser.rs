use crate::datastructs::token::{Token, TokenType};
use crate::datastructs::literal::Literal;
use crate::datastructs::expr::Expr;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::exceptions::ParseError;

pub struct Parser {
    tokens: Vec<Token>,
    errors: Vec<ParseError>,
    current: usize,
    expr_id_counter: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, errors: Vec::new(), current: 0, expr_id_counter: 0 }
    }

    fn next_expr_id(&mut self) -> usize {
        let id = self.expr_id_counter;
        self.expr_id_counter += 1;
        id
    }

    pub fn parse_prompt_line(&mut self) -> Result<Vec<Stmt>, Vec<ParseError>> {
        let checkpoint = self.current;
        
        if let Ok(expr) = self.expression() {
            // must consume everything up to EOF to count as "just an expression".
            if self.check(&TokenType::Semicolon) {
                self.advance();
            }
            if self.is_at_end() {
                return Ok(vec![Stmt::Print{ expression: Box::new(expr) }]);
            }
        }
        // backtrack and parse normally.
        self.current = checkpoint;
        self.parse()
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, Vec<ParseError>> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            match self.declaration() {
                Ok(stmt) => statements.push(stmt),
                Err(err) => {
                    self.errors.push(err);
                    self.synchronize();
                }
            }
        }

        if self.errors.is_empty() { Ok(statements) } else { Err(self.errors.clone()) }
    }

    // refer to grammar.md for grammar rules

    fn declaration(&mut self) -> Result<Stmt, ParseError> {
        if self.match_token(&[TokenType::Var]) {
            return self.var_declaration();
        } else if self.match_token(&[TokenType::Fun]) {
            return self.function_declaration("function".into());
        } else if self.match_token(&[TokenType::Class]) {
            return self.class_declaration();
        }
        self.statement()
    }

    fn class_declaration(&mut self) -> Result<Stmt, ParseError> {
        let name = self.consume(TokenType::Identifier, "Expect class name.")?.clone();

        let superclass = if self.match_token(&[TokenType::Less]) {
            let superclass_name = self.consume(TokenType::Identifier, "Expect superclass name.")?.clone();
            Some(Box::new(Expr::Variable { name: superclass_name, id: self.next_expr_id() }))
        } else {
            None
        };
        self.consume(TokenType::LeftBrace, "Expect '{' before class body.")?;

        let mut methods = Vec::new();
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            methods.push(self.function_declaration("method".into())?);
        }

        self.consume(TokenType::RightBrace, "Expect '}' after class body.")?;
        Ok(Stmt::Class { name, superclass, methods })
    }

    fn function_declaration(&mut self, kind: String) -> Result<Stmt, ParseError> {
        let name = self.consume(TokenType::Identifier, &format!("Expect {} name.", kind))?.clone();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.")?;

        let mut params = Vec::new();
        if !self.check(&TokenType::RightParen) {
            loop {
                if params.len() >= 255 {
                    return Err(self.error(self.peek(), "Cannot have more than 255 parameters."));
                }
                params.push(self.consume(TokenType::Identifier, "Expect parameter name.")?.clone());
                if !self.match_token(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;
        self.consume(TokenType::LeftBrace, &format!("Expect {} body.", kind))?;
        let body = self.block()?;

        Ok(Stmt::Function { name, params, body })
    }

    fn var_declaration(&mut self) -> Result<Stmt, ParseError> {
        let name = self.consume(TokenType::Identifier, "Expect variable name.")?.clone();

        let mut initializer = Expr::Literal { value: Literal::Nil, id: self.next_expr_id() };
        if self.match_token(&[TokenType::Equal]) {
            initializer = self.expression()?;
        }

        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration.")?;

        Ok(Stmt::Var { name, initializer: Box::new(initializer) })
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        if self.match_token(&[TokenType::Print]) {
            return self.print_statement();
        } else if self.match_token(&[TokenType::LeftBrace]) {
            let statements = self.block()?;
            return Ok(Stmt::Block { statements });
        } else if self.match_token(&[TokenType::If]) {
            return self.if_statement();
        } else if self.match_token(&[TokenType::While]) {
            return self.while_statement();
        } else if self.match_token(&[TokenType::For]) {
            return self.for_statement();
        } else if self.match_token(&[TokenType::Return]) {
            return self.return_statement();
        }
        self.expression_statement()
    }

    fn return_statement(&mut self) -> Result<Stmt, ParseError> {
        let keyword = self.previous().clone();
        let value = if !self.check(&TokenType::Semicolon) {
            Some(Box::new(self.expression()?))
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after return value.")?;
        Ok(Stmt::Return { keyword, value })
    }

    fn for_statement(&mut self) -> Result<Stmt, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.")?;

        let initializer = if self.match_token(&[TokenType::Semicolon]) {
            None
        } else if self.match_token(&[TokenType::Var]) {
            Some(self.var_declaration()?)
        } else {
            Some(self.expression_statement()?)
        };

        let condition = if !self.check(&TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after loop condition.")?;

        let increment = if !self.check(&TokenType::RightParen) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

        let mut body = Box::new(self.statement()?);

        //desugar into while loop

        if let Some(inc) = increment {
            body = Box::new(Stmt::Block {
                statements: vec![
                    *body,
                    Stmt::Expression { expression: Box::new(inc) },
                ],
            });
        }

        let condition = condition.unwrap_or(Expr::Literal { value: Literal::Bool(true), id: self.next_expr_id() });

        body = Box::new(Stmt::While {
            condition: Box::new(condition),
            body,
        });

        if let Some(init) = initializer {
            body = Box::new(Stmt::Block {
                statements: vec![
                    init,
                    *body,
                ],
            });
        }

        Ok(*body)
    }

    fn if_statement(&mut self) -> Result<Stmt, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.")?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after if condition.")?;

        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_token(&[TokenType::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If { condition: Box::new(condition), then_branch, else_branch })
    }

    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after while condition.")?;

        let body = Box::new(self.statement()?);
        Ok(Stmt::While { condition: Box::new(condition), body })
    }

    fn expression_statement(&mut self) -> Result<Stmt, ParseError> {
        let expr = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after expression.")?;
        Ok(Stmt::Expression {
            expression: Box::new(expr),
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(statements)
    }

    fn print_statement(&mut self) -> Result<Stmt, ParseError> {
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after value.")?;
        Ok(Stmt::Print {
            expression: Box::new(value),
        })
    }

    // Grammar entry point
    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.comma()
    }

    fn comma(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.assignment()?;

        while self.match_token(&[TokenType::Comma]) {
            let operator = self.previous().clone();
            let right = self.assignment()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn assignment(&mut self) -> Result<Expr, ParseError> {
        let expr = self.or()?;

        if self.match_token(&[TokenType::Equal]) {
            let equals = self.previous().clone();
            let value = self.assignment()?;

            if let Expr::Variable { name, .. } = expr {
                return Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                    id: self.next_expr_id(),
                });
            } else if let Expr::Get { object, name, .. } = expr {
                return Ok(Expr::Set {
                    object,
                    name,
                    value: Box::new(value),
                    id: self.next_expr_id(),
                });
            }

            return Err(self.error(&equals, "Invalid assignment target."));
        }

        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.and()?;

        while self.match_token(&[TokenType::Or]) {
            let operator = self.previous().clone();
            let right = self.and()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.ternary()?;

        while self.match_token(&[TokenType::And]) {
            let operator = self.previous().clone();
            let right = self.ternary()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn ternary(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.equality()?;

        if self.match_token(&[TokenType::Question]) {
            let then_branch = self.expression()?;
            self.consume(TokenType::Colon, "Expect ':' after then branch of ternary expression.")?;
            let else_branch = self.ternary()?;
            expr = Expr::Ternary {
                condition: Box::new(expr),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    // Binary precedence levels
    fn equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.comparison()?;

        while self.match_token(&[TokenType::BangEqual, TokenType::EqualEqual]) {
            let operator = self.previous().clone();
            let right = self.comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.term()?;

        while self.match_token(&[
            TokenType::Greater,
            TokenType::GreaterEqual,
            TokenType::Less,
            TokenType::LessEqual,
        ]) {
            let operator = self.previous().clone();
            let right = self.term()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.factor()?;

        while self.match_token(&[TokenType::Minus, TokenType::Plus]) {
            let operator = self.previous().clone();
            let right = self.factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.unary()?;

        while self.match_token(&[TokenType::Slash, TokenType::Star]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            };
        }

        Ok(expr)
    }

    // Unary and primary expressions
    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.match_token(&[TokenType::Bang, TokenType::Minus]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary {
                operator,
                right: Box::new(right),
                id: self.next_expr_id(),
            });
        }

        self.call()
    }

    fn call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(&[TokenType::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_token(&[TokenType::Dot]) {
                let name = self.consume(TokenType::Identifier, "Expect property name after '.'.")?.clone();
                expr = Expr::Get {
                    object: Box::new(expr),
                    name,
                    id: self.next_expr_id(),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    // helper for call
    fn finish_call(&mut self, callee: Expr) -> Result<Expr, ParseError> {
        let mut arguments = Vec::new();

        if !self.check(&TokenType::RightParen) {
            loop {
                if arguments.len() >= 255 {
                    return Err(self.error(self.peek(), "Cannot have more than 255 arguments."));
                }
                arguments.push(self.assignment()?);
                if !self.match_token(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        let paren = self.consume(TokenType::RightParen, "Expect ')' after arguments.")?.clone();
        Ok(Expr::Call {
            callee: Box::new(callee),
            paren,
            arguments,
            id: self.next_expr_id(),
        })
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        if self.match_token(&[TokenType::False]) {
            return Ok(Expr::Literal { value: Literal::Bool(false), id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::True]) {
            return Ok(Expr::Literal { value: Literal::Bool(true), id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::Nil]) {
            return Ok(Expr::Literal { value: Literal::Nil, id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::Number, TokenType::String]) {
            let literal = self.previous().literal()
                .cloned()
                .ok_or_else(|| self.error(self.previous(), "Expected a literal value."))?;
            return Ok(Expr::Literal { value: literal, id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::This]) {
            return Ok(Expr::This { keyword: self.previous().clone(), id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::Identifier]) {
            return Ok(Expr::Variable { name: self.previous().clone(), id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::LeftParen]) {
            let expr = self.expression()?;
            self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
            return Ok(Expr::Grouping { expression: Box::new(expr), id: self.next_expr_id() });
        }
        if self.match_token(&[TokenType::Super]) {
            let keyword = self.previous().clone();
            self.consume(TokenType::Dot, "Expect '.' after 'super'.")?;
            let method = self.consume(TokenType::Identifier, "Expect superclass method name.")?.clone();
            return Ok(Expr::Super { keyword, method, id: self.next_expr_id() });
        }

        // error productions

        Err(self.error(self.peek(), "Expected expression."))
    }

    // Token stream helpers
    fn match_token(&mut self, types: &[TokenType]) -> bool {
        for token_type in types {
            if self.check(token_type) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, token_type: &TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek().token_type() == *token_type
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().token_type() == TokenType::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<&Token, ParseError> {
        if self.check(&token_type) {
            return Ok(self.advance());
        }
        Err(self.error(self.peek(), message))
    }

    // Error helper
    fn error(&self, token: &Token, message: &str) -> ParseError {
        ParseError {
            token: token.clone(),
            message: message.to_string(),
        }
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().token_type() == TokenType::Semicolon {
                return;
            }

            match self.peek().token_type() {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => {}
            }

            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Result<Vec<Stmt>, Vec<ParseError>> {
        let mut lexer = Lexer::new(source.to_string());
        let (tokens, _) = lexer.scan_tokens();
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    fn parse_one(source: &str) -> Stmt {
        let mut stmts = parse(source).expect("parsing failed");
        assert_eq!(stmts.len(), 1, "expected exactly one statement");
        stmts.remove(0)
    }

    fn assert_expression_stmt(stmt: &Stmt) -> &Expr {
        match stmt {
            Stmt::Expression { expression } => expression.as_ref(),
            other => panic!("expected Expression stmt, got {:?}", other),
        }
    }

    fn assert_var(stmt: &Stmt) -> (&Token, &Expr) {
        match stmt {
            Stmt::Var { name, initializer } => (name, initializer.as_ref()),
            other => panic!("expected Var stmt, got {:?}", other),
        }
    }

    fn assert_print(stmt: &Stmt) -> &Expr {
        match stmt {
            Stmt::Print { expression } => expression.as_ref(),
            other => panic!("expected Print stmt, got {:?}", other),
        }
    }

    fn assert_literal(expr: &Expr) {
        match expr {
            Expr::Literal { .. } => {},
            _other => panic!("expected Literal expr, got variant"),
        }
    }

    fn assert_binary(expr: &Expr) -> (&Token, &Expr, &Expr) {
        match expr {
            Expr::Binary { operator, left, right, .. } => (operator, left, right),
            other => panic!("expected Binary expr, got {:?}", other),
        }
    }

    fn assert_unary(expr: &Expr) -> (&Token, &Expr) {
        match expr {
            Expr::Unary { operator, right, .. } => (operator, right),
            other => panic!("expected Unary expr, got {:?}", other),
        }
    }

    fn assert_grouping(expr: &Expr) -> &Expr {
        match expr {
            Expr::Grouping { expression, .. } => expression.as_ref(),
            other => panic!("expected Grouping expr, got {:?}", other),
        }
    }

    fn assert_variable(expr: &Expr) -> &Token {
        match expr {
            Expr::Variable { name, .. } => name,
            other => panic!("expected Variable expr, got {:?}", other),
        }
    }

    // --- Statement parsing ---

    #[test]
    fn test_parse_expression_stmt() {
        let stmt = parse_one("x;");
        let expr = assert_expression_stmt(&stmt);
        let name = assert_variable(expr);
        assert_eq!(name.lexeme(), "x");
    }

    #[test]
    fn test_parse_print() {
        let stmt = parse_one("print 42;");
        let expr = assert_print(&stmt);
        assert_literal(expr);
    }

    #[test]
    fn test_parse_var_no_init() {
        let stmt = parse_one("var x;");
        let (name, init) = assert_var(&stmt);
        assert_eq!(name.lexeme(), "x");
        assert_literal(init);
    }

    #[test]
    fn test_parse_var_with_init() {
        let stmt = parse_one("var x = 99;");
        let (name, init) = assert_var(&stmt);
        assert_eq!(name.lexeme(), "x");
        assert_literal(init);
    }

    #[test]
    fn test_parse_block() {
        let stmts = parse("{ var a = 1; print a; }").expect("parse failed");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 2);
                assert!(matches!(&statements[0], Stmt::Var { .. }));
                assert!(matches!(&statements[1], Stmt::Print { .. }));
            }
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_if() {
        let stmts = parse("if (a) print 1; else print 2;").expect("parse failed");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If { condition, then_branch, else_branch } => {
                assert!(matches!(condition.as_ref(), Expr::Variable { .. }));
                assert!(matches!(then_branch.as_ref(), Stmt::Print { .. }));
                assert!(else_branch.is_some());
            }
            other => panic!("expected If, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_if_no_else() {
        let stmts = parse("if (a) print 1;").expect("parse failed");
        match &stmts[0] {
            Stmt::If { else_branch, .. } => assert!(else_branch.is_none()),
            other => panic!("expected If, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_while() {
        let stmts = parse("while (a) print 1;").expect("parse failed");
        match &stmts[0] {
            Stmt::While { condition, body } => {
                assert!(matches!(condition.as_ref(), Expr::Variable { .. }));
                assert!(matches!(body.as_ref(), Stmt::Print { .. }));
            }
            other => panic!("expected While, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_for() {
        let stmts = parse("for (var i = 0; i < 3; i = i + 1) print i;").expect("parse failed");
        assert_eq!(stmts.len(), 1);
        // for desugars into a Block containing initialier + while loop
        match &stmts[0] {
            Stmt::Block { .. } => {}
            other => panic!("expected Block (desugared for), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_for_no_init() {
        let stmts = parse("for (; a < 3; a = a + 1) print a;").expect("parse failed");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::While { .. } => {}
            other => panic!("expected While (desugared for), got {:?}", other),
        }
    }

    // --- Expression parsing ---

    #[test]
    fn test_parse_literal_number() {
        let stmt = parse_one("3.14;");
        let expr = assert_expression_stmt(&stmt);
        assert_literal(expr);
    }

    #[test]
    fn test_parse_literal_string() {
        let stmt = parse_one("\"hello\";");
        let expr = assert_expression_stmt(&stmt);
        assert_literal(expr);
    }

    #[test]
    fn test_parse_literal_bool() {
        let stmt = parse_one("true;");
        let expr = assert_expression_stmt(&stmt);
        assert_literal(expr);
    }

    #[test]
    fn test_parse_variable() {
        let stmt = parse_one("foo;");
        let expr = assert_expression_stmt(&stmt);
        assert_variable(expr);
    }

    #[test]
    fn test_parse_binary_precedence() {
        // a + b * c should parse as a + (b * c)
        let stmt = parse_one("a + b * c;");
        let expr = assert_expression_stmt(&stmt);
        let (op, left, right) = assert_binary(expr);
        assert_eq!(op.lexeme(), "+");
        assert_variable(left); // left is 'a'
        // right should be Binary(b * c)
        let (op2, left2, right2) = assert_binary(right);
        assert_eq!(op2.lexeme(), "*");
        assert_variable(left2); // 'b'
        assert_variable(right2); // 'c'
    }

    #[test]
    fn test_parse_grouping() {
        let stmt = parse_one("(a);");
        let expr = assert_expression_stmt(&stmt);
        let inner = assert_grouping(expr);
        assert_variable(inner);
    }

    #[test]
    fn test_parse_unary_minus() {
        let stmt = parse_one("-42;");
        let expr = assert_expression_stmt(&stmt);
        let (op, right) = assert_unary(expr);
        assert_eq!(op.lexeme(), "-");
        assert_literal(right);
    }

    #[test]
    fn test_parse_unary_bang() {
        let stmt = parse_one("!true;");
        let expr = assert_expression_stmt(&stmt);
        let (op, right) = assert_unary(expr);
        assert_eq!(op.lexeme(), "!");
        assert_literal(right);
    }

    #[test]
    fn test_parse_ternary() {
        let stmt = parse_one("a ? b : c;");
        let expr = assert_expression_stmt(&stmt);
        match expr {
            Expr::Ternary { condition, then_branch, else_branch, .. } => {
                assert_variable(condition);
                assert_variable(then_branch);
                assert_variable(else_branch);
            }
            other => panic!("expected Ternary, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_logical_and() {
        let stmt = parse_one("a and b;");
        let expr = assert_expression_stmt(&stmt);
        match expr {
            Expr::Logical { operator, left, right, .. } => {
                assert_eq!(operator.lexeme(), "and");
                assert_variable(left);
                assert_variable(right);
            }
            other => panic!("expected Logical, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_logical_or() {
        let stmt = parse_one("a or b;");
        let expr = assert_expression_stmt(&stmt);
        match expr {
            Expr::Logical { operator, left, right, .. } => {
                assert_eq!(operator.lexeme(), "or");
                assert_variable(left);
                assert_variable(right);
            }
            other => panic!("expected Logical, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_assignment() {
        let stmt = parse_one("x = 5;");
        let expr = assert_expression_stmt(&stmt);
        match expr {
            Expr::Assign { name, value, .. } => {
                assert_eq!(name.lexeme(), "x");
                assert_literal(value);
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_comma() {
        let stmt = parse_one("a, b;");
        let expr = assert_expression_stmt(&stmt);
        let (op, left, right) = assert_binary(expr);
        assert_eq!(op.lexeme(), ",");
        assert_variable(left);
        assert_variable(right);
    }

    #[test]
    fn test_parse_function_call() {
        let stmt = parse_one("f(a, b);");
        let expr = assert_expression_stmt(&stmt);
        match expr {
            Expr::Call { callee, arguments, .. } => {
                assert_variable(callee);
                assert_eq!(arguments.len(), 2);
            }
            other => panic!("expected Call, got {:?}", other),
        }
    }

    // --- Error cases ---

    #[test]
    fn test_parse_missing_semicolon() {
        let result = parse("var x = 5");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_right_paren() {
        let result = parse("(a");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_assignment_target() {
        let result = parse("(a) = 1;");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_input() {
        let stmts = parse("").expect("empty input should parse");
        assert!(stmts.is_empty());
    }

    #[test]
    fn test_parse_multiple_statements() {
        let stmts = parse("var a = 1; print a; a = 2;").expect("parse failed");
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_parse_function_declaration() {
        let stmts = parse("fun foo(x, y) { return x + y; }").expect("parse failed");
        match &stmts[0] {
            Stmt::Function { name, params, body } => {
                assert_eq!(name.lexeme(), "foo");
                assert_eq!(params.len(), 2);
                assert_eq!(body.len(), 1);
                assert!(matches!(&body[0], Stmt::Return { .. }));
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_class_declaration() {
        let stmts = parse("class Foo { bar() { print 1; } }").expect("parse failed");
        match &stmts[0] {
            Stmt::Class { name, superclass, methods } => {
                assert_eq!(name.lexeme(), "Foo");
                assert!(superclass.is_none());
                assert_eq!(methods.len(), 1);
            }
            other => panic!("expected Class, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_class_with_superclass() {
        let stmts = parse("class Foo < Bar { }").expect("parse failed");
        match &stmts[0] {
            Stmt::Class { name, superclass, .. } => {
                assert_eq!(name.lexeme(), "Foo");
                assert!(superclass.is_some());
            }
            other => panic!("expected Class, got {:?}", other),
        }
    }

    #[test]
    fn test_each_expr_has_unique_id() {
        let stmt = parse_one("1 + 2;");
        let expr = assert_expression_stmt(&stmt);
        let (_, left, right) = assert_binary(expr);
        // Every Expr node should have a unique id
        assert_ne!(expr.id(), left.id());
        assert_ne!(expr.id(), right.id());
        assert_ne!(left.id(), right.id());
    }
}