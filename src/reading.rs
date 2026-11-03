//! One reading ladder for the three file surfaces of the tool.
//!
//! The backlog file, the brief file, and the resolutions file are
//! all read the same way, as UTF-8 text decoded as JSON and handed
//! to a tail parse, and they differ only in the error type a refusal
//! raises, the noun the refusal messages name, and the tail parse
//! itself. TS-34 keeps that ladder in this one place, so the reading
//! discipline of the three surfaces cannot drift apart.

use std::path::Path;

use crate::pyjson::{self, Value};

/// Read one file surface and return what its tail parse returns.
///
/// A file that cannot be read, a file that is not valid UTF-8, and a
/// file that is not valid JSON are refused under the surface error
/// with a message that names the surface through noun and names the
/// defect and the path. make_error builds the surface error type
/// from that message, which is the error type parameter of the
/// Python ladder.
pub fn read_json_file<T, E>(
    path: &Path,
    noun: &str,
    make_error: impl Fn(String) -> E,
    parse: impl FnOnce(Value) -> Result<T, E>,
) -> Result<T, E> {
    let shown = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(make_error(format!("{} cannot be read, {}", noun, shown)));
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Err(make_error(format!(
                "{} is not valid UTF-8, {}",
                noun, shown
            )));
        }
    };
    let data = match pyjson::parse(&text) {
        Ok(data) => data,
        Err(_) => {
            return Err(make_error(format!(
                "{} is not valid JSON, {}",
                noun, shown
            )));
        }
    };
    parse(data)
}
