use crate::value::{Value, ValueArray};

#[repr(u8)]
pub enum OpCode {
    OpConstant = 0,
    OpReturn = 1,
}

impl OpCode {
    pub fn from_u8(byte: u8) -> Option<OpCode> {
        match byte {
            0 => Some(OpCode::OpConstant),
            1 => Some(OpCode::OpReturn),
            _ => None,
        }
    }
}

pub struct Chunk {
    code: Vec<u8>,
    constants: ValueArray,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: ValueArray::new(),
        }
    }

    pub fn write_byte(&mut self, byte: u8, _line: u32) {
        self.code.push(byte);
    }

    pub fn write_opcode(&mut self, opcode: OpCode, line: u32) {
        self.write_byte(opcode as u8, line);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.write(value);
        self.constants.values().len() - 1
    }

    pub fn code(&self) -> &[u8] {
        &self.code
    }

    pub fn constants(&self) -> &[Value] {
        self.constants.values()
    }
}
