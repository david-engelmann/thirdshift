//! A JSON value model, reader, and writer that mirror the Python json
//! module closely enough for byte parity with the implementation the
//! earlier rounds pinned.
//!
//! The reader accepts what Python json.loads accepts, including
//! integers of any length (kept exact as their canonical decimal
//! text), the NaN and Infinity constants, and float literals too
//! large for the float range (read as infinity). The writer prints
//! what Python json.dumps prints, with sorted keys when asked, with
//! ensure_ascii string escaping, and with floats in the Python repr
//! form. The reading ladder only needs to know whether a text parses,
//! so a parse failure is reported without a message.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

/// One JSON value as Python would carry it after json.loads.
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    /// An integer, held as its canonical decimal text so integers of
    /// any length survive exactly, as Python ints do.
    Int(String),
    /// A float. May be infinite or NaN, as a Python float may.
    Float(f64),
    Str(String),
    List(Vec<Value>),
    /// An object. Python dicts keep insertion order and this vector
    /// does the same, with at most one entry per key (a repeated key
    /// in a parsed document keeps its first position and its last
    /// value, as a Python dict does).
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn int(value: i64) -> Value {
        Value::Int(value.to_string())
    }

    pub fn uint(value: u64) -> Value {
        Value::Int(value.to_string())
    }

    pub fn usize(value: usize) -> Value {
        Value::Int(value.to_string())
    }

    pub fn object(entries: Vec<(String, Value)>) -> Value {
        let mut out: Vec<(String, Value)> = Vec::new();
        for (key, value) in entries {
            set_key(&mut out, key, value);
        }
        Value::Object(out)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Value::Object(entries) => entries
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        match self {
            Value::Object(entries) => set_key(entries, key.to_string(), value),
            _ => panic!("set called on a value that is not an object"),
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_list_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// Python truthiness for the shapes a record entry field can take.
    /// A non-empty string is true, an empty string and null are false.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(flag) => *flag,
            Value::Str(text) => !text.is_empty(),
            Value::Int(text) => text != "0",
            Value::Float(number) => *number != 0.0,
            Value::List(items) => !items.is_empty(),
            Value::Object(entries) => !entries.is_empty(),
        }
    }
}

fn set_key(entries: &mut Vec<(String, Value)>, key: String, value: Value) {
    for pair in entries.iter_mut() {
        if pair.0 == key {
            pair.1 = value;
            return;
        }
    }
    entries.push((key, value));
}

impl PartialEq for Value {
    /// Mirrors Python value equality within one variant. Objects
    /// compare without regard to key order, as dicts do. Floats
    /// compare as Python floats do, so NaN is not equal to itself
    /// here either. Values of different variants never compare
    /// equal here, where Python equates some of them, an integer
    /// with the float of the same value or a boolean with the
    /// integer one. No contracted path compares values across
    /// variants. The one equality a command consults, a stored
    /// task id against the text the operator typed, is false
    /// across types in Python too, and the readers validate the
    /// identifier and kind fields as strings before any other
    /// comparison can consult them.
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                let map: BTreeMap<&str, &Value> =
                    b.iter().map(|(k, v)| (k.as_str(), v)).collect();
                a.iter().all(|(k, v)| map.get(k.as_str()) == Some(&v))
            }
            _ => false,
        }
    }
}

/// The Python repr of a float, which is also the form Python str
/// produces for a float and the form json.dumps writes for one.
/// A minimal big unsigned integer, little endian base 2^32, with
/// only the operations the exact float rendering below needs.
struct BigUint(Vec<u32>);

impl BigUint {
    fn from_u64(value: u64) -> BigUint {
        let mut limbs = vec![value as u32, (value >> 32) as u32];
        while limbs.len() > 1 && *limbs.last().unwrap() == 0 {
            limbs.pop();
        }
        BigUint(limbs)
    }

    fn from_digits(digits: &[u8]) -> BigUint {
        let mut out = BigUint::from_u64(0);
        for &digit in digits {
            out.mul_small(10);
            out.add_small(digit as u32);
        }
        out
    }

    fn is_zero(&self) -> bool {
        self.0.iter().all(|limb| *limb == 0)
    }

    fn mul_small(&mut self, factor: u32) {
        let mut carry: u64 = 0;
        for limb in self.0.iter_mut() {
            let step = (*limb as u64) * (factor as u64) + carry;
            *limb = step as u32;
            carry = step >> 32;
        }
        while carry > 0 {
            self.0.push(carry as u32);
            carry >>= 32;
        }
    }

    fn add_small(&mut self, value: u32) {
        let mut carry = value as u64;
        let mut index = 0;
        while carry > 0 {
            if index == self.0.len() {
                self.0.push(0);
            }
            let step = self.0[index] as u64 + carry;
            self.0[index] = step as u32;
            carry = step >> 32;
            index += 1;
        }
    }

    fn shl(&mut self, bits: usize) {
        if bits == 0 || self.is_zero() {
            return;
        }
        let rem = (bits % 32) as u32;
        if rem > 0 {
            let mut carry = 0u32;
            for limb in self.0.iter_mut() {
                let next = (*limb << rem) | carry;
                carry = *limb >> (32 - rem);
                *limb = next;
            }
            if carry > 0 {
                self.0.push(carry);
            }
        }
        let words = bits / 32;
        if words > 0 {
            let mut shifted = vec![0u32; words];
            shifted.extend_from_slice(&self.0);
            self.0 = shifted;
        }
    }

    fn divmod_small(&mut self, divisor: u32) -> u32 {
        let mut remainder: u64 = 0;
        for limb in self.0.iter_mut().rev() {
            let step = (remainder << 32) | (*limb as u64);
            *limb = (step / divisor as u64) as u32;
            remainder = step % divisor as u64;
        }
        while self.0.len() > 1 && *self.0.last().unwrap() == 0 {
            self.0.pop();
        }
        remainder as u32
    }

    fn decimal_digits(&self) -> Vec<u8> {
        if self.is_zero() {
            return vec![0];
        }
        let mut copy = BigUint(self.0.clone());
        let mut digits = Vec::new();
        while !copy.is_zero() {
            digits.push(copy.divmod_small(10) as u8);
        }
        digits.reverse();
        digits
    }

    fn cmp_big(&self, other: &BigUint) -> Ordering {
        let a = self.trimmed_len();
        let b = other.trimmed_len();
        if a != b {
            return a.cmp(&b);
        }
        for index in (0..a).rev() {
            if self.0[index] != other.0[index] {
                return self.0[index].cmp(&other.0[index]);
            }
        }
        Ordering::Equal
    }

    fn trimmed_len(&self) -> usize {
        let mut len = self.0.len();
        while len > 1 && self.0[len - 1] == 0 {
            len -= 1;
        }
        len
    }
}

/// Compare the decimal number digits times ten to the scale with
/// the exact value of a positive finite double, exactly. Digit
/// strings and binary floats alike are exact, so the comparison is
/// done with big integer arithmetic on both sides.
fn cmp_decimal_to_double(digits: &[u8], scale: i32, value: f64) -> Ordering {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    let (significand, exponent) = if biased == 0 {
        (fraction, -1074i64)
    } else {
        (fraction + (1u64 << 52), biased - 1075)
    };
    let mut left = BigUint::from_digits(digits);
    let mut right = BigUint::from_u64(significand);
    if scale >= 0 {
        for _ in 0..scale {
            left.mul_small(10);
        }
    } else {
        for _ in 0..(-scale) {
            right.mul_small(10);
        }
    }
    if exponent >= 0 {
        right.shl(exponent as usize);
    } else {
        left.shl((-exponent) as usize);
    }
    left.cmp_big(&right)
}

/// The digit string of the integer the digits name, shifted by a
/// small delta, at the same length, or None when the shift would
/// change the length.
fn shift_digit_string(digits: &[u8], delta: i32) -> Option<Vec<u8>> {
    let mut out = digits.to_vec();
    if delta > 0 {
        let mut carry = delta as u32;
        let mut index = out.len();
        while carry > 0 {
            if index == 0 {
                return None;
            }
            index -= 1;
            let step = out[index] as u32 + carry;
            out[index] = (step % 10) as u8;
            carry = step / 10;
        }
    } else if delta < 0 {
        let mut borrow = (-delta) as u32;
        let mut index = out.len();
        while borrow > 0 {
            if index == 0 {
                return None;
            }
            index -= 1;
            if out[index] as u32 >= borrow {
                out[index] -= borrow as u8;
                borrow = 0;
            } else {
                out[index] = out[index] + 10 - borrow as u8;
                borrow = 1;
            }
        }
    }
    if out[0] == 0 {
        return None;
    }
    Some(out)
}

fn parse_candidate(digits: &[u8], exponent: i32) -> f64 {
    let mut text = String::new();
    for (index, digit) in digits.iter().enumerate() {
        if index == 1 {
            text.push('.');
        }
        text.push(char::from(b'0' + digit));
    }
    text.push('e');
    text.push_str(&exponent.to_string());
    text.parse().expect("candidate parses")
}

/// The shortest digits Python repr would print for a positive
/// finite double, with the decimal exponent of the first digit.
///
/// Rust prints a shortest digit string that round trips, and so
/// does Python, but the two disagree when the exact value sits
/// exactly between two shortest candidates, where Python takes
/// the one whose last digit is even. They can also sit a digit
/// apart when the closest candidate is not the one Rust chose.
/// So the neighbors of the Rust digits that also round trip are
/// gathered, and the one closest to the exact value wins, with
/// the even rule on an exact tie, all decided exactly.
fn python_shortest_digits(magnitude: f64) -> (Vec<u8>, i32) {
    let scientific = format!("{:e}", magnitude);
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("integer exponent");
    let digits: Vec<u8> = mantissa
        .bytes()
        .filter(|byte| *byte != b'.')
        .map(|byte| byte - b'0')
        .collect();
    let precision = digits.len() as i32;
    let scale = exponent - (precision - 1);
    let mut pool: Vec<Vec<u8>> = vec![digits.clone()];
    for delta in [-2, -1, 1, 2] {
        if let Some(candidate) = shift_digit_string(&digits, delta) {
            if parse_candidate(&candidate, exponent) == magnitude {
                pool.push(candidate);
            }
        }
    }
    if pool.len() == 1 {
        return (digits, exponent);
    }
    pool.sort();
    pool.dedup();
    let mut below: Option<&Vec<u8>> = None;
    let mut above: Option<&Vec<u8>> = None;
    for candidate in &pool {
        match cmp_decimal_to_double(candidate, scale, magnitude) {
            Ordering::Equal => return (candidate.clone(), exponent),
            Ordering::Less => below = Some(candidate),
            Ordering::Greater => {
                if above.is_none() {
                    above = Some(candidate);
                }
            }
        }
    }
    let (low, high) = match (below, above) {
        (Some(low), Some(high)) => (low.clone(), high.clone()),
        (Some(_), None) => return (pool[pool.len() - 1].clone(), exponent),
        (None, Some(_)) => return (pool[0].clone(), exponent),
        (None, None) => return (digits, exponent),
    };
    // The winner is decided by which side of the midpoint of the
    // straddling pair the exact value sits on. The midpoint is
    // ten times the low candidate plus five, at one finer scale.
    let mut doubled = BigUint::from_digits(&low);
    doubled.mul_small(10);
    doubled.add_small(5);
    let midpoint = doubled.decimal_digits();
    match cmp_decimal_to_double(&midpoint, scale - 1, magnitude) {
        Ordering::Equal => {
            if low[low.len() - 1] % 2 == 0 {
                (low, exponent)
            } else {
                (high, exponent)
            }
        }
        Ordering::Greater => (low, exponent),
        Ordering::Less => (high, exponent),
    }
}

pub fn py_float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let negative = value < 0.0;
    let magnitude = value.abs();
    // The shortest digits are Python's own selection among the
    // shortest round trip candidates, and the Python layout rules
    // below place them.
    let (digit_values, exponent) = python_shortest_digits(magnitude);
    let digits: String = digit_values
        .iter()
        .map(|digit| char::from(b'0' + digit))
        .collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if exponent < -4 || exponent >= 16 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        let magnitude_exponent = exponent.abs();
        if magnitude_exponent < 10 {
            out.push('0');
        }
        out.push_str(&magnitude_exponent.to_string());
    } else {
        let point = exponent + 1;
        if point <= 0 {
            out.push_str("0.");
            for _ in 0..(-point) {
                out.push('0');
            }
            out.push_str(&digits);
        } else if point as usize >= digits.len() {
            out.push_str(&digits);
            for _ in 0..(point as usize - digits.len()) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    }
    out
}

/// The Python str of a record value, for the Markdown brief. Strings
/// print as themselves, integers as their digits, floats in repr.
/// The brief renders only fields the runner built from validated
/// strings, integer counts, and the float window value through
/// this function, so the composite fallback at the end cannot
/// occur in a record the tool builds. A composite value in a
/// backlog field is recorded as a malformed entry before the
/// brief is written, on both readers.
pub fn py_str(value: &Value) -> String {
    match value {
        Value::Str(text) => text.clone(),
        Value::Int(text) => text.clone(),
        Value::Float(number) => py_float_repr(*number),
        Value::Bool(flag) => if *flag { "True" } else { "False" }.to_string(),
        Value::Null => "None".to_string(),
        _ => dumps_compact(value),
    }
}

fn escape_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buffer = [0u16; 2];
                for unit in c.encode_utf16(&mut buffer) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_float(value: f64, out: &mut String) {
    if value.is_nan() {
        out.push_str("NaN");
    } else if value.is_infinite() {
        out.push_str(if value > 0.0 { "Infinity" } else { "-Infinity" });
    } else {
        out.push_str(&py_float_repr(value));
    }
}

fn sorted_entries<'a>(entries: &'a [(String, Value)]) -> Vec<&'a (String, Value)> {
    let mut order: Vec<&(String, Value)> = entries.iter().collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    order
}

fn write_compact(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Int(text) => out.push_str(text),
        Value::Float(number) => write_float(*number, out),
        Value::Str(text) => escape_string(text, out),
        Value::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (index, (key, item)) in sorted_entries(entries).iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                escape_string(key, out);
                out.push_str(": ");
                write_compact(item, out);
            }
            out.push('}');
        }
    }
}

/// Python json.dumps with sort_keys and the default separators.
pub fn dumps_compact(value: &Value) -> String {
    let mut out = String::new();
    write_compact(value, &mut out);
    out
}

fn write_indented(value: &Value, out: &mut String, level: usize) {
    match value {
        Value::List(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&"  ".repeat(level + 1));
                write_indented(item, out, level + 1);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push(']');
        }
        Value::Object(entries) if !entries.is_empty() => {
            out.push_str("{\n");
            let ordered = sorted_entries(entries);
            for (index, (key, item)) in ordered.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&"  ".repeat(level + 1));
                escape_string(key, out);
                out.push_str(": ");
                write_indented(item, out, level + 1);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push('}');
        }
        _ => write_compact(value, out),
    }
}

/// Python json.dumps with sort_keys and indent 2, without the
/// trailing newline the callers add themselves.
pub fn dumps_indent2(value: &Value) -> String {
    let mut out = String::new();
    write_indented(value, &mut out, 0);
    out
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&dumps_compact(self))
    }
}

/// A parse failure. The ladder names the defect itself, so the
/// reader reports no message of its own. The too deep failure is
/// its own kind because the ladder reports it in its own words
/// and the command line answers it with a different exit status.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("the text does not parse as a JSON document")]
    Syntax,
    #[error("the document is nested too deeply")]
    TooDeep,
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    pos: usize,
    depth: usize,
    max_depth: usize,
}

/// The deepest container path the readers of this crate accept
/// before refusing a document as nested too deeply, measured on
/// the Python reader the port mirrors. The backlog and
/// resolutions surfaces reach a path of 9994 containers, the
/// brief surface one less, and a plain read takes the larger.
pub const MAX_DEPTH: usize = 9994;

/// Read a JSON document the way Python json.loads reads it.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    parse_with_depth(text, MAX_DEPTH)
}

/// Read a JSON document under a ceiling on container nesting.
///
/// The Python reader recurses once per container on the deepest
/// path of a document and stops with an uncaught recursion error
/// past its ceiling, so the port refuses a document whose deepest
/// container path passes max_depth with the too deep failure
/// instead of exhausting its own stack the same way.
pub fn parse_with_depth(text: &str, max_depth: usize) -> Result<Value, ParseError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        pos: 0,
        depth: 0,
        max_depth,
    };
    parser.skip_whitespace();
    let value = parser.read_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(ParseError::Syntax);
    }
    Ok(value)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn read_value(&mut self) -> Result<Value, ParseError> {
        match self.peek().ok_or(ParseError::Syntax)? {
            b'{' => self.read_container(|parser| parser.read_object()),
            b'[' => self.read_container(|parser| parser.read_list()),
            b'"' => Ok(Value::Str(self.read_string()?)),
            b't' => self.read_keyword("true", Value::Bool(true)),
            b'f' => self.read_keyword("false", Value::Bool(false)),
            b'n' => self.read_keyword("null", Value::Null),
            b'N' => self.read_keyword("NaN", Value::Float(f64::NAN)),
            b'I' => self.read_keyword("Infinity", Value::Float(f64::INFINITY)),
            b'-' => {
                if self.text[self.pos..].starts_with("-Infinity") {
                    self.pos += "-Infinity".len();
                    Ok(Value::Float(f64::NEG_INFINITY))
                } else {
                    self.read_number()
                }
            }
            b'0'..=b'9' => self.read_number(),
            _ => Err(ParseError::Syntax),
        }
    }

    /// Read one container under the nesting ceiling. The root
    /// container of a document stands at depth one, so a document
    /// is refused when a container would stand deeper than the
    /// ceiling the surface reading it allows.
    fn read_container(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<Value, ParseError>,
    ) -> Result<Value, ParseError> {
        if self.depth >= self.max_depth {
            return Err(ParseError::TooDeep);
        }
        self.depth += 1;
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn read_keyword(&mut self, word: &str, value: Value) -> Result<Value, ParseError> {
        if self.text[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(ParseError::Syntax)
        }
    }

    fn read_number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek().ok_or(ParseError::Syntax)? {
            b'0' => {
                self.pos += 1;
            }
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(ParseError::Syntax),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseError::Syntax);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseError::Syntax);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let literal = &self.text[start..self.pos];
        if is_float {
            literal
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| ParseError::Syntax)
        } else if literal == "-0" {
            Ok(Value::Int("0".to_string()))
        } else {
            Ok(Value::Int(literal.to_string()))
        }
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        // The caller has seen the opening quote.
        self.pos += 1;
        let mut out = String::new();
        loop {
            let byte = self.peek().ok_or(ParseError::Syntax)?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let escaped = self.peek().ok_or(ParseError::Syntax)?;
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.read_hex_unit()?;
                            if (0xD800..0xDC00).contains(&unit) {
                                // A high surrogate. Python keeps a
                                // following low surrogate as a pair,
                                // and a Rust string cannot hold a lone
                                // surrogate, so an unpaired one is read
                                // as the replacement character.
                                if self.text[self.pos..].starts_with("\\u") {
                                    let saved = self.pos;
                                    self.pos += 2;
                                    let low = self.read_hex_unit()?;
                                    if (0xDC00..0xE000).contains(&low) {
                                        let combined = 0x10000
                                            + (((unit as u32) - 0xD800) << 10)
                                            + ((low as u32) - 0xDC00);
                                        out.push(
                                            char::from_u32(combined).ok_or(ParseError::Syntax)?,
                                        );
                                    } else {
                                        self.pos = saved;
                                        out.push('\u{FFFD}');
                                    }
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else if (0xDC00..0xE000).contains(&unit) {
                                out.push('\u{FFFD}');
                            } else {
                                out.push(char::from_u32(unit as u32).ok_or(ParseError::Syntax)?);
                            }
                        }
                        _ => return Err(ParseError::Syntax),
                    }
                }
                0x00..=0x1F => return Err(ParseError::Syntax),
                _ => {
                    // One full UTF-8 character. The text is a Rust
                    // string, so its bytes are valid UTF-8 throughout.
                    let rest = &self.text[self.pos..];
                    let ch = rest.chars().next().ok_or(ParseError::Syntax)?;
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn read_hex_unit(&mut self) -> Result<u16, ParseError> {
        let slice = self
            .text
            .get(self.pos..self.pos + 4)
            .ok_or(ParseError::Syntax)?;
        if !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseError::Syntax);
        }
        self.pos += 4;
        u16::from_str_radix(slice, 16).map_err(|_| ParseError::Syntax)
    }

    fn read_object(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut entries: Vec<(String, Value)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(ParseError::Syntax);
            }
            let key = self.read_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(ParseError::Syntax);
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.read_value()?;
            set_key(&mut entries, key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(entries));
                }
                _ => return Err(ParseError::Syntax),
            }
        }
    }

    fn read_list(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::List(items));
        }
        loop {
            self.skip_whitespace();
            let value = self.read_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::List(items));
                }
                _ => return Err(ParseError::Syntax),
            }
        }
    }
}

/// The deepest container path in a document, counted with the
/// root container at depth one and a scalar at depth zero.
///
/// The walk is iterative, on an explicit stack, so measuring a
/// document the reader accepted at its ceiling cannot itself
/// exhaust the call stack.
pub fn deepest_path(value: &Value) -> usize {
    let mut deepest = 0;
    let mut work: Vec<(&Value, usize)> = vec![(value, 0)];
    while let Some((item, depth)) = work.pop() {
        match item {
            Value::List(items) => {
                deepest = deepest.max(depth + 1);
                work.extend(items.iter().map(|child| (child, depth + 1)));
            }
            Value::Object(entries) => {
                deepest = deepest.max(depth + 1);
                work.extend(entries.iter().map(|(_, child)| (child, depth + 1)));
            }
            _ => {}
        }
    }
    deepest
}

/// Runs of Unicode decimal digit characters, as the first and
/// last codepoint of each run and the digit value of its first
/// character. Python float() and the \d of a Python text pattern
/// accept every decimal digit, not only the ASCII ten, so the
/// crate carries the set (Unicode 15.0, 680 digits in 68 runs of
/// ten) rather than an ASCII approximation of it.
const DECIMAL_DIGIT_RUNS: &[(u32, u32, u32)] = &[
    (0x30, 0x39, 0),
    (0x660, 0x669, 0),
    (0x6F0, 0x6F9, 0),
    (0x7C0, 0x7C9, 0),
    (0x966, 0x96F, 0),
    (0x9E6, 0x9EF, 0),
    (0xA66, 0xA6F, 0),
    (0xAE6, 0xAEF, 0),
    (0xB66, 0xB6F, 0),
    (0xBE6, 0xBEF, 0),
    (0xC66, 0xC6F, 0),
    (0xCE6, 0xCEF, 0),
    (0xD66, 0xD6F, 0),
    (0xDE6, 0xDEF, 0),
    (0xE50, 0xE59, 0),
    (0xED0, 0xED9, 0),
    (0xF20, 0xF29, 0),
    (0x1040, 0x1049, 0),
    (0x1090, 0x1099, 0),
    (0x17E0, 0x17E9, 0),
    (0x1810, 0x1819, 0),
    (0x1946, 0x194F, 0),
    (0x19D0, 0x19D9, 0),
    (0x1A80, 0x1A89, 0),
    (0x1A90, 0x1A99, 0),
    (0x1B50, 0x1B59, 0),
    (0x1BB0, 0x1BB9, 0),
    (0x1C40, 0x1C49, 0),
    (0x1C50, 0x1C59, 0),
    (0xA620, 0xA629, 0),
    (0xA8D0, 0xA8D9, 0),
    (0xA900, 0xA909, 0),
    (0xA9D0, 0xA9D9, 0),
    (0xA9F0, 0xA9F9, 0),
    (0xAA50, 0xAA59, 0),
    (0xABF0, 0xABF9, 0),
    (0xFF10, 0xFF19, 0),
    (0x104A0, 0x104A9, 0),
    (0x10D30, 0x10D39, 0),
    (0x11066, 0x1106F, 0),
    (0x110F0, 0x110F9, 0),
    (0x11136, 0x1113F, 0),
    (0x111D0, 0x111D9, 0),
    (0x112F0, 0x112F9, 0),
    (0x11450, 0x11459, 0),
    (0x114D0, 0x114D9, 0),
    (0x11650, 0x11659, 0),
    (0x116C0, 0x116C9, 0),
    (0x11730, 0x11739, 0),
    (0x118E0, 0x118E9, 0),
    (0x11950, 0x11959, 0),
    (0x11C50, 0x11C59, 0),
    (0x11D50, 0x11D59, 0),
    (0x11DA0, 0x11DA9, 0),
    (0x11F50, 0x11F59, 0),
    (0x16A60, 0x16A69, 0),
    (0x16AC0, 0x16AC9, 0),
    (0x16B50, 0x16B59, 0),
    (0x1D7CE, 0x1D7D7, 0),
    (0x1D7D8, 0x1D7E1, 0),
    (0x1D7E2, 0x1D7EB, 0),
    (0x1D7EC, 0x1D7F5, 0),
    (0x1D7F6, 0x1D7FF, 0),
    (0x1E140, 0x1E149, 0),
    (0x1E2F0, 0x1E2F9, 0),
    (0x1E4F0, 0x1E4F9, 0),
    (0x1E950, 0x1E959, 0),
    (0x1FBF0, 0x1FBF9, 0),
];

/// The digit value of a Unicode decimal digit, or None for any
/// other character. This is the digit set of Python float() and
/// of \d in a Python text pattern.
pub fn decimal_digit_value(ch: char) -> Option<u32> {
    let point = ch as u32;
    for &(first, last, base) in DECIMAL_DIGIT_RUNS {
        if point < first {
            return None;
        }
        if point <= last {
            return Some(base + (point - first));
        }
    }
    None
}

/// Whether Python float() trims this character from the ends of
/// its input. The set is the one probed against the runtime. The
/// file separator characters U+001C to U+001F are whitespace to
/// str.isspace but are not trimmed by float(), so they are absent.
fn py_float_whitespace(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t'
            | '\n'
            | '\r'
            | '\u{0b}'
            | '\u{0c}'
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// Parse the text Python float() accepts into a float.
///
/// Python float() trims surrounding whitespace from the set
/// py_float_whitespace names, accepts the words inf, infinity,
/// and nan in any ASCII case with an optional sign, accepts any
/// Unicode decimal digit in the mantissa and the exponent, and
/// accepts single underscores between digits. Rust float parsing
/// accepts the words but neither the trimming, the underscores,
/// nor the digit set, so all three are normalized here before
/// the value is read.
pub fn py_float_parse(raw: &str) -> Option<f64> {
    let trimmed = raw.trim_matches(py_float_whitespace);
    if trimmed.is_empty() {
        return None;
    }
    // Underscores are legal only between two digits, and every
    // digit is rewritten to its ASCII form. Removing a legal
    // underscore never changes the value, and any other placement
    // is refused here rather than passed through.
    let chars: Vec<char> = trimmed.chars().collect();
    let mut cleaned = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if *ch == '_' {
            let before = index > 0 && decimal_digit_value(chars[index - 1]).is_some();
            let after =
                index + 1 < chars.len() && decimal_digit_value(chars[index + 1]).is_some();
            if !before || !after {
                return None;
            }
        } else if let Some(value) = decimal_digit_value(*ch) {
            cleaned.push(char::from(b'0' + value as u8));
        } else {
            cleaned.push(*ch);
        }
    }
    cleaned.parse::<f64>().ok()
}
