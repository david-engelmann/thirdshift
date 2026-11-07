//! The morning brief, one content in Markdown and in JSON.
//!
//! Both renderers take the run record itself. The brief carries
//! exactly what the record carries, so the two forms cannot drift
//! apart in what they show.

use crate::pyjson::{self, Value};

pub fn brief_json(record: &Value) -> String {
    pyjson::dumps_indent2(record) + "\n"
}

fn field<'a>(entry: &'a Value, name: &str) -> &'a Value {
    entry.get(name).unwrap_or(&Value::Null)
}

fn text(value: &Value) -> String {
    pyjson::py_str(value)
}

pub fn brief_markdown(record: &Value) -> String {
    let mut lines: Vec<String> = vec![
        "# Morning brief".to_string(),
        String::new(),
        "## What ran".to_string(),
        String::new(),
    ];
    if let Some(ran) = record.get("ran").and_then(Value::as_list) {
        for entry in ran {
            let mut parts: Vec<String> = vec![format!("- {}", text(field(entry, "id")))];
            if field(entry, "title").truthy() {
                parts.push(text(field(entry, "title")));
            }
            if field(entry, "worker").truthy() {
                parts.push(format!("by {}", text(field(entry, "worker"))));
            }
            parts.push(text(field(entry, "status")));
            if field(entry, "output").truthy() {
                parts.push(text(field(entry, "output")));
            }
            if field(entry, "reason").truthy() {
                parts.push(format!("reason - {}", text(field(entry, "reason"))));
            }
            lines.push(parts.join(" - "));
        }
    }
    lines.push(String::new());
    lines.push("## What it produced".to_string());
    lines.push(String::new());
    let produced = record.get("produced").and_then(Value::as_list);
    match produced {
        Some(outputs) if !outputs.is_empty() => {
            for output in outputs {
                lines.push(format!("- {}", text(output)));
            }
        }
        _ => lines.push("- nothing".to_string()),
    }
    lines.push(String::new());
    lines.push("## Decisions queued".to_string());
    lines.push(String::new());
    let decisions = record.get("decisions").and_then(Value::as_list);
    match decisions {
        Some(decisions) if !decisions.is_empty() => {
            for decision in decisions {
                lines.push(format!(
                    "- {} for item {} - {} - answer approve or reject",
                    text(field(decision, "id")),
                    text(field(decision, "item")),
                    text(field(decision, "question"))
                ));
            }
        }
        _ => lines.push("- none".to_string()),
    }
    lines.push(String::new());
    lines.push("## Counts".to_string());
    lines.push(String::new());
    let counts = record.get("counts").cloned().unwrap_or(Value::Null);
    lines.push(format!("- backlog {}", text(field(&counts, "backlog"))));
    lines.push(format!("- finished {}", text(field(&counts, "finished"))));
    lines.push(format!(
        "- unfinished {}",
        text(field(&counts, "unfinished"))
    ));
    lines.push(format!("- recorded {}", text(field(&counts, "recorded"))));
    lines.push(String::new());
    lines.push("## Stock".to_string());
    lines.push(String::new());
    let stock = record.get("stock").cloned().unwrap_or(Value::Null);
    lines.push(format!("- workers {}", text(field(&stock, "workers"))));
    lines.push(format!(
        "- stocked at start {}",
        text(field(&stock, "stocked_at_start"))
    ));
    lines.push(format!(
        "- stockouts recorded {}",
        text(field(&stock, "stockouts_recorded"))
    ));
    if let Some(window) = record.get("window") {
        lines.push(String::new());
        lines.push("## Window".to_string());
        lines.push(String::new());
        lines.push(format!(
            "- window seconds {}",
            text(field(window, "window_seconds"))
        ));
        lines.push(format!(
            "- elapsed seconds {}",
            text(field(window, "elapsed_seconds"))
        ));
        lines.push(format!("- started {}", text(field(window, "started"))));
        lines.push(format!("- unstarted {}", text(field(window, "unstarted"))));
    }
    lines.push(String::new());
    lines.join("\n")
}
