use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use crate::datastructs::callable::Callable;
use crate::datastructs::instance::Instance;

#[derive(Clone)]
pub enum Literal {
    Bool(bool),
    String(String),
    Number(f64),
    Callable(Rc<dyn Callable>),
    Instance(Rc<RefCell<Instance>>),
    Nil,
}

impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Literal::Bool(a), Literal::Bool(b)) => a == b,
            (Literal::String(a), Literal::String(b)) => a == b,
            (Literal::Number(a), Literal::Number(b)) => a == b,
            (Literal::Nil, Literal::Nil) => true,
            (Literal::Callable(_), Literal::Callable(_)) => false,
            (Literal::Instance(a), Literal::Instance(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Bool(v) => write!(f, "{:?}", v),
            Literal::String(v) => write!(f, "{:?}", v),
            Literal::Number(v) => write!(f, "{:?}", v),
            Literal::Callable(c) => write!(f, "{}", c),
            Literal::Instance(i) => write!(f, "{}", i.borrow()),
            Literal::Nil => write!(f, "Nil"),
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Bool(value) => write!(f, "{}", value),
            Literal::String(value) => write!(f, "{}", value),
            Literal::Number(value) => {
                if value.fract() == 0.0 {
                    write!(f, "{:.0}", value)
                } else {
                    write!(f, "{}", value)
                }
            }
            Literal::Nil => write!(f, "nil"),
            Literal::Callable(c) => write!(f, "{}", c),
            Literal::Instance(i) => write!(f, "{}", i.borrow()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_eq_bool() {
        assert_eq!(Literal::Bool(true), Literal::Bool(true));
        assert_ne!(Literal::Bool(true), Literal::Bool(false));
    }

    #[test]
    fn test_literal_eq_string() {
        assert_eq!(Literal::String("hi".into()), Literal::String("hi".into()));
        assert_ne!(Literal::String("hi".into()), Literal::String("bye".into()));
    }

    #[test]
    fn test_literal_eq_number() {
        assert_eq!(Literal::Number(3.14), Literal::Number(3.14));
        assert_ne!(Literal::Number(1.0), Literal::Number(2.0));
    }

    #[test]
    fn test_literal_eq_nil() {
        assert_eq!(Literal::Nil, Literal::Nil);
    }

    #[test]
    fn test_literal_cross_type_not_equal() {
        assert_ne!(Literal::Number(1.0), Literal::String("1".into()));
        assert_ne!(Literal::Bool(true), Literal::Nil);
    }

    #[test]
    fn test_literal_display_bool() {
        assert_eq!(format!("{}", Literal::Bool(true)), "true");
        assert_eq!(format!("{}", Literal::Bool(false)), "false");
    }

    #[test]
    fn test_literal_display_integer_number() {
        assert_eq!(format!("{}", Literal::Number(42.0)), "42");
    }

    #[test]
    fn test_literal_display_float_number() {
        let s = format!("{}", Literal::Number(3.14));
        assert!(s == "3.14" || s == "3.1400000000000001");
    }

    #[test]
    fn test_literal_display_string() {
        assert_eq!(format!("{}", Literal::String("hello".into())), "hello");
    }

    #[test]
    fn test_literal_display_nil() {
        assert_eq!(format!("{}", Literal::Nil), "nil");
    }
}