//! One reading ladder for the three file surfaces of the tool.
//!
//! The backlog file, the brief file, and the resolutions file are
//! all read the same way, as UTF-8 text decoded as JSON and handed
//! to a tail parse, and they differ only in the error type a refusal
//! raises, the noun the refusal messages name, and the tail parse
//! itself. TS-34 keeps that ladder in this one place, so the reading
//! discipline of the three surfaces cannot drift apart.

use std::path::Path;

use crate::pyjson::{self, ParseError, Value};

/// The deepest container path the Python reader reaches on the
/// file surfaces before its recursion error, counted with the
/// root container at depth one. Measured against the Round 4
/// tool at the Round 5 fix, where a backlog wrapped in an object
/// first failed at a path of 9995 containers.
pub const READER_MAX_DEPTH: usize = 9994;

/// A reading failure on one of the file surfaces.
///
/// The surface variant carries the surface error type, the refusal
/// the ladder and the tail parse have always raised. The too deep
/// variant carries the refusal line for a document nested past the
/// Python reader's own ceiling. That failure escapes the Python
/// tool as an uncaught error, so the command line answers it with
/// a different exit status than a surface refusal, and the two
/// kinds stay distinct here so the status can follow the kind.
#[derive(Debug, thiserror::Error)]
pub enum ReadError<E: std::error::Error> {
    #[error("{0}")]
    Surface(#[from] E),
    #[error("{0}")]
    TooDeep(String),
}

/// Read one file surface and return what its tail parse returns.
///
/// A file that cannot be read, a file that is not valid UTF-8, and a
/// file that is not valid JSON are refused under the surface error
/// with a message that names the surface through noun and names the
/// defect and the path. make_error builds the surface error type
/// from that message, which is the error type parameter of the
/// Python ladder. A file nested past max_depth containers is
/// refused as too deep instead, in the ladder's own words.
pub fn read_json_file<T, E: std::error::Error>(
    path: &Path,
    noun: &str,
    max_depth: usize,
    make_error: impl Fn(String) -> E,
    parse: impl FnOnce(Value) -> Result<T, E>,
) -> Result<T, ReadError<E>> {
    let shown = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(ReadError::Surface(make_error(format!(
                "{} cannot be read, {}",
                noun, shown
            ))));
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Err(ReadError::Surface(make_error(format!(
                "{} is not valid UTF-8, {}",
                noun, shown
            ))));
        }
    };
    let data = match pyjson::parse_with_depth(&text, max_depth) {
        Ok(data) => data,
        Err(ParseError::TooDeep) => {
            return Err(ReadError::TooDeep(format!(
                "{} is nested too deeply, {}",
                noun, shown
            )));
        }
        Err(ParseError::Syntax) => {
            return Err(ReadError::Surface(make_error(format!(
                "{} is not valid JSON, {}",
                noun, shown
            ))));
        }
    };
    parse(data).map_err(ReadError::Surface)
}
