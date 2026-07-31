use crate::common::{Chunk,OpCode};
use crate::value::{print_value, Value};
use crate::debug::disassemble_instruction;

pub struct VM<'a> { 
    chunk: Option<&'a Chunk>,
    ip: usize
}

impl<'a> VM<'a> {
    pub fn new() -> Self {
        VM {
            chunk: None,
            ip: 0
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
            #[cfg(feature = "debug_trace_execution")]
            disassemble_instruction(chunk, self.ip);
            match OpCode::from_u8(self.read_byte(code)) {
                Some(OpCode::OpReturn) => {
                    return InterpretResult::InterpretOk;
                }
                Some(OpCode::OpConstant) => {
                    let constant = self.read_constant(chunk, code);
                    print_value(constant);
                    println!();
                }
                Some(OpCode::OpConstantLong) => {
                    let index = ((self.read_byte(code) as usize) << 16)
                        | ((self.read_byte(code) as usize) << 8)
                        | (self.read_byte(code) as usize);
                    let constant = chunk.constants()[index];
                    print_value(constant);
                    println!();
                }
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
}

pub enum InterpretResult {
    InterpretOk,
    InterpretCompileError,
    InterpretRuntimeError
}