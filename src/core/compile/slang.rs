//! Opt-in Slang compilation into owned frontend observations.
//!
//! The initial capture includes diagnostics and hierarchy facts, not a complete
//! simulator database. A successful checked compilation establishes frontend
//! validity only; it does not authorize simulation of an incomplete capture.

pub use crate::ffi::slang::{
    CompileOptions, CompileRequest, Constant, ConstantValue, Define, Diagnostic,
    DiagnosticProvider, DiagnosticSeverity, File, Instance, InstanceKind, Limits, Parameter,
    ParameterKind, ParameterOverride, RelatedDiagnostic, SlangError, SlangErrorKind, Snapshot,
    Source, SourceRange, Type, TypeKind,
};

/// Failure to compile a valid Slang design into owned observations.
#[derive(Debug)]
pub enum CompileError {
    /// Invalid input, resource limit, or native bridge failure.
    Startup(SlangError),
    /// Compilation completed with blocking diagnostics.
    Diagnostics(Vec<Diagnostic>),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Startup(error) => error.fmt(formatter),
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "Slang compilation failed ({} diagnostic records)",
                diagnostics.len()
            ),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Startup(error) => Some(error),
            Self::Diagnostics(_) => None,
        }
    }
}

/// Compile admitted in-memory sources and retain diagnostics after HDL errors.
///
/// All native owners are destroyed before this function returns. Input policy
/// and resource limits are enforced by the safe FFI facade. This performs no
/// fallback to Surelog and does not produce an executable simulator model.
pub fn compile(request: &CompileRequest<'_>) -> Result<Snapshot, SlangError> {
    crate::ffi::slang::compile(request)
}

/// Compile sources, returning observations only when Slang reports success.
///
/// Warnings do not by themselves reject a result. Frontend success is distinct
/// from the capability checks required before simulation lowering.
pub fn compile_checked(request: &CompileRequest<'_>) -> Result<Snapshot, CompileError> {
    let snapshot = compile(request).map_err(CompileError::Startup)?;
    if snapshot.has_errors() {
        Err(CompileError::Diagnostics(snapshot.diagnostics))
    } else {
        Ok(snapshot)
    }
}
