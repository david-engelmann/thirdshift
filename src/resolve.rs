//! Decision resolution, the operator answers the decisions the brief queued.

use std::collections::HashSet;
use std::path::Path;

use crate::pyjson::{self, Value};
use crate::reading::{READER_MAX_DEPTH, ReadError, read_json_file};

pub const VERDICTS: [&str; 2] = ["approve", "reject"];

/// A resolutions file or brief record could not be read as recorded.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct ResolutionError(pub String);

fn refusal(message: String) -> ResolutionError {
    ResolutionError(message)
}

/// Unwrap parsed resolutions JSON into its raw resolution entries.
///
/// One shared reader for every caller, parallel to the backlog
/// reader. An object must carry a resolutions list. Anything else
/// is a load error, never a silent empty resolutions list.
pub fn parse_resolutions_entries(data: Value) -> Result<Vec<Value>, ResolutionError> {
    if data.is_object() {
        return match data.get("resolutions") {
            Some(Value::List(resolutions)) => Ok(resolutions.clone()),
            _ => Err(refusal(
                "resolutions object carries no resolutions list".to_string(),
            )),
        };
    }
    Err(refusal(
        "resolutions file is not an object with a resolutions list".to_string(),
    ))
}

pub fn read_resolutions_file(path: &Path) -> Result<Vec<Value>, ReadError<ResolutionError>> {
    read_json_file(
        path,
        "resolutions file",
        READER_MAX_DEPTH,
        ResolutionError,
        parse_resolutions_entries,
    )
}

/// Read a resolutions file and return its raw resolution entries.
///
/// Entries come back unvalidated on purpose, parallel to the backlog
/// reader. apply_resolutions checks every entry before it folds any
/// of them in, so a defective entry refuses the whole application.
pub fn load_resolutions(path: &Path) -> Result<Vec<Value>, ReadError<ResolutionError>> {
    read_resolutions_file(path)
}

/// Check the brief shape on a decoded value and return the record.
///
/// The tail parse of the brief surface. The value must be an object
/// carrying a decisions list, the shape the brief step writes.
/// Anything else is a load error.
fn parse_brief_record(data: Value) -> Result<Value, ResolutionError> {
    let valid = data.is_object() && matches!(data.get("decisions"), Some(Value::List(_)));
    if !valid {
        return Err(refusal(
            "brief file is not a brief record, it carries no decisions list".to_string(),
        ));
    }
    Ok(data)
}

/// The deepest copy cost a brief record can carry into
/// resolution. The Python resolver folds the record through
/// two recursive copies, one of the decisions it carries and
/// one of the record itself, and each copy spends two frames
/// on every container along the deepest path it walks and one
/// frame on a scalar at the foot of that path. The copies
/// first fail when the cost of the deepest path passes 994,
/// far below the reader's own ceiling, so a brief past it
/// fails in the Python tool the uncaught way and the port
/// refuses it at the same cost with the too deep refusal.
/// The cost is a frame count and not a container count,
/// because the two counts part at the edge. List shaped
/// depth with an empty list at its foot reaches a path of
/// 497 containers, object shaped depth with a scalar at its
/// foot reaches a path of 496, and both fail one step past
/// those reaches at the first cost over the ceiling.
pub const BRIEF_COPY_MAX_COST: usize = 994;

/// Read a brief JSON file and return the record it carries.
///
/// The file must hold a JSON object with a decisions list, the shape
/// the brief step writes. Anything else is a load error.
pub fn read_brief_file(path: &Path) -> Result<Value, ReadError<ResolutionError>> {
    read_json_file(
        path,
        "brief file",
        READER_MAX_DEPTH,
        ResolutionError,
        parse_brief_record,
    )
}

/// Validate the decisions a brief record carries and return
/// their ids, ahead of the copies the resolver makes.
///
/// The Python resolver validates the brief entries before it
/// copies anything, so an entry defect answers at status 2
/// even on a brief deep enough that a copy would fail, and
/// this check runs where the resolver's own validation runs.
pub fn check_brief_decision_entries(record: &Value) -> Result<HashSet<String>, ResolutionError> {
    let decisions = match record.get("decisions") {
        Some(Value::List(decisions)) => decisions,
        _ => {
            return Err(refusal(
                "brief record carries no decisions list".to_string(),
            ));
        }
    };
    check_brief_decisions(decisions)
}

/// Refuse a value whose deepest copy cost passes the ceiling
/// the Python resolver's recursive copies reach.
///
/// The resolver copies the decisions list before it validates
/// the resolutions and the whole record after, and this
/// refusal stands where the matching copy stands in that
/// order, with the too deep refusal of the brief surface.
pub fn refuse_brief_past_copy_ceiling(
    path: &Path,
    value: &Value,
) -> Result<(), ReadError<ResolutionError>> {
    if pyjson::deepest_copy_cost(value) > BRIEF_COPY_MAX_COST {
        return Err(ReadError::TooDeep(format!(
            "brief file is nested too deeply, {}",
            path.display()
        )));
    }
    Ok(())
}

/// Validate one resolution entry and return its decision, verdict,
/// and note.
///
/// Every defect is a refusal that names it, in the order the checks
/// run. Structural defects come first, then the TS-25 refusals, so a
/// resolutions list is either fully applicable or refused whole.
fn check_resolution(
    entry: &Value,
    carried_ids: &HashSet<String>,
    seen_ids: &mut HashSet<String>,
) -> Result<(String, String, String), ResolutionError> {
    if !entry.is_object() {
        return Err(refusal(
            "malformed resolution, it is not an object".to_string(),
        ));
    }
    let decision = match entry.get("decision") {
        Some(Value::Str(text)) if !text.is_empty() => text.clone(),
        _ => {
            return Err(refusal(
                "malformed resolution, its decision is not a non-empty string".to_string(),
            ));
        }
    };
    let verdict = match entry.get("verdict") {
        Some(Value::Str(text)) => text.clone(),
        _ => {
            return Err(refusal(format!(
                "malformed resolution for decision {}, its verdict is not a string",
                decision
            )));
        }
    };
    let note = match entry.get("note") {
        Some(Value::Str(text)) => text.clone(),
        _ => {
            return Err(refusal(format!(
                "malformed resolution for decision {}, its note is not a string",
                decision
            )));
        }
    };
    if seen_ids.contains(&decision) {
        return Err(refusal(format!(
            "decision {} is named twice in the resolutions",
            decision
        )));
    }
    if !carried_ids.contains(&decision) {
        return Err(refusal(format!(
            "resolution names decision {}, which the brief does not carry",
            decision
        )));
    }
    if !VERDICTS.contains(&verdict.as_str()) {
        return Err(refusal(format!(
            "verdict for decision {} is neither approve nor reject",
            decision
        )));
    }
    seen_ids.insert(decision.clone());
    Ok((decision, verdict, note))
}

/// Validate the decisions a brief record carries and return their
/// ids.
///
/// Every entry must be an object carrying a non-empty string id and
/// string item and question fields, the shape the brief step writes,
/// and no id may repeat. A record outside that shape is refused
/// whole, so no defect in the brief can escape the refusal
/// discipline or fold one decision twice.
fn check_brief_decisions(decisions: &[Value]) -> Result<HashSet<String>, ResolutionError> {
    let mut carried_ids: HashSet<String> = HashSet::new();
    for entry in decisions {
        if !entry.is_object() {
            return Err(refusal(
                "brief record carries a decision that is not an object".to_string(),
            ));
        }
        let decision_id = match entry.get("id") {
            Some(Value::Str(text)) if !text.is_empty() => text.clone(),
            _ => {
                return Err(refusal(
                    "brief record carries a decision whose id is not a non-empty string"
                        .to_string(),
                ));
            }
        };
        let item_ok = matches!(entry.get("item"), Some(Value::Str(_)));
        let question_ok = matches!(entry.get("question"), Some(Value::Str(_)));
        if !item_ok || !question_ok {
            return Err(refusal(format!(
                "brief record carries decision {} without a string item and question",
                decision_id
            )));
        }
        if carried_ids.contains(&decision_id) {
            return Err(refusal(format!(
                "brief record carries decision {} twice",
                decision_id
            )));
        }
        carried_ids.insert(decision_id);
    }
    Ok(carried_ids)
}

/// Fold recorded verdicts into a brief record and return the
/// resolved record.
///
/// Every resolution is validated before any is folded in, so a
/// refusal produces no record and leaves the brief record exactly as
/// it was. The run record is carried across untouched, and the
/// outcome lands alongside it under the resolved list and the
/// decision counts block. The returned record shares no state with
/// the record it was resolved from, because every value here is
/// owned and cloned.
pub fn apply_resolutions(record: &Value, resolutions: &[Value]) -> Result<Value, ResolutionError> {
    let decisions = match record.get("decisions") {
        Some(Value::List(decisions)) => decisions.clone(),
        _ => {
            return Err(refusal(
                "brief record carries no decisions list".to_string(),
            ));
        }
    };
    let carried_ids = check_brief_decisions(&decisions)?;
    let mut verdicts: Vec<(String, String, String)> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    for entry in resolutions {
        let (decision, verdict, note) = check_resolution(entry, &carried_ids, &mut seen_ids)?;
        verdicts.push((decision, verdict, note));
    }

    let mut queued: Vec<Value> = Vec::new();
    let mut resolved: Vec<Value> = Vec::new();
    for entry in &decisions {
        let decision_id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        match verdicts.iter().find(|(name, _, _)| name == decision_id) {
            Some((_, verdict, note)) => {
                let mut folded = entry.clone();
                folded.set("verdict", Value::Str(verdict.clone()));
                folded.set("note", Value::Str(note.clone()));
                resolved.push(folded);
            }
            None => queued.push(entry.clone()),
        }
    }

    let carried = decisions.len();
    let queued_count = queued.len();
    let resolved_count = resolved.len();
    let mut result = record.clone();
    result.set("decisions", Value::List(queued));
    result.set("resolved", Value::List(resolved));
    result.set(
        "decision_counts",
        Value::object(vec![
            ("carried".to_string(), Value::usize(carried)),
            ("queued".to_string(), Value::usize(queued_count)),
            ("resolved".to_string(), Value::usize(resolved_count)),
        ]),
    );
    Ok(result)
}
