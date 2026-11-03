//! Command line entry point, the thirdshift binary.
//!
//! Subcommands
//!   enqueue  append one task to a backlog file
//!   run      work a backlog file and write the run record as JSON
//!   brief    work a backlog file and write the morning brief as Markdown and JSON
//!   resolve  fold a resolutions file into a brief JSON file and print the resolved record
//!
//! The argument parsing is clap, adopted under the maintainer's
//! tool choice ruling. Its functional behaviour matches the
//! command line the Python implementation ran. The subcommands
//! and flags are the same, misuse exits with status 2, help exits
//! with status 0, and unambiguous long flag abbreviations still
//! execute. Help text and malformed invocation error text are
//! clap's own forms. The documents the tool emits, the records it
//! writes, and the one error line discipline for its own refusals
//! are unchanged. Errors in main ride on anyhow. The library's
//! error types derive thiserror.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use thirdshift::brief::{brief_json, brief_markdown};
use thirdshift::pyjson::{self, Value, decimal_digit_value};
use thirdshift::resolve::{apply_resolutions, load_resolutions, read_brief_file};
use thirdshift::runner::{RunOptions, WindowValue, load_backlog, read_backlog_file, run_backlog};

#[derive(Parser)]
#[command(subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Append one task to a backlog file.
    #[command(infer_long_args = true, args_override_self = true, allow_negative_numbers = true)]
    Enqueue(EnqueueArgs),
    /// Work a backlog file and write the run record as JSON.
    #[command(infer_long_args = true, args_override_self = true, allow_negative_numbers = true)]
    Run(RunArgs),
    /// Work a backlog file and write the morning brief.
    #[command(infer_long_args = true, args_override_self = true, allow_negative_numbers = true)]
    Brief(BriefArgs),
    /// Fold a resolutions file into a brief JSON file.
    #[command(infer_long_args = true, args_override_self = true, allow_negative_numbers = true)]
    Resolve(ResolveArgs),
}

#[derive(Args)]
struct EnqueueArgs {
    #[arg(long)]
    file: String,
    #[arg(long)]
    id: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    kind: Option<String>,
    #[arg(long)]
    question: Option<String>,
}

#[derive(Args)]
struct RunArgs {
    #[arg(long)]
    backlog: String,
    #[arg(long)]
    result: Option<String>,
    #[arg(long)]
    window_seconds: Option<String>,
}

#[derive(Args)]
struct BriefArgs {
    #[arg(long)]
    backlog: String,
    #[arg(long)]
    md: Option<String>,
    #[arg(long)]
    json: Option<String>,
    #[arg(long)]
    window_seconds: Option<String>,
}

#[derive(Args)]
struct ResolveArgs {
    #[arg(long)]
    brief: String,
    #[arg(long)]
    resolutions: String,
}

fn print_out(text: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(text.as_bytes());
    let _ = lock.flush();
}

fn write_file(path: &Path, text: &str) -> anyhow::Result<()> {
    std::fs::write(path, text)
        .map_err(|error| anyhow::anyhow!("{}: {}", path.display(), error))
}

fn write_tasks(path: &Path, tasks: &[Value]) -> anyhow::Result<()> {
    let document = Value::object(vec![(
        "tasks".to_string(),
        Value::List(tasks.to_vec()),
    )]);
    write_file(path, &(pyjson::dumps_indent2(&document) + "\n"))
}

fn cmd_enqueue(args: &EnqueueArgs) -> anyhow::Result<i32> {
    let target = PathBuf::from(&args.file);
    let mut tasks = if target.exists() {
        read_backlog_file(&target)?
    } else {
        Vec::new()
    };
    let duplicate = tasks
        .iter()
        .any(|task| task.is_object() && task.get("id") == Some(&Value::Str(args.id.clone())));
    if duplicate {
        eprintln!("duplicate id {}", args.id);
        return Ok(2);
    }
    let task = Value::object(vec![
        ("id".to_string(), Value::Str(args.id.clone())),
        ("title".to_string(), Value::Str(args.title.clone())),
        (
            "kind".to_string(),
            Value::Str(args.kind.clone().unwrap_or_else(|| "work".to_string())),
        ),
        (
            "question".to_string(),
            Value::Str(args.question.clone().unwrap_or_default()),
        ),
    ]);
    tasks.push(task);
    write_tasks(&target, &tasks)?;
    println!("enqueued {}", args.id);
    Ok(0)
}

/// Turn the window option text into a number of seconds, or None.
///
/// A bad window follows the usual error discipline of the tool, so
/// it is reported as one error line with exit status 2 rather than
/// as an argument parsing failure.
fn window_value(raw: Option<&str>) -> anyhow::Result<Option<WindowValue>> {
    let raw = match raw {
        None => return Ok(None),
        Some(raw) => raw,
    };
    let value = match pyjson::py_float_parse(raw) {
        Some(value) => value,
        None => {
            return Err(anyhow::anyhow!(
                "window seconds is not a number, {}",
                raw
            ))
        }
    };
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow::anyhow!(
            "window seconds must be a non-negative number, {}",
            raw
        ));
    }
    Ok(Some(WindowValue::Float(value)))
}

fn run_record(backlog_path: &str, window_raw: Option<&str>) -> anyhow::Result<Value> {
    let backlog = load_backlog(Path::new(backlog_path))?;
    let window = window_value(window_raw)?;
    let mut run_options = RunOptions::default();
    run_options.window = window;
    Ok(run_backlog(&backlog, run_options)?)
}

fn cmd_run(args: &RunArgs) -> anyhow::Result<i32> {
    let record = run_record(&args.backlog, args.window_seconds.as_deref())?;
    let text = pyjson::dumps_indent2(&record) + "\n";
    if let Some(result) = &args.result {
        write_file(Path::new(result), &text)?;
    }
    print_out(&text);
    Ok(0)
}

fn cmd_brief(args: &BriefArgs) -> anyhow::Result<i32> {
    let record = run_record(&args.backlog, args.window_seconds.as_deref())?;
    if let Some(md) = &args.md {
        write_file(Path::new(md), &brief_markdown(&record))?;
    }
    if let Some(json) = &args.json {
        write_file(Path::new(json), &brief_json(&record))?;
    }
    if args.md.is_none() && args.json.is_none() {
        print_out(&brief_markdown(&record));
    }
    Ok(0)
}

fn cmd_resolve(args: &ResolveArgs) -> anyhow::Result<i32> {
    let record = read_brief_file(Path::new(&args.brief))?;
    let resolutions = load_resolutions(Path::new(&args.resolutions))?;
    let resolved = apply_resolutions(&record, &resolutions)?;
    print_out(&(pyjson::dumps_indent2(&resolved) + "\n"));
    Ok(0)
}

/// Whether a token after its dash reads as a number, in the
/// pattern the Python CLI joins onto the window flag. A decimal
/// digit, a point and a digit, or the whole rest reading as nan,
/// inf, or infinity in any case. The digit set is the Unicode
/// decimal set of a Python text pattern, and the case rule is
/// the pattern's, under which the letter i also matches its
/// dotted and dotless forms.
fn window_token_is_value(token: &str) -> bool {
    let Some(rest) = token.strip_prefix('-') else {
        return false;
    };
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if decimal_digit_value(first).is_some() {
        return true;
    }
    if first == '.' && chars.next().is_some_and(|c| decimal_digit_value(c).is_some()) {
        return true;
    }
    join_word_matches(rest, "nan")
        || join_word_matches(rest, "inf")
        || join_word_matches(rest, "infinity")
}

/// Whether the text reads as the word under the case rule of a
/// Python ignore case pattern.
fn join_word_matches(text: &str, word: &str) -> bool {
    let mut text_chars = text.chars();
    for pattern in word.chars() {
        let Some(ch) = text_chars.next() else {
            return false;
        };
        let matches = if pattern == 'i' {
            matches!(ch, 'i' | 'I' | '\u{130}' | '\u{131}')
        } else {
            ch == pattern || ch == pattern.to_ascii_uppercase()
        };
        if !matches {
            return false;
        }
    }
    text_chars.next().is_none()
}

/// Join a dash-prefixed window value onto its flag before parsing.
///
/// A value that starts with a dash (-inf, -nan, -2.5) is not a
/// clap value candidate, so without the join it never reaches the
/// validator and the refusal comes out as an argument error
/// instead of the one error line TS-32 requires. A token that
/// reads as a number after its dash is the flag's value, so it is
/// joined into the equals form here and validated on the usual
/// path. The join matches only the flag's full name, so an
/// abbreviated flag keeps clap's own refusal for such a value.
fn join_window_value(argv: &[String]) -> Vec<String> {
    let mut joined = Vec::new();
    let mut index = 0;
    while index < argv.len() {
        let token = &argv[index];
        if token == "--window-seconds"
            && index + 1 < argv.len()
            && window_token_is_value(&argv[index + 1])
        {
            joined.push(format!("{}={}", token, argv[index + 1]));
            index += 2;
        } else {
            joined.push(token.clone());
            index += 1;
        }
    }
    joined
}

/// The value flags of one subcommand, for the negative value
/// join. Every flag in this command line takes a value.
fn subcommand_flags(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "enqueue" => Some(&["--file", "--id", "--title", "--kind", "--question"]),
        "run" => Some(&["--backlog", "--result", "--window-seconds"]),
        "brief" => Some(&["--backlog", "--md", "--json", "--window-seconds"]),
        "resolve" => Some(&["--brief", "--resolutions"]),
        _ => None,
    }
}

/// Whether the token after its dash reads as a negative number
/// under the argparse pattern, one run of decimal digits, or a
/// run of digits, a point, and another run. The digit set is the
/// full Unicode decimal set, as it is in a Python text pattern.
fn is_negative_number(token: &str) -> bool {
    let Some(body) = token.strip_prefix('-') else {
        return false;
    };
    let digit = |ch: char| decimal_digit_value(ch).is_some();
    if !body.is_empty() && body.chars().all(digit) {
        return true;
    }
    match body.split_once('.') {
        Some((whole, fraction)) => {
            whole.chars().all(digit)
                && !fraction.is_empty()
                && fraction.chars().all(digit)
        }
        None => false,
    }
}

/// Whether a typed long token names exactly one value flag of
/// the subcommand, in full or as an unambiguous abbreviation. The
/// help flag takes no value and never counts, and no flag set
/// here shares a first letter, so a prefix matches at most one.
fn names_one_value_flag(flags: &[&str], token: &str) -> bool {
    if token.contains('=') || !token.starts_with("--") {
        return false;
    }
    flags.iter().filter(|flag| flag.starts_with(token)).count() == 1
}

/// Join a negative number value onto its flag before parsing.
///
/// Argparse reads a token that matches its negative number
/// pattern as a positional string, so such a token after a flag
/// is the flag's value, in the full Unicode decimal digit set.
/// Clap reads only ASCII negative numbers that way, so a value
/// written with any other decimal digit would be refused as an
/// unexpected argument and never reach the tool. A token that
/// reads as a negative number after a value flag is joined into
/// the equals form here, the same device as the window join.
fn join_negative_values(argv: &[String]) -> Vec<String> {
    let mut joined = Vec::new();
    let mut flags: Option<&[&str]> = None;
    let mut index = 0;
    while index < argv.len() {
        let token = &argv[index];
        if token == "--" {
            joined.extend_from_slice(&argv[index..]);
            break;
        }
        match flags {
            None => {
                if let Some(found) = subcommand_flags(token) {
                    flags = Some(found);
                }
                joined.push(token.clone());
                index += 1;
            }
            Some(flags) => {
                if names_one_value_flag(flags, token)
                    && index + 1 < argv.len()
                    && is_negative_number(&argv[index + 1])
                {
                    joined.push(format!("{}={}", token, argv[index + 1]));
                    index += 2;
                } else {
                    joined.push(token.clone());
                    index += 1;
                }
            }
        }
    }
    joined
}

fn main() -> ExitCode {
    let mut args = std::env::args();
    let program = args.next().unwrap_or_else(|| "thirdshift".to_string());
    let rest: Vec<String> = args.collect();
    let joined = join_negative_values(&join_window_value(&rest));
    let cli = match Cli::try_parse_from(std::iter::once(program).chain(joined)) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            return ExitCode::from(code as u8);
        }
    };
    let outcome = match &cli.command {
        Command::Enqueue(args) => cmd_enqueue(args),
        Command::Run(args) => cmd_run(args),
        Command::Brief(args) => cmd_brief(args),
        Command::Resolve(args) => cmd_resolve(args),
    };
    match outcome {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("error - {}", error);
            ExitCode::from(2)
        }
    }
}
