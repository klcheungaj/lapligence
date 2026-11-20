//! Source locations carried inside plain-string pipeline failures.
//!
//! Lowering (`codegen`) and C emission (`emit_c`) pass failures up as
//! `String`s through many layers. The location of the failing construct rides
//! framed inside that string; the public error types (`CodegenError`,
//! `CodegenWarning`, `EmitError`) take it out again, so no frame reaches a
//! message text. A wrapper that adds a location keeps one that is already
//! there: the innermost construct names the failure most precisely.

use std::fmt;

use crate::sim::semantic::Origin;

/// Frames the source location a failure string carries ([`located`]);
/// never part of a message text.
const LOCATION_MARK: char = '\u{1}';

/// A failure located at `location` (`file:line:col`). A message that already
/// carries a location keeps it.
pub(crate) fn located(location: &str, message: impl fmt::Display) -> String {
    let message = message.to_string();
    if is_located(&message) {
        message
    } else {
        format!("{LOCATION_MARK}{location}{LOCATION_MARK}{message}")
    }
}

/// Whether `message` already carries a framed location ([`located`]).
pub(crate) fn is_located(message: &str) -> bool {
    message.contains(LOCATION_MARK)
}

/// `error` located at `origin` ([`Origin::location`]) unless it already
/// carries a location or the origin is synthetic (no source position), when
/// an enclosing construct locates it.
pub(crate) fn locate_at_origin(origin: &Origin, error: String) -> String {
    match origin {
        Origin::Source { .. } if !is_located(&error) => located(&origin.location(), error),
        _ => error,
    }
}

/// `error` located at `location`, a `file:line:col` string the IR carries
/// (from `Origin::location` or lowering's `source_location`), unless it
/// already carries a location or `location` names no source position
/// (`<synthetic: …>`, `<unknown>:0:0`).
pub(crate) fn locate_at(location: &str, error: String) -> String {
    if location.starts_with('<') || is_located(&error) {
        error
    } else {
        located(location, error)
    }
}

/// Split the framed locations out of `detail`: the first is the location
/// (the innermost construct, since every wrapper keeps an existing one); a
/// later one, left by text that joins two located failures, is dropped so
/// no frame reaches the message text.
pub(crate) fn take_location(detail: String) -> (String, Option<String>) {
    if !detail.contains(LOCATION_MARK) {
        return (detail, None);
    }
    let mut text = String::with_capacity(detail.len());
    let mut location = None;
    let mut rest = detail.as_str();
    while let Some(start) = rest.find(LOCATION_MARK) {
        let after = start + LOCATION_MARK.len_utf8();
        let Some(length) = rest[after..].find(LOCATION_MARK) else {
            break;
        };
        text.push_str(&rest[..start]);
        location.get_or_insert_with(|| rest[after..after + length].to_owned());
        rest = &rest[after + length + LOCATION_MARK.len_utf8()..];
    }
    text.push_str(rest);
    (text, location)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_locate_only_unlocated_failures_at_a_source_position() {
        let source = Origin::Source {
            path: "a.sv".into(),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 9,
            logical: None,
        };
        let synthetic = Origin::Synthetic {
            reason: "test".to_owned(),
        };
        assert_eq!(
            take_location(locate_at_origin(&source, "bad".to_owned())),
            ("bad".to_owned(), Some("a.sv:3:5".to_owned()))
        );
        assert_eq!(
            take_location(locate_at_origin(&synthetic, "bad".to_owned())),
            ("bad".to_owned(), None)
        );
        assert_eq!(
            take_location(locate_at("<unknown>:0:0", "bad".to_owned())),
            ("bad".to_owned(), None)
        );
        assert_eq!(
            take_location(locate_at("a.sv:3:5", "bad".to_owned())),
            ("bad".to_owned(), Some("a.sv:3:5".to_owned()))
        );
        let inner = located("b.sv:1:2", "bad");
        assert_eq!(
            take_location(locate_at_origin(&source, inner)),
            ("bad".to_owned(), Some("b.sv:1:2".to_owned()))
        );
    }
}
