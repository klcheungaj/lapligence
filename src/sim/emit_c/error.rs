use std::error::Error;
use std::fmt;

use crate::sim::ir::IrValidationError;

/// Failure while validating or translating simulator IR to C11.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmitError {
    InvalidIr(IrValidationError),
    Unsupported(String),
}

impl EmitError {
    pub(super) fn new(detail: impl Into<String>) -> Self {
        Self::Unsupported(detail.into())
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::InvalidIr(error) => error.detail(),
            Self::Unsupported(detail) => detail,
        }
    }
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIr(error) => error.fmt(f),
            Self::Unsupported(detail) => f.write_str(detail),
        }
    }
}

impl Error for EmitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidIr(error) => Some(error),
            Self::Unsupported(_) => None,
        }
    }
}

impl From<EmitError> for String {
    fn from(error: EmitError) -> Self {
        error.to_string()
    }
}
