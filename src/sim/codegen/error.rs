use std::error::Error;
use std::fmt;

/// Failure while capturing or lowering an elaborated design into simulator C.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    detail: String,
}

impl CodegenError {
    pub(super) fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
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
