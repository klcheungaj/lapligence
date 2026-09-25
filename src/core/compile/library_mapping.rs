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
    original_map: Option<String>,
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
                        original_map: None,
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
                        original_map: None,
                    },
                )
                .is_some()
            {
                return Err(duplicate_source(&source.name));
            }
        }
        Ok(result)
    }

    /// Retain a same-length configuration-only projection of an admitted map.
    /// Maps already count against source/byte admission limits. Charge the
    /// projection's extra workspace separately, and reuse any existing mapped
    /// buffer/explicit library choice rather than assigning it a second time.
    pub(super) fn retain_configuration(
        &mut self,
        name: &str,
        original: &str,
        projection: String,
        work: &mut LibraryMapWorkBudget,
    ) -> Result<(), StartupError> {
        let key = logical_path_key(Path::new(name), work, "configuration source identity")?;
        if let Some(entry) = self.entries.get_mut(&key) {
            for source in self.sources.iter_mut() {
                let source_key = logical_path_key(
                    Path::new(&source.name), work, "configuration source lookup",
                )?;
                if source_key == key {
                    check_map_source(&source.text, original, &projection, name, work)?;
                    retain_original_map(entry, original, work)?;
                    source.text = projection;
                    source.is_compilation_unit = true;
                    return Ok(());
                }
            }
            for source in self.libraries.iter_mut() {
                let source_key = logical_path_key(
                    Path::new(&source.name), work, "configuration library lookup",
                )?;
                if source_key == key {
                    check_map_source(&source.text, original, &projection, name, work)?;
                    retain_original_map(entry, original, work)?;
                    source.text = projection;
                    return Ok(());
                }
            }
            return Err(StartupError::new(
                StartupErrorKind::Internal, "configuration candidate has no retained source",
            ));
        }
        charge_library_map_clone(work, name, "configuration entry name")?;
        charge_library_map_clone(work, name, "configuration source name")?;
        charge_workspace::<OwnedSource>(work, 1, "configuration source record")?;
        let mut entry = Entry {
            name: name.to_owned(),
            choice: None,
            order: usize::MAX,
            original_map: None,
        };
        retain_original_map(&mut entry, original, work)?;
        self.entries.insert(key, entry);
        self.sources.push(OwnedSource::compilation_unit(name, projection));
        Ok(())
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
                        original_map: None,
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
    ) -> Result<Vec<OwnedSource>, StartupError> {
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
        charge_workspace::<OwnedSource>(
            work,
            self.entries.values().filter(|entry| entry.original_map.is_some()).count(),
            "original map publication",
        )?;
        let mut available: Vec<_> = std::mem::take(self.sources).into_iter().map(Some).collect();
        for (_, index, library) in selected {
            let source = available[index].take().ok_or_else(|| {
                StartupError::new(StartupErrorKind::Internal, "map source was published twice")
            })?;
            self.libraries.push(LibrarySource::new(source.name, source.text, library));
        }
        *self.sources = available.into_iter().flatten().collect();
        Ok(self.entries.into_values().filter_map(|entry| {
            entry.original_map.map(|text| OwnedSource::include(entry.name, text))
        }).collect())
    }
}

fn retain_original_map(
    entry: &mut Entry,
    original: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    work.charge_usize(original.len(), "original configuration map")?;
    if let Some(retained) = &entry.original_map {
        if retained != original {
            return Err(StartupError::new(
                StartupErrorKind::InvalidArgument,
                format!("conflicting configuration map contents: {}", entry.name),
            ));
        }
    } else {
        work.charge_allocation_usize(original.len(), "original configuration map")?;
        entry.original_map = Some(original.to_owned());
    }
    Ok(())
}

fn check_map_source(
    retained: &str,
    original: &str,
    projection: &str,
    name: &str,
    work: &mut LibraryMapWorkBudget,
) -> Result<(), StartupError> {
    work.charge_usize(retained.len(), "configuration source comparison")?;
    work.charge_usize(original.len(), "configuration source comparison")?;
    work.charge_usize(projection.len(), "configuration source comparison")?;
    if retained != original && retained != projection {
        return Err(StartupError::new(
            StartupErrorKind::InvalidArgument,
            format!("library map configuration conflicts with admitted source: {name}"),
        ));
    }
    Ok(())
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
