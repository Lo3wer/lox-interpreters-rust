use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use crate::datastructs::literal::Literal;
use crate::datastructs::instance::Instance;
use crate::datastructs::token::Token;
use crate::datastructs::stmt::Stmt;
use crate::datastructs::exceptions::RuntimeException;
use crate::datastructs::callable::Callable;
use crate::environment::{Environment, EnvRef};
use crate::evaluator::Evaluator;

pub struct FunctionCallable {
    name: String,
    params: Vec<Token>,
    body: Vec<Stmt>,
    closure: EnvRef,
    is_initializer: bool,
}

impl FunctionCallable {
    pub fn new(name: String, params: Vec<Token>, body: Vec<Stmt>, closure: EnvRef, is_initializer: bool) -> Self {
        FunctionCallable { name, params, body, closure, is_initializer }
    }
}

impl fmt::Display for FunctionCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<fn {}>", self.name)
    }
}

impl Callable for FunctionCallable {
    fn as_any(&self) -> &dyn std::any::Any { self }

    fn bind(&self, instance: Rc<RefCell<Instance>>) -> Rc<dyn Callable> {
        let bound_env = Environment::new_enclosed(self.closure.clone());
        bound_env.borrow_mut().define(&Token::identifier("this"), Literal::Instance(instance));
        Rc::new(FunctionCallable {
            name: self.name.clone(),
            params: self.params.clone(),
            body: self.body.clone(),
            closure: bound_env,
            is_initializer: self.is_initializer,
        })
    }

    fn arity(&self) -> usize {
        self.params.len()
    }

    fn call(&self, evaluator: &mut Evaluator, arguments: &[Literal]) -> Result<Literal, RuntimeException> {
        let function_env = Environment::new_enclosed(self.closure.clone());
        for (param, arg) in self.params.iter().zip(arguments.iter()) {
            function_env.borrow_mut().define(param, arg.clone());
        }
        let result = match evaluator.execute_block(&self.body, function_env) {
            Ok(()) => Literal::Nil,
            Err(RuntimeException::Return { value }) => value,
            Err(err) => return Err(err),
        };
        if self.is_initializer {
            return self.closure.borrow().get_at(0, &Token::identifier("this"));
        }
        Ok(result)
    }
}