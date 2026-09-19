//! Safe, owned Rust facade over the Slang C ABI.
//!
//! Native snapshots are borrowed only while being decoded. No Slang pointer or
//! native allocation escapes [`compile`].

#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]
// Each archive must propagate through the Rust library boundary.
#![allow(clippy::duplicated_attributes)]

use std::collections::HashSet;
use std::{fmt, ptr, slice, str};

mod snapshot;
use snapshot::decode_snapshot;
mod semantics;
use semantics::{decode_semantic_edges, decode_semantic_nodes};
#[cfg(test)]
use semantics::{decode_semantic_operation, validate_semantic_subkind};
mod tokens;
use tokens::decode_lexical_tokens;
mod diagnostics;
use diagnostics::{decode_diagnostics, decode_related};
mod values;
use values::{
    decode_constants, decode_instances, decode_parameters, decode_types, validate_parameter_windows,
};

const ABI_VERSION: u32 = 4;
const INVALID_ID: u64 = u64::MAX;

const STATUS_OK: u32 = 0;
const STATUS_INVALID_ARGUMENT: u32 = 1;
const STATUS_LIMIT_EXCEEDED: u32 = 2;
const STATUS_FRONTEND_ERROR: u32 = 3;
const STATUS_INTERNAL_ERROR: u32 = 4;

const COMPILE_LIBRARY_UNITS: u32 = 1 << 0;
const COMPILE_EDITION_VERILOG_2001: u32 = 1 << 1;
const COMPILE_EDITION_SYSTEMVERILOG_2009: u32 = 1 << 2;
const COMPILE_MERGED_COMPILATION_UNITS: u32 = 1 << 3;
const SNAPSHOT_HAS_ERRORS: u32 = 1 << 0;
const SNAPSHOT_ANALYSIS_RAN: u32 = 1 << 1;
const SNAPSHOT_EDITION_VERILOG_2001: u32 = 1 << 8;
const SNAPSHOT_EDITION_SYSTEMVERILOG_2009: u32 = 1 << 9;
const SNAPSHOT_MERGED_COMPILATION_UNITS: u32 = 1 << 10;
const SNAPSHOT_EDITION_MASK: u32 =
    SNAPSHOT_EDITION_VERILOG_2001 | SNAPSHOT_EDITION_SYSTEMVERILOG_2009;
const SNAPSHOT_KNOWN_FLAGS: u32 = SNAPSHOT_HAS_ERRORS
    | SNAPSHOT_ANALYSIS_RAN
    | SNAPSHOT_EDITION_MASK
    | SNAPSHOT_MERGED_COMPILATION_UNITS;
const MAX_SOURCES: usize = 4_096;
const MAX_DEFINES: usize = 4_096;
const MAX_TOP_MODULES: usize = 4_096;
const MAX_INCLUDE_DIRS: usize = 4_096;
const MAX_PARAMETER_OVERRIDES: usize = 4_096;
const MAX_SYSTEM_SUBROUTINES: usize = 4_096;
const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

/// One admitted in-memory SystemVerilog compilation unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source<'a> {
    pub name: &'a str,
    pub text: &'a str,
    /// Parse this source as a compilation unit. `false` preloads an admitted
    /// include buffer that can only be opened through Slang's in-memory cache.
    pub is_compilation_unit: bool,
}

impl<'a> Source<'a> {
    pub fn compilation_unit(name: &'a str, text: &'a str) -> Self {
        Self {
            name,
            text,
            is_compilation_unit: true,
        }
    }

    pub fn include(name: &'a str, text: &'a str) -> Self {
        Self {
            name,
            text,
            is_compilation_unit: false,
        }
    }
}

/// One preprocessor definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Define {
    pub name: String,
    pub value: Option<String>,
}

/// One top-level parameter override, passed to Slang as `NAME=VALUE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterOverride {
    pub name: String,
    pub value: String,
}

/// Explicit language policy applied to the complete compilation.
///
/// Preprocessor `` `begin_keywords `` regions can change lexical keyword
/// lookup, but they never change this compilation-wide semantic target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LanguageEdition {
    /// IEEE 1364-2001 Verilog semantics.
    Verilog2001,
    /// IEEE 1800-2009 SystemVerilog semantics.
    #[default]
    SystemVerilog2009,
}

impl LanguageEdition {
    fn compile_flag(self) -> u32 {
        match self {
            Self::Verilog2001 => COMPILE_EDITION_VERILOG_2001,
            Self::SystemVerilog2009 => COMPILE_EDITION_SYSTEMVERILOG_2009,
        }
    }

    fn from_snapshot_flags(flags: u32) -> Result<Self, SlangError> {
        match flags & SNAPSHOT_EDITION_MASK {
            SNAPSHOT_EDITION_VERILOG_2001 => Ok(Self::Verilog2001),
            SNAPSHOT_EDITION_SYSTEMVERILOG_2009 => Ok(Self::SystemVerilog2009),
            0 => Err(invalid_native(
                "snapshot does not identify its language edition",
            )),
            _ => Err(invalid_native(
                "snapshot identifies multiple language editions",
            )),
        }
    }
}

impl fmt::Display for LanguageEdition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Verilog2001 => "2001",
            Self::SystemVerilog2009 => "2009",
        })
    }
}

impl str::FromStr for LanguageEdition {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "2001" | "1364-2001" | "verilog-2001" => Ok(Self::Verilog2001),
            "2009" | "1800-2009" | "systemverilog-2009" => Ok(Self::SystemVerilog2009),
            _ => Err(format!(
                "unsupported language edition `{value}` (expected 2001 or 2009)"
            )),
        }
    }
}

/// Explicitly selects whether admitted compilation-unit buffers are parsed as
/// one unit or as one unit per buffer. Include-only buffers remain cache-only
/// inputs in either mode and retain the identity of their including source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompilationUnitMode {
    /// Each admitted compilation-unit buffer has an independent preprocessor
    /// and `$unit` scope. This is the compatibility default.
    #[default]
    Separate,
    /// All admitted compilation-unit buffers are parsed in caller order as
    /// one syntax tree while retaining per-buffer source locations.
    Merged,
}

impl CompilationUnitMode {
    fn compile_flag(self) -> u32 {
        match self {
            Self::Separate => 0,
            Self::Merged => COMPILE_MERGED_COMPILATION_UNITS,
        }
    }

    fn from_snapshot_flags(flags: u32) -> Self {
        if flags & SNAPSHOT_MERGED_COMPILATION_UNITS != 0 {
            Self::Merged
        } else {
            Self::Separate
        }
    }
}

impl fmt::Display for CompilationUnitMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Separate => "separate",
            Self::Merged => "merged",
        })
    }
}

impl str::FromStr for CompilationUnitMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "separate" | "per-file" | "per_file" => Ok(Self::Separate),
            "merged" | "all" => Ok(Self::Merged),
            _ => Err(format!(
                "unsupported compilation-unit mode `{value}` (expected separate or merged)"
            )),
        }
    }
}

/// Native and Rust-side capture limits for one compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_sources: u64,
    pub max_source_bytes: u64,
    pub max_diagnostics: u64,
    pub max_instances: u64,
    pub max_parameters: u64,
    pub max_types: u64,
    pub max_value_bits: u64,
    pub max_related_diagnostics: u64,
    pub max_output_bytes: u64,
    pub max_semantic_nodes: u64,
    pub max_semantic_edges: u64,
    pub max_lexical_tokens: u64,
    pub max_type_ranges: u64,
    pub max_type_members: u64,
    pub max_constants: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_sources: 4_096,
            max_source_bytes: 128 * 1024 * 1024,
            max_diagnostics: 10_000,
            max_instances: 1_000_000,
            max_parameters: 1_000_000,
            max_types: 1_000_000,
            max_value_bits: 256 * 1024 * 1024,
            max_related_diagnostics: 80_000,
            max_output_bytes: 256 * 1024 * 1024,
            max_semantic_nodes: 4_000_000,
            max_semantic_edges: 16_000_000,
            max_lexical_tokens: 8_000_000,
            max_type_ranges: 4_000_000,
            max_type_members: 4_000_000,
            max_constants: 1_000_000,
        }
    }
}

/// Typed options for an in-memory Slang compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileOptions {
    /// Complete compilation language policy. The default is SystemVerilog-2009.
    pub edition: LanguageEdition,
    /// Preprocessor definitions applied before parsing compilation units.
    pub defines: Vec<Define>,
    /// Explicit top module names; empty lets Slang select tops.
    pub top_modules: Vec<String>,
    /// Logical lookup prefixes for admitted include buffers. These paths do
    /// not authorize filesystem reads because the shim uses cache-only mode.
    pub include_dirs: Vec<String>,
    /// Top-level elaboration parameter overrides.
    pub parameter_overrides: Vec<ParameterOverride>,
    /// User-defined system-task/function prototypes accepted by Slang. Each
    /// entry uses the standard prototype spelling, for example
    /// `function int $probe(input logic value)`.
    pub system_subroutines: Vec<String>,
    /// Treat compilation units as library units so definitions are checked
    /// once without inferring and recursively elaborating design tops.
    pub library_units: bool,
    /// Group admitted compilation-unit buffers before preprocessing. The
    /// default keeps each buffer as an independent compilation unit.
    pub compilation_unit_mode: CompilationUnitMode,
    pub limits: Limits,
}

/// Sources and options borrowed for the duration of one blocking compile call.
#[derive(Debug, Clone, Copy)]
pub struct CompileRequest<'a> {
    pub sources: &'a [Source<'a>],
    pub options: &'a CompileOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlangErrorKind {
    InvalidArgument,
    LimitExceeded,
    Frontend,
    Internal,
    InvalidNativeData,
}

/// Failure to invoke Slang or safely decode its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlangError {
    kind: SlangErrorKind,
    message: String,
}

impl SlangError {
    fn new(kind: SlangErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> SlangErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SlangError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SlangError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticProvider {
    Compilation,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Ignored,
    Note,
    Warning,
    Error,
    Fatal,
}

/// Repository-owned projection of Slang v11's diagnostic subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSubsystem {
    Invalid,
    General,
    Lexer,
    Numeric,
    Preprocessor,
    Parser,
    Declarations,
    Expressions,
    Statements,
    Types,
    Lookup,
    SysFuncs,
    ConstEval,
    Compilation,
    Analysis,
    Meta,
    Driver,
    Tidy,
    Netlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub file_id: u64,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedDiagnostic {
    pub range: Option<SourceRange>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub provider: DiagnosticProvider,
    pub severity: DiagnosticSeverity,
    pub subsystem: DiagnosticSubsystem,
    pub code: u32,
    pub name: String,
    pub option_name: String,
    pub message: String,
    pub primary: Option<SourceRange>,
    pub related: Vec<RelatedDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub id: u64,
    pub name: String,
    pub byte_len: u64,
    /// Exact admitted in-memory contents; never populated by a disk read.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceKind {
    Module,
    Interface,
    Program,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub kind: InstanceKind,
    pub name: String,
    pub definition_name: String,
    pub declaration: Option<SourceRange>,
    pub parameter_start: u64,
    pub parameter_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Integral,
    Floating,
    String,
    Aggregate,
    Enum,
    PackedArray,
    FixedUnpackedArray,
    DynamicArray,
    AssociativeArray,
    Queue,
    PackedStruct,
    PackedUnion,
    UnpackedStruct,
    UnpackedUnion,
    Class,
    Chandle,
    Event,
    Void,
    VirtualInterface,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRangeKind {
    Packed,
    Unpacked,
    QueueBound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRange {
    pub left: i64,
    pub right: i64,
    pub kind: TypeRangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeMember {
    pub name: String,
    pub type_id: u64,
    pub bit_offset: u64,
    pub bit_width: u64,
    /// Constant-table identity of an explicit aggregate member initializer.
    pub initializer_constant_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub id: u64,
    pub kind: TypeKind,
    pub is_signed: bool,
    pub is_four_state: bool,
    pub is_fixed_size: bool,
    pub bit_width: u64,
    pub display_name: String,
    pub element_type_id: Option<u64>,
    pub index_type_id: Option<u64>,
    pub range_start: u64,
    pub range_count: u64,
    pub member_start: u64,
    pub member_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    None,
    Integer {
        is_signed: bool,
        bit_width: u64,
        value_words: Vec<u64>,
        unknown_words: Vec<u64>,
    },
    Real(f64),
    ShortReal(f32),
    /// Raw SystemVerilog string bytes; the language permits non-UTF-8 values.
    String(Vec<u8>),
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    pub value: ConstantValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Value,
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub owner_instance_id: u64,
    pub kind: ParameterKind,
    pub is_local: bool,
    pub is_port: bool,
    pub name: String,
    pub declaration: Option<SourceRange>,
    pub type_id: Option<u64>,
    pub constant_id: Option<u64>,
}

/// Frontend-independent category of one elaborated semantic record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticKind {
    Instance,
    Package,
    Class,
    GenerateScope,
    Port,
    Modport,
    InterfaceConnection,
    Net,
    Variable,
    Array,
    NamedEvent,
    Parameter,
    Process,
    ContinuousAssign,
    Primitive,
    Subroutine,
    Argument,
    Statement,
    Expression,
    SystemCall,
    MethodCall,
    FunctionCall,
    EnumConstant,
    Definition,
    Scope,
    TimingControl,
    NetAlias,
    AssertionExpr,
    Unsupported,
}

const ARGUMENT_CONST_REF: u64 = 1 << 0;
const ARGUMENT_REF_STATIC: u64 = 1 << 1;
pub(crate) const SUBROUTINE_STATIC: u64 = 1 << 0;
/// DPI-C import metadata copied from Slang's method flags and syntax.
pub(crate) const SUBROUTINE_DPI_IMPORT: u64 = 1 << 8;
pub(crate) const SUBROUTINE_DPI_CONTEXT: u64 = 1 << 9;
pub(crate) const SUBROUTINE_DPI_PURE: u64 = 1 << 10;
pub(crate) const SUBROUTINE_VIRTUAL: u64 = 1 << 1;
pub(crate) const SUBROUTINE_PURE: u64 = 1 << 2;
pub(crate) const SUBROUTINE_FINAL: u64 = 1 << 3;
pub(crate) const SUBROUTINE_CONSTRUCTOR: u64 = 1 << 4;
pub(crate) const CLASS_ABSTRACT: u64 = 1 << 0;
pub(crate) const CLASS_FINAL: u64 = 1 << 1;
pub(crate) const CLASS_INTERFACE: u64 = 1 << 2;
pub(crate) const NEW_CLASS_SUPER: u64 = 1 << 0;
pub(crate) const CALL_SUPER: u64 = 1 << 0;

/// Repository-owned qualifier tags stored in a statement's auxiliary field.
/// Keep these values in lockstep with the C ABI, rather than exposing Slang's
/// enum representation across the FFI boundary.
pub const SEMANTIC_UNIQUE_PRIORITY_NONE: u64 = 0;
pub const SEMANTIC_UNIQUE_PRIORITY_UNIQUE: u64 = 1;
pub const SEMANTIC_UNIQUE_PRIORITY_UNIQUE0: u64 = 2;
pub const SEMANTIC_UNIQUE_PRIORITY_PRIORITY: u64 = 3;

/// Immediate assertion statement tags owned by the C ABI.
pub const SEMANTIC_STMT_IMMEDIATE_ASSERT: u32 = 61;
pub const SEMANTIC_STMT_IMMEDIATE_ASSUME: u32 = 62;
pub const SEMANTIC_STMT_IMMEDIATE_COVER: u32 = 63;
pub const SEMANTIC_STMT_CONCURRENT_ASSERT: u32 = 64;
pub const SEMANTIC_STMT_CONCURRENT_ASSUME: u32 = 65;
pub const SEMANTIC_STMT_CONCURRENT_COVER: u32 = 66;
pub const SEMANTIC_STMT_CONCURRENT_EXPECT: u32 = 67;
/// Pattern-matching case statements are tagged apart from ordinary case so
/// lowering rejects them with their source location instead of treating a
/// surviving pattern as an empty case.
pub const SEMANTIC_STMT_PATTERN_CASE: u32 = 233;

pub const SEMANTIC_ASSERTION_EXPR_INVALID: u32 = 1;
pub const SEMANTIC_ASSERTION_EXPR_SIMPLE: u32 = 2;
pub const SEMANTIC_ASSERTION_EXPR_SEQUENCE_CONCAT: u32 = 3;
pub const SEMANTIC_ASSERTION_EXPR_SEQUENCE_WITH_MATCH: u32 = 4;
pub const SEMANTIC_ASSERTION_EXPR_UNARY: u32 = 5;
pub const SEMANTIC_ASSERTION_EXPR_BINARY: u32 = 6;
pub const SEMANTIC_ASSERTION_EXPR_FIRST_MATCH: u32 = 7;
pub const SEMANTIC_ASSERTION_EXPR_CLOCKING: u32 = 8;
pub const SEMANTIC_ASSERTION_EXPR_STRONG_WEAK: u32 = 9;
pub const SEMANTIC_ASSERTION_EXPR_ABORT: u32 = 10;
pub const SEMANTIC_ASSERTION_EXPR_CONDITIONAL: u32 = 11;
pub const SEMANTIC_ASSERTION_EXPR_CASE: u32 = 12;
pub const SEMANTIC_ASSERTION_EXPR_DISABLE_IFF: u32 = 13;

pub const SEMANTIC_ASSERTION_REPETITION: u64 = 1 << 0;
pub const SEMANTIC_ASSERTION_RANGE: u64 = 1 << 1;
pub const SEMANTIC_ASSERTION_STRONG: u64 = 1 << 2;
pub const SEMANTIC_ASSERTION_ABORT_REJECT: u64 = 1 << 3;
pub const SEMANTIC_ASSERTION_ABORT_SYNC: u64 = 1 << 4;
pub const SEMANTIC_ASSERTION_RANGE_UNBOUNDED: u32 = u32::MAX;
pub const SEMANTIC_ASSERTION_REPEAT_CONSECUTIVE: u32 = 1;
pub const SEMANTIC_ASSERTION_REPEAT_NONCONSECUTIVE: u32 = 2;
pub const SEMANTIC_ASSERTION_REPEAT_GOTO: u32 = 3;
pub const SEMANTIC_EXPR_ASSERTION_INSTANCE: u32 = 90;
/// Expression tag for a sampled-value `@(event)` argument.
pub const SEMANTIC_EXPR_CLOCKING_EVENT: u32 = 91;

/// Immediate assertion metadata carried in [`SemanticNode::auxiliary`].
pub const SEMANTIC_ASSERTION_DEFERRED: u64 = 1 << 0;
pub const SEMANTIC_ASSERTION_FINAL: u64 = 1 << 1;

/// Clocking metadata tags defined by the repository-owned semantic ABI.
pub const SEMANTIC_TIMING_CYCLE_DELAY: u32 = 117;
pub const SEMANTIC_TIMING_ONE_STEP_DELAY: u32 = 118;
pub const SEMANTIC_SCOPE_CLOCKING_BLOCK: u32 = 230;
pub const SEMANTIC_VARIABLE_CLOCKING: u32 = 231;
/// Local assertion variables are materialized by Slang per assertion attempt;
/// they are not members of the enclosing instance scope.
pub const SEMANTIC_VARIABLE_ASSERTION_LOCAL: u32 = 232;
pub const CLOCKING_BLOCK_DEFAULT: u64 = 1 << 0;
pub const CLOCKING_BLOCK_GLOBAL: u64 = 1 << 1;
pub const CLOCKING_INPUT_EDGE_SHIFT: u32 = 2;
pub const CLOCKING_OUTPUT_EDGE_SHIFT: u32 = 4;
pub const CLOCKING_VAR_OUTPUT_EDGE_SHIFT: u32 = 2;
pub const CLOCKING_EDGE_MASK: u64 = 0x3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticOperation {
    None,
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Power,
    BitNot,
    BitAnd,
    BitOr,
    BitXor,
    BitNand,
    BitNor,
    BitXnor,
    LogicalNot,
    LogicalAnd,
    LogicalOr,
    LogicalImplication,
    LogicalEquivalence,
    Equal,
    NotEqual,
    CaseEqual,
    CaseNotEqual,
    WildcardEqual,
    WildcardNotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    ShiftLeft,
    ShiftRight,
    ArithmeticShiftLeft,
    ArithmeticShiftRight,
    PreIncrement,
    PreDecrement,
    PostIncrement,
    PostDecrement,
    Concat,
    Replicate,
    Conditional,
    StreamLeft,
    StreamRight,
    Assign,
    Inside,
    AssignmentPattern,
    MinTypMax,
    MultiAssignmentPattern,
    List,
    AssertionAnd,
    AssertionOr,
    AssertionIntersect,
    AssertionThroughout,
    AssertionWithin,
    AssertionIff,
    AssertionUntil,
    AssertionSUntil,
    AssertionUntilWith,
    AssertionSUntilWith,
    AssertionImplies,
    AssertionOverlappedImplies,
    AssertionNonOverlappedImplies,
    AssertionOverlappedFollowedBy,
    AssertionNonOverlappedFollowedBy,
    AssertionNot,
    AssertionNextTime,
    AssertionSNextTime,
    AssertionAlways,
    AssertionSAlways,
    AssertionEventually,
    AssertionSEventually,
}

/// Exact source time scale attached by Slang to a definition or instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticTimeScale {
    pub unit: SemanticTimeUnit,
    pub magnitude: u32,
    pub precision_unit: SemanticTimeUnit,
    pub precision_magnitude: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
    Picoseconds,
    Femtoseconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticDefinitionKind {
    Module,
    Interface,
    Program,
}

/// Meaning of one relationship in the flat semantic graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticEdgeRole {
    Child,
    HighConnection,
    LowConnection,
    Initializer,
    Lhs,
    Rhs,
    Condition,
    Then,
    Else,
    Body,
    Operand,
    Index,
    Left,
    Right,
    Base,
    Width,
    Delay,
    Event,
    Argument,
    Receiver,
    Callee,
    Actual,
    DefaultValue,
    CaseItem,
    CaseExpression,
    Branch,
    Increment,
    Declaration,
    Reference,
    SourceIdentity,
    ReturnOwner,
    AliasNet,
    PropertySpec,
    Clocking,
    AssertionFormal,
    AssertionActual,
    BaseConstructor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticEdge {
    pub role: SemanticEdgeRole,
    pub index: u32,
    pub target_id: u64,
    /// SequenceConcat delay metadata, when this edge is a sequence element.
    pub sequence_delay: Option<SemanticSequenceRange>,
}

/// A checked inclusive sequence cycle range. `None` for `max` means the
/// frontend supplied an unbounded upper endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticSequenceRange {
    pub min: u32,
    pub max: Option<u32>,
}

/// One node in the bounded, owned elaborated semantic graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNode {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub kind: SemanticKind,
    /// Repository-owned subtype tag defined by the C ABI.
    pub subkind: u32,
    pub operation: SemanticOperation,
    pub is_bad: bool,
    pub is_uninstantiated: bool,
    pub is_automatic: bool,
    pub is_static: bool,
    pub is_top: bool,
    pub is_implicit: bool,
    pub is_local: bool,
    pub is_nonblocking: bool,
    pub is_input: bool,
    pub is_output: bool,
    pub is_inout: bool,
    pub is_ref: bool,
    /// `const ref` qualification for a formal argument.
    pub is_const_ref: bool,
    /// `ref static` qualification for a formal argument.
    pub is_ref_static: bool,
    pub is_implicit_conversion: bool,
    pub is_propagated_conversion: bool,
    pub is_indexed_up: bool,
    pub is_indexed_down: bool,
    pub case_wildcard_x_or_z: bool,
    pub case_wildcard_z: bool,
    pub case_inside: bool,
    pub is_posedge: bool,
    pub is_negedge: bool,
    pub is_both_edges: bool,
    pub is_primitive_declaration: bool,
    pub is_primitive_instance: bool,
    pub is_primitive_port: bool,
    pub is_task: bool,
    pub port_connection_present: bool,
    pub port_connection_open: bool,
    pub method_with_clause: bool,
    pub definition_kind: Option<SemanticDefinitionKind>,
    /// Symbol/expression name; a time literal carries its exact expanded token.
    pub name: String,
    /// Exact Slang kind spelling, retained for unsupported constructs.
    pub detail: String,
    pub definition_name: String,
    pub range: Option<SourceRange>,
    pub type_id: Option<u64>,
    pub constant_id: Option<u64>,
    pub target_id: Option<u64>,
    pub edge_start: u64,
    pub edge_count: u64,
    pub time_scale: Option<SemanticTimeScale>,
    pub strength0: SemanticDriveStrength,
    pub strength1: SemanticDriveStrength,
    /// Kind-specific scalar metadata. Streaming expressions store their exact
    /// Slang slice size; variables store their resolved lifetime tag;
    /// conditional/case statements store a `SEMANTIC_UNIQUE_PRIORITY_*` tag.
    pub auxiliary: u64,
    /// Assertion sequence repetition/range metadata copied from Slang.
    pub assertion_range_min: u32,
    pub assertion_range_max: Option<u32>,
    pub assertion_repetition_kind: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticDriveStrength {
    Unspecified,
    Supply,
    Strong,
    Pull,
    Weak,
    HighZ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalKind {
    Unknown,
    Module,
    Interface,
    Program,
    Package,
    Class,
    Struct,
    Union,
    Enum,
    EnumMember,
    TypeAlias,
    Parameter,
    Port,
    Variable,
    Net,
    Function,
    Task,
    Method,
    Macro,
    Keyword,
    String,
    Number,
    Operator,
    Identifier,
    Genvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalRole {
    None,
    Declaration,
    Reference,
    ConnectionLabel,
    Keyword,
    ConnectionActual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalToken {
    pub range: Option<SourceRange>,
    pub kind: LexicalKind,
    pub role: LexicalRole,
    pub is_missing: bool,
    pub is_skipped: bool,
    pub is_macro_expansion: bool,
    /// Token belongs to a preprocessor directive, not its expansion at a use site.
    pub is_directive: bool,
    /// Resolved compilation-unit value is declared after this reference,
    /// using native compilation-unit order across includes and source buffers.
    pub is_unit_forward_reference: bool,
    pub semantic_id: Option<u64>,
    pub text: String,
}

/// Fully owned observations from one Slang compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    flags: u32,
    /// Complete compilation language policy recorded by the native bridge.
    pub edition: LanguageEdition,
    /// Compilation-unit grouping selected for this snapshot.
    pub compilation_unit_mode: CompilationUnitMode,
    pub files: Vec<File>,
    pub diagnostics: Vec<Diagnostic>,
    pub instances: Vec<Instance>,
    pub parameters: Vec<Parameter>,
    pub types: Vec<Type>,
    pub constants: Vec<Constant>,
    pub semantic_nodes: Vec<SemanticNode>,
    pub semantic_edges: Vec<SemanticEdge>,
    pub lexical_tokens: Vec<LexicalToken>,
    pub type_ranges: Vec<TypeRange>,
    pub type_members: Vec<TypeMember>,
}

impl Snapshot {
    /// Slang's authoritative compilation-error flag.
    pub fn has_errors(&self) -> bool {
        self.flags & SNAPSHOT_HAS_ERRORS != 0
    }

    pub fn analysis_ran(&self) -> bool {
        self.flags & SNAPSHOT_ANALYSIS_RAN != 0
    }

    pub fn edition(&self) -> LanguageEdition {
        self.edition
    }
}

// The remaining declarations exactly mirror `src/wrapper/slang_c_api.h`.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawString {
    data: *const u8,
    len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawSource {
    name: RawString,
    text: RawString,
    flags: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawDefine {
    name: RawString,
    value: RawString,
    has_value: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawLimits {
    max_sources: u64,
    max_source_bytes: u64,
    max_diagnostics: u64,
    max_instances: u64,
    max_parameters: u64,
    max_types: u64,
    max_value_bits: u64,
    max_related_diagnostics: u64,
    max_output_bytes: u64,
    max_semantic_nodes: u64,
    max_semantic_edges: u64,
    max_lexical_tokens: u64,
    max_type_ranges: u64,
    max_type_members: u64,
    max_constants: u64,
}

#[repr(C)]
struct RawCompileRequest {
    abi_version: u32,
    flags: u32,
    sources: *const RawSource,
    source_count: u64,
    defines: *const RawDefine,
    define_count: u64,
    top_modules: *const RawString,
    top_module_count: u64,
    include_dirs: *const RawString,
    include_dir_count: u64,
    parameter_overrides: *const RawDefine,
    parameter_override_count: u64,
    system_subroutines: *const RawString,
    system_subroutine_count: u64,
    limits: RawLimits,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawRange {
    file_id: u64,
    start: u64,
    end: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawFile {
    id: u64,
    name: RawString,
    byte_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawDiagnostic {
    provider: u32,
    severity: u32,
    subsystem: u32,
    code: u32,
    name: RawString,
    option_name: RawString,
    message: RawString,
    primary: RawRange,
    related_start: u64,
    related_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawRelatedDiagnostic {
    range: RawRange,
    message: RawString,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawInstance {
    id: u64,
    parent_id: u64,
    kind: u32,
    reserved: u32,
    name: RawString,
    definition_name: RawString,
    declaration: RawRange,
    parameter_start: u64,
    parameter_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawType {
    id: u64,
    kind: u32,
    flags: u32,
    bit_width: u64,
    display_name: RawString,
    element_type_id: u64,
    index_type_id: u64,
    range_start: u64,
    range_count: u64,
    member_start: u64,
    member_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawTypeRange {
    left: i64,
    right: i64,
    kind: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawTypeMember {
    name: RawString,
    type_id: u64,
    bit_offset: u64,
    bit_width: u64,
    initializer_constant_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawConstant {
    kind: u32,
    is_signed: u32,
    bit_width: u64,
    value_word_start: u64,
    unknown_word_start: u64,
    word_count: u64,
    real_bits: u64,
    text: RawString,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawParameter {
    owner_instance_id: u64,
    kind: u32,
    flags: u32,
    name: RawString,
    declaration: RawRange,
    type_id: u64,
    constant_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawSemanticNode {
    id: u64,
    parent_id: u64,
    kind: u32,
    subkind: u32,
    operation: u32,
    flags: u32,
    name: RawString,
    detail: RawString,
    definition_name: RawString,
    range: RawRange,
    type_id: u64,
    constant_id: u64,
    target_id: u64,
    edge_start: u64,
    edge_count: u64,
    time_unit: u32,
    time_unit_magnitude: u32,
    time_precision_unit: u32,
    time_precision_magnitude: u32,
    strength0: u32,
    strength1: u32,
    auxiliary: u64,
    assertion_range_min: u32,
    assertion_range_max: u32,
    assertion_repetition_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawSemanticEdge {
    role: u32,
    index: u32,
    target_id: u64,
    sequence_delay_valid: u32,
    sequence_delay_min: u32,
    sequence_delay_max: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawLexicalToken {
    range: RawRange,
    kind: u32,
    role: u32,
    flags: u32,
    reserved: u32,
    semantic_id: u64,
    text: RawString,
}

#[repr(C)]
struct RawSnapshotView {
    abi_version: u32,
    flags: u32,
    files: *const RawFile,
    file_count: u64,
    diagnostics: *const RawDiagnostic,
    diagnostic_count: u64,
    related_diagnostics: *const RawRelatedDiagnostic,
    related_diagnostic_count: u64,
    instances: *const RawInstance,
    instance_count: u64,
    parameters: *const RawParameter,
    parameter_count: u64,
    types: *const RawType,
    type_count: u64,
    constants: *const RawConstant,
    constant_count: u64,
    value_words: *const u64,
    value_word_count: u64,
    semantic_nodes: *const RawSemanticNode,
    semantic_node_count: u64,
    semantic_edges: *const RawSemanticEdge,
    semantic_edge_count: u64,
    lexical_tokens: *const RawLexicalToken,
    lexical_token_count: u64,
    type_ranges: *const RawTypeRange,
    type_range_count: u64,
    type_members: *const RawTypeMember,
    type_member_count: u64,
}

#[repr(C)]
struct RawErrorView {
    status: u32,
    reserved: u32,
    message: RawString,
}

enum RawSnapshot {}
enum RawError {}

// SAFETY: declarations mirror `src/wrapper/slang_c_api.h`; inputs remain valid
// for each blocking call, returned owners are destroyed by the matching shim
// function, and borrowed views are copied before their owner is destroyed.
#[link(name = "llg_slang_wrapper", kind = "static")]
#[link(name = "svlang", kind = "static")]
#[link(name = "fmt", kind = "static")]
unsafe extern "C" {
    fn llg_slang_compile(
        request: *const RawCompileRequest,
        out_snapshot: *mut *mut RawSnapshot,
        out_error: *mut *mut RawError,
    ) -> u32;
    fn llg_slang_snapshot_view(
        snapshot: *const RawSnapshot,
        out_view: *mut RawSnapshotView,
        out_error: *mut *mut RawError,
    ) -> u32;
    fn llg_slang_error_view(error: *const RawError, out_view: *mut RawErrorView) -> u32;
    fn llg_slang_snapshot_destroy(snapshot: *mut RawSnapshot);
    fn llg_slang_error_destroy(error: *mut RawError);
}

struct SnapshotOwner(*mut RawSnapshot);

impl Drop for SnapshotOwner {
    fn drop(&mut self) {
        // SAFETY: this pointer is null or the unique owner returned by the shim.
        unsafe { llg_slang_snapshot_destroy(self.0) };
    }
}

struct ErrorOwner(*mut RawError);

impl Drop for ErrorOwner {
    fn drop(&mut self) {
        // SAFETY: this pointer is null or the unique owner returned by the shim.
        unsafe { llg_slang_error_destroy(self.0) };
    }
}

/// Compile admitted in-memory sources and copy the native snapshot into Rust.
pub fn compile(request: &CompileRequest<'_>) -> Result<Snapshot, SlangError> {
    validate_request(request)?;

    let raw_sources: Vec<_> = request
        .sources
        .iter()
        .map(|source| RawSource {
            name: raw_string(source.name),
            text: raw_string(source.text),
            flags: u32::from(source.is_compilation_unit),
            reserved: 0,
        })
        .collect();
    let raw_defines: Vec<_> = request
        .options
        .defines
        .iter()
        .map(|define| RawDefine {
            name: raw_string(&define.name),
            value: define
                .value
                .as_deref()
                .map_or(empty_raw_string(), raw_string),
            has_value: u32::from(define.value.is_some()),
            reserved: 0,
        })
        .collect();
    let raw_tops: Vec<_> = request
        .options
        .top_modules
        .iter()
        .map(|name| raw_string(name))
        .collect();
    let raw_include_dirs: Vec<_> = request
        .options
        .include_dirs
        .iter()
        .map(|path| raw_string(path))
        .collect();
    let raw_parameter_overrides: Vec<_> = request
        .options
        .parameter_overrides
        .iter()
        .map(|parameter| RawDefine {
            name: raw_string(&parameter.name),
            value: raw_string(&parameter.value),
            has_value: 1,
            reserved: 0,
        })
        .collect();
    let raw_system_subroutines: Vec<_> = request
        .options
        .system_subroutines
        .iter()
        .map(|prototype| raw_string(prototype))
        .collect();
    let limits = request.options.limits;
    let raw_request = RawCompileRequest {
        abi_version: ABI_VERSION,
        flags: (if request.options.library_units {
            COMPILE_LIBRARY_UNITS
        } else {
            0
        }) | request.options.edition.compile_flag()
            | request.options.compilation_unit_mode.compile_flag(),
        sources: raw_sources.as_ptr(),
        source_count: raw_sources.len() as u64,
        defines: raw_defines.as_ptr(),
        define_count: raw_defines.len() as u64,
        top_modules: raw_tops.as_ptr(),
        top_module_count: raw_tops.len() as u64,
        include_dirs: raw_include_dirs.as_ptr(),
        include_dir_count: raw_include_dirs.len() as u64,
        parameter_overrides: raw_parameter_overrides.as_ptr(),
        parameter_override_count: raw_parameter_overrides.len() as u64,
        system_subroutines: raw_system_subroutines.as_ptr(),
        system_subroutine_count: raw_system_subroutines.len() as u64,
        limits: RawLimits {
            max_sources: limits.max_sources,
            max_source_bytes: limits.max_source_bytes,
            max_diagnostics: limits.max_diagnostics,
            max_instances: limits.max_instances,
            max_parameters: limits.max_parameters,
            max_types: limits.max_types,
            max_value_bits: limits.max_value_bits,
            max_related_diagnostics: limits.max_related_diagnostics,
            max_output_bytes: limits.max_output_bytes,
            max_semantic_nodes: limits.max_semantic_nodes,
            max_semantic_edges: limits.max_semantic_edges,
            max_lexical_tokens: limits.max_lexical_tokens,
            max_type_ranges: limits.max_type_ranges,
            max_type_members: limits.max_type_members,
            max_constants: limits.max_constants,
        },
    };

    let mut snapshot = ptr::null_mut();
    let mut error = ptr::null_mut();
    // SAFETY: all request pointers refer to live vectors or borrowed strings
    // that remain valid for this blocking call; output pointers are writable.
    let status = unsafe { llg_slang_compile(&raw_request, &mut snapshot, &mut error) };
    if status != STATUS_OK {
        let _unexpected_snapshot = SnapshotOwner(snapshot);
        return Err(take_native_error(status, error));
    }
    let unexpected_error = ErrorOwner(error);
    if !unexpected_error.0.is_null() {
        let _snapshot = SnapshotOwner(snapshot);
        return Err(invalid_native(
            "successful compile returned an unexpected error owner",
        ));
    }
    if snapshot.is_null() {
        return Err(invalid_native("successful compile returned no snapshot"));
    }
    let snapshot = SnapshotOwner(snapshot);
    let mut decoded = decode_snapshot(&snapshot, &limits)?;
    if decoded.edition != request.options.edition {
        return Err(invalid_native(
            "native snapshot language edition does not match the compile request",
        ));
    }
    for file in &mut decoded.files {
        let source = request
            .sources
            .iter()
            .find(|source| source.name == file.name)
            .ok_or_else(|| invalid_native("snapshot file was not an admitted source"))?;
        if source.text.len() as u64 != file.byte_len {
            return Err(invalid_native(
                "snapshot file length does not match admitted source",
            ));
        }
        file.text = source.text.to_owned();
    }
    drop(unexpected_error);
    Ok(decoded)
}

fn validate_request(request: &CompileRequest<'_>) -> Result<(), SlangError> {
    let limits = request.options.limits;
    if request.sources.is_empty() {
        return Err(invalid_argument("at least one source is required"));
    }
    if request.sources.len() > MAX_SOURCES {
        return Err(limit_exceeded("source count exceeds the native limit"));
    }
    if request.sources.len() as u64 > limits.max_sources {
        return Err(limit_exceeded("source count exceeds max_sources"));
    }
    let mut total = 0_u64;
    let mut names = HashSet::with_capacity(request.sources.len());
    for source in request.sources {
        validate_name(source.name, "source name")?;
        if !names.insert(source.name) {
            return Err(invalid_argument("source names must be unique"));
        }
        total = total
            .checked_add(source.name.len() as u64)
            .ok_or_else(|| limit_exceeded("source byte count overflowed"))?
            .checked_add(source.text.len() as u64)
            .ok_or_else(|| limit_exceeded("source byte count overflowed"))?;
        if total > limits.max_source_bytes {
            return Err(limit_exceeded("source bytes exceed max_source_bytes"));
        }
    }
    if request.options.defines.len() > MAX_DEFINES {
        return Err(limit_exceeded("define count exceeds the native limit"));
    }
    if request.options.top_modules.len() > MAX_TOP_MODULES {
        return Err(limit_exceeded("top module count exceeds the native limit"));
    }
    if request.options.include_dirs.len() > MAX_INCLUDE_DIRS {
        return Err(limit_exceeded(
            "include directory count exceeds the native limit",
        ));
    }
    if request.options.parameter_overrides.len() > MAX_PARAMETER_OVERRIDES {
        return Err(limit_exceeded(
            "parameter override count exceeds the native limit",
        ));
    }
    if request.options.system_subroutines.len() > MAX_SYSTEM_SUBROUTINES {
        return Err(limit_exceeded(
            "system subroutine count exceeds the native limit",
        ));
    }
    let mut config_bytes = 0_u64;
    for define in &request.options.defines {
        validate_name(&define.name, "define name")?;
        if define
            .value
            .as_deref()
            .is_some_and(|value| value.contains('\0'))
        {
            return Err(invalid_argument("define value contains a NUL byte"));
        }
        add_input_bytes(&mut config_bytes, define.name.len(), "configuration")?;
        if let Some(value) = &define.value {
            add_input_bytes(&mut config_bytes, value.len(), "configuration")?;
        }
    }
    for top in &request.options.top_modules {
        validate_name(top, "top module name")?;
        add_input_bytes(&mut config_bytes, top.len(), "configuration")?;
    }
    for path in &request.options.include_dirs {
        if path.contains('\0') {
            return Err(invalid_argument("include directory contains a NUL byte"));
        }
        add_input_bytes(&mut config_bytes, path.len(), "configuration")?;
    }
    for parameter in &request.options.parameter_overrides {
        validate_name(&parameter.name, "parameter override name")?;
        if parameter.value.contains('\0') {
            return Err(invalid_argument(
                "parameter override value contains a NUL byte",
            ));
        }
        add_input_bytes(&mut config_bytes, parameter.name.len(), "configuration")?;
        add_input_bytes(&mut config_bytes, parameter.value.len(), "configuration")?;
    }
    for prototype in &request.options.system_subroutines {
        if prototype.is_empty() || prototype.contains('\0') {
            return Err(invalid_argument(
                "system subroutine prototypes must be nonempty and contain no NUL bytes",
            ));
        }
        add_input_bytes(&mut config_bytes, prototype.len(), "configuration")?;
    }
    if !request
        .sources
        .iter()
        .any(|source| source.is_compilation_unit)
    {
        return Err(invalid_argument(
            "at least one compilation unit source is required",
        ));
    }
    if [
        limits.max_sources,
        limits.max_source_bytes,
        limits.max_diagnostics,
        limits.max_instances,
        limits.max_parameters,
        limits.max_types,
        limits.max_value_bits,
        limits.max_related_diagnostics,
        limits.max_output_bytes,
        limits.max_semantic_nodes,
        limits.max_semantic_edges,
        limits.max_lexical_tokens,
        limits.max_type_ranges,
        limits.max_type_members,
        limits.max_constants,
    ]
    .contains(&0)
    {
        return Err(invalid_argument("Slang capture limits must be positive"));
    }
    Ok(())
}

fn add_input_bytes(total: &mut u64, amount: usize, label: &str) -> Result<(), SlangError> {
    *total = total
        .checked_add(amount as u64)
        .ok_or_else(|| limit_exceeded(format!("{label} byte count overflowed")))?;
    if *total > MAX_CONFIG_BYTES {
        return Err(limit_exceeded(format!(
            "{label} bytes exceed the native limit"
        )));
    }
    Ok(())
}

fn validate_name(value: &str, label: &str) -> Result<(), SlangError> {
    if value.is_empty() {
        return Err(invalid_argument(format!("{label} must not be empty")));
    }
    if value.contains('\0') {
        return Err(invalid_argument(format!("{label} contains a NUL byte")));
    }
    Ok(())
}

fn raw_string(value: &str) -> RawString {
    RawString {
        data: value.as_ptr(),
        len: value.len() as u64,
    }
}

fn empty_raw_string() -> RawString {
    RawString {
        data: ptr::null(),
        len: 0,
    }
}

fn decode_range(raw: RawRange, files: &[File]) -> Result<Option<SourceRange>, SlangError> {
    if raw.file_id == INVALID_ID {
        if raw.start != 0 || raw.end != 0 {
            return Err(invalid_native("unknown source range has nonzero offsets"));
        }
        return Ok(None);
    }
    let file = files
        .iter()
        .find(|file| file.id == raw.file_id)
        .ok_or_else(|| invalid_native("source range refers to an unknown file"))?;
    if raw.start > raw.end || raw.end > file.byte_len {
        return Err(invalid_native("source range lies outside its file"));
    }
    Ok(Some(SourceRange {
        file_id: raw.file_id,
        start: raw.start,
        end: raw.end,
    }))
}

fn enforce_count(actual: u64, limit: u64, label: &str) -> Result<(), SlangError> {
    if actual > limit {
        Err(invalid_native(format!(
            "native {label} exceed the configured limit"
        )))
    } else {
        Ok(())
    }
}

fn checked_window(
    start: u64,
    count: u64,
    len: usize,
    label: &str,
) -> Result<std::ops::Range<usize>, SlangError> {
    let start = usize::try_from(start)
        .map_err(|_| invalid_native(format!("{label} start does not fit usize")))?;
    let count = usize::try_from(count)
        .map_err(|_| invalid_native(format!("{label} count does not fit usize")))?;
    let end = start
        .checked_add(count)
        .ok_or_else(|| invalid_native(format!("{label} range overflowed")))?;
    if end > len {
        return Err(invalid_native(format!("{label} range is out of bounds")));
    }
    Ok(start..end)
}

/// Borrow one foreign array after validating its pointer and representable size.
///
/// # Safety
/// For nonzero `len`, `data` must reference `len` aligned initialized values
/// owned by the live native snapshot and remain valid for the returned borrow.
unsafe fn foreign_slice<'a, T>(
    data: *const T,
    len: u64,
    label: &str,
) -> Result<&'a [T], SlangError> {
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() {
        return Err(invalid_native(format!(
            "nonempty {label} has a null pointer"
        )));
    }
    if data.align_offset(std::mem::align_of::<T>()) != 0 {
        return Err(invalid_native(format!(
            "nonempty {label} has a misaligned pointer"
        )));
    }
    let len = usize::try_from(len)
        .map_err(|_| invalid_native(format!("{label} length does not fit usize")))?;
    if len > isize::MAX as usize / std::mem::size_of::<T>() {
        return Err(invalid_native(format!(
            "{label} byte length exceeds isize::MAX"
        )));
    }
    // SAFETY: the caller provides the alignment, initialization, ownership,
    // and lifetime guarantees documented above; length arithmetic was checked.
    Ok(unsafe { slice::from_raw_parts(data, len) })
}

/// Copy and validate one UTF-8 string borrowed from the native snapshot.
///
/// # Safety
/// For nonzero length, `raw.data` must reference that many initialized bytes
/// owned by the live snapshot.
unsafe fn copy_string(raw: RawString, label: &str) -> Result<String, SlangError> {
    // SAFETY: the caller establishes the snapshot lifetime and byte validity;
    // foreign_slice validates null and representable length.
    let bytes = unsafe { foreign_slice(raw.data, raw.len, label)? };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| invalid_native(format!("{label} is not valid UTF-8")))
}

/// Copy arbitrary bytes borrowed from the native snapshot.
///
/// # Safety
/// For nonzero length, `raw.data` must reference that many initialized bytes
/// owned by the live snapshot.
unsafe fn copy_bytes(raw: RawString, label: &str) -> Result<Vec<u8>, SlangError> {
    // SAFETY: the caller establishes the snapshot lifetime and byte validity;
    // foreign_slice validates null and representable length.
    Ok(unsafe { foreign_slice(raw.data, raw.len, label)? }.to_vec())
}

fn charge_output_string(total: &mut u64, raw: RawString, limit: u64) -> Result<(), SlangError> {
    *total = total
        .checked_add(raw.len)
        .ok_or_else(|| invalid_native("native output string byte count overflowed"))?;
    if *total > limit {
        return Err(invalid_native(
            "native output strings exceed the configured max_output_bytes",
        ));
    }
    Ok(())
}

fn take_native_error(status: u32, error: *mut RawError) -> SlangError {
    let kind = status_kind(status);
    let owner = ErrorOwner(error);
    if owner.0.is_null() {
        return SlangError::new(kind, format!("Slang failed with status {status}"));
    }
    let mut view = RawErrorView {
        status: 0,
        reserved: 0,
        message: empty_raw_string(),
    };
    // SAFETY: owner contains a live error and `view` is writable.
    let view_status = unsafe { llg_slang_error_view(owner.0, &mut view) };
    if view_status != STATUS_OK {
        return SlangError::new(kind, format!("Slang failed with status {status}"));
    }
    if view.status != status {
        return invalid_native("native error status does not match the call status");
    }
    if view.reserved != 0 {
        return invalid_native("native error reserved field is nonzero");
    }
    // SAFETY: the message borrows from `owner`, which remains live through copy.
    let message = unsafe { copy_string(view.message, "native error message") }
        .unwrap_or_else(|_| format!("Slang failed with status {status}"));
    SlangError::new(kind, message)
}

fn status_kind(status: u32) -> SlangErrorKind {
    match status {
        STATUS_INVALID_ARGUMENT => SlangErrorKind::InvalidArgument,
        STATUS_LIMIT_EXCEEDED => SlangErrorKind::LimitExceeded,
        STATUS_FRONTEND_ERROR => SlangErrorKind::Frontend,
        STATUS_INTERNAL_ERROR => SlangErrorKind::Internal,
        _ => SlangErrorKind::InvalidNativeData,
    }
}

fn invalid_argument(message: impl Into<String>) -> SlangError {
    SlangError::new(SlangErrorKind::InvalidArgument, message)
}

fn limit_exceeded(message: impl Into<String>) -> SlangError {
    SlangError::new(SlangErrorKind::LimitExceeded, message)
}

fn invalid_native(message: impl Into<String>) -> SlangError {
    SlangError::new(SlangErrorKind::InvalidNativeData, message)
}

#[cfg(test)]
mod tests;
