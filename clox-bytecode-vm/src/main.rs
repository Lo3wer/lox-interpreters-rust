mod chunk;
mod compiler;
#[cfg(any(feature = "debug_print_code", feature = "debug_trace_execution"))]
mod debug;
mod value;
mod vm;
mod lox;

use crate::lox::Lox;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut lox = Lox::new();

    let result = if args.len() > 2 {
        println!("Usage: clox [script]");
        process::exit(64);
    } else if args.len() == 2 {
        lox.run_file(&args[1])
    } else {
        lox.run_prompt()
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(74);
    }
}
