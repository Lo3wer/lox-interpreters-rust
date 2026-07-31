use crate::chunk::{Chunk,OpCode};
use crate::value::{print_value, Value};
#[cfg(feature = "debug_trace_execution")]
use crate::debug::disassemble_instruction;

macro_rules! pop {
    ($vm:expr) => {
        match $vm.stack.pop() {
            Some(value) => value,
            None => return InterpretResult::InterpretRuntimeError,
        }
    };
}

macro_rules! binary_op {
    ($vm:expr, $op:tt) => {{
        let b = pop!($vm);
        let a = pop!($vm);
        $vm.stack.push(a $op b);
    }};
}

pub struct VM<'a> { 
    chunk: Option<&'a Chunk>,
    ip: usize,
    stack: Vec<Value>,
}

impl<'a> VM<'a> {
    pub fn new() -> Self {
        VM {
            chunk: None,
            ip: 0,
            stack: Vec::new(),
        }
    }

    pub fn interpret(&mut self, chunk: &'a Chunk) -> InterpretResult {
        self.chunk = Some(chunk);
        self.ip = 0;
        self.run()
    }

    fn run(&mut self) -> InterpretResult {
        let chunk = self.chunk.unwrap();
        let code = chunk.code();
        loop {
            #[cfg(feature = "debug_trace_execution")] {
                print!("          ");
                for value in &self.stack {
                    print!("[ ");
                    print_value(*value);
                    print!(" ]");
                }
                println!("");
                disassemble_instruction(chunk, self.ip);
            }
            match OpCode::from_u8(self.read_byte(code)) {
                Some(OpCode::OpReturn) => {
                    let value = pop!(self);
                    print_value(value);
                    println!("");
                    return InterpretResult::InterpretOk;
                }
                Some(OpCode::OpConstant) => {
                    let constant = self.read_constant(chunk, code);
                    self.stack.push(constant);
                }
                Some(OpCode::OpConstantLong) => {
                    let constant = self.read_long_constant(chunk, code);
                    self.stack.push(constant);
                }
                Some(OpCode::OpNegate) => {
                    let value = pop!(self);
                    self.stack.push(-value);
                }
                Some(OpCode::OpAdd) => binary_op!(self, +),
                Some(OpCode::OpSubtract) => binary_op!(self, -),
                Some(OpCode::OpMultiply) => binary_op!(self, *),
                Some(OpCode::OpDivide) => binary_op!(self, /),
                None => {
                    return InterpretResult::InterpretRuntimeError;
                }
            }
        }
    }

    fn read_byte(&mut self, code: &[u8]) -> u8 {
        let byte = code[self.ip];
        self.ip += 1;
        byte
    }

    fn read_constant(&mut self, chunk: &Chunk, code: &[u8]) -> Value {
        chunk.constants()[self.read_byte(code) as usize]
    }

    fn read_long_constant(&mut self, chunk: &Chunk, code: &[u8]) -> Value {
        let index = ((self.read_byte(code) as usize) << 16)
            | ((self.read_byte(code) as usize) << 8)
            | (self.read_byte(code) as usize);
        chunk.constants()[index]
    }
}

pub enum InterpretResult {
    InterpretOk,
    InterpretCompileError,
    InterpretRuntimeError
}