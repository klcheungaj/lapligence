//! Definition search through path-mode include directories.
//!
//! After the given sources and their includes are admitted, names they
//! reference but nobody declares (instantiated modules, interfaces, programs
//! and primitives, imported or `::`-scoped packages and classes, interface
//! port types) are looked up in the `.v`/`.sv` files directly inside each
//! include directory. A file that declares a missing name is admitted as a
//! library unit of the default library, so it never becomes an implicit top,
//! and its own includes and missing names are processed the same way until no
//! more files are found. A missing name declared by several candidate files is
//! a startup error naming every file; a name declared nowhere is left to
//! Slang's unknown-definition diagnostic.
//!
//! Names come from the native parse-only metadata request
//! ([`slang::parse_metadata`]); candidate files are read with the same bounded,
//! handle-checked admission as other sources and scanned at most once, in one
//! batch, without their own includes.

use super::*;
use std::collections::BTreeSet;
use std::ffi::OsStr;

/// One `.v`/`.sv` file directly inside an include directory.
struct Candidate {
    name: String,
    target: AdmittedTarget,
    text: String,
    declared: BTreeSet<String>,
    referenced: BTreeSet<String>,
}

/// Mutable admission state shared with the path-mode admission in `compile`.
pub(super) struct SearchState<'a> {
    pub owned: &'a mut Vec<OwnedSource>,
    pub library_owned: &'a mut Vec<LibrarySource>,
    pub admitted_targets: &'a mut HashMap<PathBuf, AdmittedTarget>,
    pub identities: &'a mut HashSet<PathBuf>,
    pub source_count: &'a mut usize,
    pub remaining: &'a mut u64,
    pub expansion_budget: &'a MacroExpansionBudget,
    /// Macro state shared by one library's sources in merged mode.
    pub merged_library_macros: &'a mut HashMap<String, MacroEnvironment>,
}

fn is_definition_file(name: &OsStr) -> bool {
    matches!(
        Path::new(name).extension().and_then(OsStr::to_str),
        Some("v" | "sv")
    )
}

fn metadata_defines(opts: &CompileOpts) -> Vec<Define> {
    opts.defines
        .iter()
        .map(|value| parse_define(value))
        .collect()
}

fn metadata_error(error: slang::SlangError) -> StartupError {
    let mut error = startup_from_slang(error);
    error.message = format!(
        "include-directory definition search failed: {}",
        error.message
    );
    error
}

/// Include-only buffers already admitted, offered to every metadata parse
/// for cache-only include lookup.
fn include_buffers(owned: &[OwnedSource]) -> impl Iterator<Item = Source<'_>> {
    owned
        .iter()
        .filter(|source| !source.is_compilation_unit)
        .map(|source| Source {
            name: &source.name,
            text: &source.text,
            is_compilation_unit: false,
            is_library_map: false,
        })
}

/// Names the admitted sources declare and those they reference without a
/// declaration anywhere in the admitted set.
fn admitted_names(
    opts: &CompileOpts,
    metadata_dirs: &[String],
    state: &SearchState<'_>,
) -> Result<(HashSet<String>, BTreeSet<String>), StartupError> {
    let mut sources: Vec<Source<'_>> = state
        .owned
        .iter()
        .filter(|source| source.is_compilation_unit && !source.is_library_map)
        .map(|source| Source::compilation_unit(&source.name, &source.text))
        .collect();
    sources.extend(
        state
            .library_owned
            .iter()
            .filter(|source| !source.is_library_map)
            .map(|source| Source::compilation_unit(&source.name, &source.text)),
    );
    sources.extend(include_buffers(state.owned));
    let defines = metadata_defines(opts);
    let units = slang::parse_metadata(&slang::MetadataRequest {
        sources: &sources,
        defines: &defines,
        include_dirs: metadata_dirs,
        edition: opts.edition,
        compilation_unit_mode: opts.compilation_unit_mode,
        max_source_bytes: effective_source_byte_limit(opts.limits),
    })
    .map_err(metadata_error)?;
    let declared: HashSet<String> = units
        .iter()
        .flat_map(|unit| unit.declared.iter().cloned())
        .collect();
    let missing = units
        .into_iter()
        .flat_map(|unit| unit.referenced)
        .filter(|name| !declared.contains(name))
        .collect();
    Ok((declared, missing))
}

/// List the `.v`/`.sv` regular files directly inside each include directory
/// that are not already admitted, in directory order then file-name order,
/// once per canonical file.
fn list_candidates(
    include_dirs: &[String],
    identities: &HashSet<PathBuf>,
) -> Result<Vec<(String, AdmittedTarget)>, StartupError> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for dir in include_dirs {
        // Missing or non-directory include entries admit nothing, as for
        // `include lookup.
        let Ok(directory) = secure_fs::open_path(Path::new(dir)) else {
            continue;
        };
        if !directory.is_dir() {
            continue;
        }
        let read_error = |error: std::io::Error| {
            StartupError::new(
                StartupErrorKind::Input,
                format!(
                    "cannot read include directory {}: {error}",
                    directory.actual_path().display()
                ),
            )
        };
        let mut names = Vec::new();
        for entry in directory.read_dir().map_err(read_error)? {
            let name = entry.map_err(read_error)?;
            if is_definition_file(&name) {
                names.push(name);
            }
        }
        names.sort();
        for name in names {
            let Ok(child) = directory.open_child(&name) else {
                continue;
            };
            if !child.is_file() {
                continue;
            }
            let target = child.admitted_target();
            let path = target.actual_path().to_path_buf();
            if identities.contains(&path) || !seen.insert(path.clone()) {
                continue;
            }
            candidates.push((path.to_string_lossy().into_owned(), target));
        }
    }
    Ok(candidates)
}

/// Read every candidate (bounded by the source budget, not yet charged) and
/// learn its declared and referenced names in one metadata parse.
fn scan_candidates(
    opts: &CompileOpts,
    listed: Vec<(String, AdmittedTarget)>,
    metadata_dirs: &[String],
    state: &SearchState<'_>,
) -> Result<Vec<Candidate>, StartupError> {
    let mut candidates = Vec::new();
    for (name, target) in listed {
        let limit = state
            .remaining
            .checked_sub(name.len() as u64)
            .ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::LimitExceeded,
                    format!("source path {name} exceeds the configured Slang byte limit"),
                )
            })?;
        let text = read_bounded_target(&name, &target, limit, "SystemVerilog source")?;
        candidates.push(Candidate {
            name,
            target,
            text,
            declared: BTreeSet::new(),
            referenced: BTreeSet::new(),
        });
    }
    if candidates.is_empty() {
        return Ok(candidates);
    }
    let mut sources: Vec<Source<'_>> = candidates
        .iter()
        .map(|candidate| Source::compilation_unit(&candidate.name, &candidate.text))
        .collect();
    sources.extend(include_buffers(state.owned));
    let defines = metadata_defines(opts);
    let units = slang::parse_metadata(&slang::MetadataRequest {
        sources: &sources,
        defines: &defines,
        include_dirs: metadata_dirs,
        edition: opts.edition,
        compilation_unit_mode: CompilationUnitMode::Separate,
        max_source_bytes: effective_source_byte_limit(opts.limits),
    })
    .map_err(metadata_error)?;
    for (candidate, unit) in candidates.iter_mut().zip(units) {
        candidate.declared = unit.declared;
        candidate.referenced = unit.referenced;
    }
    Ok(candidates)
}

/// Admit one found file as a default-library unit and admit its includes.
fn admit_found_file(
    opts: &CompileOpts,
    include_dirs: &[String],
    library_include_dirs: &[LibraryIncludeDir],
    library: &str,
    candidate: &Candidate,
    state: &mut SearchState<'_>,
) -> Result<(), StartupError> {
    if *state.source_count >= effective_source_count_limit(opts.limits) {
        return Err(StartupError::new(
            StartupErrorKind::LimitExceeded,
            "include-directory definition search exceeds the configured Slang source limit",
        ));
    }
    let bytes = (candidate.name.len() as u64)
        .checked_add(candidate.text.len() as u64)
        .and_then(|bytes| bytes.checked_add(library.len() as u64))
        .filter(|bytes| *bytes <= *state.remaining)
        .ok_or_else(|| {
            StartupError::new(
                StartupErrorKind::LimitExceeded,
                format!(
                    "source path {} exceeds the configured Slang byte limit",
                    candidate.name
                ),
            )
        })?;
    *state.remaining -= bytes;
    *state.source_count += 1;
    let path = candidate.target.actual_path().to_path_buf();
    state.identities.insert(path.clone());
    state
        .admitted_targets
        .insert(path.clone(), candidate.target.clone());
    state.library_owned.push(LibrarySource::new(
        candidate.name.clone(),
        candidate.text.clone(),
        library,
    ));

    // Includes follow the library-source rules in `compile`: merged mode
    // shares one macro environment per library in admission order.
    let merged = matches!(opts.compilation_unit_mode, CompilationUnitMode::Merged);
    let mut macros = if merged {
        state
            .merged_library_macros
            .remove(library)
            .unwrap_or_else(|| macro_environment_from_defines(&opts.defines))
    } else {
        macro_environment_from_defines(&opts.defines)
    };
    let mut dirs = include_dirs.to_vec();
    dirs.extend(
        library_include_dirs
            .iter()
            .filter(|dir| dir.library == library)
            .map(|dir| dir.path.clone()),
    );
    let mut include_stack = vec![path];
    admit_macro_includes(
        &candidate.name,
        &candidate.text,
        opts,
        &dirs,
        &mut macros,
        Some(&candidate.target),
        state.admitted_targets,
        state.identities,
        state.owned,
        state.source_count,
        state.remaining,
        &mut include_stack,
        state.expansion_budget,
        0,
    )?;
    if merged {
        state
            .merged_library_macros
            .insert(library.to_owned(), macros);
    }
    Ok(())
}

/// Admit include-directory files that declare definitions the admitted
/// sources reference but do not declare. See the module documentation.
pub(super) fn admit_include_dir_definitions(
    opts: &CompileOpts,
    include_dirs: &[String],
    library_include_dirs: &[LibraryIncludeDir],
    state: &mut SearchState<'_>,
) -> Result<(), StartupError> {
    let mut metadata_dirs = include_dirs.to_vec();
    metadata_dirs.extend(library_include_dirs.iter().map(|dir| dir.path.clone()));
    // Listing is cheap; the admitted sources are parsed for their names only
    // when some include directory holds a candidate file.
    let listed = list_candidates(include_dirs, state.identities)?;
    if listed.is_empty() {
        return Ok(());
    }
    let (mut known, mut missing) = admitted_names(opts, &metadata_dirs, state)?;
    if missing.is_empty() {
        return Ok(());
    }
    let candidates = scan_candidates(opts, listed, &metadata_dirs, state)?;
    let library = opts.default_library.as_deref().unwrap_or("work").to_owned();
    let mut admitted = vec![false; candidates.len()];
    while !missing.is_empty() {
        let mut found = Vec::new();
        for name in &missing {
            let declaring: Vec<usize> = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.declared.contains(name))
                .map(|(index, _)| index)
                .collect();
            match declaring.as_slice() {
                [] => {}
                [index] => {
                    if !found.contains(index) {
                        found.push(*index);
                    }
                }
                _ => {
                    let files = declaring
                        .iter()
                        .map(|index| candidates[*index].name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(StartupError::new(
                        StartupErrorKind::InvalidArgument,
                        format!(
                            "definition `{name}` is declared by more than one file in the include directories: {files}"
                        ),
                    ));
                }
            }
        }
        found.sort_unstable();
        let mut next = BTreeSet::new();
        for index in found {
            let candidate = &candidates[index];
            // A file a previously found file includes is already admitted
            // as an include and declares its names through that unit.
            if admitted[index] || state.identities.contains(candidate.target.actual_path()) {
                continue;
            }
            admitted[index] = true;
            admit_found_file(
                opts,
                include_dirs,
                library_include_dirs,
                &library,
                candidate,
                state,
            )?;
            known.extend(candidate.declared.iter().cloned());
            next.extend(candidate.referenced.iter().cloned());
        }
        missing = next
            .into_iter()
            .filter(|name| !known.contains(name))
            .collect();
    }
    Ok(())
}
