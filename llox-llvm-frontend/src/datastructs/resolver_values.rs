#[derive(PartialEq)]
pub enum FunctionType {
    Function,
    Method,
    Initializer,
}

#[derive(PartialEq)]
pub enum ClassType {
    Class,
    Subclass,
}