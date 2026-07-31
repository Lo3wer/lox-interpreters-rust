mod common;
mod debug;
mod value;
mod vm;

use crate::common::{Chunk, OpCode};
use crate::debug::disassemble_chunk;
use crate::vm::VM;

fn main() {
    let mut chunk = Chunk::new();
    let mut vm = VM::new();
    for i in 0..300 {
        chunk.add_constant(i as f64);
    }
    chunk.write_constant(42.0, 1);
    chunk.write_opcode(OpCode::OpReturn, 2);
    disassemble_chunk(&chunk, "test_program");
    vm.interpret(&chunk);
}
