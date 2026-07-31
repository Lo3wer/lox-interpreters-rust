mod chunk;
#[cfg(any(feature = "debug_print_code", feature = "debug_trace_execution"))]
mod debug;
mod value;
mod vm;

use crate::chunk::{Chunk, OpCode};
#[cfg(feature = "debug_print_code")]
use crate::debug::disassemble_chunk;
use crate::vm::VM;

fn main() {
    let mut chunk = Chunk::new();
    let mut vm = VM::new();
    chunk.write_constant(1.2, 1);
    chunk.write_constant(3.4, 1);
    chunk.write_opcode(OpCode::OpAdd, 1);
    chunk.write_constant(5.6,1);
    chunk.write_opcode(OpCode::OpDivide, 1);
    chunk.write_opcode(OpCode::OpNegate, 1);
    chunk.write_opcode(OpCode::OpReturn, 1);
    #[cfg(feature = "debug_print_code")]
    disassemble_chunk(&chunk, "test_program");
    vm.interpret(&chunk);
}
