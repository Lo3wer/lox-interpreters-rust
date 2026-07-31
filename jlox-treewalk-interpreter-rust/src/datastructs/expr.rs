use super::token::Token;
use super::literal::Literal;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum Expr {
    Assign{
        name: Token,
        value: Box<Expr>,
        id: usize,
    },
    Binary {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
        id: usize,
    },
    Call {
        callee: Box<Expr>,
        paren: Token,
        arguments: Vec<Expr>,
        id: usize,
    },
    Get {
        object: Box<Expr>,
        name: Token,
        id: usize,
    },
    Grouping {
        expression: Box<Expr>,
        id: usize,
    },
    Literal {
        value: Literal,
        id: usize,
    },
    Logical {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
        id: usize,
    },
    Set {
        object: Box<Expr>,
        name: Token,
        value: Box<Expr>,
        id: usize,
    },
    Super {
        keyword: Token,
        method: Token,
        id: usize,
    },
    This {
        keyword: Token,
        id: usize,
    },
    Unary {
        operator: Token,
        right: Box<Expr>,
        id: usize,
    },
    Ternary {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        id: usize,
    },
    Variable {
        name: Token,
        id: usize,
    }
}

impl Expr {
    pub fn id(&self) -> usize {
        match self {
            Expr::Assign { id, .. }
            | Expr::Binary { id, .. }
            | Expr::Call { id, .. }
            | Expr::Get { id, .. }
            | Expr::Grouping { id, .. }
            | Expr::Literal { id, .. }
            | Expr::Logical { id, .. }
            | Expr::Set { id, .. }
            | Expr::Super { id, .. }
            | Expr::This { id, .. }
            | Expr::Unary { id, .. }
            | Expr::Ternary { id, .. }
            | Expr::Variable { id, .. } => *id,
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Expr {}

impl Hash for Expr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datastructs::literal::Literal;
    use crate::datastructs::token::Token;
    use crate::datastructs::token::TokenType;

    fn make_token(lexeme: &str) -> Token {
        Token::new(TokenType::Identifier, lexeme.into(), None, 0)
    }

    #[test]
    fn test_expr_eq_same_id() {
        let a = Expr::Literal { value: Literal::Number(1.0), id: 42 };
        let b = Expr::Literal { value: Literal::Number(2.0), id: 42 };
        assert_eq!(a, b);
    }

    #[test]
    fn test_expr_eq_different_id() {
        let a = Expr::Literal { value: Literal::Number(1.0), id: 1 };
        let b = Expr::Literal { value: Literal::Number(1.0), id: 2 };
        assert_ne!(a, b);
    }

    #[test]
    fn test_expr_cross_variant_same_id() {
        let a = Expr::Literal { value: Literal::Nil, id: 7 };
        let b = Expr::Variable { name: make_token("x"), id: 7 };
        assert_eq!(a, b);
    }

    #[test]
    fn test_expr_id_method() {
        let e = Expr::Binary {
            left: Box::new(Expr::Literal { value: Literal::Number(1.0), id: 10 }),
            operator: make_token("+"),
            right: Box::new(Expr::Literal { value: Literal::Number(2.0), id: 11 }),
            id: 5,
        };
        assert_eq!(e.id(), 5);
    }

    #[test]
    fn test_expr_hash_uses_id() {
        use std::collections::HashSet;
        let a = Expr::Literal { value: Literal::Nil, id: 99 };
        let b = Expr::Variable { name: make_token("x"), id: 99 };
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn test_expr_clone_preserves_id() {
        let e = Expr::Variable { name: make_token("x"), id: 77 };
        let cloned = e.clone();
        assert_eq!(cloned.id(), 77);
        assert_eq!(e, cloned);
    }
}