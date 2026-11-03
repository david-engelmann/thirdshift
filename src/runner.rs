//! A runner that works a backlog of synthetic tasks through the queue.

use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use crate::clock::{Clock, FakeClock};
use crate::pyjson::{self, Value};
use crate::queue::{QueueError, ValueError, WorkQueue};
use crate::reading::{READER_MAX_DEPTH, ReadError, read_json_file};

pub const LEASE_SECONDS: f64 = 30.0;

pub const WINDOW_CLOSED_REASON: &str = "the window closed before this item was started";

const WINDOW_MESSAGE: &str = "window must be a non-negative number of seconds";

/// A window at the library surface, mirroring the shapes Python
/// accepts there. A window is any numeric value that converts to a
/// finite float, so decimal and fraction values are accepted as the
/// number they name, and a numeric value too large to convert is
/// refused with the same validation error as any other bad window.
#[derive(Clone, Debug)]
pub enum WindowValue {
    Float(f64),
    Int(i64),
    UInt(u64),
    /// The decimal text of an integer too large for a machine int.
    BigInt(String),
    /// The text of a decimal value, as the Python Decimal carries it.
    Decimal(String),
    /// A fraction as numerator and denominator.
    Fraction(i64, u64),
    /// A text value. The Python surface accepts any object and
    /// refuses the ones that are not numbers, so the port carries
    /// the refused shapes too, and validation refuses them all.
    Text(String),
    /// A boolean value, refused like any other non-number.
    Bool(bool),
}

impl From<f64> for WindowValue {
    fn from(value: f64) -> WindowValue {
        WindowValue::Float(value)
    }
}

impl From<i64> for WindowValue {
    fn from(value: i64) -> WindowValue {
        WindowValue::Int(value)
    }
}

/// Return the window as a float, or None when no window was given.
///
/// A window must be a non-negative finite number of seconds.
/// Anything else is refused before any item is touched, so a bad
/// window can never start a partial run.
fn validate_window(window: Option<WindowValue>) -> Result<Option<f64>, ValueError> {
    let window = match window {
        None => return Ok(None),
        Some(window) => window,
    };
    let value = match window {
        WindowValue::Float(value) => value,
        WindowValue::Int(value) => value as f64,
        WindowValue::UInt(value) => value as f64,
        WindowValue::BigInt(text) => match text.parse::<f64>() {
            Ok(value) => value,
            Err(_) => return Err(ValueError(WINDOW_MESSAGE.to_string())),
        },
        WindowValue::Decimal(text) => match pyjson::py_float_parse(&text) {
            Some(value) => value,
            None => return Err(ValueError(WINDOW_MESSAGE.to_string())),
        },
        WindowValue::Fraction(numerator, denominator) => {
            if denominator == 0 {
                return Err(ValueError(WINDOW_MESSAGE.to_string()));
            }
            numerator as f64 / denominator as f64
        }
        WindowValue::Text(_) | WindowValue::Bool(_) => {
            return Err(ValueError(WINDOW_MESSAGE.to_string()));
        }
    };
    if !value.is_finite() || value < 0.0 {
        return Err(ValueError(WINDOW_MESSAGE.to_string()));
    }
    Ok(Some(value))
}

/// The ways a run can be refused, named after the Python exceptions.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0}")]
    Value(ValueError),
    #[error("{0}")]
    Queue(QueueError),
}

impl From<ValueError> for RunError {
    fn from(error: ValueError) -> RunError {
        RunError::Value(error)
    }
}

impl From<QueueError> for RunError {
    fn from(error: QueueError) -> RunError {
        RunError::Queue(error)
    }
}

/// A backlog file could not be read as a backlog at all.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct BacklogError(pub String);

/// Stub worker behaviour, fixed by task kind so runs are reproducible.
///
/// A work task finishes with a plain output. A fail task never
/// finishes and says why. A decision task finishes by queueing a
/// decision. The task fields arrive as JSON values, exactly as the
/// backlog carried them.
pub fn stub_worker(item: &Value) -> Value {
    let kind = item.get("kind").cloned().unwrap_or(Value::Null);
    let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
    if kind == Value::Str("fail".to_string()) {
        return Value::object(vec![
            ("status".to_string(), Value::Str("unfinished".to_string())),
            ("output".to_string(), Value::Str(String::new())),
            (
                "reason".to_string(),
                Value::Str("stub worker could not complete this task".to_string()),
            ),
        ]);
    }
    if kind == Value::Str("decision".to_string()) {
        return Value::object(vec![
            ("status".to_string(), Value::Str("finished".to_string())),
            ("output".to_string(), Value::Str(String::new())),
            ("reason".to_string(), Value::Str(String::new())),
        ]);
    }
    Value::object(vec![
        ("status".to_string(), Value::Str("finished".to_string())),
        (
            "output".to_string(),
            Value::Str(format!("completed {}", item_id)),
        ),
        ("reason".to_string(), Value::Str(String::new())),
    ])
}

/// Unwrap parsed backlog JSON into its raw task entries.
///
/// One shared reader for every caller. An object must carry a tasks
/// list and a bare list is taken as the tasks themselves. Anything
/// else is a load error, never a silent empty backlog.
pub fn parse_backlog_entries(data: Value) -> Result<Vec<Value>, BacklogError> {
    match data {
        Value::Object(_) => match data.get("tasks") {
            Some(Value::List(tasks)) => Ok(tasks.clone()),
            _ => Err(BacklogError(
                "backlog object carries no tasks list".to_string(),
            )),
        },
        Value::List(tasks) => Ok(tasks),
        _ => Err(BacklogError(
            "backlog is neither a task list nor an object with a tasks list".to_string(),
        )),
    }
}

pub fn read_backlog_file(path: &Path) -> Result<Vec<Value>, ReadError<BacklogError>> {
    read_json_file(
        path,
        "backlog file",
        READER_MAX_DEPTH,
        BacklogError,
        parse_backlog_entries,
    )
}

/// Read a backlog file and return its raw task entries.
///
/// Entries come back unvalidated on purpose. The runner records the
/// entries that cannot become tasks in the run record instead of
/// failing the whole run over one bad entry.
pub fn load_backlog(path: &Path) -> Result<Vec<Value>, ReadError<BacklogError>> {
    read_backlog_file(path)
}

fn recorded_entry(entry_id: &str, title: Value, reason: &str) -> Value {
    Value::object(vec![
        ("id".to_string(), Value::Str(entry_id.to_string())),
        ("title".to_string(), title),
        ("worker".to_string(), Value::Str(String::new())),
        ("status".to_string(), Value::Str("unfinished".to_string())),
        ("output".to_string(), Value::Str(String::new())),
        ("reason".to_string(), Value::Str(reason.to_string())),
    ])
}

/// Name the first wrongly typed field on a backlog entry, or None.
///
/// TS-16 covers entries whose fields carry the wrong types, not only
/// entries that are missing or not objects. An id must be a non-empty
/// string, and title, kind, and question must be strings when
/// present. A field that is present as null is not wrongly typed,
/// exactly as in the Python check.
fn field_type_error(entry: &Value) -> Option<String> {
    let item_id = entry.get("id").cloned().unwrap_or(Value::Null);
    match &item_id {
        Value::Str(text) if !text.is_empty() => {}
        _ => {
            return Some(
                "malformed backlog entry, its id is not a non-empty string".to_string(),
            )
        }
    }
    for field in ["title", "kind", "question"] {
        let value = entry.get(field).cloned().unwrap_or(Value::Null);
        if value != Value::Null && !matches!(value, Value::Str(_)) {
            return Some(format!(
                "malformed backlog entry, its {} is not a string",
                field
            ));
        }
    }
    None
}

/// Sort one backlog entry into a runnable item or a recorded entry.
///
/// Returns (Some(item), None) for an entry the runner will work, or
/// (None, Some(record entry)) for an entry that is accounted for
/// without running. Nothing is dropped either way. A runnable item
/// carries the id, title, kind, and question as JSON values, as the
/// dict entry carried them.
fn prepare_entry(
    entry: &Value,
    seen_ids: &mut HashSet<String>,
) -> (Option<Value>, Option<Value>) {
    let item = if entry.is_object() {
        if entry.get("id").is_none() {
            let title = match entry.get("title") {
                Some(Value::Str(text)) => Value::Str(text.clone()),
                _ => Value::Str(String::new()),
            };
            return (
                None,
                Some(recorded_entry(
                    "malformed-entry",
                    title,
                    "malformed backlog entry, it carries no id",
                )),
            );
        }
        if let Some(field_error) = field_type_error(entry) {
            let raw_title = match entry.get("title") {
                Some(Value::Str(text)) => Value::Str(text.clone()),
                _ => Value::Str(String::new()),
            };
            return (
                None,
                Some(recorded_entry("malformed-entry", raw_title, &field_error)),
            );
        }
        let field = |name: &str, fallback: &str| -> Value {
            match entry.get(name) {
                Some(value) => value.clone(),
                None => Value::Str(fallback.to_string()),
            }
        };
        Value::object(vec![
            ("id".to_string(), field("id", "")),
            ("title".to_string(), field("title", "")),
            ("kind".to_string(), field("kind", "work")),
            ("question".to_string(), field("question", "")),
        ])
    } else {
        return (
            None,
            Some(recorded_entry(
                "malformed-entry",
                Value::Str(String::new()),
                "malformed backlog entry, it is not a task object",
            )),
        );
    };
    let question = item.get("question").cloned().unwrap_or(Value::Null);
    let kind = item.get("kind").cloned().unwrap_or(Value::Null);
    let item_id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let title = item.get("title").cloned().unwrap_or(Value::Null);
    if question.truthy() && kind != Value::Str("decision".to_string()) {
        return (
            None,
            Some(recorded_entry(
                &item_id,
                title,
                "malformed decision task, it carries a question but its kind is not decision",
            )),
        );
    }
    if seen_ids.contains(&item_id) {
        return (
            None,
            Some(recorded_entry(
                &item_id,
                title,
                &format!(
                    "duplicate item id {}, only the first entry with this id runs",
                    item_id
                ),
            )),
        );
    }
    seen_ids.insert(item_id);
    (Some(item), None)
}

/// The optional inputs of a run, with the defaults the Python
/// signature carries. Workers default to worker-1 and worker-2, the
/// clock to a fresh FakeClock when neither a clock nor a queue is
/// given, and the advance per item to one second.
pub struct RunOptions<'a> {
    pub worker_ids: Vec<String>,
    pub clock: Option<Rc<dyn Clock>>,
    pub queue: Option<&'a mut WorkQueue>,
    pub advance_per_item: f64,
    pub window: Option<WindowValue>,
}

impl<'a> Default for RunOptions<'a> {
    fn default() -> RunOptions<'a> {
        RunOptions {
            worker_ids: vec!["worker-1".to_string(), "worker-2".to_string()],
            clock: None,
            queue: None,
            advance_per_item: 1.0,
            window: None,
        }
    }
}

/// Work every entry once, claiming through the queue, and return the
/// record. The borrowed queue is worked in place, as the Python
/// runner works the queue it is handed, so callers can read its
/// claims and completions afterwards.
pub fn run_backlog(entries: &[Value], options: RunOptions) -> Result<Value, RunError> {
    let mut owned_queue: Option<WorkQueue> = None;
    let RunOptions {
        worker_ids,
        clock,
        queue,
        advance_per_item,
        window,
    } = options;
    let queue: &mut WorkQueue = match queue {
        Some(queue) => queue,
        None => {
            let clock = match clock.clone() {
                Some(clock) => clock,
                None => Rc::new(FakeClock::default()) as Rc<dyn Clock>,
            };
            owned_queue.insert(WorkQueue::new(clock))
        }
    };
    run_with_queue(entries, &worker_ids, clock, queue, advance_per_item, window)
}

/// The defaults of the Python signature in one call, for the common
/// case of a bare entry list.
pub fn run_backlog_simple(entries: &[Value]) -> Result<Value, RunError> {
    run_backlog(entries, RunOptions::default())
}

fn run_with_queue(
    entries: &[Value],
    worker_ids: &[String],
    clock: Option<Rc<dyn Clock>>,
    queue: &mut WorkQueue,
    advance_per_item: f64,
    window: Option<WindowValue>,
) -> Result<Value, RunError> {
    let window_seconds = validate_window(window)?;
    if !entries.is_empty() && worker_ids.is_empty() {
        return Err(ValueError(
            "run_backlog needs at least one worker to work a non-empty backlog".to_string(),
        )
        .into());
    }
    let clock: Rc<dyn Clock> = match clock {
        Some(clock) => clock,
        None => queue.clock.clone(),
    };
    for worker_id in worker_ids {
        queue.register_worker(worker_id);
    }
    let started_at = clock.now();

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut runnable: Vec<Value> = Vec::new();
    let mut ran: Vec<Value> = Vec::new();
    for entry in entries {
        let (item, problem) = prepare_entry(entry, &mut seen_ids);
        if let Some(problem) = problem {
            ran.push(problem);
        } else if let Some(item) = item {
            runnable.push(item);
        }
    }
    runnable.sort_by(|a, b| {
        let a_id = a.get("id").and_then(Value::as_str).unwrap_or("");
        let b_id = b.get("id").and_then(Value::as_str).unwrap_or("");
        a_id.cmp(b_id)
    });
    for item in &runnable {
        queue.enqueue(crate::queue::Item {
            item_id: item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            title: match item.get("title") {
                Some(Value::Str(text)) => text.clone(),
                _ => String::new(),
            },
            kind: match item.get("kind") {
                Some(Value::Str(text)) => text.clone(),
                _ => "work".to_string(),
            },
            question: match item.get("question") {
                Some(Value::Str(text)) => text.clone(),
                _ => String::new(),
            },
        })?;
    }
    let stocked_at_start = queue.pending_ids().len();

    let mut decisions: Vec<Value> = Vec::new();
    let mut started: u64 = 0;
    let mut unstarted: u64 = 0;
    for (index, item) in runnable.iter().enumerate() {
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let title = item.get("title").cloned().unwrap_or(Value::Null);
        if let Some(window) = window_seconds {
            if clock.now() - started_at >= window {
                unstarted += 1;
                ran.push(recorded_entry(&item_id, title, WINDOW_CLOSED_REASON));
                continue;
            }
        }
        started += 1;
        let worker_id = worker_ids[index % worker_ids.len()].clone();
        let claim = queue.claim(&item_id, &worker_id, LEASE_SECONDS)?;
        let result = stub_worker(item);
        let status = result
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        queue.complete(&item_id, &claim.worker_id, &status)?;
        ran.push(Value::object(vec![
            ("id".to_string(), Value::Str(item_id.clone())),
            ("title".to_string(), title.clone()),
            ("worker".to_string(), Value::Str(worker_id)),
            (
                "status".to_string(),
                result.get("status").cloned().unwrap_or(Value::Null),
            ),
            (
                "output".to_string(),
                result.get("output").cloned().unwrap_or(Value::Null),
            ),
            (
                "reason".to_string(),
                result.get("reason").cloned().unwrap_or(Value::Null),
            ),
        ]));
        let kind = item.get("kind").cloned().unwrap_or(Value::Null);
        if kind == Value::Str("decision".to_string()) && status == "finished" {
            let question = item.get("question").cloned().unwrap_or(Value::Null);
            let question = if question.truthy() { question } else { title };
            decisions.push(Value::object(vec![
                (
                    "id".to_string(),
                    Value::Str(format!("decision-{}", item_id)),
                ),
                ("item".to_string(), Value::Str(item_id.clone())),
                ("question".to_string(), question),
                (
                    "options".to_string(),
                    Value::List(vec![
                        Value::Str("approve".to_string()),
                        Value::Str("reject".to_string()),
                    ]),
                ),
            ]));
        }
        clock.advance(advance_per_item);
    }

    ran.sort_by(|a, b| {
        let a_id = a.get("id").and_then(Value::as_str).unwrap_or("");
        let b_id = b.get("id").and_then(Value::as_str).unwrap_or("");
        a_id.cmp(b_id)
            .then_with(|| pyjson::dumps_compact(a).cmp(&pyjson::dumps_compact(b)))
    });
    decisions.sort_by(|a, b| {
        let a_id = a.get("id").and_then(Value::as_str).unwrap_or("");
        let b_id = b.get("id").and_then(Value::as_str).unwrap_or("");
        a_id.cmp(b_id)
    });
    let produced: Vec<Value> = ran
        .iter()
        .filter_map(|entry| {
            let output = entry.get("output").cloned().unwrap_or(Value::Null);
            if output.truthy() {
                Some(output)
            } else {
                None
            }
        })
        .collect();
    let finished = ran
        .iter()
        .filter(|entry| entry.get("status") == Some(&Value::Str("finished".to_string())))
        .count();
    let recorded = ran.len();
    let mut record = Value::object(vec![
        ("ran".to_string(), Value::List(ran)),
        ("produced".to_string(), Value::List(produced)),
        ("decisions".to_string(), Value::List(decisions)),
        (
            "counts".to_string(),
            Value::object(vec![
                ("backlog".to_string(), Value::usize(entries.len())),
                ("finished".to_string(), Value::usize(finished)),
                (
                    "unfinished".to_string(),
                    Value::usize(recorded - finished),
                ),
                ("recorded".to_string(), Value::usize(recorded)),
            ]),
        ),
        (
            "stock".to_string(),
            Value::object(vec![
                ("workers".to_string(), Value::usize(worker_ids.len())),
                (
                    "stocked_at_start".to_string(),
                    Value::usize(stocked_at_start),
                ),
                (
                    "stockouts_recorded".to_string(),
                    Value::uint(queue.stockouts),
                ),
            ]),
        ),
    ]);
    if let Some(window) = window_seconds {
        record.set(
            "window",
            Value::object(vec![
                ("window_seconds".to_string(), Value::Float(window)),
                (
                    "elapsed_seconds".to_string(),
                    Value::Float(clock.now() - started_at),
                ),
                ("started".to_string(), Value::uint(started)),
                ("unstarted".to_string(), Value::uint(unstarted)),
            ]),
        );
    }
    Ok(record)
}
