use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const USAGE: &str = "\
Usage: lox-test-runner <interpreter> <test-directory> [options]

Runs every .lox file in <test-directory> through <interpreter> and compares
stdout, stderr, and the exit code against the annotations embedded in each
test file. The runner is language-agnostic: any interpreter CLI that accepts
a Lox script path as its first argument can be tested.

Annotations understood (one per line):
  // expect: <output>              an expected stdout line, in order
  // [line N] Error <message>      an expected compile error
  // [c line N] / [java line N]    implementation-specific compile errors
  // Error <message>               a compile error (uses the comment's line)
  // expect runtime error: <msg>   the expected runtime error message
  // nontest                       marks the file as not a test

Options:
  --language <tag>  Match implementation-specific '[<tag> line N]' error
                    annotations in addition to bare '[line N]' ones.
                    (e.g. --language c or --language java). Default: bare only.
  --skip <substr>   Skip any test whose path contains <substr>.
                    May be given multiple times.
  -h, --help        Show this help.
";

struct Expectations {
    expected_output: Vec<(usize, String)>,
    expected_errors: HashSet<String>,
    runtime_error: Option<(usize, String)>,
    expected_exit_code: i32,
}

fn main() {
    let mut interpreter: Option<String> = None;
    let mut test_dir: Option<String> = None;
    let mut language = String::new();
    let mut skips: Vec<String> = Vec::new();

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--language" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{USAGE}");
                    process::exit(64);
                }
                language = args[i].clone();
            }
            "--skip" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{USAGE}");
                    process::exit(64);
                }
                skips.push(args[i].clone());
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                process::exit(0);
            }
            a if interpreter.is_none() => interpreter = Some(a.to_string()),
            a if test_dir.is_none() => test_dir = Some(a.to_string()),
            a => {
                eprintln!("Unexpected argument '{a}'.\n{USAGE}");
                process::exit(64);
            }
        }
        i += 1;
    }

    let (Some(interpreter), Some(test_dir)) = (interpreter, test_dir) else {
        eprintln!("{USAGE}");
        process::exit(64);
    };

    if !Path::new(&interpreter).is_file() {
        eprintln!("Interpreter executable not found: {interpreter}");
        process::exit(66);
    }

    let mut tests: Vec<PathBuf> = Vec::new();
    collect_tests(Path::new(&test_dir), &mut tests);

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut failures: Vec<(String, Vec<String>)> = Vec::new();

    for path in &tests {
        let display = path.to_string_lossy().replace('\\', "/");
        let is_benchmark = display.split('/').any(|c| c == "benchmark");
        if is_benchmark || skips.iter().any(|s| display.contains(s.as_str())) {
            skipped += 1;
            println!("SKIP  {display}");
            continue;
        }

        let errs = run_test(path, &interpreter, &language);
        if errs.is_empty() {
            passed += 1;
            println!("PASS  {display}");
        } else {
            failed += 1;
            failures.push((display.clone(), errs));
            println!("FAIL  {display}");
        }
    }

    if !failures.is_empty() {
        println!();
        for (path, errs) in &failures {
            println!("----- {path}");
            for err in errs {
                println!("    {err}");
            }
        }
    }

    println!();
    println!(
        "{} passed, {} failed, {} skipped.",
        passed, failed, skipped
    );

    if failed > 0 {
        process::exit(1);
    }
}

fn collect_tests(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Cannot read test directory '{}': {e}", dir.display());
            process::exit(66);
        }
    };

    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_tests(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lox") {
            out.push(path);
        }
    }
}

fn run_test(path: &Path, interpreter: &str, language: &str) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => return vec![format!("Cannot read test file: {e}")],
    };

    let Some(expectations) = parse_test(&content, language) else {
        return Vec::new();
    };

    let output = match Command::new(interpreter).arg(path).output() {
        Ok(output) => output,
        Err(e) => return vec![format!("Failed to run interpreter '{interpreter}': {e}")],
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut failures = Vec::new();
    validate_errors(&mut failures, &stderr, &expectations);
    validate_exit_code(&mut failures, &output.status, &stderr, &expectations);
    validate_output(&mut failures, &stdout, &expectations);
    failures
}

fn parse_test(content: &str, language: &str) -> Option<Expectations> {
    let mut expected_output = Vec::new();
    let mut expected_errors = HashSet::new();
    let mut runtime_error = None;
    let mut expected_exit_code = 0;

    for (idx, raw_line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let line = raw_line.trim_start();

        if line.contains("// nontest") {
            return None;
        }

        if let Some(text) = extract_after(line, "// expect runtime error:") {
            let text = text.trim_start();
            if !text.is_empty() {
                runtime_error = Some((line_num, text.to_string()));
                expected_exit_code = 70;
            }
            continue;
        }

        if let Some(text) = extract_after(line, "// expect:") {
            let text = text.strip_prefix(' ').unwrap_or(text).to_string();
            expected_output.push((line_num, text));
            continue;
        }

        if let Some((number, message)) = parse_bracket_error(line, language) {
            expected_errors.insert(format!("[{number}] {message}"));
            expected_exit_code = 65;
            continue;
        }

        if let Some(pos) = line.find("// Error") {
            let message = line[pos + 3..].trim_start().to_string();
            expected_errors.insert(format!("[{line_num}] {message}"));
            expected_exit_code = 65;
            continue;
        }
    }

    if !expected_errors.is_empty() && runtime_error.is_some() {
        return None;
    }

    Some(Expectations {
        expected_output,
        expected_errors,
        runtime_error,
        expected_exit_code,
    })
}

/// Returns the text after `prefix` when it appears anywhere in `line`.
fn extract_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let pos = line.find(prefix)?;
    Some(&line[pos + prefix.len()..])
}

/// Parses a `// [line N] Error ...` annotation, honoring an optional
/// language tag such as `// [c line N]` or `// [java line N]`.
fn parse_bracket_error(line: &str, language: &str) -> Option<(usize, String)> {
    let rest = line.strip_prefix("// [")?;
    let (tag, after_tag) = rest.split_once("line ")?;
    let tag = tag.trim();
    if !tag.is_empty() && tag != language {
        return None;
    }
    let close = after_tag.find(']')?;
    let number: usize = after_tag[..close].trim().parse().ok()?;
    let message = after_tag[close + 1..].trim_start();
    if !message.starts_with("Error") {
        return None;
    }
    Some((number, message.to_string()))
}

/// Parses an interpreter stderr line like `[line N] Error <message>` or
/// `[line N] Error: <message>` into `(line, message)`.
fn parse_error_line(line: &str) -> Option<(usize, String)> {
    let rest = line.trim_start().strip_prefix('[')?;
    let (_, after_tag) = rest.split_once("line ")?;
    let close = after_tag.find(']')?;
    let number: usize = after_tag[..close].trim().parse().ok()?;
    let message = after_tag[close + 1..].trim_start();
    if !message.starts_with("Error") {
        return None;
    }
    Some((number, message.to_string()))
}

fn validate_errors(failures: &mut Vec<String>, stderr: &str, expectations: &Expectations) {
    let stderr_lines: Vec<&str> = stderr.lines().collect();

    if let Some((expected_line, expected_msg)) = &expectations.runtime_error {
        // Two accepted formats:
        //   Inline: a single stderr line "[line N] Error: <msg>" (jlox style).
        //   Stack:  the first stderr line is the bare message, followed by a
        //           stack trace whose frames look like "[line N] ...".
        let inline = stderr_lines.iter().find_map(|line| {
            parse_error_line(line).and_then(|(number, message)| {
                let message = message.strip_prefix("Error:")?.trim();
                (message == expected_msg.as_str()).then_some(number)
            })
        });
        let bare = stderr_lines
            .first()
            .map(|line| line.trim() == expected_msg.as_str())
            .unwrap_or(false);

        if let Some(number) = inline {
            if number != *expected_line {
                failures.push(format!(
                    "Expected runtime error on line {expected_line} but was reported on line {number}."
                ));
            }
        } else if bare {
            if !stderr_lines
                .iter()
                .any(|line| line.contains(&format!("[line {expected_line}]")))
            {
                failures.push(format!(
                    "Expected runtime error on line {expected_line} but no stack frame references that line."
                ));
            }
        } else {
            failures.push(format!("Expected runtime error '{expected_msg}' but got:"));
            for line in &stderr_lines {
                failures.push(format!("    {line}"));
            }
        }
        return;
    }

    let mut found = HashSet::new();
    let mut unexpected: Vec<String> = Vec::new();
    for line in &stderr_lines {
        if line.trim().is_empty() {
            continue;
        }
        match parse_error_line(line) {
            Some((number, message)) => {
                let error = format!("[{number}] {message}");
                if expectations.expected_errors.contains(&error) {
                    found.insert(error);
                } else if unexpected.len() < 10 {
                    unexpected.push(format!("Unexpected error: {line}"));
                }
            }
            None => {
                if unexpected.len() < 10 {
                    unexpected.push(format!("Unexpected output on stderr: {line}"));
                }
            }
        }
    }

    failures.extend(unexpected);
    for missing in expectations.expected_errors.difference(&found) {
        failures.push(format!("Missing expected error: {missing}"));
    }
}

fn validate_exit_code(
    failures: &mut Vec<String>,
    status: &process::ExitStatus,
    stderr: &str,
    expectations: &Expectations,
) {
    let Some(code) = status.code() else {
        failures.push(format!(
            "Expected exit code {} but the interpreter terminated abnormally (signal/abort).",
            expectations.expected_exit_code
        ));
        return;
    };

    if code != expectations.expected_exit_code {
        failures.push(format!(
            "Expected exit code {} but got {code}. Stderr:",
            expectations.expected_exit_code
        ));
        for line in stderr.lines().take(10) {
            failures.push(format!("    {line}"));
        }
    }
}

fn validate_output(failures: &mut Vec<String>, stdout: &str, expectations: &Expectations) {
    let mut output_lines: Vec<&str> = stdout.lines().collect();
    if output_lines.last() == Some(&"") {
        output_lines.pop();
    }

    for (index, line) in output_lines.iter().enumerate() {
        match expectations.expected_output.get(index) {
            Some((source_line, expected_text)) => {
                if expected_text != *line {
                    failures.push(format!(
                        "Expected output '{expected_text}' (from source line {source_line}) but got '{line}'."
                    ));
                }
            }
            None => {
                failures.push(format!("Got output '{line}' when none was expected."));
            }
        }
    }

    for (_, (source_line, expected_text)) in expectations
        .expected_output
        .iter()
        .enumerate()
        .skip(output_lines.len())
    {
        failures.push(format!(
            "Missing expected output '{expected_text}' (from source line {source_line})."
        ));
    }
}
