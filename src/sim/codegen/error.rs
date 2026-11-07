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
/// caller that wraps the string with more context keeps it.
pub(crate) fn located(location: &str, message: impl fmt::Display) -> String {
    format!("{LOCATION_MARK}{location}{LOCATION_MARK}{message}")
}

/// Split the first framed location out of `detail`.
fn take_location(detail: String) -> (String, Option<String>) {
    let Some(start) = detail.find(LOCATION_MARK) else {
        return (detail, None);
    };
    let after = start + LOCATION_MARK.len_utf8();
    let Some(length) = detail[after..].find(LOCATION_MARK) else {
        return (detail, None);
    };
    let location = detail[after..after + length].to_owned();
    let mut text = detail[..start].to_owned();
    text.push_str(&detail[after + length + LOCATION_MARK.len_utf8()..]);
    (text, Some(location))
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
