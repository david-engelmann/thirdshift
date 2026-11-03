//! Command line entry point, the thirdshift binary.
//!
//! Subcommands
//!   enqueue  append one task to a backlog file
//!   run      work a backlog file and write the run record as JSON
//!   brief    work a backlog file and write the morning brief as Markdown and JSON
//!   resolve  fold a resolutions file into a brief JSON file and print the resolved record

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use thirdshift::brief::{brief_json, brief_markdown};
use thirdshift::pyjson::{self, Value};
use thirdshift::resolve::{ResolutionError, apply_resolutions, load_resolutions, read_brief_file};
use thirdshift::runner::{
    BacklogError, RunOptions, WindowValue, load_backlog, read_backlog_file, run_backlog,
};

/// A refusal the command line reports as one error line with exit
/// status 2, from either file surface.
enum CliError {
    Backlog(BacklogError),
    Resolution(ResolutionError),
    Run(String),
}

impl CliError {
    fn message(&self) -> String {
        match self {
            CliError::Backlog(error) => error.0.clone(),
            CliError::Resolution(error) => error.0.clone(),
            CliError::Run(message) => message.clone(),
        }
    }
}

impl From<BacklogError> for CliError {
    fn from(error: BacklogError) -> CliError {
        CliError::Backlog(error)
    }
}

impl From<ResolutionError> for CliError {
    fn from(error: ResolutionError) -> CliError {
        CliError::Resolution(error)
    }
}

fn print_out(text: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(text.as_bytes());
    let _ = lock.flush();
}

fn write_file(path: &Path, text: &str) -> Result<(), CliError> {
    std::fs::write(path, text)
        .map_err(|error| CliError::Run(format!("{}: {}", path.display(), error)))
}

fn write_tasks(path: &Path, tasks: &[Value]) -> Result<(), CliError> {
    let document = Value::object(vec![(
        "tasks".to_string(),
        Value::List(tasks.to_vec()),
    )]);
    write_file(path, &(pyjson::dumps_indent2(&document) + "\n"))
}

fn cmd_enqueue(options: &Options) -> Result<i32, CliError> {
    let file = options.required("file")?;
    let target = PathBuf::from(file);
    let mut tasks = if target.exists() {
        read_backlog_file(&target)?
    } else {
        Vec::new()
    };
    let id = options.required("id")?;
    let duplicate = tasks.iter().any(|task| {
        task.is_object()
            && task.get("id") == Some(&Value::Str(id.to_string()))
    });
    if duplicate {
        eprintln!("duplicate id {}", id);
        return Ok(2);
    }
    let task = Value::object(vec![
        ("id".to_string(), Value::Str(id.to_string())),
        (
            "title".to_string(),
            Value::Str(options.required("title")?.to_string()),
        ),
        (
            "kind".to_string(),
            Value::Str(options.get("kind").unwrap_or("work").to_string()),
        ),
        (
            "question".to_string(),
            Value::Str(options.get("question").unwrap_or("").to_string()),
        ),
    ]);
    tasks.push(task);
    write_tasks(&target, &tasks)?;
    println!("enqueued {}", id);
    Ok(0)
}

/// Turn the window option text into a number of seconds, or None.
///
/// A bad window follows the usual error discipline of the tool, so
/// it raises BacklogError and the caller prints one error line and
/// exits with status 2 rather than showing an argument parsing
/// traceback.
fn window_value(raw: Option<&str>) -> Result<Option<WindowValue>, CliError> {
    let raw = match raw {
        None => return Ok(None),
        Some(raw) => raw,
    };
    let value = match pyjson::py_float_parse(raw) {
        Some(value) => value,
        None => {
            return Err(BacklogError(format!(
                "window seconds is not a number, {}",
                raw
            ))
            .into())
        }
    };
    if !value.is_finite() || value < 0.0 {
        return Err(BacklogError(format!(
            "window seconds must be a non-negative number, {}",
            raw
        ))
        .into());
    }
    Ok(Some(WindowValue::Float(value)))
}

fn run_record(options: &Options) -> Result<Value, CliError> {
    let backlog = load_backlog(Path::new(options.required("backlog")?))?;
    let window = window_value(options.get("window-seconds"))?;
    let mut run_options = RunOptions::default();
    run_options.window = window;
    run_backlog(&backlog, run_options)
        .map_err(|error| CliError::Run(error.to_string()))
}

fn cmd_run(options: &Options) -> Result<i32, CliError> {
    let record = run_record(options)?;
    let text = pyjson::dumps_indent2(&record) + "\n";
    if let Some(result) = options.get("result") {
        write_file(Path::new(result), &text)?;
    }
    print_out(&text);
    Ok(0)
}

fn cmd_brief(options: &Options) -> Result<i32, CliError> {
    let record = run_record(options)?;
    let md = options.get("md");
    let json = options.get("json");
    if let Some(md) = md {
        write_file(Path::new(md), &brief_markdown(&record))?;
    }
    if let Some(json) = json {
        write_file(Path::new(json), &brief_json(&record))?;
    }
    if md.is_none() && json.is_none() {
        print_out(&brief_markdown(&record));
    }
    Ok(0)
}

fn cmd_resolve(options: &Options) -> Result<i32, CliError> {
    let record = read_brief_file(Path::new(options.required("brief")?))?;
    let resolutions = load_resolutions(Path::new(options.required("resolutions")?))?;
    let resolved = apply_resolutions(&record, &resolutions)?;
    print_out(&(pyjson::dumps_indent2(&resolved) + "\n"));
    Ok(0)
}

/// The parsed flags of one subcommand.
struct Options {
    values: Vec<(String, String)>,
}

impl Options {
    fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn required(&self, name: &str) -> Result<&str, CliError> {
        self.get(name)
            .ok_or_else(|| CliError::Run(format!("missing required flag --{}", name)))
    }
}

fn usage_error() -> i32 {
    eprintln!("usage: thirdshift {{enqueue,run,brief,resolve}} ...");
    2
}

/// Parse the flag tokens of one subcommand against its known flags.
/// A token in the equals form carries its value with it, and any
/// other flag takes the next token as its value, as argparse does.
fn parse_options(
    tokens: &[String],
    known: &[&str],
    required: &[&str],
) -> Result<Options, i32> {
    let mut values: Vec<(String, String)> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        let Some(flag) = token.strip_prefix("--") else {
            return Err(usage_error());
        };
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (flag.to_string(), None),
        };
        if !known.contains(&name.as_str()) {
            return Err(usage_error());
        }
        let value = match inline {
            Some(value) => value,
            None => {
                index += 1;
                match tokens.get(index) {
                    Some(value) => value.clone(),
                    None => return Err(usage_error()),
                }
            }
        };
        values.push((name, value));
        index += 1;
    }
    let options = Options { values };
    for name in required {
        if options.get(name).is_none() {
            return Err(usage_error());
        }
    }
    Ok(options)
}

/// Whether a token after its dash reads as a number, in the pattern
/// the Python CLI joins onto the window flag. A digit, a point and a
/// digit, or the whole rest reading as nan, inf, or infinity in any
/// case.
fn window_token_is_value(token: &str) -> bool {
    let Some(rest) = token.strip_prefix('-') else {
        return false;
    };
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_digit() {
        return true;
    }
    if first == '.' && chars.next().map_or(false, |c| c.is_ascii_digit()) {
        return true;
    }
    let lower = rest.to_lowercase();
    lower == "nan" || lower == "inf" || lower == "infinity"
}

/// Join a dash-prefixed window value onto its flag before parsing.
///
/// Argparse treats a token that starts with a dash as an option, so
/// a window value written in two tokens (-inf, -nan, -2.5) never
/// reaches the validator and the refusal comes out as a usage block
/// instead of the one error line TS-32 requires. A token that reads
/// as a number after its dash is the flag's value, so it is joined
/// into the equals form here and validated on the usual path.
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

fn run_cli(argv: Vec<String>) -> i32 {
    let argv = join_window_value(&argv);
    let Some((command, rest)) = argv.split_first() else {
        return usage_error();
    };
    let (known, required): (&[&str], &[&str]) = match command.as_str() {
        "enqueue" => (
            &["file", "id", "title", "kind", "question"],
            &["file", "id", "title"],
        ),
        "run" => (&["backlog", "result", "window-seconds"], &["backlog"]),
        "brief" => (
            &["backlog", "md", "json", "window-seconds"],
            &["backlog"],
        ),
        "resolve" => (&["brief", "resolutions"], &["brief", "resolutions"]),
        _ => return usage_error(),
    };
    let options = match parse_options(rest, known, required) {
        Ok(options) => options,
        Err(code) => return code,
    };
    let outcome = match command.as_str() {
        "enqueue" => cmd_enqueue(&options),
        "run" => cmd_run(&options),
        "brief" => cmd_brief(&options),
        "resolve" => cmd_resolve(&options),
        _ => unreachable!(),
    };
    match outcome {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error - {}", error.message());
            2
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run_cli(argv) as u8)
}
