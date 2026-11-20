use std::error::Error;
use std::fmt;

/// Failure while capturing or lowering an elaborated design into simulator C.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    detail: String,
    location: Option<String>,
    legacy_unsupported: bool,
}

// Lowering returns plain strings, so a failure's location rides framed
// inside the string (`located`) and `CodegenError` takes it out again; a
// caller that wraps the string with more context keeps it.
use crate::sim::located::take_location;
pub(crate) use crate::sim::located::{is_located, located};

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

/// A non-fatal lowering diagnostic (CODEGEN-010): plain text, plus the
/// source location of the construct it is about when lowering had one.
/// Lowering collects warnings as strings framed like errors ([`located`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenWarning {
    text: String,
    location: Option<String>,
}

impl CodegenWarning {
    pub(super) fn new(detail: impl Into<String>) -> Self {
        let (text, location) = take_location(detail.into());
        Self { text, location }
    }

    /// Where the construct the warning is about is (`file:line:col`).
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for CodegenWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

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

        let warning = CodegenWarning::new(located("a.sv:4:2", "ignored"));
        assert_eq!(warning.location(), Some("a.sv:4:2"));
        assert_eq!(warning.text(), "ignored");
    }
}
