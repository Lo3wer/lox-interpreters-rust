use crate::datastructs::literal::Literal;
use crate::datastructs::exceptions::RuntimeException;
use crate::datastructs::token::Token;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

pub type EnvRef = Rc<RefCell<Environment>>;

pub struct Environment {
    pub enclosing: Option<EnvRef>,
    values: HashMap<String, Literal>,
}

impl Environment {
    // global constructor
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Environment {
            enclosing: None,
            values: HashMap::new(),
        }))
    }

    // local constructor
    pub fn new_enclosed(enclosing: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Environment {
            enclosing: Some(enclosing),
            values: HashMap::new(),
        }))
    }

    pub fn get(&self, name: &Token) -> Result<Literal, RuntimeException> {
        if let Some(value) = self.values.get(name.lexeme()) {
            return Ok(value.clone());
        } else if let Some(parent) = &self.enclosing {
            return parent.borrow().get(name);
        }
        Err(RuntimeException::Error {
            token: name.clone(),
            message: format!("Undefined variable '{}'.", name.lexeme()),
        })
    }

    pub fn get_at(&self, distance: usize, name: &Token) -> Result<Literal, RuntimeException> {
        if distance == 0 {
            self.get(name)
        } else if let Some(parent) = &self.enclosing {
            parent.borrow().get_at(distance - 1, name)
        } else {
            Err(RuntimeException::Error {
                token: name.clone(),
                message: format!("Undefined variable '{}'.", name.lexeme()),
            })
        }
    }

    pub fn define(&mut self, name: &Token, value: Literal) {
        self.values.insert(name.lexeme().to_string(), value);
    }

    pub fn assign(&mut self, name: &Token, value: Literal) -> Result<(), RuntimeException> {
        if self.values.contains_key(name.lexeme()) {
            self.values.insert(name.lexeme().to_string(), value);
            Ok(())
        } else if let Some(parent) = &self.enclosing {
            parent.borrow_mut().assign(name, value)
        } else {
            Err(RuntimeException::Error {
                token: name.clone(),
                message: format!("Undefined variable '{}'.", name.lexeme()),
            })
        }
    }

    pub fn assign_at(&mut self, distance: usize, name: &Token, value: Literal) -> Result<(), RuntimeException> {
        if distance == 0 {
            self.assign(name, value)
        } else if let Some(parent) = &self.enclosing {
            parent.borrow_mut().assign_at(distance - 1, name, value)
        } else {
            Err(RuntimeException::Error {
                token: name.clone(),
                message: format!("Undefined variable '{}'.", name.lexeme()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(name: &str) -> Token {
        Token::identifier(name)
    }

    #[test]
    fn test_global_define_and_get() {
        let env = Environment::new();
        env.borrow_mut().define(&tok("x"), Literal::Number(10.0));
        let val = env.borrow().get(&tok("x")).unwrap();
        assert_eq!(val, Literal::Number(10.0));
    }

    #[test]
    fn test_global_assign() {
        let env = Environment::new();
        env.borrow_mut().define(&tok("x"), Literal::Number(1.0));
        env.borrow_mut().assign(&tok("x"), Literal::Number(2.0)).unwrap();
        let val = env.borrow().get(&tok("x")).unwrap();
        assert_eq!(val, Literal::Number(2.0));
    }

    #[test]
    fn test_global_assign_undefined() {
        let env = Environment::new();
        let result = env.borrow_mut().assign(&tok("undef"), Literal::Nil);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_undefined() {
        let env = Environment::new();
        let result = env.borrow().get(&tok("nosuch"));
        assert!(result.is_err());
    }

    #[test]
    fn test_nested_define_and_get() {
        let global = Environment::new();
        let child = Environment::new_enclosed(global.clone());

        global.borrow_mut().define(&tok("a"), Literal::Number(1.0));
        child.borrow_mut().define(&tok("b"), Literal::Number(2.0));

        // child can see its own var
        assert_eq!(child.borrow().get(&tok("b")).unwrap(), Literal::Number(2.0));
        // child can see parent var via walk
        assert_eq!(child.borrow().get(&tok("a")).unwrap(), Literal::Number(1.0));
    }

    #[test]
    fn test_get_at_distance() {
        let global = Environment::new();
        let child = Environment::new_enclosed(global.clone());

        global.borrow_mut().define(&tok("x"), Literal::Number(99.0));
        // child has its own "x" shadowing parent
        child.borrow_mut().define(&tok("x"), Literal::Number(1.0));

        // get_at(0) on child reads child's x
        assert_eq!(child.borrow().get_at(0, &tok("x")).unwrap(), Literal::Number(1.0));
        // get_at(1) on child walks up to global's x
        assert_eq!(child.borrow().get_at(1, &tok("x")).unwrap(), Literal::Number(99.0));
    }

    #[test]
    fn test_assign_at_distance() {
        let global = Environment::new();
        let child = Environment::new_enclosed(global.clone());

        global.borrow_mut().define(&tok("x"), Literal::Number(0.0));
        child.borrow_mut().define(&tok("x"), Literal::Number(0.0));

        // assign_at(1) modifies parent's x
        child.borrow_mut().assign_at(1, &tok("x"), Literal::Number(999.0)).unwrap();
        assert_eq!(child.borrow().get(&tok("x")).unwrap(), Literal::Number(0.0));
        assert_eq!(global.borrow().get(&tok("x")).unwrap(), Literal::Number(999.0));
    }

    #[test]
    fn test_shadow_restores_parent() {
        let global = Environment::new();
        let child = Environment::new_enclosed(global.clone());

        global.borrow_mut().define(&tok("x"), Literal::Number(10.0));
        child.borrow_mut().define(&tok("x"), Literal::Number(20.0));

        assert_eq!(child.borrow().get(&tok("x")).unwrap(), Literal::Number(20.0));
        drop(child);

        // global unaffected
        assert_eq!(global.borrow().get(&tok("x")).unwrap(), Literal::Number(10.0));
    }
}