use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::datastructs::literal::Literal;
use crate::datastructs::callable::Callable;
use crate::datastructs::instance::Instance;
use crate::datastructs::exceptions::RuntimeException;
use crate::evaluator::Evaluator;

pub struct ClockCallable;

impl fmt::Display for ClockCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<fn clock>")
    }
}

impl Callable for ClockCallable {
    fn as_any(&self) -> &dyn std::any::Any { self }

    fn bind(&self, _instance: Rc<RefCell<Instance>>) -> Rc<dyn Callable> {
        Rc::new(ClockCallable)
    }

    fn arity(&self) -> usize {
        0
    }

    fn call(&self, _evaluator: &mut Evaluator, _arguments: &[Literal]) -> Result<Literal, RuntimeException> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        Ok(Literal::Number(current_time.as_secs_f64()))
    }
}