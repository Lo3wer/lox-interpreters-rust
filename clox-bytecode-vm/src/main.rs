mod common;
mod debug;
mod value;

use common::{Chunk, OpCode};

fn main() {
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.2);
    chunk.write_opcode(OpCode::OpConstant, 1);
    chunk.write_byte(constant as u8, 1);
    chunk.write_opcode(OpCode::OpReturn, 1);
    debug::disassemble_chunk(&chunk, "test chunk");
}
