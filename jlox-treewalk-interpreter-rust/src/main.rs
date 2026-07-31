use lox_ast_rust::lox::Lox;
use std::process;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut lox = Lox::new();

    let result = if args.len() > 2 {
        println!("Usage: jlox [script]");
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
