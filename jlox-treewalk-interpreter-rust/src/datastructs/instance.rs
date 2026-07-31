use std::fmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::datastructs::literal::Literal;
use crate::datastructs::callable::Callable;
use crate::datastructs::exceptions::RuntimeException;
use crate::datastructs::token::Token;

pub struct Instance {
    class: String,
    methods: HashMap<String, Rc<dyn Callable>>,
    fields: HashMap<String, Literal>,
}

impl Instance {
    pub fn new(class: String, methods: HashMap<String, Rc<dyn Callable>>) -> Self {
        Instance { class, methods, fields: HashMap::new() }
    }

    pub fn get(&self, this: Rc<RefCell<Instance>>, name: &Token) -> Result<Literal, RuntimeException> {
        if let Some(value) = self.fields.get(name.lexeme()) {
            return Ok(value.clone());
        }
        if let Some(method) = self.methods.get(name.lexeme()) {
            return Ok(Literal::Callable(method.bind(this)));
        }
        Err(RuntimeException::Error {
            token: name.clone(),
            message: format!("Undefined property '{}'.", name.lexeme()),
        })
    }

    pub fn set(&mut self, name: &Token, value: Literal) {
        self.fields.insert(name.lexeme().to_string(), value);
    }
}

impl fmt::Display for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} instance", self.class)
    }
}