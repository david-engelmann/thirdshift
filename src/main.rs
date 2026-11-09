//! Command line entry point, the thirdshift binary.
//!
//! Subcommands
//!   enqueue  append one task to a backlog file
//!   run      work a backlog file and write the run record as JSON
//!   brief    work a backlog file and write the morning brief as Markdown and JSON
//!   resolve  fold a resolutions file into a brief JSON file and print the resolved record
//!   carry    join a brief JSON file against its backlog and write the carried backlog
//!
//! The argument parsing is clap, adopted under the maintainer's
//! tool choice ruling. Before clap sees the arguments, a
//! preprocess walks them the way the Python parser's own scan
//! walked them. It joins onto their flags the values that scan
//! would have taken, fires help where that scan fired it, and
//! duplicates a separator token that scan refused, so the
//! functional outcomes of the two command lines agree, the same
//! subcommands and flags, misuse at exit status 2, help at exit
//! status 0, and unambiguous long flag abbreviations still
//! executing. Help text and malformed invocation error text are
//! clap's own forms. The documents the tool emits, the records it
//! writes, and the one error line discipline for its own refusals
//! are unchanged. A write that fails is reported in the same one
//! error line form, at status 1 where the Python tool lets the
//! failure escape uncaught and at status 120 where a standard
//! output failure surfaces only at its exit flush. Errors in
//! main ride on anyhow. The library's error types derive thiserror.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand};

use thirdshift::brief::{brief_json, brief_markdown};
use thirdshift::carry::{CarryError, carry_backlog, read_record_file};
use thirdshift::pyjson::{self, Value, decimal_digit_value};
use thirdshift::reading::ReadError;
use thirdshift::resolve::{
    ResolutionError, apply_resolutions, check_brief_decision_entries, load_resolutions,
    read_brief_file, refuse_brief_past_copy_ceiling,
};
use thirdshift::runner::{
    BacklogError, RunOptions, WindowValue, load_backlog, read_backlog_file, run_backlog,
};

#[derive(Parser)]
#[command(
    about = "Runs a stocked backlog of agent tasks through a usage window, records what finished and what did not, hands the unfinished items to the next shift, and writes a morning brief of the run.",
    subcommand_required = true,
    infer_long_args = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Append one task to a backlog file.
    #[command(infer_long_args = true, args_override_self = true)]
    Enqueue(EnqueueArgs),
    /// Work a backlog file and write the run record as JSON.
    #[command(infer_long_args = true, args_override_self = true)]
    Run(RunArgs),
    /// Work a backlog file and write the morning brief.
    #[command(infer_long_args = true, args_override_self = true)]
    Brief(BriefArgs),
    /// Fold a resolutions file into a brief JSON file.
    #[command(infer_long_args = true, args_override_self = true)]
    Resolve(ResolveArgs),
    /// Join a brief JSON file against its backlog and write the carried backlog.
    #[command(infer_long_args = true, args_override_self = true)]
    Carry(CarryArgs),
}

#[derive(Args)]
struct EnqueueArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    id: OsString,
    #[arg(long)]
    title: OsString,
    #[arg(long)]
    kind: Option<OsString>,
    #[arg(long)]
    question: Option<OsString>,
}

#[derive(Args)]
struct RunArgs {
    #[arg(long)]
    backlog: PathBuf,
    #[arg(long)]
    result: Option<PathBuf>,
    #[arg(long)]
    window_seconds: Option<OsString>,
}

#[derive(Args)]
struct BriefArgs {
    #[arg(long)]
    backlog: PathBuf,
    #[arg(long)]
    md: Option<PathBuf>,
    #[arg(long)]
    json: Option<PathBuf>,
    #[arg(long)]
    window_seconds: Option<OsString>,
}

#[derive(Args)]
struct ResolveArgs {
    #[arg(long)]
    brief: PathBuf,
    #[arg(long)]
    resolutions: PathBuf,
}

#[derive(Args)]
struct CarryArgs {
    #[arg(long)]
    backlog: PathBuf,
    #[arg(long)]
    brief: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

/// Decode a data value as the text the Python tool holds it as.
///
/// File paths travel as paths and keep their bytes. A data value
/// is text, and bytes that are not valid UTF-8 become the
/// replacement character here, the recorded reading for text a
/// Rust string cannot hold the way a Python string holds it.
fn text(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

/// A write the tool attempted and could not complete.
///
/// The Python tool lets such a failure escape uncaught and exits
/// with status 1, so the binary carries the failure in its own
/// type and main answers it with status 1 and one error line,
/// where every other refusal answers with status 2.
#[derive(Debug)]
struct FailedWrite(String);

impl std::fmt::Display for FailedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FailedWrite {}

/// A standard output failure the Python tool discovers only
/// when its buffer is flushed as the tool exits.
///
/// The Python tool holds its printed output in a buffer of
/// 8192 bytes, so a failure under a smaller output is never
/// discovered during the run. It surfaces at the shutdown
/// flush, where the Python tool reports it as an exception
/// ignored and exits with status 120 and no traceback, so
/// the binary carries the failure in its own type and main
/// answers it with status 120 and one error line.
#[derive(Debug)]
struct FailedFlush(String);

impl std::fmt::Display for FailedFlush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FailedFlush {}

/// The size of the buffer the Python tool holds its printed
/// output in, in bytes. A printed output of this size or more
/// is written through as it is printed, so a failure under it
/// surfaces during the run, and a smaller output is flushed
/// only as the tool exits.
const STDOUT_BUFFER_BYTES: usize = 8192;

fn print_out(text: &str) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())
        .and_then(|()| lock.flush())
        .map_err(|error| {
            let message = format!("stdout: {}", error);
            if text.len() < STDOUT_BUFFER_BYTES {
                anyhow::Error::new(FailedFlush(message))
            } else {
                anyhow::Error::new(FailedWrite(message))
            }
        })
}

fn write_file(path: &Path, text: &str) -> anyhow::Result<()> {
    std::fs::write(path, text)
        .map_err(|error| anyhow::Error::new(FailedWrite(format!("{}: {}", path.display(), error))))
}

/// The deepest container path the backlog document an enqueue
/// writes can carry. The Python tool builds that document with
/// a recursive indented writer that reaches a path of 991
/// containers, and a document past it fails there the uncaught
/// way before any byte is written, so the port refuses it at
/// the same path and the file is likewise left unwritten.
const WRITE_MAX_PATH: usize = 991;

fn write_tasks(path: &Path, tasks: &[Value]) -> anyhow::Result<()> {
    let document = Value::object(vec![("tasks".to_string(), Value::List(tasks.to_vec()))]);
    if pyjson::deepest_path(&document) > WRITE_MAX_PATH {
        return Err(anyhow::Error::new(FailedWrite(format!(
            "backlog file is nested too deeply, {}",
            path.display()
        ))));
    }
    write_file(path, &(pyjson::dumps_indent2(&document) + "\n"))
}

fn cmd_enqueue(args: &EnqueueArgs) -> anyhow::Result<i32> {
    let target = &args.file;
    let id = text(&args.id);
    let mut tasks = if target.exists() {
        read_backlog_file(target)?
    } else {
        Vec::new()
    };
    let duplicate = tasks
        .iter()
        .any(|task| task.is_object() && task.get("id") == Some(&Value::Str(id.clone())));
    if duplicate {
        eprintln!("duplicate id {}", id);
        return Ok(2);
    }
    let task = Value::object(vec![
        ("id".to_string(), Value::Str(id.clone())),
        ("title".to_string(), Value::Str(text(&args.title))),
        (
            "kind".to_string(),
            Value::Str(
                args.kind
                    .as_deref()
                    .map(text)
                    .unwrap_or_else(|| "work".to_string()),
            ),
        ),
        (
            "question".to_string(),
            Value::Str(args.question.as_deref().map(text).unwrap_or_default()),
        ),
    ]);
    tasks.push(task);
    write_tasks(target, &tasks)?;
    print_out(&format!("enqueued {}\n", id))?;
    Ok(0)
}

/// Turn the window option text into a number of seconds, or None.
///
/// A bad window follows the usual error discipline of the tool, so
/// it is reported as one error line with exit status 2 rather than
/// as an argument parsing failure.
fn window_value(raw: Option<&OsStr>) -> anyhow::Result<Option<WindowValue>> {
    let raw = match raw {
        None => return Ok(None),
        Some(raw) => text(raw),
    };
    let value = match pyjson::py_float_parse(&raw) {
        Some(value) => value,
        None => return Err(anyhow::anyhow!("window seconds is not a number, {}", raw)),
    };
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow::anyhow!(
            "window seconds must be a non-negative number, {}",
            raw
        ));
    }
    Ok(Some(WindowValue::Float(value)))
}

fn run_record(backlog_path: &Path, window_raw: Option<&OsStr>) -> anyhow::Result<Value> {
    let backlog = load_backlog(backlog_path)?;
    let window = window_value(window_raw)?;
    let run_options = RunOptions {
        window,
        ..RunOptions::default()
    };
    Ok(run_backlog(&backlog, run_options)?)
}

fn cmd_run(args: &RunArgs) -> anyhow::Result<i32> {
    let record = run_record(&args.backlog, args.window_seconds.as_deref())?;
    let text = pyjson::dumps_indent2(&record) + "\n";
    if let Some(result) = &args.result {
        write_file(result, &text)?;
    }
    print_out(&text)?;
    Ok(0)
}

fn cmd_brief(args: &BriefArgs) -> anyhow::Result<i32> {
    let record = run_record(&args.backlog, args.window_seconds.as_deref())?;
    if let Some(md) = &args.md {
        write_file(md, &brief_markdown(&record))?;
    }
    if let Some(json) = &args.json {
        write_file(json, &brief_json(&record))?;
    }
    if args.md.is_none() && args.json.is_none() {
        print_out(&brief_markdown(&record))?;
    }
    Ok(0)
}

fn cmd_resolve(args: &ResolveArgs) -> anyhow::Result<i32> {
    let record = read_brief_file(&args.brief)?;
    let resolutions = load_resolutions(&args.resolutions)?;
    check_brief_decision_entries(&record)?;
    if let Some(decisions) = record.get("decisions") {
        refuse_brief_past_copy_ceiling(&args.brief, decisions)?;
    }
    let resolved = apply_resolutions(&record, &resolutions)?;
    refuse_brief_past_copy_ceiling(&args.brief, &record)?;
    print_out(&(pyjson::dumps_indent2(&resolved) + "\n"))?;
    Ok(0)
}

fn cmd_carry(args: &CarryArgs) -> anyhow::Result<i32> {
    let entries = load_backlog(&args.backlog)?;
    let record = read_record_file(&args.brief)?;
    let carried = carry_backlog(&entries, &record)?;
    write_tasks(&args.out, &carried)?;
    let document = Value::object(vec![("tasks".to_string(), Value::List(carried))]);
    print_out(&(pyjson::dumps_indent2(&document) + "\n"))?;
    Ok(0)
}

/// The exit status an error from a command answers with.
///
/// A failed write and a document nested past the reader ceiling
/// escape the Python tool uncaught at status 1. A standard
/// output failure discovered only at the exit flush answers
/// with status 120, the status the Python tool exits with
/// when it reports that failure as an exception ignored.
/// Every other refusal the tool makes answers with status 2.
fn refusal_status(error: &anyhow::Error) -> u8 {
    if error.downcast_ref::<FailedFlush>().is_some() {
        return 120;
    }
    let uncaught = error.downcast_ref::<FailedWrite>().is_some()
        || matches!(
            error.downcast_ref::<ReadError<BacklogError>>(),
            Some(ReadError::TooDeep(_))
        )
        || matches!(
            error.downcast_ref::<ReadError<ResolutionError>>(),
            Some(ReadError::TooDeep(_))
        )
        || matches!(
            error.downcast_ref::<ReadError<CarryError>>(),
            Some(ReadError::TooDeep(_))
        );
    if uncaught { 1 } else { 2 }
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
    if first == '.'
        && chars
            .next()
            .is_some_and(|c| decimal_digit_value(c).is_some())
    {
        return true;
    }
    join_word_matches_dollar(rest, "nan")
        || join_word_matches_dollar(rest, "inf")
        || join_word_matches_dollar(rest, "infinity")
}

/// Whether the text reads as the word under the case rule of a
/// Python ignore case pattern, where the pattern's end anchor
/// also matches just before one trailing newline, as it does in
/// a Python text pattern.
fn join_word_matches_dollar(text: &str, word: &str) -> bool {
    join_word_matches(text, word)
        || text
            .strip_suffix('\n')
            .is_some_and(|rest| join_word_matches(rest, word))
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
/// The Python CLI joins the same pair before its parser runs,
/// over the whole argument list and wherever the flag token
/// stands, and this join keeps that order and that reach.
fn join_window_value(tokens: &[OsString]) -> Vec<OsString> {
    let mut joined = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if token.to_string_lossy() == "--window-seconds"
            && index + 1 < tokens.len()
            && window_token_is_value(&tokens[index + 1].to_string_lossy())
        {
            let mut merged = token.clone();
            merged.push("=");
            merged.push(&tokens[index + 1]);
            joined.push(merged);
            index += 2;
        } else {
            joined.push(token.clone());
            index += 1;
        }
    }
    joined
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
            whole.chars().all(digit) && !fraction.is_empty() && fraction.chars().all(digit)
        }
        None => false,
    }
}

/// Whether the negative number pattern matches the token, where
/// the pattern's end anchor also matches just before one
/// trailing newline, as a Python text pattern's does.
fn negative_number_matches(token: &str) -> bool {
    is_negative_number(token) || token.strip_suffix('\n').is_some_and(is_negative_number)
}

/// Whether a token reads as a positional string rather than as
/// an option, the classification the Python parser makes when it
/// meets the token. A bare word and a lone dash are positional.
/// A dash token carrying a space reads as positional whatever
/// its prefix, the one exception being the short help family,
/// whose letter after the dash decides ahead of the space rule,
/// as the Python parser refuses a help token with a trailing
/// word in a value position rather than consuming it. A dash
/// token without a space is positional only when it matches
/// the negative number pattern. Any long token without a space
/// is an option.
fn reads_as_positional(token: &str) -> bool {
    if token == "--" {
        return false;
    }
    if !token.starts_with('-') || token == "-" {
        return true;
    }
    if token.chars().nth(1) == Some('h') {
        return false;
    }
    if token.contains(' ') {
        return true;
    }
    if token.starts_with("--") {
        return false;
    }
    negative_number_matches(token)
}

/// One subcommand of the command line, with the long flags the
/// derive model declares for it.
///
/// The flag tables of the preprocess come from clap's own model
/// of the derive structs, read through the command factory, so
/// the derive model stays the one owner of the flag set and this
/// layer restates none of it.
struct SubcommandSpec {
    name: String,
    value_flags: Vec<String>,
    longs: Vec<String>,
}

fn subcommand_specs() -> Vec<SubcommandSpec> {
    Cli::command()
        .get_subcommands()
        .map(|sub| {
            let mut value_flags: Vec<String> = Vec::new();
            let mut longs: Vec<String> = Vec::new();
            for arg in sub.get_arguments() {
                let Some(long) = arg.get_long() else {
                    continue;
                };
                let name = format!("--{}", long);
                if !longs.contains(&name) {
                    longs.push(name.clone());
                }
                if matches!(arg.get_action(), clap::ArgAction::Set) {
                    value_flags.push(name);
                }
            }
            if !longs.iter().any(|long| long == "--help") {
                longs.push("--help".to_string());
            }
            SubcommandSpec {
                name: sub.get_name().to_string(),
                value_flags,
                longs,
            }
        })
        .collect()
}

/// How a token reads as a help token on the current parser.
enum HelpRead {
    /// Not a help token.
    No,
    /// The help action fires.
    Fire,
    /// A help token carrying an explicit value, an error in both
    /// parsers. The scan carries the token in the form clap
    /// refuses in its own voice, and nothing later can fire help.
    Refuse(OsString),
}

/// Read a token as a help token against the long flags of the
/// parser the scan stands on. The Python parser fires help when
/// its scan reaches the token, ahead of extra tokens it has
/// already collected, and it refuses a help token that carries an
/// explicit value, ahead of any help token that follows. A short
/// help token fires when its bundle continues with anything but
/// an equals sign or a dash, the two continuations the parser
/// reads as an explicit value arriving with the token.
fn help_read(token: &str, longs: &[String]) -> HelpRead {
    if token.starts_with("--") {
        let name = token.split('=').next().unwrap_or(token);
        if name.len() > 2 {
            let matches: Vec<&String> =
                longs.iter().filter(|long| long.starts_with(name)).collect();
            if matches.len() == 1 && matches[0] == "--help" {
                if token.contains('=') {
                    return HelpRead::Refuse(OsString::from(token));
                }
                return HelpRead::Fire;
            }
        }
        return HelpRead::No;
    }
    if token.starts_with('-') && token.chars().nth(1) == Some('h') {
        return match token.chars().nth(2) {
            None => HelpRead::Fire,
            Some('=') => HelpRead::Refuse(OsString::from(format!("--help={}", &token[3..]))),
            Some('-') => HelpRead::Refuse(OsString::from(format!("--help={}", &token[2..]))),
            Some(_) => HelpRead::Fire,
        };
    }
    HelpRead::No
}

/// The outcome of the preprocessed argument walk.
enum Preprocess {
    /// Hand these arguments to clap.
    Run(Vec<OsString>),
    /// Print the help of the named subcommand parser, or of the
    /// top level parser when the name is absent, at status 0.
    FireHelp(Option<String>),
}

/// The hand set the preprocess walk reads help tokens
/// against on the root, holding the help long alone
/// because the root command declares no other long
/// (TS-58). The set stands in one function so the pin
/// of TS-58 can read the same table the walk reads
/// (TS-62).
fn root_longs() -> Vec<String> {
    vec!["--help".to_string()]
}

/// Walk the arguments the way the Python parser's scan walked
/// them, after the window join, and prepare them for clap.
///
/// The Python parser consumes its arguments from the left. A
/// value flag takes the next token when that token reads as a
/// positional string, and this walk joins such a value onto its
/// flag in the equals form, the device the window join already
/// uses. A value flag that meets an option token is an error the
/// parser exits on at once, as are a word the subcommand slot
/// cannot take and a help token carrying an explicit value, and
/// past any of those this walk hands the arguments to clap
/// untouched, since clap refuses the same invocation in its own
/// voice. Help fires where the scan reaches it, ahead of extra
/// tokens already collected, and this walk answers a fired help
/// token itself. A separator token ends the scan. The Python
/// parser refuses a separator that follows an otherwise complete
/// invocation, where clap accepts it silently, so this walk
/// duplicates the separator and clap refuses the copy as an
/// unexpected argument in its own voice.
fn preprocess(tokens: &[OsString]) -> Preprocess {
    let joined = join_window_value(tokens);
    let specs = subcommand_specs();
    let root_longs: Vec<String> = root_longs();
    let mut out: Vec<OsString> = Vec::new();
    let mut current: Option<usize> = None;
    let mut pending = false;
    let mut blocked = false;
    let mut separator: Option<usize> = None;
    for token in &joined {
        if separator.is_some() || blocked {
            out.push(token.clone());
            continue;
        }
        let text = token.to_string_lossy().into_owned();
        if pending {
            pending = false;
            if text == "--" {
                separator = Some(out.len());
                out.push(token.clone());
            } else if reads_as_positional(&text) {
                let mut flag = out.pop().expect("a pending flag precedes its value");
                flag.push("=");
                flag.push(token);
                out.push(flag);
            } else {
                blocked = true;
                out.push(token.clone());
            }
            continue;
        }
        if text == "--" {
            separator = Some(out.len());
            out.push(token.clone());
            continue;
        }
        let longs: &[String] = match current {
            Some(index) => &specs[index].longs,
            None => &root_longs,
        };
        match help_read(&text, longs) {
            HelpRead::Fire => {
                return Preprocess::FireHelp(current.map(|index| specs[index].name.clone()));
            }
            HelpRead::Refuse(replacement) => {
                blocked = true;
                out.push(replacement);
                continue;
            }
            HelpRead::No => {}
        }
        if current.is_none() {
            if let Some(index) = specs.iter().position(|spec| spec.name == text) {
                current = Some(index);
                out.push(token.clone());
                continue;
            }
            if reads_as_positional(&text) {
                blocked = true;
            }
            out.push(token.clone());
            continue;
        }
        let spec = &specs[current.expect("a subcommand is selected")];
        if text.starts_with("--") {
            let name = text.split('=').next().unwrap_or(&text);
            if spec
                .value_flags
                .iter()
                .filter(|flag| flag.starts_with(name))
                .count()
                == 1
            {
                if !text.contains('=') {
                    pending = true;
                }
                out.push(token.clone());
                continue;
            }
        }
        out.push(token.clone());
    }
    if let Some(index) = separator {
        out.insert(index + 1, OsString::from("--"));
    }
    Preprocess::Run(out)
}

fn main() -> ExitCode {
    // The Python reader reaches documents nested thousands of
    // containers deep before its own ceiling refuses them, and
    // the port's reader, writer, and value walks recurse once per
    // container on the deepest path, as the Python tool's do. The
    // binary therefore runs on a worker thread with a stack sized
    // for that depth, so a document at the ceiling is refused or
    // carried on the tool's own terms and never on the stack's.
    match std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
    {
        Ok(handle) => handle.join().unwrap_or(ExitCode::from(2)),
        Err(_) => run(),
    }
}

fn run() -> ExitCode {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_else(|| OsString::from("thirdshift"));
    let rest: Vec<OsString> = args.collect();
    let joined = match preprocess(&rest) {
        Preprocess::Run(joined) => joined,
        Preprocess::FireHelp(subcommand) => {
            let mut argv = vec![program];
            if let Some(name) = subcommand {
                argv.push(OsString::from(name));
            }
            argv.push(OsString::from("--help"));
            return match Cli::try_parse_from(argv) {
                Ok(_) => ExitCode::from(0),
                Err(error) => {
                    let code = error.exit_code();
                    let _ = error.print();
                    ExitCode::from(code as u8)
                }
            };
        }
    };
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
        Command::Carry(args) => cmd_carry(args),
    };
    match outcome {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("error - {}", error);
            ExitCode::from(refusal_status(&error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Derive the root long set from the built command in
    /// the manner subcommand_specs derives the long set of
    /// a subcommand, collecting the long of every argument
    /// the root command carries and completing the set with
    /// the help long where it stands absent.
    fn derived_root_longs() -> Vec<String> {
        let mut command = Cli::command();
        command.build();
        let mut longs: Vec<String> = Vec::new();
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else {
                continue;
            };
            let name = format!("--{}", long);
            if !longs.contains(&name) {
                longs.push(name);
            }
        }
        if !longs.iter().any(|long| long == "--help") {
            longs.push("--help".to_string());
        }
        longs
    }

    /// TS-58. The hand set the preprocess walk reads help
    /// tokens against on the root holds the help long alone,
    /// and it holds it because the root command declares no
    /// other long. This pin derives the root long set from
    /// the built command and asserts the derived set is the
    /// one help long, so a long added to the root command,
    /// declared as an argument or raised by an attribute,
    /// fails here in the open instead of leaving the hand
    /// set standing behind the parser in silence.
    ///
    /// TS-62 adds the second assertion, that the table
    /// root_longs returns equals the derived set, so an
    /// edit to the table alone fails here as well.
    #[test]
    fn root_long_set_is_the_one_help_long() {
        assert_eq!(derived_root_longs(), vec!["--help".to_string()]);
        assert_eq!(root_longs(), derived_root_longs());
    }

    /// Derive the short of the help argument of every
    /// command in the tree, the root command first and
    /// then its subcommands in their own order, finding
    /// each argument by its help long in the manner
    /// derived_root_longs collects the longs of the root
    /// command, and standing absent in a command's place
    /// where that command carries no argument with the
    /// help long.
    fn derived_help_shorts() -> Vec<Option<char>> {
        fn help_short(command: &clap::Command) -> Option<char> {
            for arg in command.get_arguments() {
                if arg.get_long() == Some("help") {
                    return arg.get_short();
                }
            }
            None
        }
        fn collect(command: &clap::Command, shorts: &mut Vec<Option<char>>) {
            shorts.push(help_short(command));
            for sub in command.get_subcommands() {
                collect(sub, shorts);
            }
        }
        let mut command = Cli::command();
        command.build();
        let mut shorts = Vec::new();
        collect(&command, &mut shorts);
        shorts
    }

    /// TS-61. The two reads that test the second character
    /// of a token against the letter h, the short help test
    /// of reads_as_positional and the short help branch of
    /// help_read, test that character in every context the
    /// walk reaches, on the root and behind a subcommand
    /// alike, and they rest on the help argument of every
    /// command in the tree carrying the short h. This pin
    /// derives the help short of every command from the
    /// built command and asserts each one is the letter h,
    /// so a help short customized to another letter on any
    /// command, or a command left with no help argument at
    /// all, fails here in the open instead of leaving the
    /// two reads standing behind the parser in silence.
    #[test]
    fn every_help_short_is_the_letter_h() {
        let shorts = derived_help_shorts();
        assert_eq!(shorts.len(), 6);
        assert!(shorts.iter().all(|short| *short == Some('h')));
    }

    /// Build the token stream a pin of TS-70 hands to the
    /// walk from the text forms the pin writes.
    fn walk_tokens(tokens: &[&str]) -> Vec<OsString> {
        tokens.iter().map(OsString::from).collect()
    }

    /// The text forms a pin of TS-70 expects the walk to
    /// hand over.
    fn walk_strings(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| token.to_string()).collect()
    }

    /// Assert the walk hands the stream over, token for
    /// token, in the forms the pin names.
    fn assert_walk_run(tokens: &[&str], expected: &[&str]) {
        match preprocess(&walk_tokens(tokens)) {
            Preprocess::Run(out) => {
                let out: Vec<String> = out
                    .iter()
                    .map(|token| token.to_string_lossy().into_owned())
                    .collect();
                assert_eq!(out, walk_strings(expected));
            }
            Preprocess::FireHelp(name) => {
                panic!("expected the walk to run, and it fired help for {name:?}")
            }
        }
    }

    /// Assert the walk fires help, for the root where the
    /// pin names no subcommand and for the named subcommand
    /// where it does.
    fn assert_walk_fires(tokens: &[&str], expected: Option<&str>) {
        match preprocess(&walk_tokens(tokens)) {
            Preprocess::FireHelp(name) => {
                assert_eq!(name.as_deref(), expected);
            }
            Preprocess::Run(out) => {
                panic!("expected the walk to fire help, and it ran {out:?}")
            }
        }
    }

    /// TS-70, the window join. A separate token that reads
    /// as a number after its dash joins onto the window flag
    /// in the equals form, wherever the flag stands.
    #[test]
    fn walk_join_dash_value_joins_onto_the_window_flag() {
        assert_walk_run(
            &["--window-seconds", "-2.5", "run"],
            &["--window-seconds=-2.5", "run"],
        );
    }

    /// TS-70, the window join. The value read asks only
    /// about the first character after the dash, so a value
    /// with a trailing point joins here where the negative
    /// number pattern would refuse it.
    #[test]
    fn walk_join_first_digit_decides_the_value_read() {
        assert_walk_run(
            &["run", "--window-seconds", "-5."],
            &["run", "--window-seconds=-5."],
        );
    }

    /// TS-70, the window join. A point after the dash with a
    /// digit after it reads as a value and joins.
    #[test]
    fn walk_join_point_and_digit_value_joins() {
        assert_walk_run(
            &["--window-seconds", "-.5", "run"],
            &["--window-seconds=-.5", "run"],
        );
    }

    /// TS-70, the window join. The whole rest after the dash
    /// reading as one of the three words reads as a value
    /// and joins.
    #[test]
    fn walk_join_word_value_joins() {
        assert_walk_run(
            &["run", "--window-seconds", "-inf"],
            &["run", "--window-seconds=-inf"],
        );
    }

    /// TS-70, the window join. The word read follows the
    /// ignore case rule of the pattern, so a mixed case word
    /// joins the same way.
    #[test]
    fn walk_join_word_value_reads_in_any_case() {
        assert_walk_run(
            &["run", "--window-seconds", "-NaN"],
            &["run", "--window-seconds=-NaN"],
        );
    }

    /// TS-70, the window join. The pattern end anchor
    /// matches just before one trailing newline, so the word
    /// with a newline after it still reads as a value.
    #[test]
    fn walk_join_word_value_before_a_trailing_newline_joins() {
        assert_walk_run(
            &["run", "--window-seconds", "-infinity\n"],
            &["run", "--window-seconds=-infinity\n"],
        );
    }

    /// TS-70, the window join. A next token that reads as no
    /// value leaves the pair unjoined, and the walk reads
    /// the pair on its own terms from there.
    #[test]
    fn walk_join_token_that_reads_as_no_value_stands_unjoined() {
        assert_walk_run(
            &["run", "--window-seconds", "-x"],
            &["run", "--window-seconds", "-x"],
        );
    }

    /// TS-70, the window join. The flag standing at the end
    /// of the stream has no next token and stands unjoined.
    #[test]
    fn walk_join_flag_at_the_end_of_the_stream_stands_unjoined() {
        assert_walk_run(&["run", "--window-seconds"], &["run", "--window-seconds"]);
    }

    /// TS-70, the window join. The join reads the flag at
    /// every standing in the list, each against the token
    /// that follows it.
    #[test]
    fn walk_join_matches_the_flag_at_a_second_standing() {
        assert_walk_run(
            &["run", "--window-seconds", "-1", "--window-seconds", "-inf"],
            &["run", "--window-seconds=-1", "--window-seconds=-inf"],
        );
    }

    /// TS-70, help. The exact long help token fires for the
    /// root.
    #[test]
    fn walk_help_long_exact_fires_at_the_root() {
        assert_walk_fires(&["--help"], None);
    }

    /// TS-70, help. An abbreviation of the help long that
    /// exactly one entry of the root table carries fires the
    /// same way.
    #[test]
    fn walk_help_long_abbreviation_fires_at_the_root() {
        assert_walk_fires(&["--he"], None);
    }

    /// TS-70, help. The exact long help token behind a
    /// subcommand fires for that subcommand, because the
    /// table subcommand_specs builds carries the help long.
    #[test]
    fn walk_help_long_exact_fires_behind_a_subcommand() {
        assert_walk_fires(&["carry", "--help"], Some("carry"));
    }

    /// TS-70, help. The abbreviated long help token behind a
    /// subcommand fires the same way, no declared long of
    /// any subcommand beginning with the letter h.
    #[test]
    fn walk_help_long_abbreviation_fires_behind_a_subcommand() {
        assert_walk_fires(&["brief", "--hel"], Some("brief"));
    }

    /// TS-70, help. A long token matching exactly one table
    /// entry that is not the help long reads as no help.
    #[test]
    fn walk_help_long_matching_one_other_flag_reads_no_help() {
        assert_walk_run(&["run", "--r=x"], &["run", "--r=x"]);
    }

    /// TS-70, help. A help token fires ahead of an unmatched
    /// option token the root already collected, the extra
    /// never stopping the scan from reaching it.
    #[test]
    fn walk_help_fires_ahead_of_a_collected_extra() {
        assert_walk_fires(&["--bogus", "--help"], None);
    }

    /// TS-70, help. A long token matching no table entry
    /// reads as no help.
    #[test]
    fn walk_help_long_matching_no_flag_reads_no_help() {
        assert_walk_run(&["--helpme"], &["--helpme"]);
    }

    /// TS-70, help. A long token matching two entries of
    /// the table reads as no help, the match never being
    /// unique.
    #[test]
    fn walk_help_long_ambiguous_abbreviation_reads_no_help() {
        assert_walk_run(&["carry", "--b", "x"], &["carry", "--b", "x"]);
    }

    /// TS-70, help. The help long carrying an equals value
    /// refuses, the token stands as it was written, and a
    /// help token after it never fires.
    #[test]
    fn walk_help_long_with_an_equals_argument_refuses() {
        assert_walk_run(&["--help=x", "-h"], &["--help=x", "-h"]);
    }

    /// TS-70, help. The help long carrying an empty equals
    /// value refuses the same way.
    #[test]
    fn walk_help_long_with_an_empty_equals_argument_refuses() {
        assert_walk_run(&["--help=", "-h"], &["--help=", "-h"]);
    }

    /// TS-70, help. An abbreviation of the help long
    /// carrying an equals value refuses on the same arm as
    /// the exact name.
    #[test]
    fn walk_help_abbreviated_long_with_an_equals_argument_refuses() {
        assert_walk_run(&["--he=x", "-h"], &["--he=x", "-h"]);
    }

    /// TS-70, help. A token whose name before the equals
    /// sign is the bare double dash never reaches the match,
    /// by the length guard in the read.
    #[test]
    fn walk_help_bare_double_dash_name_reads_no_help() {
        assert_walk_run(&["--=x"], &["--=x"]);
    }

    /// TS-70, help. The bare short help token fires for the
    /// root.
    #[test]
    fn walk_help_short_fires_at_the_root() {
        assert_walk_fires(&["-h"], None);
    }

    /// TS-70, help. The bare short help token behind a
    /// subcommand fires for that subcommand.
    #[test]
    fn walk_help_short_fires_behind_a_subcommand() {
        assert_walk_fires(&["run", "-h"], Some("run"));
    }

    /// TS-70, help. A short bundle continuing with a letter
    /// fires, the continuation being neither of the two the
    /// read takes as a value arriving.
    #[test]
    fn walk_help_short_bundle_fires_at_the_root() {
        assert_walk_fires(&["-hx"], None);
    }

    /// TS-70, help. A short bundle continuing with a digit
    /// behind a subcommand fires the same way.
    #[test]
    fn walk_help_short_bundle_fires_behind_a_subcommand() {
        assert_walk_fires(&["resolve", "-h5"], Some("resolve"));
    }

    /// TS-70, help. The short help token with an equals
    /// continuation refuses, and the stream carries the
    /// replacement in the long equals form.
    #[test]
    fn walk_help_short_with_an_equals_argument_refuses() {
        assert_walk_run(&["-h=x", "--help"], &["--help=x", "--help"]);
    }

    /// TS-70, help. The short help token with an empty
    /// equals continuation refuses into the bare long equals
    /// form.
    #[test]
    fn walk_help_short_with_an_empty_equals_argument_refuses() {
        assert_walk_run(&["-h="], &["--help="]);
    }

    /// TS-70, help. The short help token with a dash
    /// continuation refuses, the replacement carrying the
    /// bytes from the dash on.
    #[test]
    fn walk_help_short_with_a_dash_argument_refuses() {
        assert_walk_run(&["-h-x"], &["--help=-x"]);
    }

    /// TS-70, help. The short help token continuing with
    /// two dashes refuses on the same arm.
    #[test]
    fn walk_help_short_with_a_double_dash_argument_refuses() {
        assert_walk_run(&["-h--"], &["--help=--"]);
    }

    /// TS-70, help. The capital letter is not the letter
    /// the short read tests, so the token reads as no help.
    #[test]
    fn walk_help_capital_letter_reads_no_help() {
        assert_walk_run(&["-H"], &["-H"]);
    }

    /// TS-70, help. A short token of another letter reads as
    /// no help.
    #[test]
    fn walk_help_other_short_token_reads_no_help() {
        assert_walk_run(&["-x"], &["-x"]);
    }

    /// TS-70, help. A standing flag reads its value
    /// candidate first, so the short help token blocks as a
    /// value instead of firing or standing as one.
    #[test]
    fn walk_help_short_never_fires_as_a_pending_value() {
        assert_walk_run(&["run", "--backlog", "-h"], &["run", "--backlog", "-h"]);
    }

    /// TS-70, help. The long help token as a standing flag's
    /// value candidate blocks on the same arm.
    #[test]
    fn walk_help_long_never_fires_as_a_pending_value() {
        assert_walk_run(
            &["run", "--backlog", "--help"],
            &["run", "--backlog", "--help"],
        );
    }

    /// TS-70, the value flags. A value flag takes the bare
    /// word after it and the walk joins the pair in the
    /// equals form.
    #[test]
    fn walk_flag_value_joins_onto_its_flag() {
        assert_walk_run(
            &["run", "--backlog", "b.json"],
            &["run", "--backlog=b.json"],
        );
    }

    /// TS-70, the value flags. An abbreviation standing for
    /// exactly one value flag of the subcommand sets the
    /// pending state the same way as the full name.
    #[test]
    fn walk_flag_abbreviation_takes_its_value_the_same_way() {
        assert_walk_run(&["run", "--win", "30"], &["run", "--win=30"]);
    }

    /// TS-70, the value flags. An abbreviated window flag
    /// keeps its own spelling in the joined form, the window
    /// join never having read it under its full name.
    #[test]
    fn walk_flag_abbreviated_window_flag_keeps_its_own_spelling() {
        assert_walk_run(&["run", "--window", "-2.5"], &["run", "--window=-2.5"]);
    }

    /// TS-70, the value flags. A value arriving already
    /// joined sets no pending state, so a help token after
    /// it still fires.
    #[test]
    fn walk_flag_value_arriving_joined_sets_no_pending() {
        assert_walk_fires(&["run", "--backlog=b.json", "-h"], Some("run"));
    }

    /// TS-70, the value flags. A plain separate value the
    /// window join never reads joins by the pending arm
    /// instead.
    #[test]
    fn walk_flag_plain_value_joins_by_the_pending_arm() {
        assert_walk_run(
            &["run", "--window-seconds", "5"],
            &["run", "--window-seconds=5"],
        );
    }

    /// TS-70, the value flags. A long token behind a
    /// subcommand matching no value flag takes no value, and
    /// the word after it stands as its own token.
    #[test]
    fn walk_flag_name_matching_no_flag_takes_no_value() {
        assert_walk_run(&["run", "--bogus", "x"], &["run", "--bogus", "x"]);
    }

    /// TS-70, the value flags. A bare word behind a
    /// subcommand with no flag standing joins nothing and
    /// blocks nothing.
    #[test]
    fn walk_flag_bare_word_behind_a_subcommand_stands_unjoined() {
        assert_walk_run(&["run", "word"], &["run", "word"]);
    }

    /// TS-70, the value flags. A short token behind a
    /// subcommand stands as its own token the same way.
    #[test]
    fn walk_flag_short_token_behind_a_subcommand_stands_unjoined() {
        assert_walk_run(&["run", "-x"], &["run", "-x"]);
    }

    /// TS-70, the value flags. A negative number reads as a
    /// positional, so it joins onto a standing flag.
    #[test]
    fn walk_flag_negative_number_value_joins() {
        assert_walk_run(&["run", "--result", "-2.5"], &["run", "--result=-2.5"]);
    }

    /// TS-70, the value flags. A dash token carrying a
    /// space reads as a positional whatever its prefix, so
    /// it joins onto a standing flag.
    #[test]
    fn walk_flag_dash_value_carrying_a_space_joins() {
        assert_walk_run(&["run", "--backlog", "-a b"], &["run", "--backlog=-a b"]);
    }

    /// TS-70, the value flags. A lone dash reads as a
    /// positional and joins.
    #[test]
    fn walk_flag_lone_dash_value_joins() {
        assert_walk_run(&["run", "--backlog", "-"], &["run", "--backlog=-"]);
    }

    /// TS-70, the value flags. An empty token reads as a
    /// positional, never starting with a dash, and joins.
    #[test]
    fn walk_flag_empty_value_joins() {
        assert_walk_run(&["run", "--backlog", ""], &["run", "--backlog="]);
    }

    /// TS-70, the value flags. The negative number pattern
    /// lets the digits before the point stand empty, so a
    /// dotted value with digits only after the point joins.
    #[test]
    fn walk_flag_dotted_value_with_no_digit_before_the_point_joins() {
        assert_walk_run(&["run", "--result", "-.5"], &["run", "--result=-.5"]);
    }

    /// TS-70, the value flags. A trailing point with no
    /// digit after it fails the pattern, so the token blocks
    /// instead of joining, the standing flag left unjoined.
    #[test]
    fn walk_flag_dotted_value_with_a_trailing_point_blocks() {
        assert_walk_run(&["run", "--result", "-5."], &["run", "--result", "-5."]);
    }

    /// TS-70, the value flags. The pattern end anchor
    /// matches just before one trailing newline, so a number
    /// with a newline after it joins.
    #[test]
    fn walk_flag_number_value_before_a_trailing_newline_joins() {
        assert_walk_run(&["run", "--result", "-42\n"], &["run", "--result=-42\n"]);
    }

    /// TS-70, the value flags. The short help family decides
    /// ahead of the space rule, so a help letter token
    /// carrying a space blocks as a standing flag's value
    /// instead of joining.
    #[test]
    fn walk_flag_help_family_value_blocks_instead_of_joining() {
        assert_walk_run(&["run", "--backlog", "-h x"], &["run", "--backlog", "-h x"]);
    }

    /// TS-70, the value flags. A standing flag meeting an
    /// option token blocks, and every later token passes
    /// through untouched, a help token among them.
    #[test]
    fn walk_flag_option_token_after_a_standing_flag_blocks_the_stream() {
        assert_walk_run(
            &["run", "--backlog", "--result", "-h"],
            &["run", "--backlog", "--result", "-h"],
        );
    }

    /// TS-70, the value flags. A flag standing at the end of
    /// the stream is handed over unjoined.
    #[test]
    fn walk_flag_standing_at_the_end_of_the_stream_stands_unjoined() {
        assert_walk_run(&["run", "--backlog"], &["run", "--backlog"]);
    }

    /// TS-70, the value flags. No pending state ever stands
    /// at the root, the root declaring no value flags, so a
    /// flag shaped token there takes no value.
    #[test]
    fn walk_flag_at_the_root_takes_no_value() {
        assert_walk_run(&["--backlog", "x"], &["--backlog", "x"]);
    }

    /// TS-70, the value flags. A repeated flag sets the
    /// pending state at each standing, and each value joins
    /// onto its own flag token.
    #[test]
    fn walk_flag_repeated_joins_each_value_onto_its_own_flag() {
        assert_walk_run(
            &["run", "--backlog", "a", "--backlog", "b"],
            &["run", "--backlog=a", "--backlog=b"],
        );
    }

    /// TS-70, recognition. A word equal to a subcommand name
    /// enters its spec, and the reads after it stand on that
    /// subcommand's tables.
    #[test]
    fn walk_recognition_subcommand_word_enters_its_spec() {
        assert_walk_fires(&["carry", "--out", "o", "-h"], Some("carry"));
    }

    /// TS-70, recognition. Subcommand names match exactly,
    /// so an abbreviation enters nothing and the word blocks
    /// as a positional at the root.
    #[test]
    fn walk_recognition_subcommand_names_never_abbreviate() {
        assert_walk_run(&["car", "-h"], &["car", "-h"]);
    }

    /// TS-70, recognition. A case variant of a name enters
    /// nothing the same way.
    #[test]
    fn walk_recognition_subcommand_names_match_in_their_own_case() {
        assert_walk_run(&["Run", "-h"], &["Run", "-h"]);
    }

    /// TS-70, recognition. A positional word at the root
    /// blocks, and a help token after it passes through
    /// untouched.
    #[test]
    fn walk_recognition_positional_at_the_root_blocks_the_stream() {
        assert_walk_run(&["word", "-h"], &["word", "-h"]);
    }

    /// TS-70, recognition. A negative number at the root
    /// reads as a positional and blocks the same way.
    #[test]
    fn walk_recognition_negative_number_at_the_root_blocks_the_stream() {
        assert_walk_run(&["-2.5", "-h"], &["-2.5", "-h"]);
    }

    /// TS-70, recognition. A dash token carrying a space at
    /// the root reads as a positional and blocks the same
    /// way.
    #[test]
    fn walk_recognition_dash_token_carrying_a_space_blocks_the_stream() {
        assert_walk_run(&["-a b", "-h"], &["-a b", "-h"]);
    }

    /// TS-70, recognition. An unmatched option token at the
    /// root blocks nothing, so a subcommand word after it
    /// still enters its spec.
    #[test]
    fn walk_recognition_unmatched_option_at_the_root_blocks_nothing() {
        assert_walk_fires(&["--bogus", "run", "-h"], Some("run"));
    }

    /// TS-70, recognition. A second subcommand name behind a
    /// subcommand is an ordinary token and enters nothing.
    #[test]
    fn walk_recognition_second_subcommand_word_stands_as_a_token() {
        assert_walk_run(&["run", "brief"], &["run", "brief"]);
    }

    /// TS-70, recognition. A help token after a second
    /// subcommand word answers for the entered subcommand,
    /// never for the word.
    #[test]
    fn walk_recognition_help_after_a_second_word_names_the_entered_subcommand() {
        assert_walk_fires(&["run", "brief", "-h"], Some("run"));
    }

    /// TS-70, recognition. A separator after a blocked word
    /// is a plain token under the blocked arm and draws no
    /// duplication after it.
    #[test]
    fn walk_recognition_separator_after_a_blocked_word_is_a_plain_token() {
        assert_walk_run(&["word", "--", "-h"], &["word", "--", "-h"]);
    }

    /// TS-70, recognition. An empty stream answers as an
    /// empty handed over stream.
    #[test]
    fn walk_recognition_empty_stream_runs_empty() {
        assert_walk_run(&[], &[]);
    }

    /// TS-70, recognition. A stream of only a subcommand
    /// word hands that word over as it stands.
    #[test]
    fn walk_recognition_subcommand_word_alone_runs_as_it_stands() {
        assert_walk_run(&["run"], &["run"]);
    }

    /// TS-70, the separator. A separator the walk reads ends
    /// the scan, and a second separator stands inserted
    /// immediately after it in the handed over stream.
    #[test]
    fn walk_separator_ends_the_scan_and_is_duplicated() {
        assert_walk_run(&["run", "--", "x"], &["run", "--", "--", "x"]);
    }

    /// TS-70, the separator. A separator at the end of the
    /// stream is duplicated the same way, the copy standing
    /// last.
    #[test]
    fn walk_separator_at_the_end_of_the_stream_is_duplicated() {
        assert_walk_run(&["run", "--"], &["run", "--", "--"]);
    }

    /// TS-70, the separator. Only the first separator is
    /// recorded, so a second written separator passes
    /// through as a plain token and the inserted copy stands
    /// between the two written ones.
    #[test]
    fn walk_separator_second_separator_is_a_plain_token() {
        assert_walk_run(&["run", "--", "--"], &["run", "--", "--", "--"]);
    }

    /// TS-70, the separator. A separator at the root ends
    /// the scan there, and the subcommand word after it
    /// never enters its spec.
    #[test]
    fn walk_separator_at_the_root_ends_the_scan() {
        assert_walk_run(&["--", "run"], &["--", "--", "run"]);
    }

    /// TS-70, the separator. A help token after the
    /// separator never fires, the separated arm running
    /// ahead of the help read.
    #[test]
    fn walk_separator_help_after_it_never_fires() {
        assert_walk_run(&["--", "-h"], &["--", "--", "-h"]);
    }

    /// TS-70, the separator. A help token behind a
    /// subcommand after its separator never fires the same
    /// way.
    #[test]
    fn walk_separator_help_behind_a_subcommand_after_it_never_fires() {
        assert_walk_run(&["run", "--", "-h"], &["run", "--", "--", "-h"]);
    }

    /// TS-70, the separator. The window join runs ahead of
    /// the walk, so a pair behind the separator stands
    /// joined in the handed over stream.
    #[test]
    fn walk_separator_window_join_runs_ahead_of_it() {
        assert_walk_run(
            &["run", "--", "--window-seconds", "-2.5"],
            &["run", "--", "--", "--window-seconds=-2.5"],
        );
    }

    /// TS-70, the separator. A separator a standing flag
    /// meets is recorded by the pending arm and duplicated
    /// the same way, the flag left unjoined.
    #[test]
    fn walk_separator_met_by_a_standing_flag_is_duplicated_the_same_way() {
        assert_walk_run(
            &["run", "--backlog", "--", "x"],
            &["run", "--backlog", "--", "--", "x"],
        );
    }
}
