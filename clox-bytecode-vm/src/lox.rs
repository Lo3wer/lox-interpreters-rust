use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

use crate::chunk::Chunk;
use crate::compiler::compile;
use crate::vm::{InterpretResult, VM};

pub struct Lox {
    vm: VM,
    had_error: bool,
    had_runtime_error: bool,
}

impl Lox {
    pub fn new() -> Self {
        Lox {
            vm: VM::new(),
            had_error: false,
            had_runtime_error: false,
        }
    }

    pub fn run_file(&mut self, path: &str) -> io::Result<()> {
        let source = read_file(path);
        self.run(&source);

        if self.had_error {
            process::exit(65);
        }
        if self.had_runtime_error {
            process::exit(70);
        }
        Ok(())
    }

    pub fn run_prompt(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut line = String::new();

        loop {
            print!("> ");
            io::stdout().flush()?;
            line.clear();
            if stdin.lock().read_line(&mut line)? == 0 {
                break;
            }
            self.run(line.trim_end());
            self.had_error = false;
            self.had_runtime_error = false;
        }
        Ok(())
    }

    fn run(&mut self, source: &str) {
        let mut chunk = Chunk::new();

        let result = match compile(source, &mut chunk) {
            Ok(()) => {
                #[cfg(feature = "debug_print_code")]
                crate::debug::disassemble_chunk(&chunk, "code");
                self.vm.interpret(&chunk)
            }
            Err(()) => InterpretResult::InterpretCompileError,
        };

        match result {
            InterpretResult::InterpretOk => {}
            InterpretResult::InterpretCompileError => self.had_error = true,
            InterpretResult::InterpretRuntimeError => self.had_runtime_error = true,
        }
    }
}

fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(source) => source,
        Err(_) => {
            eprintln!("Could not open file \"{path}\".");
            process::exit(74);
        }
    }
}
