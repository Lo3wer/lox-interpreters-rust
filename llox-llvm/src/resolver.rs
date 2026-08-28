use crate::datastructs::exceptions::ResolveError;
use crate::datastructs::token::Token;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::expr::Expr;
use crate::datastructs::resolver_values::{ClassType, FunctionType};
use std::collections::HashMap;

pub struct Resolver {
    scopes: Vec<HashMap<String, bool>>,
    current_function: Option<FunctionType>,
    current_class: Option<ClassType>,
    errors: Vec<ResolveError>,
    locals: HashMap<Expr, usize>,
}

impl Resolver {
    pub fn new() -> Self {
        Resolver {
            scopes: Vec::new(),
            current_function: None,
            current_class: None,
            errors: Vec::new(),
            locals: HashMap::new(),
        }
    }

    pub fn locals(&self) -> &HashMap<Expr, usize> {
        &self.locals
    }

    pub fn resolve(&mut self, statements: &[Stmt]) -> Result<(), Vec<ResolveError>> {
        self.resolve_stmts(statements);
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn error(&mut self, token: &Token, message: &str) {
        self.errors.push(ResolveError {
            token: token.clone(),
            message: message.to_string(),
        });
    }

    fn resolve_stmts(&mut self, statements: &[Stmt]) {
        for statement in statements {
            self.resolve_stmt(statement);
        }
    }

    fn resolve_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Block { statements } => {
                self.begin_scope();
                self.resolve_stmts(statements);
                self.end_scope();
            }
            Stmt::Var { name, initializer } => {
                self.declare(name);
                self.resolve_expr(initializer);
                self.define(name);
            }
            Stmt::Function { name, params, body } => {
                self.declare(name);
                self.define(name);
                self.resolve_function(params, body, FunctionType::Function);
            }
            Stmt::Expression { expression } => {
                self.resolve_expr(expression);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(else_stmt) = else_branch {
                    self.resolve_stmt(else_stmt);
                }
            }
            Stmt::Print { expression } => {
                self.resolve_expr(expression);
            }
            Stmt::Return { keyword, value } => {
                if self.current_function.is_none() {
                    self.error(keyword, "Can't return from top-level code.");
                }
                if let Some(val) = value {
                    if self.current_function == Some(FunctionType::Initializer) {
                        self.error(keyword, "Can't return a value from an initializer.");
                    }
                    self.resolve_expr(val);
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            Stmt::Class { name, superclass, methods } => {
                let enclosing_class = self.current_class.take();
                self.current_class = Some(ClassType::Class);
                self.declare(name);
                self.define(name);

                if let Some(superclass_expr) = superclass {
                    if let Expr::Variable { name: superclass_name, .. } = superclass_expr.as_ref() {
                        if name.lexeme() == superclass_name.lexeme() {
                            self.error(superclass_name, "A class can't inherit from itself.");
                        }
                    }
                    self.current_class = Some(ClassType::Subclass);
                    self.resolve_expr(superclass_expr);
                    self.begin_scope();
                    self.scopes.last_mut().unwrap().insert("super".to_string(), true);
                }

                self.begin_scope();
                self.scopes.last_mut().unwrap().insert("this".to_string(), true);

                for method in methods {
                    if let Stmt::Function { name: method_name, params, body } = method {
                        let function_type = if method_name.lexeme() == "init" {
                            FunctionType::Initializer
                        } else {
                            FunctionType::Method
                        };
                        self.resolve_function(params, body, function_type);
                    }
                }

                self.end_scope();
                if superclass.is_some() {
                    self.end_scope();
                }
                self.current_class = enclosing_class;
            }
        }
    }

    fn resolve_function(&mut self, params: &[Token], body: &[Stmt], function_type: FunctionType) {
        let enclosing_function = self.current_function.take();
        self.current_function = Some(function_type);
        self.begin_scope();
        for param in params {
            self.declare(param);
            self.define(param);
        }
        self.resolve_stmts(body);
        self.end_scope();
        self.current_function = enclosing_function;
    }

    fn resolve_expr(&mut self, expression: &Expr) {
        match expression {
            Expr::Assign { name, value, .. } => {
                self.resolve_expr(value);
                self.resolve_local(expression, name);
            }
            Expr::Variable { name, .. } => {
                if let Some(scope) = self.scopes.last() {
                    if let Some(false) = scope.get(name.lexeme()) {
                        self.error(name, "Can't read local variable in its own initializer.");
                    }
                }
                self.resolve_local(expression, name);
            }
            Expr::Binary { left, operator: _, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Call { callee, paren: _, arguments, .. } => {
                self.resolve_expr(callee);
                for argument in arguments {
                    self.resolve_expr(argument);
                }
            }
            Expr::Get { object, name: _, .. } => {
                self.resolve_expr(object);
            }
            Expr::Grouping { expression, .. } => {
                self.resolve_expr(expression);
            }
            Expr::Literal { value: _, .. } => {}
            Expr::Logical { left, operator: _, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Set { object, name: _, value, .. } => {
                self.resolve_expr(object);
                self.resolve_expr(value);
            }
            Expr::Super { keyword, method: _, .. } => {
                if self.current_class.is_none() {
                    self.error(keyword, "Can't use 'super' outside of a class.");
                } else if self.current_class != Some(ClassType::Subclass) {
                    self.error(keyword, "Can't use 'super' in a class with no superclass.");
                }
                self.resolve_local(expression, keyword);
            }
            Expr::This { keyword, .. } => {
                if self.current_class.is_none() {
                    self.error(keyword, "Can't use 'this' outside of a class.");
                }
                self.resolve_local(expression, keyword);
            }
            Expr::Unary { operator: _, right, .. } => {
                self.resolve_expr(right);
            }
            Expr::Ternary { condition, then_branch, else_branch, .. } => {
                self.resolve_expr(condition);
                self.resolve_expr(then_branch);
                self.resolve_expr(else_branch);
            }
        }
    }

    fn resolve_local(&mut self, expression: &Expr, name: &Token) {
        for (i, scope) in self.scopes.iter().rev().enumerate() {
            if scope.contains_key(name.lexeme()) {
                self.locals.insert(expression.clone(), i);
                return;
            }
        }
    }

    fn declare(&mut self, name: &Token) {
        let duplicate = self
            .scopes
            .last()
            .map(|scope| scope.contains_key(name.lexeme()))
            .unwrap_or(false);
        if duplicate {
            self.error(name, "Already a variable with this name in this scope.");
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.lexeme().to_string(), false);
        }
    }

    fn define(&mut self, name: &Token) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.lexeme().to_string(), true);
        }
    }

    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        self.scopes.pop();
    }
}
