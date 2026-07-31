pub type Value = f64;

pub struct ValueArray {
    values: Vec<Value>
}

impl ValueArray {
    pub fn new() -> Self {
        ValueArray { values: Vec::new() }
    }

    pub fn write(&mut self, value: Value) {
        self.values.push(value);
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

pub fn print_value(value: Value) {
    print!("{}",value)
}

