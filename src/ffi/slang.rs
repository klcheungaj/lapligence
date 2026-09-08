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

const ABI_VERSION: u32 = 1;
const INVALID_ID: u64 = u64::MAX;

const STATUS_OK: u32 = 0;
const STATUS_INVALID_ARGUMENT: u32 = 1;
const STATUS_LIMIT_EXCEEDED: u32 = 2;
const STATUS_FRONTEND_ERROR: u32 = 3;
const STATUS_INTERNAL_ERROR: u32 = 4;

const SNAPSHOT_HAS_ERRORS: u32 = 1 << 0;
const SNAPSHOT_ANALYSIS_RAN: u32 = 1 << 1;
const SNAPSHOT_KNOWN_FLAGS: u32 = SNAPSHOT_HAS_ERRORS | SNAPSHOT_ANALYSIS_RAN;
const MAX_SOURCES: usize = 4_096;
const MAX_DEFINES: usize = 4_096;
const MAX_TOP_MODULES: usize = 4_096;
const MAX_INCLUDE_DIRS: usize = 4_096;
const MAX_PARAMETER_OVERRIDES: usize = 4_096;
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
            max_output_bytes: 128 * 1024 * 1024,
        }
    }
}

/// Typed options for an in-memory Slang compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileOptions {
    /// Preprocessor definitions applied before parsing compilation units.
    pub defines: Vec<Define>,
    /// Explicit top module names; empty lets Slang select tops.
    pub top_modules: Vec<String>,
    /// Logical lookup prefixes for admitted include buffers. These paths do
    /// not authorize filesystem reads because the shim uses cache-only mode.
    pub include_dirs: Vec<String>,
    /// Top-level elaboration parameter overrides.
    pub parameter_overrides: Vec<ParameterOverride>,
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
    pub subsystem: u32,
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
    Other,
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

/// Fully owned observations from one Slang compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    flags: u32,
    pub files: Vec<File>,
    pub diagnostics: Vec<Diagnostic>,
    pub instances: Vec<Instance>,
    pub parameters: Vec<Parameter>,
    pub types: Vec<Type>,
    pub constants: Vec<Constant>,
}

impl Snapshot {
    /// Slang's authoritative compilation-error flag.
    pub fn has_errors(&self) -> bool {
        self.flags & SNAPSHOT_HAS_ERRORS != 0
    }

    pub fn analysis_ran(&self) -> bool {
        self.flags & SNAPSHOT_ANALYSIS_RAN != 0
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
    let limits = request.options.limits;
    let raw_request = RawCompileRequest {
        abi_version: ABI_VERSION,
        flags: 0,
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
    let decoded = decode_snapshot(&snapshot, &limits)?;
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
    enforce_count(view.constant_count, limits.max_parameters, "constants")?;

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
        });
    }

    let related = decode_related(raw_related, &files)?;
    let diagnostics = decode_diagnostics(raw_diagnostics, &related, &files)?;
    let types = decode_types(raw_types)?;
    let constants = decode_constants(raw_constants, value_words, limits)?;
    let instances = decode_instances(raw_instances, &files, raw_parameters.len())?;
    let parameters =
        decode_parameters(raw_parameters, &files, &instances, &types, constants.len())?;
    validate_parameter_windows(&instances, &parameters)?;

    drop(unexpected_error);
    Ok(Snapshot {
        flags: view.flags,
        files,
        diagnostics,
        instances,
        parameters,
        types,
        constants,
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
                subsystem: item.subsystem,
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

fn decode_types(raw: &[RawType]) -> Result<Vec<Type>, SlangError> {
    let mut ids = HashSet::with_capacity(raw.len());
    raw.iter()
        .map(|item| {
            if item.id == INVALID_ID || !ids.insert(item.id) {
                return Err(invalid_native(
                    "snapshot contains duplicate or invalid type ids",
                ));
            }
            if item.flags & !0b111 != 0 {
                return Err(invalid_native("type contains unknown flags"));
            }
            Ok(Type {
                id: item.id,
                kind: match item.kind {
                    1 => TypeKind::Integral,
                    2 => TypeKind::Floating,
                    3 => TypeKind::String,
                    4 => TypeKind::Aggregate,
                    255 => TypeKind::Other,
                    _ => return Err(invalid_native("type has an unknown kind")),
                },
                is_signed: item.flags & 1 != 0,
                is_four_state: item.flags & 2 != 0,
                is_fixed_size: item.flags & 4 != 0,
                bit_width: item.bit_width,
                // SAFETY: native strings borrow from the live snapshot.
                display_name: unsafe { copy_string(item.display_name, "type display name")? },
            })
        })
        .collect()
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
}
