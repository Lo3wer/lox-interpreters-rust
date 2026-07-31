use crate::common::{Chunk, OpCode};
use crate::value::print_value;

pub fn disassemble_chunk(chunk: &Chunk, name: &str) {
    println!("== {} ==", name);

    let mut offset = 0;
    while offset < chunk.code().len() {
        offset = disassemble_instruction(chunk, offset);
    }
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}

fn constant_instruction(name: &str, chunk: &Chunk, offset: usize) -> usize {
    let constant = chunk.code()[offset + 1] as usize;
    print!("{} {:4} '", name, constant);
    print_value(chunk.constants()[constant]);
    println!("'");
    offset + 2
}

fn constant_long_instruction(name: &str, chunk: &Chunk, offset: usize) -> usize {
    let constant = ((chunk.code()[offset + 1] as usize) << 16)
        | ((chunk.code()[offset + 2] as usize) << 8)
        | (chunk.code()[offset + 3] as usize);
    print!("{} {:4} '", name, constant);
    print_value(chunk.constants()[constant]);
    println!("'");
    offset + 4
}

pub fn disassemble_instruction(chunk: &Chunk, offset: usize) -> usize {
    print!("{:04} {:4} ", offset, chunk.get_line(offset));
    match OpCode::from_u8(chunk.code()[offset]) {
        Some(OpCode::OpReturn) => simple_instruction("OP_RETURN", offset),
        Some(OpCode::OpConstant) => constant_instruction("OP_CONSTANT", chunk, offset),
        Some(OpCode::OpConstantLong) => constant_long_instruction("OP_CONSTANT_LONG", chunk, offset),
        _ => {
            println!("Unknown opcode {}", chunk.code()[offset]);
            offset + 1
        }
    }
}
