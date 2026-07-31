use crate::datastructs::exceptions::ResolveError;
use crate::datastructs::token::Token;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::expr::Expr;
use crate::evaluator::Evaluator;
use crate::datastructs::resolver_values::{ClassType, FunctionType};
use std::collections::HashMap;

pub struct Resolver<'a> {
    evaluator: &'a mut Evaluator,
    scopes: Vec<HashMap<String, bool>>,
    current_function: Option<FunctionType>,
    current_class: Option<ClassType>,
}

impl<'a> Resolver<'a> {
    pub fn new(evaluator: &'a mut Evaluator) -> Self {
        Resolver { evaluator, scopes: Vec::new(), current_function: None, current_class: None }
    }

    pub fn resolve(&mut self, statements: &[Stmt]) -> Result<(), ResolveError> {
        for statement in statements {
            self.resolve_stmt(statement)?;
        }
        Ok(())
    }

    fn resolve_stmt(&mut self, statement: &Stmt) -> Result<(), ResolveError> {
        match statement {
            Stmt::Block { statements } => {
                self.begin_scope();
                self.resolve(statements)?;
                self.end_scope();
            }
            Stmt::Var { name, initializer } => {
                self.declare(name)?;
                self.resolve_expr(initializer)?;
                self.define(name);
            }
            Stmt::Function { name, params, body } => {
                self.declare(name)?;
                self.define(name);
                self.resolve_function(params, body, FunctionType::Function)?;
            }
            Stmt::Expression { expression } => {
                self.resolve_expr(expression)?;
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.resolve_expr(condition)?;
                self.resolve_stmt(then_branch)?;
                if let Some(else_stmt) = else_branch {
                    self.resolve_stmt(else_stmt)?;
                }
            }
            Stmt::Print { expression } => {
                self.resolve_expr(expression)?;
            }
            Stmt::Return { keyword, value } => {
                if self.current_function.is_none() {
                    return Err(ResolveError {
                        token: keyword.clone(),
                        message: "Cannot return from top-level code.".to_string(),
                    });
                }
                if self.current_function == Some(FunctionType::Initializer) {
                    return Err(ResolveError {
                        token: keyword.clone(),
                        message: "Cannot return a value from an initializer.".to_string(),
                    });
                }
                if let Some(val) = value {
                    self.resolve_expr(val)?;
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition)?;
                self.resolve_stmt(body)?;
            }
            Stmt::Class { name, superclass, methods } => {
                let enclosing_class = self.current_class.take();
                self.current_class = Some(ClassType::Class);
                self.declare(name)?;
                self.define(name);

                if let Some(superclass_expr) = superclass {
                    if let Expr::Variable { name: superclass_name, .. } = superclass_expr.as_ref() {
                        if name.lexeme() == superclass_name.lexeme() {
                            return Err(ResolveError {
                                token: superclass_name.clone(),
                                message: "A class cannot inherit from itself.".to_string(),
                            });
                        }
                    }
                    self.current_class = Some(ClassType::Subclass);
                    self.resolve_expr(&superclass_expr)?;
                    self.begin_scope();
                    self.scopes.last_mut().unwrap().insert("super".to_string(), true);
                }

                self.begin_scope();
                self.scopes.last_mut().unwrap().insert("this".to_string(), true);

                for method in methods {
                    if let Stmt::Function { name: _method_name, params, body } = method {
                        if name.lexeme() == "init" {
                            self.resolve_function(params, body, FunctionType::Initializer)?;
                        } else {
                            self.resolve_function(params, body, FunctionType::Method)?;
                        }
                    }
                }

                self.end_scope();
                if superclass.is_some() {
                    self.end_scope();
                }
                self.current_class = enclosing_class;
            }
        }
        Ok(())
    }

    fn resolve_function(&mut self, params: &[Token], body: &[Stmt], function_type: FunctionType) -> Result<(), ResolveError> {
        let enclosing_function = self.current_function.take();
        self.current_function = Some(function_type);
        self.begin_scope();
        for param in params {
            self.declare(param)?;
            self.define(param);
        }
        self.resolve(body)?;
        self.end_scope();
        self.current_function = enclosing_function;
        Ok(())
    }

    fn resolve_expr(&mut self, expression: &Expr) -> Result<(), ResolveError> {
        match expression {
            Expr::Assign { name, value, .. } => {
                self.resolve_expr(value)?;
                self.resolve_local(expression, name);
            }
            Expr::Variable { name, .. } => {
                if let Some(scope) = self.scopes.last() {
                    if let Some(false) = scope.get(name.lexeme()) {
                        return Err(ResolveError {
                            token: name.clone(),
                            message: "Cannot read local variable in its own initializer.".to_string(),
                        });
                    }
                }
                self.resolve_local(expression, name);
            }
            Expr::Binary { left, operator: _, right, .. } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)?;
            }
            Expr::Call { callee, paren: _, arguments, .. } => {
                self.resolve_expr(callee)?;
                for argument in arguments {
                    self.resolve_expr(argument)?;
                }
            }
            Expr::Get { object, name: _, .. } => {
                self.resolve_expr(object)?;
            }
            Expr::Grouping { expression, .. } => {
                self.resolve_expr(expression)?;
            }
            Expr::Literal { value: _, .. } => {}
            Expr::Logical { left, operator: _, right, .. } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)?;
            }
            Expr::Set { object, name: _, value, .. } => {
                self.resolve_expr(object)?;
                self.resolve_expr(value)?;
            }
            Expr::Super { keyword, method: _, .. } => {
                if self.current_class.is_none() {
                    return Err(ResolveError {
                        token: keyword.clone(),
                        message: "Cannot use 'super' outside of a class.".to_string(),
                    });
                } else if self.current_class != Some(ClassType::Subclass) {
                    return Err(ResolveError {
                        token: keyword.clone(),
                        message: "Cannot use 'super' in a class with no superclass.".to_string(),
                    });
                }
                self.resolve_local(expression, keyword);
            }
            Expr::This { keyword, .. } => {
                if self.current_class.is_none() {
                    return Err(ResolveError {
                        token: keyword.clone(),
                        message: "Cannot use 'this' outside of a class.".to_string(),
                    });
                }
                self.resolve_local(expression, keyword);
            }
            Expr::Unary { operator: _, right, .. } => {
                self.resolve_expr(right)?;
            }
            Expr::Ternary { condition, then_branch, else_branch, .. } => {
                self.resolve_expr(condition)?;
                self.resolve_expr(then_branch)?;
                self.resolve_expr(else_branch)?;
            }
        }
        Ok(())
    }

    fn resolve_local(&mut self, expression: &Expr, name: &Token) {
        for (i, scope) in self.scopes.iter().rev().enumerate() {
            if scope.contains_key(name.lexeme()) {
                self.evaluator.resolve(expression, i);
                return;
            }
        }
    }

    fn declare(&mut self, name: &Token) -> Result<(), ResolveError> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(name.lexeme()) {
                return Err(ResolveError {
                    token: name.clone(),
                    message: "Variable with this name already declared in this scope.".to_string(),
                });
            }
            scope.insert(name.lexeme().to_string(), false);
        }
        Ok(())
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