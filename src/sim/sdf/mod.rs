//! Standard Delay Format input: a versioned streaming parser, the typed
//! records it produces, and the resolution of those records into an owned
//! annotation plan against a generated model's timing targets.
//!
//! The grammar is the published OVI Standard Delay Format Specification,
//! version 3.0 (May 1995), and, for files whose SDFVERSION names 2.1, the
//! OVI version 2.1 grammar (February 1994). IEEE 1800-2009 clause 32 and
//! IEEE 1364-2001 clause 16 define how the records map to specify paths,
//! timing checks and interconnect. Other SDF versions are rejected.
//!
//! * [`SdfReader`] streams a file cell by cell with bounded memory
//!   ([`SdfLimits`]); [`parse`] collects a whole file.
//! * [`SdfTargets`] describes what a model can annotate: its instance tree,
//!   ports, specify paths and timing checks with their model indices.
//! * [`resolve`] turns a file into an [`SdfPlan`]: every record matched to
//!   its targets in file order, unmatched records as warnings, or no plan
//!   at all when the file is malformed. Applying a plan is a separate step;
//!   resolution never changes the model and never opens a file: SDF names
//!   are matched against the in-memory targets only.

mod expr;
mod lexer;
mod number;
mod parser;
mod plan;
mod resolve;
mod syntax;
mod targets;

pub use expr::{parse_hdl_condition, BinaryOp, CondExpr, SdfBits, UnaryOp};
pub use number::{SdfNumber, SdfValue};
pub use parser::{parse, SdfFile, SdfReader};
pub use plan::{
    SdfDelayOp, SdfLimitSlot, SdfMode, SdfOp, SdfOpKind, SdfPlan, SdfPortTarget, SdfPulseOp,
    SdfWarning,
};
pub use resolve::resolve;
pub use syntax::{
    SdfCell, SdfCellInstance, SdfCheckKind, SdfCondition, SdfDelDef, SdfDelType, SdfDelval,
    SdfDelvalList, SdfEdge, SdfHeader, SdfIdent, SdfPathCondition, SdfPortRef, SdfPortSpec,
    SdfPortTchk, SdfPulse, SdfPulseLimits, SdfTimeUnit, SdfTimescale, SdfTimingCheck,
    SdfTimingSpec, SdfVersion,
};
pub use targets::{
    SdfCheckEvent, SdfCheckRow, SdfPathRow, SdfPortDirection, SdfScope, SdfScopeKind,
    SdfTargetCheck, SdfTargetCondition, SdfTargetEdge, SdfTargetPath, SdfTargetPort, SdfTargets,
    SdfTerminal, SdfTimingCheckKind,
};

use std::fmt;

/// "Identifiers can be up to 1024 characters long" (OVI SDF 3.0 4-3).
pub const MAX_IDENTIFIER_CHARS: usize = 1024;

/// A 1-based line and column in the SDF file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SdfPos {
    pub line: u32,
    pub col: u32,
}

impl SdfPos {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }
}

impl fmt::Display for SdfPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// A malformed, truncated or over-limit file. Reading stops at the first
/// error; no plan is produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfError {
    pub pos: SdfPos,
    pub message: String,
}

impl SdfError {
    pub fn new(pos: SdfPos, message: impl Into<String>) -> Self {
        Self {
            pos,
            message: message.into(),
        }
    }
}

impl fmt::Display for SdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.pos, self.message)
    }
}

impl std::error::Error for SdfError {}

/// Resource limits of reading and resolving one file. Every limit is a
/// hard error naming the limit, so a hostile or corrupt file cannot make
/// the reader use unbounded memory or time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdfLimits {
    /// Bytes read from the file.
    pub max_input_bytes: u64,
    /// Bytes of one word or identifier token, escapes included. A path
    /// holds many identifiers of up to [`MAX_IDENTIFIER_CHARS`] each.
    pub max_token_bytes: usize,
    /// Bytes of one QSTRING.
    pub max_string_bytes: usize,
    /// Components of one hierarchical path.
    pub max_path_components: usize,
    /// Nesting of parentheses, unary operators and concatenations in one
    /// condition.
    pub max_condition_depth: usize,
    /// Operands and operators of one condition.
    pub max_condition_nodes: usize,
    /// Operations of one plan, which bounds the plan's memory.
    pub max_plan_ops: usize,
}

impl Default for SdfLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1 << 36,
            max_token_bytes: 1 << 16,
            max_string_bytes: 1 << 12,
            max_path_components: 1 << 10,
            max_condition_depth: 64,
            max_condition_nodes: 1 << 12,
            max_plan_ops: 1 << 26,
        }
    }
}
