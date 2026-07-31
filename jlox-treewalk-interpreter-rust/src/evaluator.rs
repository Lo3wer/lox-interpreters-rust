use crate::datastructs::exceptions::RuntimeException;
use crate::datastructs::expr::Expr;
use crate::datastructs::token::{Token, TokenType};
use crate::datastructs::literal::Literal;
use crate::datastructs::callable::{Callable, FunctionCallable, Class, ClockCallable};
use crate::datastructs::stmt::Stmt;
use crate::environment::{Environment, EnvRef};

use std::cmp::Ordering;
use std::rc::Rc;
use std::collections::HashMap;

pub struct Evaluator {
    globals: EnvRef,
    locals: HashMap<Expr, usize>,
    environment: EnvRef,
}

impl Evaluator {
    pub fn new() -> Self {
        let globals = Environment::new();
        let environment = globals.clone();
        globals.borrow_mut().define(&Token::identifier("clock"), Literal::Callable(Rc::new(ClockCallable)));
        let locals = HashMap::new();
        Evaluator { globals, locals, environment }
    }

    pub fn interpret(&mut self, statements: Vec<Stmt>) -> Result<(), RuntimeException> {
        for statement in statements {
            self.execute(&statement)?;
        }
        Ok(())
    }

    pub fn resolve(&mut self, expression: &Expr, depth: usize) {
        self.locals.insert(expression.clone(), depth);
    }

    fn execute(&mut self, stmt: &Stmt) -> Result<(), RuntimeException> {
        match stmt {
            Stmt::Expression { expression } => self.expression_stmt(expression),
            Stmt::If { condition, then_branch, else_branch } => {
                self.if_stmt(condition, then_branch, else_branch)
            }
            Stmt::Print { expression } => self.print_stmt(expression),
            Stmt::Var { name, initializer } => self.var_stmt(name, initializer),
            Stmt::Function { name, params, body } => self.function_stmt(name, params, body),
            Stmt::Return { keyword, value } => self.return_stmt(keyword, value),
            Stmt::Block { statements } => {
                self.execute_block(statements, Environment::new_enclosed(self.environment.clone()))
            }
            Stmt::While { condition, body } => self.while_stmt(condition, body),
            Stmt::Class { name, superclass, methods } => self.class_stmt(name, superclass, methods),
        }
    }

    pub fn execute_block(&mut self, statements: &[Stmt], environment: EnvRef) -> Result<(), RuntimeException> {
        let previous = self.environment.clone();
        self.environment = environment;

        // If any statement errors, we still want to restore the environment
        // before propagating the error, so don't use `?` directly here.
        let result = (|| {
            for statement in statements {
                self.execute(statement)?;
            }
            Ok(())
        })();

        self.environment = previous;
        result
    }

    fn class_stmt(&mut self, name: &Token, superclass: &Option<Box<Expr>>, methods: &[Stmt]) -> Result<(), RuntimeException> {
        let mut superclass_value: Option<Rc<Class>> = None;
        if let Some(superclass_expr) = superclass {
            let value = self.evaluate(superclass_expr)?;
            if let Literal::Callable(callable) = value {
                if let Some(class) = callable.as_any().downcast_ref::<Class>() {
                    superclass_value = Some(Rc::new(class.clone()));
                } else {
                    return Err(self.runtime_error(name, "Superclass must be a class."));
                }
            } else {
                return Err(self.runtime_error(name, "Superclass must be a class."));
            }
        }
        self.environment.borrow_mut().define(name, Literal::Nil);

        if superclass_value.is_some() {
            self.environment = Environment::new_enclosed(self.environment.clone());
            let sv = superclass_value.clone().unwrap();
            self.environment.borrow_mut().define(&Token::identifier("super"), Literal::Callable(sv as Rc<dyn Callable>));
        }

        let has_superclass = superclass_value.is_some();

        let mut method_map = HashMap::new();
        for method in methods {
            if let Stmt::Function { name: method_name, params, body } = method {
                let function = Rc::new(FunctionCallable::new(params.to_vec(), body.to_vec(), self.environment.clone(), method_name.lexeme() == "init"));
                method_map.insert(method_name.lexeme().to_string(), function as Rc<dyn Callable>);
            }
        }
        let class = Literal::Callable(Rc::new(Class::new(name.lexeme().to_string(), superclass_value, method_map)));

        if has_superclass {
            let enclosing = self.environment.borrow().enclosing.as_ref().unwrap().clone();
            self.environment = enclosing;
        }

        self.environment.borrow_mut().assign(name, class)?;
        Ok(())
    }

    fn return_stmt(&mut self, _keyword: &Token, value: &Option<Box<Expr>>) -> Result<(), RuntimeException> {
        let return_value = if let Some(expr) = value {
            self.evaluate(expr)?
        } else {
            Literal::Nil
        };
        Err(RuntimeException::Return {
            value: return_value,
        })
    }

    fn expression_stmt(&mut self, expression: &Expr) -> Result<(), RuntimeException> {
        self.evaluate(expression)?;
        Ok(())
    }

    fn if_stmt(
        &mut self,
        condition: &Expr,
        then_branch: &Stmt,
        else_branch: &Option<Box<Stmt>>,
    ) -> Result<(), RuntimeException> {
        let condition_value = self.evaluate(condition)?;
        if self.is_truthy(&condition_value) {
            self.execute(then_branch)?;
        } else if let Some(else_stmt) = else_branch {
            self.execute(else_stmt)?;
        }
        Ok(())
    }

    fn while_stmt(&mut self, condition: &Expr, body: &Stmt) -> Result<(), RuntimeException> {
        loop {
            let value = self.evaluate(condition)?;
            if !self.is_truthy(&value) {
                break;
            }
            self.execute(body)?;
        }
        Ok(())
    }

    fn print_stmt(&mut self, expression: &Expr) -> Result<(), RuntimeException> {
        let value = self.evaluate(expression)?;
        println!("{}", value);
        Ok(())
    }

    fn var_stmt(&mut self, name: &Token, initializer: &Expr) -> Result<(), RuntimeException> {
        let value = self.evaluate(initializer)?;
        self.environment.borrow_mut().define(name, value);
        Ok(())
    }

    fn function_stmt(&mut self, name: &Token, params: &[Token], body: &[Stmt]) -> Result<(), RuntimeException> {
        let function = Literal::Callable(Rc::new(FunctionCallable::new(params.to_vec(), body.to_vec(), self.environment.clone(), false)));
        self.environment.borrow_mut().define(name, function);
        Ok(())
    }

    fn evaluate(&mut self, expr: &Expr) -> Result<Literal, RuntimeException> {
        match expr {
            Expr::Binary { left, operator, right, .. } => {
                let left_val = self.evaluate(left)?;
                let right_val = self.evaluate(right)?;
                self.evaluate_binary(&left_val, operator, &right_val)
            }
            Expr::Call { callee, paren, arguments, .. } => {
                let callee_val = self.evaluate(callee)?;
                let mut arg_values = Vec::new();
                for arg in arguments {
                    arg_values.push(self.evaluate(arg)?);
                }
                self.evaluate_call(&callee_val, paren, &arg_values)
            }
            Expr::Get { object, name, .. } => {
                let object_val = self.evaluate(object)?;
                self.evaluate_get(&object_val, name)
            }
            Expr::Grouping { expression, .. } => self.evaluate(expression),
            Expr::Literal { value, .. } => Ok(value.clone()),
            Expr::Logical { left, operator, right, .. } => self.evaluate_logical(left, operator, right),
            Expr::Set { object, name, value, .. } => {
                let object_val = self.evaluate(object)?;
                let value_val = self.evaluate(value)?;
                self.evaluate_set(&object_val, name, &value_val)
            },
            Expr::Unary { operator, right, .. } => {
                let right_val = self.evaluate(right)?;
                self.evaluate_unary(operator, &right_val)
            }
            Expr::Ternary { condition, then_branch, else_branch, .. } => {
                let condition_val = self.evaluate(condition)?;
                self.evaluate_ternary(&condition_val, then_branch, else_branch)
            }
            Expr::Variable { name, .. } => self.look_up_variable(name, expr),
            Expr::This { keyword, .. } => self.look_up_variable(keyword, expr),
            Expr::Assign { name, value, .. } => {
                let depth = self.locals.get(expr).copied();
                self.evaluate_assign(name, value, depth)
            }
            Expr::Super { keyword, method, .. } => {
                if let Some(depth) = self.locals.get(expr) {
                    let superclass = self.environment.borrow().get_at(*depth, keyword)?;
                    let this = self.environment.borrow().get_at(*depth - 1, &Token::identifier("this"))?;
                    if let Literal::Callable(callable) = superclass {
                        if let Some(class) = callable.as_any().downcast_ref::<Class>() {
                            if let Some(method) = class.find_method(&method.lexeme()) {
                                if let Literal::Instance(instance) = this {
                                    return Ok(Literal::Callable(method.bind(instance)));
                                }
                            } else {
                                return Err(self.runtime_error(method, &format!("Undefined property '{}'.", method.lexeme())));
                            }
                        }
                    }
                }
                Err(self.runtime_error(keyword, "Superclass not found."))
            }
        }
    }

    fn evaluate_assign(&mut self, name: &Token, value: &Expr, depth: Option<usize>) -> Result<Literal, RuntimeException> {
        let value_val = self.evaluate(value)?;
        if let Some(depth) = depth {
            self.environment.borrow_mut().assign_at(depth, name, value_val.clone())?;
        } else {
            self.globals.borrow_mut().assign(name, value_val.clone())?;
        }
        Ok(value_val)
    }

    fn look_up_variable(&self, name: &Token, expr: &Expr) -> Result<Literal, RuntimeException> {
        if let Some(depth) = self.locals.get(expr) {           
            return self.environment.borrow().get_at(*depth, name);
        }
        self.globals.borrow().get(name)
    }

    fn evaluate_call(&mut self, callee: &Literal, paren: &Token, arguments: &[Literal]) -> Result<Literal, RuntimeException> {
        match callee {
            Literal::Callable(callable) => {
                if arguments.len() != callable.arity() {
                    return Err(self.runtime_error(paren, &format!("Expected {} arguments but got {}.", callable.arity(), arguments.len())));
                }
                callable.call(self, arguments)
            }
            _ => Err(self.runtime_error(paren, "Can only call functions and classes.")),
        }
    }

    fn evaluate_get(&mut self, object: &Literal, name: &Token) -> Result<Literal, RuntimeException> {
        match object {
            Literal::Instance(instance) => instance.borrow().get(instance.clone(), name),
            _ => Err(self.runtime_error(name, "Only instances have properties.")),
        }
    }

    fn evaluate_set(&mut self, object: &Literal, name: &Token, value: &Literal) -> Result<Literal, RuntimeException> {
        match object {
            Literal::Instance(instance) => {
                instance.borrow_mut().set(name, value.clone());
                Ok(value.clone())
            }
            _ => Err(self.runtime_error(name, "Only instances have fields.")),
        }
    }

    fn evaluate_unary(&self, operator: &Token, right: &Literal) -> Result<Literal, RuntimeException> {
        match operator.token_type() {
            TokenType::Minus => match right {
                Literal::Number(n) => Ok(Literal::Number(-n)),
                _ => Err(self.runtime_error(operator, "Operand must be a number.")),
            },
            TokenType::Bang => Ok(Literal::Bool(!self.is_truthy(right))),
            _ => Err(self.runtime_error(operator, "Unknown unary operator.")),
        }
    }

    /// Evaluates a binary expression based on the operator and operands.
    fn evaluate_binary(&self, left: &Literal, operator: &Token, right: &Literal) -> Result<Literal, RuntimeException> {
        match operator.token_type() {
            TokenType::Plus => self.addition_binary(left, operator, right),
            TokenType::Minus => self.numeric_binary(left, operator, right, |l, r| Literal::Number(l - r)),
            TokenType::Star => self.numeric_binary(left, operator, right, |l, r| Literal::Number(l * r)),
            TokenType::Slash => self.division_binary(left, operator, right),
            TokenType::Greater => self.comparison_binary(left, operator, right, |ord| ord == Ordering::Greater),
            TokenType::GreaterEqual => self.comparison_binary(left, operator, right, |ord| ord != Ordering::Less),
            TokenType::Less => self.comparison_binary(left, operator, right, |ord| ord == Ordering::Less),
            TokenType::LessEqual => self.comparison_binary(left, operator, right, |ord| ord != Ordering::Greater),
            TokenType::EqualEqual => Ok(Literal::Bool(self.is_equal(left, right))),
            TokenType::BangEqual => Ok(Literal::Bool(!self.is_equal(left, right))),
            _ => Err(self.runtime_error(operator, "Unknown binary operator.")),
        }
    }

    /// Handles addition for numbers and string concatenation.
    fn addition_binary(&self, left: &Literal, operator: &Token, right: &Literal) -> Result<Literal, RuntimeException> {
        match (left, right) {
                (Literal::Number(l), Literal::Number(r)) => {
                    Ok(Literal::Number(l + r))
                }
                (Literal::String(l), Literal::String(r)) => {
                    Ok(Literal::String(format!("{}{}", l, r)))
                }
                (Literal::String(l), Literal::Number(r)) => {
                    Ok(Literal::String(format!("{}{}", l, r)))
                }
                (Literal::Number(l), Literal::String(r)) => {
                    Ok(Literal::String(format!("{}{}", l, r)))
                }
                _ => Err(self.runtime_error(operator, "Operands must be two numbers or atleast one string.")),
        }
    }

    /// Handles numeric binary operations like subtraction, multiplication, and division.
    fn numeric_binary<F>(&self, left: &Literal, operator: &Token, right: &Literal, combine: F) -> Result<Literal, RuntimeException>
    where
        F: Fn(f64, f64) -> Literal,
    {
        match (left, right) {
            (Literal::Number(l), Literal::Number(r)) => {
                Ok(combine(*l, *r))
            }
            _ => Err(self.runtime_error(operator, "Operands must be numbers.")),
        }
    }

    /// Handles division and checks for division by zero.
    fn division_binary(&self, left: &Literal, operator: &Token, right: &Literal) -> Result<Literal, RuntimeException> {
        match (left, right) {
            (Literal::Number(l), Literal::Number(r)) if *r == 0.0 => {
                Err(self.runtime_error(operator, "Division by zero."))
            }
            _ => self.numeric_binary(left, operator, right, |l, r| Literal::Number(l / r)),
        }
    }

    /// Uses the std::cmp::Ordering to compare two literals and applies the provided comparison function.
    fn comparison_binary<F>(&self, left: &Literal, operator: &Token, right: &Literal, combine: F) -> Result<Literal, RuntimeException>
    where
        F: Fn(Ordering) -> bool,
    {
        let ordering = match (left, right) {
            (Literal::Number(l), Literal::Number(r)) => l.partial_cmp(r),
            (Literal::String(l), Literal::String(r)) => l.partial_cmp(r),
            _ => None,
        };

        match ordering {
            Some(ord) => Ok(Literal::Bool(combine(ord))),
            None => Err(self.runtime_error(operator, "Operands must be two numbers or two strings.")),
        }
    }

    fn runtime_error(&self, token: &Token, message: &str) -> RuntimeException {
        RuntimeException::Error {
            token: token.clone(),
            message: message.to_string(),
        }
    }

    fn evaluate_ternary(&mut self, condition: &Literal, then_branch: &Expr, else_branch: &Expr) -> Result<Literal, RuntimeException> {
        if let Literal::Bool(true) = condition {
            self.evaluate(then_branch)
        } else {
            self.evaluate(else_branch)
        }
    }

    fn evaluate_logical(&mut self, left: &Expr, operator: &Token, right: &Expr) -> Result<Literal, RuntimeException> {
        let left_literal = self.evaluate(left)?;
        match operator.token_type() {
            TokenType::Or => {
                if self.is_truthy(&left_literal) {
                    Ok(left_literal)
                } else {
                    self.evaluate(right)
                }
            }
            TokenType::And => {
                if !self.is_truthy(&left_literal) {
                    Ok(left_literal)
                } else {
                    self.evaluate(right)
                }
            }
            _ => Err(self.runtime_error(operator, "Unknown logical operator.")),
        }
    }

    fn is_truthy(&self, literal: &Literal) -> bool {
        match literal {
            Literal::Bool(b) => *b,
            Literal::Nil => false,
            _ => true,
        }
    }

    fn is_equal(&self, a: &Literal, b: &Literal) -> bool {
        match (a, b) {
            (Literal::Number(l), Literal::Number(r)) => l == r,
            (Literal::String(l), Literal::String(r)) => l == r,
            (Literal::Bool(l), Literal::Bool(r)) => l == r,
            (Literal::Nil, Literal::Nil) => true,
            _ => false,
        }
    }
}
