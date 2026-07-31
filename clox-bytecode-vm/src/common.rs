use crate::value::{Value, ValueArray};

#[repr(u8)]
pub enum OpCode {
    OpReturn = 0,
    OpConstant = 1,
    OpConstantLong = 2,
}

impl OpCode {
    pub fn from_u8(byte: u8) -> Option<OpCode> {
        match byte {
            0 => Some(OpCode::OpReturn),
            1 => Some(OpCode::OpConstant),
            2 => Some(OpCode::OpConstantLong),
            _ => None,
        }
    }
}

pub struct Chunk {
    code: Vec<u8>,
    constants: ValueArray,
    lines: Vec<(usize, u32)>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: ValueArray::new(),
            lines: Vec::new(),
        }
    }

    pub fn write_byte(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.write_line(line);
    }

    pub fn write_opcode(&mut self, opcode: OpCode, line: u32) {
        self.write_byte(opcode as u8, line);
    }

    pub fn write_constant(&mut self, value: Value, line: u32) {
        let index = self.add_constant(value);
        if index <= u8::MAX as usize {
            self.write_opcode(OpCode::OpConstant, line);
            self.write_byte(index as u8, line);
        } else {
            self.write_opcode(OpCode::OpConstantLong, line);
            self.write_byte((index >> 16) as u8, line);
            self.write_byte((index >> 8) as u8, line);
            self.write_byte(index as u8, line);
        }
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.write(value);
        self.constants.values().len() - 1
    }

    pub fn get_line(&self, offset: usize) -> u32 {
        let mut remaining = offset;
        for &(count, line) in &self.lines {
            if remaining < count {
                return line;
            }
            remaining -= count;
        }
        0
    }

    fn write_line(&mut self, line: u32) {
        match self.lines.last_mut() {
            Some((count, last_line)) if *last_line == line => *count += 1,
            _ => self.lines.push((1, line)),
        }
    }

    pub fn code(&self) -> &[u8] {
        &self.code
    }

    pub fn constants(&self) -> &[Value] {
        self.constants.values()
    }
}
