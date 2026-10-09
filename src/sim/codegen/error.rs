use std::error::Error;
use std::fmt;

/// Failure while capturing or lowering an elaborated design into simulator C.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    detail: String,
    legacy_unsupported: bool,
}

impl CodegenError {
    pub(super) fn new(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        Self {
            legacy_unsupported: crate::sim::legacy_unsupported::is_diagnostic_list(&detail),
            detail,
        }
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
