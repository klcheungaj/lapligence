use std::error::Error;
use std::fmt;

/// Failure while capturing or lowering an elaborated design into simulator C.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    detail: String,
    location: Option<String>,
    legacy_unsupported: bool,
}

/// Frames the source location a lowering failure string carries
/// ([`located`]); never part of a message text.
const LOCATION_MARK: char = '\u{1}';

/// A lowering failure located at `location` (`file:line:col`, from
/// `source_location`). Lowering returns plain strings, so the location rides
/// framed inside the string and [`CodegenError`] takes it out again; a
/// caller that wraps the string with more context keeps it. A message that
/// already carries a location keeps it: the innermost construct names the
/// failure most precisely.
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

/// Split the framed locations out of `detail`: the first is the location
/// (the innermost construct, since every wrapper keeps an existing one); a
/// later one, left by text that joins two located failures, is dropped so
/// no frame reaches the message text.
fn take_location(detail: String) -> (String, Option<String>) {
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

impl CodegenError {
    pub(super) fn new(detail: impl Into<String>) -> Self {
        let (detail, location) = take_location(detail.into());
        Self {
            legacy_unsupported: crate::sim::legacy_unsupported::is_diagnostic_list(&detail),
            detail,
            location,
        }
    }

    /// Where the failing construct is, when lowering had its source node.
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    /// Whether [`detail`](Self::detail) is a list of stable legacy-construct
    /// rejections (`sim::legacy_unsupported::diagnostic`), one per line,
    /// rather than a lowering failure.
    pub fn is_legacy_unsupported(&self) -> bool {
        self.legacy_unsupported
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl Error for CodegenError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_innermost_location_wins_and_no_frame_reaches_the_text() {
        let inner = located("a.sv:3:5", "bad operand");
        let outer = located("a.sv:2:1", format!("in statement: {inner}"));
        let error = CodegenError::new(outer);
        assert_eq!(error.location(), Some("a.sv:3:5"));
        assert_eq!(error.detail(), "in statement: bad operand");

        let joined = format!("{}; {}", located("a.sv:1:1", "x"), located("b.sv:2:2", "y"));
        let error = CodegenError::new(joined);
        assert_eq!(error.location(), Some("a.sv:1:1"));
        assert_eq!(error.detail(), "x; y");

        let error = CodegenError::new("plain");
        assert_eq!(error.location(), None);
        assert_eq!(error.detail(), "plain");
    }
}
