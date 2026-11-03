//! Pins for the Python float renderer and parser in pyjson.
//!
//! The renderer pins name the exact values the Round 5 review
//! found exercised only by uncommitted differential runs. The
//! parser pins name the whitespace set and the decimal digit set
//! Python float() accepts, both probed against the runtime.

use thirdshift::pyjson::{
    ParseError, decimal_digit_value, deepest_path, parse, parse_with_depth, py_float_parse,
    py_float_repr,
};

#[test]
fn parse_with_depth_refuses_past_its_ceiling_with_the_too_deep_kind() {
    assert!(parse_with_depth("[[[1]]]", 3).is_ok());
    assert!(matches!(
        parse_with_depth("[[[1]]]", 2),
        Err(ParseError::TooDeep)
    ));
    assert!(matches!(
        parse_with_depth("{\"a\": {\"a\": 1}}", 1),
        Err(ParseError::TooDeep)
    ));
    // A syntax failure under a generous ceiling stays syntax, so
    // the two kinds cannot be confused by the ladder.
    assert!(matches!(
        parse_with_depth("[1, 2", 9),
        Err(ParseError::Syntax)
    ));
    // Scalars are not containers and never trip the ceiling.
    assert!(parse_with_depth("5", 0).is_ok());
    assert!(parse("[[[[5]]]]").is_ok());
}

#[test]
fn deepest_path_counts_containers_with_the_root_at_one() {
    assert_eq!(deepest_path(&parse("5").unwrap()), 0);
    assert_eq!(deepest_path(&parse("[]").unwrap()), 1);
    assert_eq!(deepest_path(&parse("{\"a\": [[1]]}").unwrap()), 3);
    assert_eq!(
        deepest_path(&parse("{\"a\": [1, {\"b\": [2]}]}").unwrap()),
        4
    );
}

#[test]
fn float_repr_tie_picks_the_even_last_digit() {
    // The exact value ends in .25 at a spacing of .25, so the
    // sixteen digit candidates ending in 2 and in 3 sit exactly
    // the same distance away. Python keeps the even last digit.
    assert_eq!(py_float_repr(2163015292276093.25), "2163015292276093.2");
}

#[test]
fn float_repr_layout_boundaries() {
    assert_eq!(py_float_repr(1e16), "1e+16");
    assert_eq!(py_float_repr(9999999999999998.0), "9999999999999998.0");
    assert_eq!(py_float_repr(0.0001), "0.0001");
    assert_eq!(py_float_repr(1e-05), "1e-05");
}

#[test]
fn float_repr_signed_zero() {
    assert_eq!(py_float_repr(-0.0), "-0.0");
    assert_eq!(py_float_repr(0.0), "0.0");
}

#[test]
fn float_parse_halfway_rounds_to_even() {
    // Each string is the exact midpoint between two neighbouring
    // doubles, so the winner is decided by the even rule alone.
    assert_eq!(
        py_float_parse("1.00000000000000011102230246251565404236316680908203125"),
        Some(1.0)
    );
    assert_eq!(
        py_float_parse("0.500000000000000055511151231257827021181583404541015625"),
        Some(0.5)
    );
    assert_eq!(
        py_float_parse("1.00000000000000033306690738754696212708950042724609375"),
        Some(1.0000000000000004)
    );
}

#[test]
fn float_parse_trims_the_python_whitespace_set() {
    assert_eq!(py_float_parse("\u{a0}2.5"), Some(2.5));
    assert_eq!(py_float_parse("2.5\u{3000}"), Some(2.5));
    assert_eq!(py_float_parse("\u{85}2.5"), Some(2.5));
    assert_eq!(py_float_parse("\u{2003}2.5\u{202f}"), Some(2.5));
    assert_eq!(py_float_parse("\u{1680}2.5\u{2028}"), Some(2.5));
    // The file separators are whitespace to str.isspace but are
    // not trimmed by float(), and a zero width space is neither.
    assert_eq!(py_float_parse("\u{1c}2.5"), None);
    assert_eq!(py_float_parse("\u{1f}2.5"), None);
    assert_eq!(py_float_parse("2.5\u{200b}"), None);
}

#[test]
fn float_parse_accepts_unicode_decimal_digits() {
    assert_eq!(py_float_parse("٢.٥"), Some(2.5));
    assert_eq!(py_float_parse("２.５"), Some(2.5));
    assert_eq!(py_float_parse("١٢٣"), Some(123.0));
    assert_eq!(py_float_parse("1e٢"), Some(100.0));
    assert_eq!(py_float_parse("१२.५"), Some(12.5));
}

#[test]
fn float_parse_underscores_sit_between_any_digits() {
    assert_eq!(py_float_parse("1_0"), Some(10.0));
    assert_eq!(py_float_parse("1_٢"), Some(12.0));
    assert_eq!(py_float_parse("1__0"), None);
    assert_eq!(py_float_parse("_10"), None);
    assert_eq!(py_float_parse("10_"), None);
}

#[test]
fn float_parse_words_are_ascii_case_insensitive_only() {
    assert_eq!(py_float_parse("iNfInItY"), Some(f64::INFINITY));
    assert_eq!(py_float_parse("-INF"), Some(f64::NEG_INFINITY));
    assert!(py_float_parse("+nan").is_some_and(|value| value.is_nan()));
    assert_eq!(py_float_parse("ınf"), None);
    assert_eq!(py_float_parse("İNFINITY"), None);
}

#[test]
fn decimal_digit_values_cover_the_whole_set() {
    assert_eq!(decimal_digit_value('5'), Some(5));
    assert_eq!(decimal_digit_value('٥'), Some(5));
    assert_eq!(decimal_digit_value('𝟙'), Some(1));
    assert_eq!(decimal_digit_value('a'), None);
    assert_eq!(decimal_digit_value('٫'), None);
    // Every run in the table holds ten digits counting from its
    // base, and the run after ASCII starts at the Arabic-Indic
    // block rather than anywhere inside the ASCII run.
    assert_eq!(decimal_digit_value('٠'), Some(0));
    assert_eq!(decimal_digit_value('٩'), Some(9));
    assert_eq!(decimal_digit_value(':'), None);
}
