use crate::expr::Expr;
use crate::values::Literal;

pub struct AstPrinter {

}

impl AstPrinter {
    pub fn new() -> Self {
        AstPrinter {}
    }
    pub fn print(&self, expr: &Expr) -> String {
        match expr {
            Expr::Binary { left, operator, right } => {
                format!("({} {} {})", operator.lexeme(), self.print(left), self.print(right))
            }
            Expr::Grouping { expression } => {
                format!("(group {})", self.print(expression))
            }
            Expr::Literal { value } => {
                match value {
                    Literal::Number(n) => n.to_string(),
                    Literal::String(s) => s.clone(),
                    Literal::Bool(b) => b.to_string(),
                    Literal::Nil => "nil".to_string(),
                }
            }
            Expr::Unary { operator, right } => {
                format!("({} {})", operator.lexeme(), self.print(right))
            }
            Expr::Ternary { condition, then_branch, else_branch } => {
                format!("(ternary {} {} {})", self.print(condition), self.print(then_branch), self.print(else_branch))
            }
        }
    }

    // pub fn print_rpn(&self, expr: &Expr) -> String {
    //     match expr {
    //         Expr::Binary { left, operator, right } => {
    //             format!("{} {} {}", self.print_rpn(left), self.print_rpn(right), operator.lexeme())
    //         }
    //         Expr::Grouping { expression } => {
    //             format!("{}", self.print_rpn(expression))
    //         }
    //         Expr::Literal { value } => {
    //             match value {
    //                 Some(Literal::Number(n)) => n.to_string(),
    //                 Some(Literal::String(s)) => s.clone(),
    //                 Some(Literal::Bool(b)) => b.to_string(),
    //                 Some(Literal::Nil) => "nil".to_string(),
    //                 None => "nil".to_string(),
    //             }
    //         }
    //         Expr::Unary { operator, right } => {
    //             format!("({} {})", operator.lexeme(), self.print_rpn(right))
    //         }
    //     }
    // }
}