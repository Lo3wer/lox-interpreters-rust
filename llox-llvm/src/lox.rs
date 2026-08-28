use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::resolver::Resolver;
use crate::datastructs::token::TokenType;
use crate::datastructs::exceptions::{LexError, ParseError, ResolveError};
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

pub struct Lox {
    had_error: bool,
    prompt: bool,
}

impl Lox {
    pub fn new() -> Self {
        Lox { had_error: false, prompt: false }
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
        let where_ = if error.token.token_type() == TokenType::Eof {
            " at end".to_string()
        } else {
            format!(" at '{}'", error.token.lexeme())
        };
        self.report(error.token.line(), &where_, &error.message);
    }

    pub fn run_file(&mut self, path: &str) -> io::Result<()> {
        self.prompt = false;
        let contents = fs::read_to_string(path)?;
        self.run(&contents);
        if self.had_error {
            process::exit(65);
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

        // parsing
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
        let mut resolver = Resolver::new();
        match resolver.resolve(&statements) {
            Ok(()) => {},
            Err(errors) => {
                for error in &errors {
                    self.report_resolve_error(error);
                }
                return;
            }
        }

        // Phase 3: codegen consumes `statements` and `resolver.locals()` here.
        let _locals = resolver.locals();
    }
}
