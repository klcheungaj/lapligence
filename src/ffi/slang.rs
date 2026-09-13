//! Safe, owned Rust facade over the Slang C ABI.
//!
//! Native snapshots are borrowed only while being decoded. No Slang pointer or
//! native allocation escapes [`compile`].

#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]
// Each archive must propagate through the Rust library boundary.
#![allow(clippy::duplicated_attributes)]

use std::collections::HashSet;
use std::fmt;
use std::ptr;
use std::slice;
use std::str;

const ABI_VERSION: u32 = 3;
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

fn decode_snapshot(owner: &SnapshotOwner, limits: &Limits) -> Result<Snapshot, SlangError> {
    let mut view = RawSnapshotView {
        abi_version: 0,
        flags: 0,
        files: ptr::null(),
        file_count: 0,
        diagnostics: ptr::null(),
        diagnostic_count: 0,
        related_diagnostics: ptr::null(),
        related_diagnostic_count: 0,
        instances: ptr::null(),
        instance_count: 0,
        parameters: ptr::null(),
        parameter_count: 0,
        types: ptr::null(),
        type_count: 0,
        constants: ptr::null(),
        constant_count: 0,
        value_words: ptr::null(),
        value_word_count: 0,
        semantic_nodes: ptr::null(),
        semantic_node_count: 0,
        semantic_edges: ptr::null(),
        semantic_edge_count: 0,
        lexical_tokens: ptr::null(),
        lexical_token_count: 0,
        type_ranges: ptr::null(),
        type_range_count: 0,
        type_members: ptr::null(),
        type_member_count: 0,
    };
    let mut error = ptr::null_mut();
    // SAFETY: owner contains a live snapshot and output pointers are writable.
    let status = unsafe { llg_slang_snapshot_view(owner.0, &mut view, &mut error) };
    if status != STATUS_OK {
        return Err(take_native_error(status, error));
    }
    let unexpected_error = ErrorOwner(error);
    if !unexpected_error.0.is_null() {
        return Err(invalid_native(
            "successful snapshot view returned an unexpected error owner",
        ));
    }
    if view.abi_version != ABI_VERSION {
        return Err(invalid_native(format!(
            "Slang ABI version mismatch: expected {ABI_VERSION}, received {}",
            view.abi_version
        )));
    }
    if view.flags & !SNAPSHOT_KNOWN_FLAGS != 0 {
        return Err(invalid_native("snapshot contains unknown flags"));
    }
    enforce_count(view.file_count, limits.max_sources, "files")?;
    enforce_count(view.diagnostic_count, limits.max_diagnostics, "diagnostics")?;
    enforce_count(view.instance_count, limits.max_instances, "instances")?;
    enforce_count(view.parameter_count, limits.max_parameters, "parameters")?;
    enforce_count(view.type_count, limits.max_types, "types")?;
    enforce_count(
        view.semantic_node_count,
        limits.max_semantic_nodes,
        "semantic nodes",
    )?;
    enforce_count(
        view.semantic_edge_count,
        limits.max_semantic_edges,
        "semantic edges",
    )?;
    enforce_count(
        view.lexical_token_count,
        limits.max_lexical_tokens,
        "lexical tokens",
    )?;
    enforce_count(view.type_range_count, limits.max_type_ranges, "type ranges")?;
    enforce_count(
        view.type_member_count,
        limits.max_type_members,
        "type members",
    )?;
    enforce_count(
        view.related_diagnostic_count,
        limits.max_related_diagnostics,
        "related diagnostics",
    )?;
    // Each integer constant can require a partially filled word, so the
    // padded-word bound includes one extra word per constant and per plane.
    let max_words = limits
        .max_value_bits
        .div_ceil(64)
        .saturating_add(view.constant_count)
        .saturating_mul(2);
    enforce_count(view.value_word_count, max_words, "constant value words")?;
    enforce_count(view.constant_count, limits.max_constants, "constants")?;

    let mut output_bytes = 0_u64;
    for (count, size) in [
        (view.file_count, std::mem::size_of::<RawFile>()),
        (view.diagnostic_count, std::mem::size_of::<RawDiagnostic>()),
        (
            view.related_diagnostic_count,
            std::mem::size_of::<RawRelatedDiagnostic>(),
        ),
        (view.instance_count, std::mem::size_of::<RawInstance>()),
        (view.parameter_count, std::mem::size_of::<RawParameter>()),
        (view.type_count, std::mem::size_of::<RawType>()),
        (view.constant_count, std::mem::size_of::<RawConstant>()),
        (view.value_word_count, std::mem::size_of::<u64>()),
        (
            view.semantic_node_count,
            std::mem::size_of::<RawSemanticNode>(),
        ),
        (
            view.semantic_edge_count,
            std::mem::size_of::<RawSemanticEdge>(),
        ),
        (
            view.lexical_token_count,
            std::mem::size_of::<RawLexicalToken>(),
        ),
        (view.type_range_count, std::mem::size_of::<RawTypeRange>()),
        (view.type_member_count, std::mem::size_of::<RawTypeMember>()),
    ] {
        let bytes = count
            .checked_mul(size as u64)
            .ok_or_else(|| invalid_native("native output record byte count overflowed"))?;
        output_bytes = output_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid_native("native output byte count overflowed"))?;
    }
    if output_bytes > limits.max_output_bytes {
        return Err(invalid_native(
            "native output records exceed the configured max_output_bytes",
        ));
    }

    // SAFETY: the shim guarantees every nonempty view pointer references a
    // properly aligned initialized array owned by the live snapshot.
    let raw_files = unsafe { foreign_slice(view.files, view.file_count, "files")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_related = unsafe {
        foreign_slice(
            view.related_diagnostics,
            view.related_diagnostic_count,
            "related diagnostics",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_diagnostics =
        unsafe { foreign_slice(view.diagnostics, view.diagnostic_count, "diagnostics")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_instances = unsafe { foreign_slice(view.instances, view.instance_count, "instances")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_parameters =
        unsafe { foreign_slice(view.parameters, view.parameter_count, "parameters")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_types = unsafe { foreign_slice(view.types, view.type_count, "types")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_constants = unsafe { foreign_slice(view.constants, view.constant_count, "constants")? };
    // SAFETY: same snapshot-view contract as above.
    let value_words = unsafe {
        foreign_slice(
            view.value_words,
            view.value_word_count,
            "constant value words",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_semantic_nodes = unsafe {
        foreign_slice(
            view.semantic_nodes,
            view.semantic_node_count,
            "semantic nodes",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_semantic_edges = unsafe {
        foreign_slice(
            view.semantic_edges,
            view.semantic_edge_count,
            "semantic edges",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_lexical_tokens = unsafe {
        foreign_slice(
            view.lexical_tokens,
            view.lexical_token_count,
            "lexical tokens",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_type_ranges =
        unsafe { foreign_slice(view.type_ranges, view.type_range_count, "type ranges")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_type_members =
        unsafe { foreign_slice(view.type_members, view.type_member_count, "type members")? };

    for item in raw_files {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }
    for item in raw_diagnostics {
        for value in [item.name, item.option_name, item.message] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_related {
        charge_output_string(&mut output_bytes, item.message, limits.max_output_bytes)?;
    }
    for item in raw_instances {
        for value in [item.name, item.definition_name] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_parameters {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }
    for item in raw_types {
        charge_output_string(
            &mut output_bytes,
            item.display_name,
            limits.max_output_bytes,
        )?;
    }
    for item in raw_constants {
        charge_output_string(&mut output_bytes, item.text, limits.max_output_bytes)?;
    }
    for item in raw_semantic_nodes {
        for value in [item.name, item.detail, item.definition_name] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_lexical_tokens {
        charge_output_string(&mut output_bytes, item.text, limits.max_output_bytes)?;
    }
    for item in raw_type_members {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }

    let mut file_ids = HashSet::with_capacity(raw_files.len());
    let mut files = Vec::with_capacity(raw_files.len());
    for raw in raw_files {
        if raw.id == INVALID_ID || !file_ids.insert(raw.id) {
            return Err(invalid_native(
                "snapshot contains an invalid or duplicate file id",
            ));
        }
        if raw.byte_len > limits.max_source_bytes {
            return Err(invalid_native(
                "file byte length exceeds the configured limit",
            ));
        }
        files.push(File {
            id: raw.id,
            // SAFETY: native strings borrow from the live snapshot.
            name: unsafe { copy_string(raw.name, "file name")? },
            byte_len: raw.byte_len,
            text: String::new(),
        });
    }

    let related = decode_related(raw_related, &files)?;
    let diagnostics = decode_diagnostics(raw_diagnostics, &related, &files)?;
    let (types, type_ranges, type_members) =
        decode_types(raw_types, raw_type_ranges, raw_type_members)?;
    let constants = decode_constants(raw_constants, value_words, limits)?;
    let instances = decode_instances(raw_instances, &files, raw_parameters.len())?;
    let parameters =
        decode_parameters(raw_parameters, &files, &instances, &types, constants.len())?;
    validate_parameter_windows(&instances, &parameters)?;
    let semantic_edges = decode_semantic_edges(raw_semantic_edges, raw_semantic_nodes)?;
    let semantic_nodes = decode_semantic_nodes(
        raw_semantic_nodes,
        &semantic_edges,
        &files,
        &types,
        constants.len(),
    )?;
    let lexical_tokens = decode_lexical_tokens(raw_lexical_tokens, &files, &semantic_nodes)?;

    drop(unexpected_error);
    Ok(Snapshot {
        flags: view.flags,
        edition: LanguageEdition::from_snapshot_flags(view.flags)?,
        compilation_unit_mode: CompilationUnitMode::from_snapshot_flags(view.flags),
        files,
        diagnostics,
        instances,
        parameters,
        types,
        constants,
        semantic_nodes,
        semantic_edges,
        lexical_tokens,
        type_ranges,
        type_members,
    })
}

fn decode_semantic_edges(
    raw: &[RawSemanticEdge],
    nodes: &[RawSemanticNode],
) -> Result<Vec<SemanticEdge>, SlangError> {
    let node_ids: HashSet<_> = nodes.iter().map(|node| node.id).collect();
    raw.iter()
        .map(|edge| {
            if !node_ids.contains(&edge.target_id) {
                return Err(invalid_native("semantic edge target does not exist"));
            }
            if edge.sequence_delay_valid > 1
                || (edge.sequence_delay_valid != 0
                    && edge.sequence_delay_max != SEMANTIC_ASSERTION_RANGE_UNBOUNDED
                    && edge.sequence_delay_max < edge.sequence_delay_min)
            {
                return Err(invalid_native(
                    "semantic edge has an invalid sequence delay range",
                ));
            }
            let role = match edge.role {
                1 => SemanticEdgeRole::Child,
                2 => SemanticEdgeRole::HighConnection,
                3 => SemanticEdgeRole::LowConnection,
                4 => SemanticEdgeRole::Initializer,
                5 => SemanticEdgeRole::Lhs,
                6 => SemanticEdgeRole::Rhs,
                7 => SemanticEdgeRole::Condition,
                8 => SemanticEdgeRole::Then,
                9 => SemanticEdgeRole::Else,
                10 => SemanticEdgeRole::Body,
                11 => SemanticEdgeRole::Operand,
                12 => SemanticEdgeRole::Index,
                13 => SemanticEdgeRole::Left,
                14 => SemanticEdgeRole::Right,
                15 => SemanticEdgeRole::Base,
                16 => SemanticEdgeRole::Width,
                17 => SemanticEdgeRole::Delay,
                18 => SemanticEdgeRole::Event,
                19 => SemanticEdgeRole::Argument,
                20 => SemanticEdgeRole::Receiver,
                21 => SemanticEdgeRole::Callee,
                22 => SemanticEdgeRole::Actual,
                23 => SemanticEdgeRole::DefaultValue,
                24 => SemanticEdgeRole::CaseItem,
                25 => SemanticEdgeRole::CaseExpression,
                26 => SemanticEdgeRole::Branch,
                27 => SemanticEdgeRole::Increment,
                28 => SemanticEdgeRole::Declaration,
                29 => SemanticEdgeRole::Reference,
                30 => SemanticEdgeRole::SourceIdentity,
                31 => SemanticEdgeRole::ReturnOwner,
                32 => SemanticEdgeRole::AliasNet,
                33 => SemanticEdgeRole::PropertySpec,
                34 => SemanticEdgeRole::Clocking,
                35 => SemanticEdgeRole::AssertionFormal,
                36 => SemanticEdgeRole::AssertionActual,
                37 => SemanticEdgeRole::BaseConstructor,
                _ => return Err(invalid_native("semantic edge has an unknown role")),
            };
            Ok(SemanticEdge {
                role,
                index: edge.index,
                target_id: edge.target_id,
                sequence_delay: (edge.sequence_delay_valid != 0).then(|| SemanticSequenceRange {
                    min: edge.sequence_delay_min,
                    max: (edge.sequence_delay_max != u32::MAX).then_some(edge.sequence_delay_max),
                }),
            })
        })
        .collect()
}

fn decode_semantic_nodes(
    raw: &[RawSemanticNode],
    edges: &[SemanticEdge],
    files: &[File],
    types: &[Type],
    constant_len: usize,
) -> Result<Vec<SemanticNode>, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|node| node.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid semantic node ids",
        ));
    }
    if raw
        .iter()
        .enumerate()
        .any(|(index, node)| node.id != index as u64)
    {
        return Err(invalid_native(
            "semantic node ids are not contiguous arena indices",
        ));
    }
    let type_ids: HashSet<_> = types.iter().map(|ty| ty.id).collect();
    let mut claimed_edges = vec![false; edges.len()];
    for node in raw {
        let window = checked_window(
            node.edge_start,
            node.edge_count,
            edges.len(),
            "semantic node edges",
        )?;
        for index in window {
            if claimed_edges[index] {
                return Err(invalid_native("semantic node edge windows overlap"));
            }
            claimed_edges[index] = true;
        }
    }
    if claimed_edges.iter().any(|claimed| !claimed) {
        return Err(invalid_native("semantic edge is not owned by a node"));
    }
    raw.iter()
        .map(|node| {
            if (node.flags & ((1 << 8) | (1 << 9) | (1 << 10) | (1 << 11))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting direction flags",
                ));
            }
            if node.flags & (1 << 2) != 0 && node.flags & (1 << 3) != 0 {
                return Err(invalid_native("semantic node is both automatic and static"));
            }
            if (node.flags & ((1 << 13) | (1 << 14) | (1 << 15))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting definition-kind flags",
                ));
            }
            if node.flags & (1 << 29) != 0 && node.flags & (1 << 28) == 0 {
                return Err(invalid_native(
                    "semantic node has an open port connection without a connection",
                ));
            }
            if node.flags & (1 << 31) != 0 && node.kind != 21 {
                return Err(invalid_native(
                    "semantic with-clause flag is set on a non-method call",
                ));
            }
            validate_semantic_subkind(node.kind, node.subkind)?;
            validate_semantic_auxiliary(node)?;
            if (node.flags & ((1 << 16) | (1 << 17))).count_ones() > 1
                || (node.flags & ((1 << 18) | (1 << 19) | (1 << 20))).count_ones() > 1
                || (node.flags & ((1 << 21) | (1 << 22) | (1 << 23))).count_ones() > 1
            {
                return Err(invalid_native(
                    "semantic node has conflicting subtype flags",
                ));
            }
            if (node.flags & ((1 << 24) | (1 << 25) | (1 << 26))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting primitive-role flags",
                ));
            }
            let parent_id = (node.parent_id != INVALID_ID).then_some(node.parent_id);
            let target_id = (node.target_id != INVALID_ID).then_some(node.target_id);
            if parent_id.is_some_and(|id| !ids.contains(&id))
                || target_id.is_some_and(|id| !ids.contains(&id))
            {
                return Err(invalid_native(
                    "semantic node refers to an unknown semantic node",
                ));
            }
            let type_id = (node.type_id != INVALID_ID).then_some(node.type_id);
            if type_id.is_some_and(|id| !type_ids.contains(&id)) {
                return Err(invalid_native("semantic node type does not exist"));
            }
            let constant_id = (node.constant_id != INVALID_ID).then_some(node.constant_id);
            if constant_id
                .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
            {
                return Err(invalid_native("semantic node constant does not exist"));
            }
            let window = checked_window(
                node.edge_start,
                node.edge_count,
                edges.len(),
                "semantic node edges",
            )?;
            let mut edge_keys = HashSet::new();
            for edge in &edges[window] {
                if !edge_keys.insert((edge.role, edge.index)) {
                    return Err(invalid_native(
                        "semantic node has duplicate role/index edges",
                    ));
                }
            }
            let kind = decode_semantic_kind(node.kind)?;
            Ok(SemanticNode {
                id: node.id,
                parent_id,
                kind,
                subkind: node.subkind,
                operation: decode_semantic_operation(node.operation)?,
                is_bad: node.flags & 1 != 0,
                is_uninstantiated: node.flags & 2 != 0,
                is_automatic: node.flags & 4 != 0,
                is_static: node.flags & 8 != 0,
                is_top: node.flags & (1 << 4) != 0,
                is_implicit: node.flags & (1 << 5) != 0,
                is_local: node.flags & (1 << 6) != 0,
                is_nonblocking: node.flags & (1 << 7) != 0,
                is_input: node.flags & (1 << 8) != 0,
                is_output: node.flags & (1 << 9) != 0,
                is_inout: node.flags & (1 << 10) != 0,
                is_ref: node.flags & (1 << 11) != 0,
                is_const_ref: kind == SemanticKind::Argument
                    && node.auxiliary & ARGUMENT_CONST_REF != 0,
                is_ref_static: kind == SemanticKind::Argument
                    && node.auxiliary & ARGUMENT_REF_STATIC != 0,
                is_implicit_conversion: node.flags & (1 << 12) != 0,
                is_propagated_conversion: node.flags & (1 << 30) != 0,
                is_indexed_up: node.flags & (1 << 16) != 0,
                is_indexed_down: node.flags & (1 << 17) != 0,
                case_wildcard_x_or_z: node.flags & (1 << 18) != 0,
                case_wildcard_z: node.flags & (1 << 19) != 0,
                case_inside: node.flags & (1 << 20) != 0,
                is_posedge: node.flags & (1 << 21) != 0,
                is_negedge: node.flags & (1 << 22) != 0,
                is_both_edges: node.flags & (1 << 23) != 0,
                is_primitive_declaration: node.flags & (1 << 24) != 0,
                is_primitive_instance: node.flags & (1 << 25) != 0,
                is_primitive_port: node.flags & (1 << 26) != 0,
                is_task: node.flags & (1 << 27) != 0,
                port_connection_present: node.flags & (1 << 28) != 0,
                port_connection_open: node.flags & (1 << 29) != 0,
                method_with_clause: node.flags & (1 << 31) != 0,
                definition_kind: if node.flags & (1 << 13) != 0 {
                    Some(SemanticDefinitionKind::Module)
                } else if node.flags & (1 << 14) != 0 {
                    Some(SemanticDefinitionKind::Interface)
                } else if node.flags & (1 << 15) != 0 {
                    Some(SemanticDefinitionKind::Program)
                } else {
                    None
                },
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(node.name, "semantic node name")? },
                // SAFETY: native strings borrow from the live snapshot.
                detail: unsafe { copy_string(node.detail, "semantic node detail")? },
                // SAFETY: native strings borrow from the live snapshot.
                definition_name: unsafe {
                    copy_string(node.definition_name, "semantic node definition name")?
                },
                range: decode_range(node.range, files)?,
                type_id,
                constant_id,
                target_id,
                edge_start: node.edge_start,
                edge_count: node.edge_count,
                time_scale: decode_time_scale(node)?,
                strength0: decode_drive_strength(node.strength0)?,
                strength1: decode_drive_strength(node.strength1)?,
                auxiliary: node.auxiliary,
                assertion_range_min: node.assertion_range_min,
                assertion_range_max: (node.kind == 28 && node.assertion_range_max != u32::MAX)
                    .then_some(node.assertion_range_max),
                assertion_repetition_kind: node.assertion_repetition_kind,
            })
        })
        .collect()
}

fn decode_semantic_kind(raw: u32) -> Result<SemanticKind, SlangError> {
    Ok(match raw {
        1 => SemanticKind::Instance,
        2 => SemanticKind::Package,
        3 => SemanticKind::Class,
        4 => SemanticKind::GenerateScope,
        5 => SemanticKind::Port,
        6 => SemanticKind::Modport,
        7 => SemanticKind::InterfaceConnection,
        8 => SemanticKind::Net,
        9 => SemanticKind::Variable,
        10 => SemanticKind::Array,
        11 => SemanticKind::NamedEvent,
        12 => SemanticKind::Parameter,
        13 => SemanticKind::Process,
        14 => SemanticKind::ContinuousAssign,
        15 => SemanticKind::Primitive,
        16 => SemanticKind::Subroutine,
        17 => SemanticKind::Argument,
        18 => SemanticKind::Statement,
        19 => SemanticKind::Expression,
        20 => SemanticKind::SystemCall,
        21 => SemanticKind::MethodCall,
        22 => SemanticKind::FunctionCall,
        23 => SemanticKind::EnumConstant,
        24 => SemanticKind::Definition,
        25 => SemanticKind::Scope,
        26 => SemanticKind::TimingControl,
        27 => SemanticKind::NetAlias,
        28 => SemanticKind::AssertionExpr,
        255 => SemanticKind::Unsupported,
        _ => return Err(invalid_native("semantic node has an unknown kind")),
    })
}

fn validate_semantic_subkind(kind: u32, subkind: u32) -> Result<(), SlangError> {
    let valid = match kind {
        1 => matches!(subkind, 0 | 192 | 193),
        4 => matches!(subkind, 0 | 195 | 196),
        8 => matches!(subkind, 0 | 128..=141),
        13 => matches!(subkind, 0..=6),
        14 => matches!(subkind, 0 | 228),
        15 => matches!(subkind, 0 | 160..=164 | 200..=227),
        18 => matches!(subkind, 0 | 32..=67),
        19 => matches!(subkind, 0 | 64..=78 | 80..=91),
        25 => matches!(subkind, 0 | 194 | SEMANTIC_SCOPE_CLOCKING_BLOCK),
        26 => matches!(subkind, 0 | 112..=118),
        28 => matches!(subkind, 0..=13),
        20..=22 => matches!(subkind, 0 | 76),
        9 => matches!(
            subkind,
            0 | 229 | SEMANTIC_VARIABLE_CLOCKING | SEMANTIC_VARIABLE_ASSERTION_LOCAL
        ),
        2 | 3 | 5..=7 | 10..=12 | 16 | 17 | 23 | 24 | 27 | 255 => subkind == 0,
        _ => true,
    };
    if !valid {
        return Err(invalid_native(
            "semantic node has a subkind incompatible with its kind",
        ));
    }
    Ok(())
}

fn validate_semantic_auxiliary(node: &RawSemanticNode) -> Result<(), SlangError> {
    let valid = match (node.kind, node.subkind, node.operation) {
        // An incomplete declaration placeholder may not carry its resolved
        // lifetime yet; complete variable nodes use static or automatic.
        (9 | 11, _, _) if node.subkind == SEMANTIC_VARIABLE_CLOCKING => {
            node.auxiliary
                & !(CLOCKING_EDGE_MASK | (CLOCKING_EDGE_MASK << CLOCKING_VAR_OUTPUT_EDGE_SHIFT))
                == 0
        }
        (9 | 11, _, _) => matches!(node.auxiliary, 0..=2),
        (25, SEMANTIC_SCOPE_CLOCKING_BLOCK, _) => {
            let edge_mask = CLOCKING_EDGE_MASK << CLOCKING_INPUT_EDGE_SHIFT
                | CLOCKING_EDGE_MASK << CLOCKING_OUTPUT_EDGE_SHIFT;
            node.auxiliary & !(CLOCKING_BLOCK_DEFAULT | CLOCKING_BLOCK_GLOBAL | edge_mask) == 0
        }
        // Parameter auxiliary metadata carries the frontend's override bit.
        (12, _, _) => node.auxiliary <= 1,
        // Subroutine qualifiers carry method and DPI-C metadata. Context and
        // DPI purity are meaningful only for imports.
        (16, _, _) => {
            let allowed = SUBROUTINE_STATIC
                | SUBROUTINE_VIRTUAL
                | SUBROUTINE_PURE
                | SUBROUTINE_FINAL
                | SUBROUTINE_CONSTRUCTOR
                | SUBROUTINE_DPI_IMPORT
                | SUBROUTINE_DPI_CONTEXT
                | SUBROUTINE_DPI_PURE;
            node.auxiliary & !allowed == 0
                && (node.auxiliary & (SUBROUTINE_DPI_CONTEXT | SUBROUTINE_DPI_PURE) == 0
                    || node.auxiliary & SUBROUTINE_DPI_IMPORT != 0)
        }
        // Argument qualifiers carry const-ref and ref-static bits.
        (17, _, _) => node.auxiliary <= 3,
        // Statement subkind 42 covers both `wait` and `wait_order`; the
        // auxiliary marker distinguishes the ordered form. Conditional and
        // case statements use the same scalar for their qualifier.
        (18, 42, _) => node.auxiliary <= 1,
        // Immediate assertions reserve two bits to preserve deferred/final
        // syntax until the simulator can either execute or reject it.
        (18, 61..=63, _) => node.auxiliary <= 3,
        // Concurrent assertion expressions use a small set of owned flags;
        // unknown flags would make the property shape ambiguous downstream.
        (28, 1..=13, _) => {
            let flags_valid = node.auxiliary
                & !(SEMANTIC_ASSERTION_REPETITION
                    | SEMANTIC_ASSERTION_RANGE
                    | SEMANTIC_ASSERTION_STRONG
                    | SEMANTIC_ASSERTION_ABORT_REJECT
                    | SEMANTIC_ASSERTION_ABORT_SYNC)
                == 0;
            let kind_valid = matches!(node.assertion_repetition_kind, 0..=3)
                && (node.assertion_range_max == SEMANTIC_ASSERTION_RANGE_UNBOUNDED
                    || node.assertion_range_max >= node.assertion_range_min)
                && ((node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0)
                    == (node.assertion_repetition_kind != 0));
            flags_valid && kind_valid
        }
        // Foreach uses the auxiliary field for the number of source iterator
        // slots so omitted trailing dimensions survive the owned snapshot.
        // Keep the count bounded independently of the later DB allocation.
        (18, 59, _) => node.auxiliary <= 4096,
        (18, 33 | 34, _) => node.auxiliary <= SEMANTIC_UNIQUE_PRIORITY_PRIORITY,
        // Class qualifiers and `new super` are repository-owned flags.
        (3, _, _) => node.auxiliary & !(CLASS_ABSTRACT | CLASS_FINAL | CLASS_INTERFACE) == 0,
        (19, 87, _) => node.auxiliary & !NEW_CLASS_SUPER == 0,
        // A call qualified with `super` must bind directly to its declaring
        // base implementation instead of participating in virtual dispatch.
        (22, 76, _) => node.auxiliary & !CALL_SUPER == 0,
        (19, 69, 40) => node.auxiliary == 0 || node.flags & 1 != 0,
        (19, 69, 41) => node.auxiliary > 0 || node.flags & 1 != 0,
        _ => node.auxiliary == 0,
    };
    if !valid {
        return Err(invalid_native(
            "semantic node has invalid kind-specific auxiliary metadata",
        ));
    }
    Ok(())
}

fn decode_semantic_operation(raw: u32) -> Result<SemanticOperation, SlangError> {
    Ok(match raw {
        0 => SemanticOperation::None,
        1 => SemanticOperation::Plus,
        2 => SemanticOperation::Minus,
        3 => SemanticOperation::Multiply,
        4 => SemanticOperation::Divide,
        5 => SemanticOperation::Modulo,
        6 => SemanticOperation::Power,
        7 => SemanticOperation::BitNot,
        8 => SemanticOperation::BitAnd,
        9 => SemanticOperation::BitOr,
        10 => SemanticOperation::BitXor,
        11 => SemanticOperation::BitNand,
        12 => SemanticOperation::BitNor,
        13 => SemanticOperation::BitXnor,
        14 => SemanticOperation::LogicalNot,
        15 => SemanticOperation::LogicalAnd,
        16 => SemanticOperation::LogicalOr,
        17 => SemanticOperation::LogicalImplication,
        18 => SemanticOperation::LogicalEquivalence,
        19 => SemanticOperation::Equal,
        20 => SemanticOperation::NotEqual,
        21 => SemanticOperation::CaseEqual,
        22 => SemanticOperation::CaseNotEqual,
        23 => SemanticOperation::WildcardEqual,
        24 => SemanticOperation::WildcardNotEqual,
        25 => SemanticOperation::Greater,
        26 => SemanticOperation::GreaterEqual,
        27 => SemanticOperation::Less,
        28 => SemanticOperation::LessEqual,
        29 => SemanticOperation::ShiftLeft,
        30 => SemanticOperation::ShiftRight,
        31 => SemanticOperation::ArithmeticShiftLeft,
        32 => SemanticOperation::ArithmeticShiftRight,
        33 => SemanticOperation::PreIncrement,
        34 => SemanticOperation::PreDecrement,
        35 => SemanticOperation::PostIncrement,
        36 => SemanticOperation::PostDecrement,
        37 => SemanticOperation::Concat,
        38 => SemanticOperation::Replicate,
        39 => SemanticOperation::Conditional,
        40 => SemanticOperation::StreamLeft,
        41 => SemanticOperation::StreamRight,
        42 => SemanticOperation::Assign,
        43 => SemanticOperation::Inside,
        44 => SemanticOperation::AssignmentPattern,
        45 => SemanticOperation::MinTypMax,
        46 => SemanticOperation::MultiAssignmentPattern,
        47 => SemanticOperation::List,
        48 => SemanticOperation::AssertionAnd,
        49 => SemanticOperation::AssertionOr,
        50 => SemanticOperation::AssertionIntersect,
        51 => SemanticOperation::AssertionThroughout,
        52 => SemanticOperation::AssertionWithin,
        53 => SemanticOperation::AssertionIff,
        54 => SemanticOperation::AssertionUntil,
        55 => SemanticOperation::AssertionSUntil,
        56 => SemanticOperation::AssertionUntilWith,
        57 => SemanticOperation::AssertionSUntilWith,
        58 => SemanticOperation::AssertionImplies,
        59 => SemanticOperation::AssertionOverlappedImplies,
        60 => SemanticOperation::AssertionNonOverlappedImplies,
        61 => SemanticOperation::AssertionOverlappedFollowedBy,
        62 => SemanticOperation::AssertionNonOverlappedFollowedBy,
        63 => SemanticOperation::AssertionNot,
        64 => SemanticOperation::AssertionNextTime,
        65 => SemanticOperation::AssertionSNextTime,
        66 => SemanticOperation::AssertionAlways,
        67 => SemanticOperation::AssertionSAlways,
        68 => SemanticOperation::AssertionEventually,
        69 => SemanticOperation::AssertionSEventually,
        _ => return Err(invalid_native("semantic node has an unknown operation")),
    })
}

fn decode_time_scale(node: &RawSemanticNode) -> Result<Option<SemanticTimeScale>, SlangError> {
    let values = [
        node.time_unit,
        node.time_unit_magnitude,
        node.time_precision_unit,
        node.time_precision_magnitude,
    ];
    if values.iter().all(|value| *value == 0) {
        return Ok(None);
    }
    if values.contains(&0)
        || !matches!(node.time_unit_magnitude, 1 | 10 | 100)
        || !matches!(node.time_precision_magnitude, 1 | 10 | 100)
    {
        return Err(invalid_native("semantic node has an invalid time scale"));
    }
    let scale = SemanticTimeScale {
        unit: decode_time_unit(node.time_unit)?,
        magnitude: node.time_unit_magnitude,
        precision_unit: decode_time_unit(node.time_precision_unit)?,
        precision_magnitude: node.time_precision_magnitude,
    };
    if semantic_time_exponent(scale.precision_unit, scale.precision_magnitude)
        > semantic_time_exponent(scale.unit, scale.magnitude)
    {
        return Err(invalid_native(
            "semantic node time precision is coarser than its time unit",
        ));
    }
    Ok(Some(scale))
}

fn semantic_time_exponent(unit: SemanticTimeUnit, magnitude: u32) -> i32 {
    let base = match unit {
        SemanticTimeUnit::Seconds => 0,
        SemanticTimeUnit::Milliseconds => -3,
        SemanticTimeUnit::Microseconds => -6,
        SemanticTimeUnit::Nanoseconds => -9,
        SemanticTimeUnit::Picoseconds => -12,
        SemanticTimeUnit::Femtoseconds => -15,
    };
    base + match magnitude {
        1 => 0,
        10 => 1,
        100 => 2,
        _ => unreachable!("magnitude validated by decode_time_scale"),
    }
}

fn decode_drive_strength(value: u32) -> Result<SemanticDriveStrength, SlangError> {
    Ok(match value {
        0 => SemanticDriveStrength::Unspecified,
        1 => SemanticDriveStrength::Supply,
        2 => SemanticDriveStrength::Strong,
        3 => SemanticDriveStrength::Pull,
        4 => SemanticDriveStrength::Weak,
        5 => SemanticDriveStrength::HighZ,
        _ => {
            return Err(invalid_native(
                "semantic node has an unknown drive strength",
            ))
        }
    })
}

fn decode_time_unit(value: u32) -> Result<SemanticTimeUnit, SlangError> {
    Ok(match value {
        1 => SemanticTimeUnit::Seconds,
        2 => SemanticTimeUnit::Milliseconds,
        3 => SemanticTimeUnit::Microseconds,
        4 => SemanticTimeUnit::Nanoseconds,
        5 => SemanticTimeUnit::Picoseconds,
        6 => SemanticTimeUnit::Femtoseconds,
        _ => return Err(invalid_native("semantic node has an unknown time unit")),
    })
}

fn decode_lexical_tokens(
    raw: &[RawLexicalToken],
    files: &[File],
    semantic_nodes: &[SemanticNode],
) -> Result<Vec<LexicalToken>, SlangError> {
    let semantic_ids: HashSet<_> = semantic_nodes.iter().map(|node| node.id).collect();
    raw.iter()
        .map(|token| {
            if token.reserved != 0 || token.flags & !0b111 != 0 {
                return Err(invalid_native("lexical token has unknown flags"));
            }
            let semantic_id = (token.semantic_id != INVALID_ID).then_some(token.semantic_id);
            if semantic_id.is_some_and(|id| !semantic_ids.contains(&id)) {
                return Err(invalid_native("lexical token semantic id does not exist"));
            }
            Ok(LexicalToken {
                range: decode_range(token.range, files)?,
                kind: decode_lexical_kind(token.kind)?,
                role: match token.role {
                    0 => LexicalRole::None,
                    1 => LexicalRole::Declaration,
                    2 => LexicalRole::Reference,
                    3 => LexicalRole::ConnectionLabel,
                    4 => LexicalRole::Keyword,
                    5 => LexicalRole::ConnectionActual,
                    _ => return Err(invalid_native("lexical token has an unknown role")),
                },
                is_missing: token.flags & 1 != 0,
                is_skipped: token.flags & 2 != 0,
                is_macro_expansion: token.flags & 4 != 0,
                semantic_id,
                // SAFETY: native strings borrow from the live snapshot.
                text: unsafe { copy_string(token.text, "lexical token text")? },
            })
        })
        .collect()
}

fn decode_lexical_kind(raw: u32) -> Result<LexicalKind, SlangError> {
    Ok(match raw {
        0 => LexicalKind::Unknown,
        1 => LexicalKind::Module,
        2 => LexicalKind::Interface,
        3 => LexicalKind::Program,
        4 => LexicalKind::Package,
        5 => LexicalKind::Class,
        6 => LexicalKind::Struct,
        7 => LexicalKind::Union,
        8 => LexicalKind::Enum,
        9 => LexicalKind::EnumMember,
        10 => LexicalKind::TypeAlias,
        11 => LexicalKind::Parameter,
        12 => LexicalKind::Port,
        13 => LexicalKind::Variable,
        14 => LexicalKind::Net,
        15 => LexicalKind::Function,
        16 => LexicalKind::Task,
        17 => LexicalKind::Method,
        18 => LexicalKind::Macro,
        19 => LexicalKind::Keyword,
        20 => LexicalKind::String,
        21 => LexicalKind::Number,
        22 => LexicalKind::Operator,
        23 => LexicalKind::Identifier,
        24 => LexicalKind::Genvar,
        _ => return Err(invalid_native("lexical token has an unknown kind")),
    })
}

fn decode_related(
    raw: &[RawRelatedDiagnostic],
    files: &[File],
) -> Result<Vec<RelatedDiagnostic>, SlangError> {
    raw.iter()
        .map(|item| {
            Ok(RelatedDiagnostic {
                range: decode_range(item.range, files)?,
                // SAFETY: native strings borrow from the live snapshot.
                message: unsafe { copy_string(item.message, "related diagnostic message")? },
            })
        })
        .collect()
}

fn decode_diagnostics(
    raw: &[RawDiagnostic],
    related: &[RelatedDiagnostic],
    files: &[File],
) -> Result<Vec<Diagnostic>, SlangError> {
    raw.iter()
        .map(|item| {
            let related_range = checked_window(
                item.related_start,
                item.related_count,
                related.len(),
                "diagnostic related records",
            )?;
            Ok(Diagnostic {
                provider: match item.provider {
                    1 => DiagnosticProvider::Compilation,
                    2 => DiagnosticProvider::Analysis,
                    _ => return Err(invalid_native("diagnostic has an unknown provider")),
                },
                severity: match item.severity {
                    0 => DiagnosticSeverity::Ignored,
                    1 => DiagnosticSeverity::Note,
                    2 => DiagnosticSeverity::Warning,
                    3 => DiagnosticSeverity::Error,
                    4 => DiagnosticSeverity::Fatal,
                    _ => return Err(invalid_native("diagnostic has an unknown severity")),
                },
                subsystem: decode_diagnostic_subsystem(item.subsystem)?,
                code: item.code,
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "diagnostic name")? },
                // SAFETY: native strings borrow from the live snapshot.
                option_name: unsafe { copy_string(item.option_name, "diagnostic option name")? },
                // SAFETY: native strings borrow from the live snapshot.
                message: unsafe { copy_string(item.message, "diagnostic message")? },
                primary: decode_range(item.primary, files)?,
                related: related[related_range].to_vec(),
            })
        })
        .collect()
}

fn decode_diagnostic_subsystem(raw: u32) -> Result<DiagnosticSubsystem, SlangError> {
    Ok(match raw {
        0 => DiagnosticSubsystem::Invalid,
        1 => DiagnosticSubsystem::General,
        2 => DiagnosticSubsystem::Lexer,
        3 => DiagnosticSubsystem::Numeric,
        4 => DiagnosticSubsystem::Preprocessor,
        5 => DiagnosticSubsystem::Parser,
        6 => DiagnosticSubsystem::Declarations,
        7 => DiagnosticSubsystem::Expressions,
        8 => DiagnosticSubsystem::Statements,
        9 => DiagnosticSubsystem::Types,
        10 => DiagnosticSubsystem::Lookup,
        11 => DiagnosticSubsystem::SysFuncs,
        12 => DiagnosticSubsystem::ConstEval,
        13 => DiagnosticSubsystem::Compilation,
        14 => DiagnosticSubsystem::Analysis,
        15 => DiagnosticSubsystem::Meta,
        16 => DiagnosticSubsystem::Driver,
        17 => DiagnosticSubsystem::Tidy,
        18 => DiagnosticSubsystem::Netlist,
        _ => return Err(invalid_native("diagnostic has an unknown subsystem")),
    })
}

fn decode_instances(
    raw: &[RawInstance],
    files: &[File],
    parameter_len: usize,
) -> Result<Vec<Instance>, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|item| item.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid instance ids",
        ));
    }
    raw.iter()
        .map(|item| {
            if item.reserved != 0 {
                return Err(invalid_native("instance reserved field is nonzero"));
            }
            let parent_id = (item.parent_id != INVALID_ID).then_some(item.parent_id);
            if parent_id.is_some_and(|id| !ids.contains(&id)) {
                return Err(invalid_native("instance parent does not exist"));
            }
            checked_window(
                item.parameter_start,
                item.parameter_count,
                parameter_len,
                "instance parameters",
            )?;
            Ok(Instance {
                id: item.id,
                parent_id,
                kind: match item.kind {
                    1 => InstanceKind::Module,
                    2 => InstanceKind::Interface,
                    3 => InstanceKind::Program,
                    255 => InstanceKind::Unknown,
                    _ => return Err(invalid_native("instance has an unknown kind")),
                },
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "instance name")? },
                // SAFETY: native strings borrow from the live snapshot.
                definition_name: unsafe {
                    copy_string(item.definition_name, "instance definition name")?
                },
                declaration: decode_range(item.declaration, files)?,
                parameter_start: item.parameter_start,
                parameter_count: item.parameter_count,
            })
        })
        .collect()
}

type DecodedTypes = (Vec<Type>, Vec<TypeRange>, Vec<TypeMember>);

fn decode_types(
    raw: &[RawType],
    raw_ranges: &[RawTypeRange],
    raw_members: &[RawTypeMember],
) -> Result<DecodedTypes, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|item| item.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid type ids",
        ));
    }

    let ranges = raw_ranges
        .iter()
        .map(|range| {
            if range.reserved != 0 {
                return Err(invalid_native("type range reserved field is nonzero"));
            }
            let kind = match range.kind {
                1 => TypeRangeKind::Packed,
                2 => TypeRangeKind::Unpacked,
                3 => TypeRangeKind::QueueBound,
                _ => return Err(invalid_native("type range has an unknown kind")),
            };
            Ok(TypeRange {
                left: range.left,
                right: range.right,
                kind,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let members = raw_members
        .iter()
        .map(|member| {
            if !ids.contains(&member.type_id) {
                return Err(invalid_native("type member refers to an unknown type"));
            }
            Ok(TypeMember {
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(member.name, "type member name")? },
                type_id: member.type_id,
                bit_offset: member.bit_offset,
                bit_width: member.bit_width,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut claimed_ranges = vec![false; ranges.len()];
    let mut claimed_members = vec![false; members.len()];
    let types = raw
        .iter()
        .map(|item| {
            if item.flags & !0b111 != 0 {
                return Err(invalid_native("type contains unknown flags"));
            }
            for index in checked_window(
                item.range_start,
                item.range_count,
                ranges.len(),
                "type ranges",
            )? {
                if claimed_ranges[index] {
                    return Err(invalid_native("type range windows overlap"));
                }
                claimed_ranges[index] = true;
            }
            for index in checked_window(
                item.member_start,
                item.member_count,
                members.len(),
                "type members",
            )? {
                if claimed_members[index] {
                    return Err(invalid_native("type member windows overlap"));
                }
                claimed_members[index] = true;
            }
            let element_type_id =
                (item.element_type_id != INVALID_ID).then_some(item.element_type_id);
            let index_type_id = (item.index_type_id != INVALID_ID).then_some(item.index_type_id);
            if element_type_id.is_some_and(|id| !ids.contains(&id))
                || index_type_id.is_some_and(|id| !ids.contains(&id))
            {
                return Err(invalid_native("type refers to an unknown component type"));
            }
            Ok(Type {
                id: item.id,
                kind: match item.kind {
                    1 => TypeKind::Integral,
                    2 => TypeKind::Floating,
                    3 => TypeKind::String,
                    4 => TypeKind::Aggregate,
                    5 => TypeKind::Enum,
                    6 => TypeKind::PackedArray,
                    7 => TypeKind::FixedUnpackedArray,
                    8 => TypeKind::DynamicArray,
                    9 => TypeKind::AssociativeArray,
                    10 => TypeKind::Queue,
                    11 => TypeKind::PackedStruct,
                    12 => TypeKind::PackedUnion,
                    13 => TypeKind::UnpackedStruct,
                    14 => TypeKind::UnpackedUnion,
                    15 => TypeKind::Class,
                    16 => TypeKind::Chandle,
                    17 => TypeKind::Event,
                    18 => TypeKind::Void,
                    19 => TypeKind::VirtualInterface,
                    255 => TypeKind::Other,
                    _ => return Err(invalid_native("type has an unknown kind")),
                },
                is_signed: item.flags & 1 != 0,
                is_four_state: item.flags & 2 != 0,
                is_fixed_size: item.flags & 4 != 0,
                bit_width: item.bit_width,
                // SAFETY: native strings borrow from the live snapshot.
                display_name: unsafe { copy_string(item.display_name, "type display name")? },
                element_type_id,
                index_type_id,
                range_start: item.range_start,
                range_count: item.range_count,
                member_start: item.member_start,
                member_count: item.member_count,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if claimed_ranges.iter().any(|claimed| !claimed) {
        return Err(invalid_native("type range is not owned by a type"));
    }
    if claimed_members.iter().any(|claimed| !claimed) {
        return Err(invalid_native("type member is not owned by a type"));
    }
    Ok((types, ranges, members))
}

fn decode_constants(
    raw: &[RawConstant],
    words: &[u64],
    limits: &Limits,
) -> Result<Vec<Constant>, SlangError> {
    let mut total_value_bits = 0_u64;
    raw.iter()
        .map(|item| {
            let value = match item.kind {
                0 => ConstantValue::None,
                1 => {
                    if item.bit_width > limits.max_value_bits {
                        return Err(invalid_native("constant width exceeds max_value_bits"));
                    }
                    total_value_bits = total_value_bits
                        .checked_add(item.bit_width)
                        .ok_or_else(|| invalid_native("constant bit total overflowed"))?;
                    if total_value_bits > limits.max_value_bits {
                        return Err(invalid_native(
                            "constant bits exceed the configured max_value_bits",
                        ));
                    }
                    let expected_words = item.bit_width.div_ceil(64);
                    if item.word_count != expected_words {
                        return Err(invalid_native(
                            "integer constant word count does not match its width",
                        ));
                    }
                    let value_range = checked_window(
                        item.value_word_start,
                        item.word_count,
                        words.len(),
                        "constant value words",
                    )?;
                    let unknown_range = checked_window(
                        item.unknown_word_start,
                        item.word_count,
                        words.len(),
                        "constant unknown words",
                    )?;
                    let value_words = words[value_range].to_vec();
                    let unknown_words = words[unknown_range].to_vec();
                    if item.bit_width % 64 != 0 && !value_words.is_empty() {
                        let used = item.bit_width % 64;
                        let tail_mask = !0_u64 << used;
                        if value_words.last().is_some_and(|word| word & tail_mask != 0)
                            || unknown_words
                                .last()
                                .is_some_and(|word| word & tail_mask != 0)
                        {
                            return Err(invalid_native("integer constant has nonzero tail bits"));
                        }
                    }
                    ConstantValue::Integer {
                        is_signed: match item.is_signed {
                            0 => false,
                            1 => true,
                            _ => return Err(invalid_native("constant signedness is not boolean")),
                        },
                        bit_width: item.bit_width,
                        value_words,
                        unknown_words,
                    }
                }
                2 => {
                    if item.bit_width != 64 {
                        return Err(invalid_native("real constant does not have a 64-bit width"));
                    }
                    ConstantValue::Real(f64::from_bits(item.real_bits))
                }
                3 => {
                    if item.bit_width != 32 || item.real_bits >> 32 != 0 {
                        return Err(invalid_native(
                            "shortreal constant does not have a canonical 32-bit payload",
                        ));
                    }
                    ConstantValue::ShortReal(f32::from_bits(item.real_bits as u32))
                }
                4 => ConstantValue::String(
                    // SAFETY: native bytes borrow from the live snapshot.
                    unsafe { copy_bytes(item.text, "string constant")? },
                ),
                255 => ConstantValue::Other(
                    // SAFETY: native strings borrow from the live snapshot.
                    unsafe { copy_string(item.text, "constant display text")? },
                ),
                _ => return Err(invalid_native("constant has an unknown kind")),
            };
            Ok(Constant { value })
        })
        .collect()
}

fn decode_parameters(
    raw: &[RawParameter],
    files: &[File],
    instances: &[Instance],
    types: &[Type],
    constant_len: usize,
) -> Result<Vec<Parameter>, SlangError> {
    let instance_ids: HashSet<_> = instances.iter().map(|item| item.id).collect();
    let type_ids: HashSet<_> = types.iter().map(|item| item.id).collect();
    raw.iter()
        .map(|item| {
            if !instance_ids.contains(&item.owner_instance_id) {
                return Err(invalid_native("parameter owner instance does not exist"));
            }
            if item.flags & !0b11 != 0 {
                return Err(invalid_native("parameter contains unknown flags"));
            }
            let type_id = (item.type_id != INVALID_ID).then_some(item.type_id);
            if type_id.is_some_and(|id| !type_ids.contains(&id)) {
                return Err(invalid_native("parameter type does not exist"));
            }
            let constant_id = (item.constant_id != INVALID_ID).then_some(item.constant_id);
            if constant_id
                .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
            {
                return Err(invalid_native("parameter constant does not exist"));
            }
            Ok(Parameter {
                owner_instance_id: item.owner_instance_id,
                kind: match item.kind {
                    1 => ParameterKind::Value,
                    2 => ParameterKind::Type,
                    _ => return Err(invalid_native("parameter has an unknown kind")),
                },
                is_local: item.flags & 1 != 0,
                is_port: item.flags & 2 != 0,
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "parameter name")? },
                declaration: decode_range(item.declaration, files)?,
                type_id,
                constant_id,
            })
        })
        .collect()
}

fn validate_parameter_windows(
    instances: &[Instance],
    parameters: &[Parameter],
) -> Result<(), SlangError> {
    let mut claimed = vec![false; parameters.len()];
    for instance in instances {
        let window = checked_window(
            instance.parameter_start,
            instance.parameter_count,
            parameters.len(),
            "instance parameters",
        )?;
        for index in window {
            if claimed[index] || parameters[index].owner_instance_id != instance.id {
                return Err(invalid_native(
                    "instance parameter windows overlap or contain the wrong owner",
                ));
            }
            claimed[index] = true;
        }
    }
    if claimed.iter().any(|claimed| !claimed) {
        return Err(invalid_native(
            "parameter record is not covered by its owner instance window",
        ));
    }
    Ok(())
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
mod tests {
    use super::*;

    fn raw_integer(value_start: u64, unknown_start: u64) -> RawConstant {
        RawConstant {
            kind: 1,
            is_signed: 0,
            bit_width: 1,
            value_word_start: value_start,
            unknown_word_start: unknown_start,
            word_count: 1,
            real_bits: 0,
            text: empty_raw_string(),
        }
    }

    fn raw_type(id: u64) -> RawType {
        RawType {
            id,
            kind: 1,
            flags: 0b111,
            bit_width: 1,
            display_name: empty_raw_string(),
            element_type_id: INVALID_ID,
            index_type_id: INVALID_ID,
            range_start: 0,
            range_count: 0,
            member_start: 0,
            member_count: 0,
        }
    }

    fn raw_semantic_node(edge_count: u64) -> RawSemanticNode {
        RawSemanticNode {
            id: 0,
            parent_id: INVALID_ID,
            kind: 25,
            subkind: 0,
            operation: 0,
            flags: 0,
            name: empty_raw_string(),
            detail: empty_raw_string(),
            definition_name: empty_raw_string(),
            range: RawRange {
                file_id: INVALID_ID,
                start: 0,
                end: 0,
            },
            type_id: INVALID_ID,
            constant_id: INVALID_ID,
            target_id: INVALID_ID,
            edge_start: 0,
            edge_count,
            time_unit: 0,
            time_unit_magnitude: 0,
            time_precision_unit: 0,
            time_precision_magnitude: 0,
            strength0: 0,
            strength1: 0,
            auxiliary: 0,
            assertion_range_min: 0,
            assertion_range_max: 0,
            assertion_repetition_kind: 0,
        }
    }

    #[test]
    fn narrow_constants_charge_bits_without_double_charging_word_padding() {
        let raw = [raw_integer(0, 1), raw_integer(2, 3)];
        let words = [1, 0, 0, 0];
        let limits = Limits {
            max_value_bits: 2,
            ..Limits::default()
        };

        let constants = decode_constants(&raw, &words, &limits).expect("two one-bit values");
        assert_eq!(constants.len(), 2);
    }

    #[test]
    fn malformed_native_tags_and_related_windows_are_rejected() {
        let unknown_constant = RawConstant {
            kind: 99,
            is_signed: 0,
            bit_width: 0,
            value_word_start: INVALID_ID,
            unknown_word_start: INVALID_ID,
            word_count: 0,
            real_bits: 0,
            text: empty_raw_string(),
        };
        let error = decode_constants(&[unknown_constant], &[], &Limits::default())
            .expect_err("unknown constant tag must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let diagnostic = RawDiagnostic {
            provider: 1,
            severity: 2,
            subsystem: 0,
            code: 0,
            name: empty_raw_string(),
            option_name: empty_raw_string(),
            message: empty_raw_string(),
            primary: RawRange {
                file_id: INVALID_ID,
                start: 0,
                end: 0,
            },
            related_start: 1,
            related_count: 1,
        };
        let related = [RelatedDiagnostic {
            range: None,
            message: String::new(),
        }];
        let error = decode_diagnostics(&[diagnostic], &related, &[])
            .expect_err("out-of-bounds related window must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    }

    #[test]
    fn malformed_type_component_and_windows_are_rejected() {
        let mut ty = raw_type(0);
        ty.element_type_id = 17;
        let error = decode_types(&[ty], &[], &[]).expect_err("unknown element type must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut ty = raw_type(0);
        ty.range_count = 1;
        let error =
            decode_types(&[ty], &[], &[]).expect_err("out-of-bounds type range window must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut ty = raw_type(0);
        ty.member_count = 1;
        let member = RawTypeMember {
            name: empty_raw_string(),
            type_id: 9,
            bit_offset: 0,
            bit_width: 1,
        };
        let error = decode_types(&[ty], &[], &[member]).expect_err("unknown member type must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    }

    #[test]
    fn duplicate_semantic_role_index_is_rejected() {
        let node = raw_semantic_node(2);
        let raw_edges = [
            RawSemanticEdge {
                role: 1,
                index: 0,
                target_id: 0,
                sequence_delay_valid: 0,
                sequence_delay_min: 0,
                sequence_delay_max: 0,
            },
            RawSemanticEdge {
                role: 1,
                index: 0,
                target_id: 0,
                sequence_delay_valid: 0,
                sequence_delay_min: 0,
                sequence_delay_max: 0,
            },
        ];
        let edges = decode_semantic_edges(&raw_edges, std::slice::from_ref(&node))
            .expect("edge records decode before per-owner validation");
        let error = decode_semantic_nodes(&[node], &edges, &[], &[], 0)
            .expect_err("duplicate role/index pair must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    }

    #[test]
    fn invalid_semantic_strength_and_port_state_are_rejected() {
        let mut node = raw_semantic_node(0);
        node.strength0 = 6;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("unknown drive strength must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut node = raw_semantic_node(0);
        node.flags = 1 << 29;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("open port state without a present connection must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut node = raw_semantic_node(0);
        node.flags = 1 << 31;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("method with-clause flag on a scope must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut node = raw_semantic_node(0);
        node.subkind = 32;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("statement subkind on a scope must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut node = raw_semantic_node(0);
        node.time_unit = 4;
        node.time_unit_magnitude = 1;
        node.time_precision_unit = 2;
        node.time_precision_magnitude = 1;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("coarser time precision must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut node = raw_semantic_node(0);
        node.auxiliary = 1;
        let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
            .expect_err("variable lifetime metadata on a scope must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    }

    #[test]
    fn current_statement_and_expression_subkinds_are_admitted() {
        assert!(validate_semantic_subkind(18, 49).is_ok());
        assert!(validate_semantic_subkind(18, 59).is_ok());
        assert!(validate_semantic_subkind(18, 60).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_ASSERT).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_ASSUME).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_COVER).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_ASSERT).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_ASSUME).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_COVER).is_ok());
        assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_EXPECT).is_ok());
        assert!(validate_semantic_subkind(19, 86).is_ok());
        assert!(validate_semantic_subkind(19, 89).is_ok());
        assert!(validate_semantic_subkind(19, SEMANTIC_EXPR_ASSERTION_INSTANCE).is_ok());
        assert!(validate_semantic_subkind(19, SEMANTIC_EXPR_CLOCKING_EVENT).is_ok());
        assert!(validate_semantic_subkind(9, 229).is_ok());
        assert!(validate_semantic_subkind(25, SEMANTIC_SCOPE_CLOCKING_BLOCK).is_ok());
        assert!(validate_semantic_subkind(9, SEMANTIC_VARIABLE_CLOCKING).is_ok());
        assert!(validate_semantic_subkind(26, SEMANTIC_TIMING_ONE_STEP_DELAY).is_ok());
        assert!(validate_semantic_subkind(18, 68).is_err());
        assert!(validate_semantic_subkind(19, 79).is_err());
        assert!(validate_semantic_subkind(28, SEMANTIC_ASSERTION_EXPR_SIMPLE).is_ok());
        assert!(validate_semantic_subkind(28, SEMANTIC_ASSERTION_EXPR_DISABLE_IFF).is_ok());
        assert!(validate_semantic_subkind(28, 14).is_err());
        assert_eq!(
            decode_semantic_operation(47).expect("list operation must decode"),
            SemanticOperation::List
        );

        let mut method = raw_semantic_node(0);
        method.kind = 21;
        method.subkind = 76;
        method.flags = 1 << 31;
        let decoded = decode_semantic_nodes(&[method], &[], &[], &[], 0)
            .expect("method with-clause flag must decode on a method call");
        assert!(decoded[0].method_with_clause);

        let mut qualified = raw_semantic_node(0);
        qualified.kind = 18;
        qualified.subkind = 33;
        qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY;
        let decoded = decode_semantic_nodes(&[qualified], &[], &[], &[], 0)
            .expect("all repository-owned conditional qualifier tags must decode");
        assert_eq!(decoded[0].auxiliary, SEMANTIC_UNIQUE_PRIORITY_PRIORITY);
        qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY + 1;
        let error = decode_semantic_nodes(&[qualified], &[], &[], &[], 0)
            .expect_err("unknown conditional qualifier must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let mut variable = raw_semantic_node(0);
        variable.kind = 9;
        variable.auxiliary = 2;
        let decoded = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
            .expect("resolved variable lifetime must decode");
        assert_eq!(decoded[0].auxiliary, 2);

        variable.kind = 11;
        let decoded = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
            .expect("named-event variable lifetime must decode");
        assert_eq!(decoded[0].auxiliary, 2);

        let mut clocking_block = raw_semantic_node(0);
        clocking_block.kind = 25;
        clocking_block.subkind = SEMANTIC_SCOPE_CLOCKING_BLOCK;
        clocking_block.auxiliary = CLOCKING_BLOCK_DEFAULT
            | CLOCKING_BLOCK_GLOBAL
            | (3 << CLOCKING_INPUT_EDGE_SHIFT)
            | (2 << CLOCKING_OUTPUT_EDGE_SHIFT);
        let decoded = decode_semantic_nodes(&[clocking_block], &[], &[], &[], 0)
            .expect("clocking block metadata must decode");
        assert_eq!(decoded[0].auxiliary, clocking_block.auxiliary);

        let mut clocking_var = raw_semantic_node(0);
        clocking_var.kind = 9;
        clocking_var.subkind = SEMANTIC_VARIABLE_CLOCKING;
        clocking_var.auxiliary = 1 | (3 << CLOCKING_VAR_OUTPUT_EDGE_SHIFT);
        let decoded = decode_semantic_nodes(&[clocking_var], &[], &[], &[], 0)
            .expect("clocking variable metadata must decode");
        assert_eq!(decoded[0].auxiliary, clocking_var.auxiliary);

        variable.auxiliary = 3;
        let error = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
            .expect_err("unknown resolved variable lifetime must fail");
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

        let owner = raw_semantic_node(2);
        let edges = decode_semantic_edges(
            &[
                RawSemanticEdge {
                    role: 30,
                    index: 0,
                    target_id: 0,
                    sequence_delay_valid: 0,
                    sequence_delay_min: 0,
                    sequence_delay_max: 0,
                },
                RawSemanticEdge {
                    role: 31,
                    index: 0,
                    target_id: 0,
                    sequence_delay_valid: 0,
                    sequence_delay_min: 0,
                    sequence_delay_max: 0,
                },
            ],
            std::slice::from_ref(&owner),
        )
        .expect("semantic identity roles must decode");
        assert_eq!(edges[0].role, SemanticEdgeRole::SourceIdentity);
        assert_eq!(edges[1].role, SemanticEdgeRole::ReturnOwner);
    }

    #[test]
    fn semantic_and_type_table_limits_are_exact() {
        assert!(enforce_count(4, 4, "semantic nodes").is_ok());
        assert!(enforce_count(5, 4, "semantic nodes").is_err());
        assert!(enforce_count(4, 4, "semantic edges").is_ok());
        assert!(enforce_count(5, 4, "semantic edges").is_err());
        assert!(enforce_count(4, 4, "lexical tokens").is_ok());
        assert!(enforce_count(5, 4, "lexical tokens").is_err());
        assert!(enforce_count(4, 4, "type ranges").is_ok());
        assert!(enforce_count(5, 4, "type ranges").is_err());
        assert!(enforce_count(4, 4, "type members").is_ok());
        assert!(enforce_count(5, 4, "type members").is_err());
        assert!(enforce_count(4, 4, "constants").is_ok());
        assert!(enforce_count(5, 4, "constants").is_err());
    }

    #[test]
    fn language_edition_parser_and_default_are_explicit() {
        assert_eq!(
            LanguageEdition::default(),
            LanguageEdition::SystemVerilog2009
        );
        assert_eq!(LanguageEdition::Verilog2001.to_string(), "2001");
        assert_eq!(LanguageEdition::SystemVerilog2009.to_string(), "2009");
        assert_eq!(
            "1364-2001".parse::<LanguageEdition>(),
            Ok(LanguageEdition::Verilog2001)
        );
        assert_eq!(
            "1800-2009".parse::<LanguageEdition>(),
            Ok(LanguageEdition::SystemVerilog2009)
        );
        assert!("2017".parse::<LanguageEdition>().is_err());
        assert_eq!(
            LanguageEdition::from_snapshot_flags(SNAPSHOT_EDITION_VERILOG_2001),
            Ok(LanguageEdition::Verilog2001)
        );
        assert!(LanguageEdition::from_snapshot_flags(0).is_err());
        assert!(LanguageEdition::from_snapshot_flags(SNAPSHOT_EDITION_MASK).is_err());
    }
}
