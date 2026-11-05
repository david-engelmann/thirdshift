//! The carry, a finished run hands its unfinished work to the
//! next shift.
//!
//! The run record knows which items finished and which did not,
//! but a record entry carries no kind and no question, so the
//! next backlog cannot be rebuilt from the record alone. The
//! carry joins the record against the original backlog, which
//! is the honest shape of a handoff anyway, the outgoing brief
//! in one hand and the incoming list in the other.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::pyjson::Value;
use crate::reading::{READER_MAX_DEPTH, ReadError, read_json_file};

/// A brief record or backlog could not be joined as recorded.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct CarryError(pub String);

fn refusal(message: String) -> CarryError {
    CarryError(message)
}

/// Check the brief shape on a decoded value and return the record.
///
/// The tail parse of the carry record surface. The value must be
/// an object carrying a ran list, the part of the record the
/// carry joins on. Anything else is a load error.
fn parse_carry_record(data: Value) -> Result<Value, CarryError> {
    let valid = data.is_object() && matches!(data.get("ran"), Some(Value::List(_)));
    if !valid {
        return Err(refusal(
            "brief file is not a brief record, it carries no ran list".to_string(),
        ));
    }
    Ok(data)
}

/// Read a brief JSON file and return the record it carries, for
/// the carry.
///
/// The file is read through the shared reading ladder under the
/// brief file noun, so its load refusals are the brief surface
/// refusals. The record may be an ordinary brief record or a
/// resolved record alike, because resolving never changes the
/// finished state of an item.
pub fn read_record_file(path: &Path) -> Result<Value, ReadError<CarryError>> {
    read_json_file(
        path,
        "brief file",
        READER_MAX_DEPTH,
        CarryError,
        parse_carry_record,
    )
}

/// Join a brief record against the backlog it ran and return the
/// entries the next shift inherits.
///
/// The carried entries are exactly the items the record marks
/// unfinished, each the whole backlog entry as it stands, in the
/// order the entries stand in the backlog. The join is exact and
/// refuses whole, in the order the checks run. The record entries
/// are validated first, in the order they stand in the record,
/// then the backlog entries are named, in the order they stand
/// in the backlog, then an item the record names that the
/// backlog does not carry refuses, and last an item the backlog
/// carries that the record does not mention refuses.
pub fn carry_backlog(entries: &[Value], record: &Value) -> Result<Vec<Value>, CarryError> {
    let ran = match record.get("ran") {
        Some(Value::List(ran)) => ran,
        _ => return Err(refusal("brief record carries no ran list".to_string())),
    };

    let mut outcomes: Vec<(String, bool)> = Vec::new();
    let mut record_ids: HashSet<String> = HashSet::new();
    for entry in ran {
        if !entry.is_object() {
            return Err(refusal(
                "brief record carries a ran entry that is not an object".to_string(),
            ));
        }
        let item_id = match entry.get("id") {
            Some(Value::Str(text)) if !text.is_empty() => text.clone(),
            _ => {
                return Err(refusal(
                    "brief record carries a ran entry whose id is not a non-empty string"
                        .to_string(),
                ));
            }
        };
        let unfinished = match entry.get("status") {
            Some(Value::Str(text)) if text == "finished" => false,
            Some(Value::Str(text)) if text == "unfinished" => true,
            _ => {
                return Err(refusal(format!(
                    "brief record carries item {}, whose status is neither finished nor unfinished",
                    item_id
                )));
            }
        };
        outcomes.push((item_id, unfinished));
    }
    for (item_id, _) in &outcomes {
        if record_ids.contains(item_id) {
            return Err(refusal(format!(
                "brief record carries item {} twice",
                item_id
            )));
        }
        record_ids.insert(item_id.clone());
    }

    let mut backlog_id_list: Vec<String> = Vec::new();
    for entry in entries {
        let item_id = match entry.get("id") {
            Some(Value::Str(text)) if !text.is_empty() => text.clone(),
            _ => return Err(refusal("backlog entry carries no item id".to_string())),
        };
        backlog_id_list.push(item_id);
    }
    let mut backlog_ids: HashSet<String> = HashSet::new();
    for item_id in &backlog_id_list {
        if backlog_ids.contains(item_id) {
            return Err(refusal(format!("backlog carries item {} twice", item_id)));
        }
        backlog_ids.insert(item_id.clone());
    }

    for (item_id, _) in &outcomes {
        if !backlog_ids.contains(item_id) {
            return Err(refusal(format!(
                "record names item {}, which the backlog does not carry",
                item_id
            )));
        }
    }
    for entry in entries {
        let item_id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        if !record_ids.contains(item_id) {
            return Err(refusal(format!(
                "backlog carries item {}, which the record does not mention",
                item_id
            )));
        }
    }

    let unfinished: HashMap<&str, bool> = outcomes
        .iter()
        .map(|(item_id, unfinished)| (item_id.as_str(), *unfinished))
        .collect();
    let mut carried: Vec<Value> = Vec::new();
    for entry in entries {
        let item_id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        if unfinished.get(item_id) == Some(&true) {
            carried.push(entry.clone());
        }
    }
    Ok(carried)
}
