use super::expr::Expr;
use super::token::Token;

#[derive(Debug, Clone)]
pub enum Stmt {
    Block {
        statements: Vec<Stmt>,
    },
    Class {
        name: Token,
        superclass: Option<Box<Expr>>,
        methods: Vec<Stmt>,
    },
    Expression {
        expression: Box<Expr>,
    },
    Function {
        name: Token,
        params: Vec<Token>,
        body: Vec<Stmt>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    Print {
        expression: Box<Expr>,
    },
    Return {
        keyword: Token,
        value: Option<Box<Expr>>,
    },
    Var {
        name: Token,
        initializer: Box<Expr>,
    },
    While {
        condition: Box<Expr>,
        body: Box<Stmt>,
    },
}