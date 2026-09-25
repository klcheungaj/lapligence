//! Resolve all authorized map matches before assigning source-library identity.
//!
//! V 13.2.1.1 / SV 33.3.1.1 rank the final path component, not the
//! specificity of its parent directories. A later higher-priority match can
//! resolve an earlier tie; only ties at the final winning rank are errors.

use super::{
    charge_key_comparison_work, charge_library_map_clone, charge_library_source_metadata,
    logical_path_key, text_has_wildcard, LibraryMapWorkBudget, LibrarySource, OwnedSource,
    StartupError, StartupErrorKind, MAX_LIBRARY_PATH_BYTES,
};
use std::borrow::Cow;
use std::collections::{hash_map, HashMap};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LibrarySpecificity {
    Directory,
    WildcardFile,
    ExplicitFile,
    ExplicitAssignment,
}

impl LibrarySpecificity {
    pub(super) fn of_pattern(pattern: &str) -> Self {
        if pattern.ends_with(['/', '\\']) {
            Self::Directory
        } else if text_has_wildcard(pattern.rsplit(['/', '\\']).next().unwrap_or(pattern)) {
            Self::WildcardFile
        } else {
            Self::ExplicitFile
        }
    }
}

/// A trailing directory separator means its immediate files, not recursion.
/// Keep the original spelling's Directory rank rather than promoting it to a
/// wildcard-file match just because expansion uses `*` internally.
pub(super) fn library_match_pattern<'a>(
    pattern: &'a str,
    work: &mut LibraryMapWorkBudget,
) -> Result<Cow<'a, str>, StartupError> {
    if !pattern.ends_with(['/', '\\']) {
        return Ok(Cow::Borrowed(pattern));
    }
    let length = pattern
        .len()
        .checked_add(1)
        .ok_or_else(|| work.limit_error("directory pattern"))?;
    if length > MAX_LIBRARY_PATH_BYTES {
        return Err(work.limit_error("directory pattern path bytes"));
    }
    work.charge_usize(length, "directory pattern expansion")?;
    work.charge_allocation_usize(length, "directory pattern expansion")?;
    Ok(Cow::Owned(format!("{pattern}*")))
}

struct Choice {
    library: String,
    rank: LibrarySpecificity,
    conflict: Option<String>,
}

struct Entry {
    name: String,
    choice: Option<Choice>,
    order: usize,
}

/// Retains caller-owned buffers and their first discovery order. Disk and
/// logical map collectors share this state, so neither can prematurely turn a
/// provisional map winner into an explicit override for the other collector.
pub(super) struct LibraryMapBuffers<'a> {
    pub(super) sources: &'a mut Vec<OwnedSource>,
    pub(super) libraries: &'a mut Vec<LibrarySource>,
    entries: HashMap<Vec<String>, Entry>,
    next_order: usize,
}

impl<'a> LibraryMapBuffers<'a> {
    pub(super) fn new(
        sources: &'a mut Vec<OwnedSource>,
        libraries: &'a mut Vec<LibrarySource>,
        work: &mut LibraryMapWorkBudget,
    ) -> Result<Self, StartupError> {
        let mut result = Self {
            sources,
            libraries,
            entries: HashMap::new(),
            next_order: 0,
        };
        for source in result.sources.iter() {
            let key = logical_path_key(Path::new(&source.name), work, "map source identity")?;
            charge_library_map_clone(work, &source.name, "map source name")?;
            if result
                .entries
                .insert(
                    key,
                    Entry {
                        name: source.name.clone(),
                        choice: None,
                        order: usize::MAX,
                    },
                )
                .is_some()
            {
                return Err(duplicate_source(&source.name));
            }
        }
        for source in result.libraries.iter() {
            let key = logical_path_key(Path::new(&source.name), work, "explicit library identity")?;
            charge_library_map_clone(work, &source.name, "explicit library source name")?;
            charge_library_map_clone(work, &source.library, "explicit library name")?;
            if result
                .entries
                .insert(
                    key,
                    Entry {
                        name: source.name.clone(),
                        choice: Some(Choice {
                            library: source.library.clone(),
                            rank: LibrarySpecificity::ExplicitAssignment,
                            conflict: None,
                        }),
                        order: usize::MAX,
                    },
                )
                .is_some()
            {
                return Err(duplicate_source(&source.name));
            }
        }
        Ok(result)
    }

    /// Return whether the source bytes were already admitted. Only a disk
    /// collector can offer a new name, and it must read and retain those bytes
    /// before offering another candidate. Any read failure aborts admission.
    pub(super) fn offer(
        &mut self,
        name: &str,
        library: &str,
        rank: LibrarySpecificity,
        work: &mut LibraryMapWorkBudget,
    ) -> Result<bool, StartupError> {
        let key = logical_path_key(Path::new(name), work, "map candidate identity")?;
        let (entry, known) = match self.entries.entry(key) {
            hash_map::Entry::Occupied(entry) => (entry.into_mut(), true),
            hash_map::Entry::Vacant(entry) => {
                charge_library_map_clone(work, name, "map candidate source name")?;
                (
                    entry.insert(Entry {
                        name: name.to_owned(),
                        choice: None,
                        order: usize::MAX,
                    }),
                    false,
                )
            }
        };
        if entry.order == usize::MAX {
            work.charge(1, "map candidate discovery order")?;
            entry.order = self.next_order;
            self.next_order = self
                .next_order
                .checked_add(1)
                .ok_or_else(|| work.limit_error("map candidate discovery order"))?;
        }
        if let Some(choice) = &mut entry.choice {
            if rank < choice.rank {
                return Ok(known);
            }
            if rank == choice.rank {
                work.charge_usize(choice.library.len(), "map library comparison")?;
                work.charge_usize(library.len(), "map library comparison")?;
                if choice.library != library && choice.conflict.is_none() {
                    charge_library_map_clone(work, library, "map conflicting library")?;
                    choice.conflict = Some(library.to_owned());
                }
                return Ok(known);
            }
        }
        charge_library_map_clone(work, library, "map winning library")?;
        entry.choice = Some(Choice {
            library: library.to_owned(),
            rank,
            conflict: None,
        });
        Ok(known)
    }

    pub(super) fn finish(
        self,
        remaining: &mut u64,
        work: &mut LibraryMapWorkBudget,
    ) -> Result<(), StartupError> {
        charge_key_comparison_work(
            work,
            self.entries.values().map(|entry| entry.name.len()),
            true,
            "map ambiguity ordering comparisons",
        )?;
        charge_workspace::<&Entry>(work, self.entries.len(), "map ambiguity workspace")?;
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        for entry in entries {
            if let Some(Choice {
                library,
                conflict: Some(conflict),
                ..
            }) = &entry.choice
            {
                return Err(StartupError::new(
                    StartupErrorKind::InvalidArgument,
                    format!(
                        "ambiguous library mapping for {}: {library} and {conflict} have equal precedence",
                        entry.name,
                    ),
                ));
            }
        }

        charge_workspace::<(usize, usize, String)>(
            work,
            self.sources.len(),
            "map publication workspace",
        )?;
        let mut selected = Vec::with_capacity(self.sources.len());
        for (index, source) in self.sources.iter().enumerate() {
            let key = logical_path_key(Path::new(&source.name), work, "map source publication")?;
            let entry = self.entries.get(&key).ok_or_else(|| {
                StartupError::new(
                    StartupErrorKind::Internal,
                    format!("library map source has no candidate record: {}", source.name),
                )
            })?;
            let Some(choice) = &entry.choice else {
                continue;
            };
            // This charge precedes removal, including when a caller exhausts
            // its budget immediately before the selected buffer is moved.
            work.charge(1, "in-memory library source admission")?;
            charge_library_source_metadata(remaining, &choice.library)?;
            charge_library_map_clone(work, &choice.library, "map published library")?;
            selected.push((entry.order, index, choice.library.clone()));
        }
        charge_key_comparison_work(
            work,
            selected.iter().map(|_| std::mem::size_of::<usize>()),
            true,
            "map publication ordering",
        )?;
        selected.sort_unstable_by_key(|(order, _, _)| *order);
        let amount = self
            .sources
            .len()
            .checked_mul(3)
            .ok_or_else(|| work.limit_error("map source removal"))?;
        work.charge_usize(amount, "map source removal")?;
        charge_workspace::<Option<OwnedSource>>(work, self.sources.len(), "map removal workspace")?;
        let mut available: Vec<_> = std::mem::take(self.sources).into_iter().map(Some).collect();
        for (_, index, library) in selected {
            let source = available[index].take().ok_or_else(|| {
                StartupError::new(StartupErrorKind::Internal, "map source was published twice")
            })?;
            self.libraries.push(LibrarySource::new(source.name, source.text, library));
        }
        *self.sources = available.into_iter().flatten().collect();
        Ok(())
    }
}

fn duplicate_source(name: &str) -> StartupError {
    StartupError::new(
        StartupErrorKind::InvalidArgument,
        format!("source is assigned more than once: {name}"),
    )
}

fn charge_workspace<T>(
    work: &mut LibraryMapWorkBudget,
    count: usize,
    operation: &str,
) -> Result<(), StartupError> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| work.limit_error(operation))?;
    work.charge_allocation_usize(bytes, operation)
}

#[cfg(test)]
mod tests;
