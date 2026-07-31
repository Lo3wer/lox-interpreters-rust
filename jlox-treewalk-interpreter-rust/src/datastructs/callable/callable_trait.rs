use std::any::Any;
use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use crate::datastructs::literal::Literal;
use crate::datastructs::instance::Instance;
use crate::datastructs::exceptions::RuntimeException;
use crate::evaluator::Evaluator;

pub trait Callable: fmt::Display {
    fn as_any(&self) -> &dyn Any;
    fn bind(&self, instance: Rc<RefCell<Instance>>) -> Rc<dyn Callable>;
    fn arity(&self) -> usize;
    fn call(&self, evaluator: &mut Evaluator, arguments: &[Literal]) -> Result<Literal, RuntimeException>;
}