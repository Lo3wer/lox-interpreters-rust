mod chunk;
mod debug;
mod value;
mod vm;

use crate::chunk::{Chunk, OpCode};
use crate::debug::disassemble_chunk;
use crate::vm::VM;

fn main() {
    let mut chunk = Chunk::new();
    let mut vm = VM::new();
    chunk.write_constant(42.0, 1);
    chunk.write_opcode(OpCode::OpNegate, 1);
    chunk.write_opcode(OpCode::OpReturn, 2);
    disassemble_chunk(&chunk, "test_program");
    vm.interpret(&chunk);
}
