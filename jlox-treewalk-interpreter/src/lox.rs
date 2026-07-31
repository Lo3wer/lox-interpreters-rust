use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::evaluator::Evaluator;
use crate::datastructs::token::TokenType;
use crate::datastructs::exceptions::{LexError, ParseError, ResolveError, RuntimeException};
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

pub struct Lox {
    had_error: bool,
    had_runtime_error: bool,
    prompt: bool,
}

impl Lox {
    pub fn new() -> Self {
        Lox { had_error: false, had_runtime_error: false, prompt: false }
    }

    fn report(&mut self, line: usize, where_: &str, message: &str) {
        eprintln!("[line {line}] Error{where_}: {message}");
        self.had_error = true;
    }

    fn report_lex_error(&mut self, error: &LexError) {
        self.report(error.line, "", &error.message);
    }

    fn report_parse_error(&mut self, error: &ParseError) {
        let where_ = if error.token.token_type() == TokenType::Eof {
            " at end".to_string()
        } else {
            format!(" at '{}'", error.token.lexeme())
        };
        self.report(error.token.line(), &where_, &error.message);
    }

    fn report_resolve_error(&mut self, error: &ResolveError) {
        self.report(error.token.line(), "", &error.message);
        self.had_error = true;
    }

    fn report_runtime_error(&mut self, error: &RuntimeException) {
        match error {
            RuntimeException::Error { token, message } => {
                self.report(token.line(), "", message);
            }
            RuntimeException::Return { value: _ } => {
                // Handle return statement errors if needed
            }
        }
        self.had_runtime_error = true;
    }

    pub fn run_file(&mut self, path: &str) -> io::Result<()> {
        self.prompt = false;
        let contents = fs::read_to_string(path)?;
        self.run(&contents);
        if self.had_error {
            process::exit(65);
        }
        if self.had_runtime_error {
            process::exit(70);
        }
        Ok(())
    }

    pub fn run_prompt(&mut self) -> io::Result<()> {
        self.prompt = true;
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
        // lexing
        let mut lexer = Lexer::new(source.to_string());
        let (tokens, lex_errors) = lexer.scan_tokens();
        for error in &lex_errors {
            self.report_lex_error(error);
        }
        if !lex_errors.is_empty() {
            return;
        }

        //parsing
        let mut parser = Parser::new(tokens);
        let execution_type = if self.prompt {
            parser.parse_prompt_line()
        } else {
            parser.parse()
        };
        let statements = match execution_type {
            Ok(statements) => statements,
            Err(parse_errors) => {
                for error in &parse_errors {
                    self.report_parse_error(error);
                }
                return;
            }
        };

        // resolution
        let mut evaluator = Evaluator::new();
        let mut resolver = crate::resolver::Resolver::new(&mut evaluator);
        match resolver.resolve(&statements) {
            Ok(()) => {},
            Err(error) => {
                self.report_resolve_error(&error);
                return;
            }
        }

        // evaluation
        match evaluator.interpret(statements) {
            Ok(()) => {},
            Err(error) => self.report_runtime_error(&error),
        }
    }
}
