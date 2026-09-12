# Lox in Rust (Monorepo)

A monorepo containing three independent Rust implementations of the Lox
programming language, a shared Lox test suite, and a language-agnostic test
runner that executes the suite against any interpreter's CLI.

## Layout

| Directory                          | Description                                              |
|------------------------------------|----------------------------------------------------------|
| `jlox-treewalk-interpreter/`       | jlox: AST tree-walk interpreter                          |
| `clox-bytecode-vm/`                | clox: bytecode virtual machine (scaffolded, WIP)         |
| `llox-llvm-jit/`                   | llox: LLVM JIT compiler (scaffolded, WIP)                |
| `test/`                            | Shared Lox test suite (`.lox` files with `// expect:` annotations) |
| `test-runner/`                     | Test runner that runs the shared suite against an interpreter CLI |
| `documentation/`                   | Notes on the Lox grammar and the jlox pipeline           |
| `programs/`                        | Sample Lox programs                                      |

## Building and running

### jlox (tree-walk interpreter)

```sh
cargo run --manifest-path jlox-treewalk-interpreter/Cargo.toml -- path/to/script.lox
cargo run --manifest-path jlox-treewalk-interpreter/Cargo.toml   # interactive REPL
```

### clox (bytecode VM)

```sh
cargo build --manifest-path clox-bytecode-vm/Cargo.toml
```

The clox crate is a bare `cargo init` skeleton — the VM is not implemented yet.

### llox (LLVM JIT compiler)

```sh
cargo build --manifest-path llox-llvm-jit/Cargo.toml
cargo run --manifest-path llox-llvm-jit/Cargo.toml -- path/to/script.lox
```

llox compiles Lox source to native code at runtime via LLVM (using the
[`inkwell`](https://crates.io/crates/inkwell) bindings). Building it requires
LLVM 22 installed and discoverable by `llvm-config` (the crate pins the
`llvm22-1` feature). Only `print` statements over simple numeric expressions
are code-generated so far; the lexer, parser, and resolver are reused from
jlox, and the remaining statement/expression forms are not yet emitted.

Two opt-in debug features dump the generated IR/assembly before running:

```sh
cargo run --manifest-path llox-llvm-jit/Cargo.toml --features debug_dump_ir -- path/to/script.lox
cargo run --manifest-path llox-llvm-jit/Cargo.toml --features debug_dump_assembly -- path/to/script.lox
```

## Testing

### Unit tests

Each implementation keeps its unit tests (`#[cfg(test)]` modules) inside its
own crate:

```sh
cargo test --manifest-path jlox-treewalk-interpreter/Cargo.toml
cargo test --manifest-path llox-llvm-jit/Cargo.toml
```

### Shared Lox test suite

The suite in `test/` is shared by all implementations. Build an interpreter
binary, then point the test runner at it:

```sh
cargo build --manifest-path jlox-treewalk-interpreter/Cargo.toml

cargo run --manifest-path test-runner/Cargo.toml -- \
  jlox-treewalk-interpreter/target/debug/jlox-treewalk-interpreter \
  test
```

On Windows, the jlox binary is named `jlox-treewalk-interpreter.exe`, so use
`jlox-treewalk-interpreter/target/debug/jlox-treewalk-interpreter.exe` instead.

To match exactly what CI runs (skipping the chapter and limit tests that a
complete tree-walk interpreter can never satisfy):

```sh
cargo run --manifest-path test-runner/Cargo.toml -- \
  jlox-treewalk-interpreter/target/debug/jlox-treewalk-interpreter \
  test \
  --skip test/scanning \
  --skip test/expressions \
  --skip test/limit
```

The runner:

- discovers every `.lox` file under `test/` (the `benchmark/` folder is
  always skipped),
- runs each file through the given interpreter,
- compares stdout, stderr, and the exit code against the annotations in the
  file (`// expect:`, `// [line N] Error ...`, `// expect runtime error:`),
- prints a `PASS`/`FAIL` line per test plus failure details, and
- exits non-zero if any test fails.

Options:

- `--language <tag>` — also match implementation-specific error annotations
  like `// [treewalk line N]` (jlox) or `// [bytecode vm line N]` (clox) in
  addition to bare `// [line N]` ones. Default: bare only.
- `--skip <substring>` — skip any test whose path contains the substring.
  May be repeated.

See `cargo run --manifest-path test-runner/Cargo.toml -- --help` for details.

## CI

GitHub Actions (`.github/workflows/rust.yml`) builds jlox, runs its unit
tests, and runs the shared suite against the jlox binary. The lexer/parser
chapter tests (`test/scanning`, `test/expressions`) and the clox limit tests
(`test/limit`) are skipped via `--skip`, matching how the official
craftinginterpreters suite treats a complete jlox. The workflow also installs
LLVM 22 and builds llox, running its unit tests. The clox steps are stubbed
with instructions for when the VM is implemented.
