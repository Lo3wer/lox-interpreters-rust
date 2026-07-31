mod common;
mod debug;
mod value;

use common::{Chunk, OpCode};

fn main() {
    let mut chunk = Chunk::new();
    for i in 0..300 {
        chunk.add_constant(i as f64);
    }
    chunk.write_constant(42.0, 1);
    chunk.write_opcode(OpCode::OpReturn, 1);
    chunk.write_opcode(OpCode::OpReturn, 2);
    chunk.write_opcode(OpCode::OpReturn, 2);
    debug::disassemble_chunk(&chunk, "test chunk");
}
