//! Parse-only definition metadata (ABI v16 `llg_slang_parse_metadata`).
//!
//! Path-mode admission uses it to learn which definitions (modules,
//! interfaces, programs, packages, primitives, checkers, classes) each admitted
//! buffer declares and references, without elaborating. The native side parses
//! with the compile's edition and defines and reads includes only from the
//! admitted include-only buffers, exactly like [`compile`](super::compile).

use super::*;
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

const METADATA_DECLARED: u32 = 1;
const METADATA_REFERENCED: u32 = 2;

/// Borrowed inputs of one blocking metadata parse.
#[derive(Debug, Clone, Copy)]
pub struct MetadataRequest<'a> {
    /// Compilation units to parse and include-only buffers for cache-only
    /// include lookup. Library-map sources are rejected.
    pub sources: &'a [Source<'a>],
    pub defines: &'a [Define],
    /// Logical include lookup prefixes; they authorize no filesystem reads.
    pub include_dirs: &'a [String],
    pub edition: LanguageEdition,
    /// Merged mode parses all compilation units as one tree and reports its
    /// names under the first unit.
    pub compilation_unit_mode: CompilationUnitMode,
    pub max_source_bytes: u64,
}

/// Definition names of one compilation-unit source, in sorted order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinitionNames {
    /// Outermost definitions the unit declares.
    pub declared: BTreeSet<String>,
    /// Definitions the unit uses but does not declare itself.
    pub referenced: BTreeSet<String>,
}

/// Mirrors `LlgSlangMetadataRequest`.
#[repr(C)]
pub(super) struct RawMetadataRequest {
    abi_version: u32,
    flags: u32,
    sources: *const RawSource,
    source_count: u64,
    defines: *const RawDefine,
    define_count: u64,
    include_dirs: *const RawString,
    include_dir_count: u64,
    max_source_bytes: u64,
}

/// Mirrors `LlgSlangMetadataSink`.
#[repr(C)]
pub(super) struct RawMetadataSink {
    context: *mut c_void,
    name: unsafe extern "C" fn(*mut c_void, u64, u32, RawString) -> u32,
}

struct Receiver {
    units: Vec<DefinitionNames>,
    error: Option<SlangError>,
}

impl Receiver {
    fn accept(&mut self, source_index: u64, role: u32, name: RawString) -> Result<(), SlangError> {
        let unit = usize::try_from(source_index)
            .ok()
            .and_then(|index| self.units.get_mut(index))
            .ok_or_else(|| invalid_native("metadata name refers to an unknown source"))?;
        // SAFETY: the bridge passes a view valid for this callback.
        let name = unsafe { copy_string(name, "metadata definition name") }?;
        if name.is_empty() {
            return Err(invalid_native("metadata definition name is empty"));
        }
        let names = match role {
            METADATA_DECLARED => &mut unit.declared,
            METADATA_REFERENCED => &mut unit.referenced,
            _ => return Err(invalid_native("unknown metadata name role")),
        };
        if !names.insert(name) {
            return Err(invalid_native("metadata name was delivered twice"));
        }
        Ok(())
    }
}

/// # Safety
/// `context` must be the exclusive [`Receiver`] reserved for the blocking
/// call, and `name` must be valid for this call.
unsafe extern "C" fn sink_name(
    context: *mut c_void,
    source_index: u64,
    role: u32,
    name: RawString,
) -> u32 {
    if context.is_null() {
        return stream::SINK_ABORT;
    }
    // SAFETY: the caller reserved `context` as the live, exclusive receiver.
    let receiver = unsafe { &mut *context.cast::<Receiver>() };
    if receiver.error.is_some() {
        return stream::SINK_ABORT;
    }
    // No unwind may cross the C callback boundary.
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        receiver.accept(source_index, role, name)
    }));
    let error = match outcome {
        Ok(Ok(())) => return stream::SINK_CONTINUE,
        Ok(Err(error)) => error,
        Err(_) => SlangError::new(SlangErrorKind::Internal, "metadata receiver panicked"),
    };
    receiver.error = Some(error);
    stream::SINK_ABORT
}

fn validate_metadata_request(request: &MetadataRequest<'_>) -> Result<(), SlangError> {
    if request.sources.len() > MAX_SOURCES {
        return Err(limit_exceeded("source count exceeds the native limit"));
    }
    if request.defines.len() > MAX_DEFINES {
        return Err(limit_exceeded("define count exceeds the native limit"));
    }
    if request.include_dirs.len() > MAX_INCLUDE_DIRS {
        return Err(limit_exceeded(
            "include directory count exceeds the native limit",
        ));
    }
    if request.max_source_bytes == 0 {
        return Err(invalid_argument(
            "metadata source byte limit must be positive",
        ));
    }
    let mut names = HashSet::with_capacity(request.sources.len());
    let mut total = 0_u64;
    for source in request.sources {
        validate_name(source.name, "source name")?;
        if source.is_library_map {
            return Err(invalid_argument(
                "metadata requests do not accept library map sources",
            ));
        }
        if !names.insert(source.name) {
            return Err(invalid_argument("source names must be unique"));
        }
        total = total
            .checked_add(source.name.len() as u64)
            .and_then(|total| total.checked_add(source.text.len() as u64))
            .ok_or_else(|| limit_exceeded("source byte count overflowed"))?;
        if total > request.max_source_bytes {
            return Err(limit_exceeded("source bytes exceed max_source_bytes"));
        }
    }
    for define in request.defines {
        validate_name(&define.name, "define name")?;
        if define
            .value
            .as_deref()
            .is_some_and(|value| value.contains('\0'))
        {
            return Err(invalid_argument("define value contains a NUL byte"));
        }
    }
    for dir in request.include_dirs {
        if dir.contains('\0') {
            return Err(invalid_argument("include directory contains a NUL byte"));
        }
    }
    Ok(())
}

/// Parse `request.sources` without elaborating and return the definition
/// names of each source, indexed like `request.sources`. Include-only entries
/// (and, in merged mode, every unit after the first) report no names.
pub fn parse_metadata(request: &MetadataRequest<'_>) -> Result<Vec<DefinitionNames>, SlangError> {
    validate_metadata_request(request)?;
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
    let raw_include_dirs: Vec<_> = request
        .include_dirs
        .iter()
        .map(|dir| raw_string(dir))
        .collect();
    let raw_request = RawMetadataRequest {
        abi_version: ABI_VERSION,
        flags: request.edition.compile_flag() | request.compilation_unit_mode.compile_flag(),
        sources: raw_sources.as_ptr(),
        source_count: raw_sources.len() as u64,
        defines: raw_defines.as_ptr(),
        define_count: raw_defines.len() as u64,
        include_dirs: raw_include_dirs.as_ptr(),
        include_dir_count: raw_include_dirs.len() as u64,
        max_source_bytes: request.max_source_bytes,
    };
    let mut receiver = Receiver {
        units: vec![DefinitionNames::default(); request.sources.len()],
        error: None,
    };
    let sink = RawMetadataSink {
        context: (&mut receiver as *mut Receiver).cast(),
        name: sink_name,
    };
    let mut error = ptr::null_mut();
    // SAFETY: every request pointer refers to a live vector or borrowed string
    // that outlives this blocking call; the sink context is `receiver`, which
    // is not otherwise touched until the call returns; the error output
    // pointer is writable.
    let status = unsafe { llg_slang_parse_metadata(&raw_request, &sink, &mut error) };
    let error = ErrorOwner(error);
    if let Some(receiver_error) = receiver.error.take() {
        return Err(receiver_error);
    }
    if status != STATUS_OK {
        return Err(take_native_error(status, error));
    }
    if !error.0.is_null() {
        return Err(invalid_native(
            "successful metadata parse returned an unexpected error owner",
        ));
    }
    Ok(receiver.units)
}
