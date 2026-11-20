use std::error::Error;
use std::fmt;

use crate::sim::ir::IrValidationError;
use crate::sim::located::{located, take_location};

/// Failure while validating or translating simulator IR to C11.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmitError {
    InvalidIr(IrValidationError),
    /// A construct the emitter cannot render. `location` is the source
    /// position (`file:line:col`) of the nearest IR construct that has one:
    /// the located statement, initializer, procedure or subroutine whose
    /// rendering failed.
    Unsupported {
        detail: String,
        location: Option<String>,
    },
}

impl EmitError {
    /// Rendering returns plain strings; a location framed in `detail`
    /// (`crate::sim::located`) moves to [`location`](Self::location).
    pub(super) fn new(detail: impl Into<String>) -> Self {
        let (detail, location) = take_location(detail.into());
        Self::Unsupported { detail, location }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::InvalidIr(error) => error.detail(),
            Self::Unsupported { detail, .. } => detail,
        }
    }

    /// Where the construct whose rendering failed is, when the IR has a
    /// source position for it.
    pub fn location(&self) -> Option<&str> {
        match self {
            Self::InvalidIr(_) => None,
            Self::Unsupported { location, .. } => location.as_deref(),
        }
    }
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIr(error) => error.fmt(f),
            Self::Unsupported { detail, .. } => f.write_str(detail),
        }
    }
}

impl Error for EmitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidIr(error) => Some(error),
            Self::Unsupported { .. } => None,
        }
    }
}

/// The lowering driver passes failures on as strings: the location stays
/// framed, so `CodegenError` reports it in its location field.
impl From<EmitError> for String {
    fn from(error: EmitError) -> Self {
        match error.location() {
            Some(location) => located(location, error.detail()),
            None => error.to_string(),
        }
    }
}
