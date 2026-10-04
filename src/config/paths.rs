//! Lexical path normalization, glob matching and source discovery shared by
//! the `llg.toml` schema, the simulator driver and the language server.
//!
//! Everything here is pure path/filesystem logic with no process state.
//! Paths are normalized lexically (symlinks are never resolved) because
//! roots and configured directories may not exist yet, and discovery never
//! follows symlinked files or directories.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Source extensions accepted by workspace discovery.
pub const SOURCE_EXTENSIONS: [&str; 4] = ["v", "sv", "vh", "svh"];

/// The kind of Verilog source represented by a discovered file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceFileKind {
    Verilog,
    SystemVerilog,
    VerilogHeader,
    SystemVerilogHeader,
}

impl SourceFileKind {
    /// Classify a path by its case-insensitive extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension().and_then(OsStr::to_str)?;
        match extension.to_ascii_lowercase().as_str() {
            "v" => Some(Self::Verilog),
            "sv" => Some(Self::SystemVerilog),
            "vh" => Some(Self::VerilogHeader),
            "svh" => Some(Self::SystemVerilogHeader),
            _ => None,
        }
    }
}

/// Whether `path` is a source file supported by discovery.
pub fn is_source_file(path: &Path) -> bool {
    SourceFileKind::from_path(path).is_some()
}

/// Whether `path` is a compilation-unit source (`.v`/`.sv`). Headers enter
/// analysis only through include resolution.
pub fn is_compilation_unit(path: &Path) -> bool {
    matches!(
        SourceFileKind::from_path(path),
        Some(SourceFileKind::Verilog | SourceFileKind::SystemVerilog)
    )
}

/// Normalize an absolute path lexically, without resolving symlinks.
///
/// Lexical normalization is intentional: roots may not exist yet, and
/// canonicalization would make ownership depend on filesystem symlink state.
pub fn normalize_absolute_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

/// Normalize a root-relative path.  `/` and `..` are rejected so a pattern
/// cannot escape its workspace root.
pub fn normalize_relative_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

/// Return a normalized path relative to `root`, or `None` when it is outside.
pub fn root_relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = normalize_absolute_path(root)?;
    let path = normalize_absolute_path(path)?;
    normalize_relative_path(path.strip_prefix(root).ok()?)
}

/// Normalize a root-relative glob pattern.
///
/// Patterns use `/` separators and support `*` and `?` within a component and
/// `**` as a whole component matching zero or more path components.  Empty
/// components and `.` are removed; `..` and absolute patterns are rejected.
pub fn normalize_relative_pattern(pattern: &str) -> Option<String> {
    let pattern = pattern.replace('\\', "/");
    if pattern.starts_with('/') {
        return None;
    }

    let mut components = Vec::new();
    for component in pattern.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            return None;
        }
        components.push(component);
    }
    Some(components.join("/"))
}

/// Match a root-relative path against a normalized or unnormalized glob.
pub fn matches_root_relative_pattern(path: &Path, pattern: &str) -> bool {
    let Some(path) = relative_components(path) else {
        return false;
    };
    let Some(pattern) = normalized_pattern_components(pattern) else {
        return false;
    };
    glob_components_match(&path, &pattern)
}

/// Alias with a shorter name for callers doing path-filter checks.
pub fn glob_matches(path: &Path, pattern: &str) -> bool {
    matches_root_relative_pattern(path, pattern)
}

/// Normalized executable discovery filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryFilters {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl DiscoveryFilters {
    /// Construct filters from root-relative directory glob strings.
    pub fn new<I, S, E, T>(include: I, exclude: E) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        E: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        let include = include
            .into_iter()
            .filter_map(|pattern| normalize_relative_pattern(pattern.as_ref()))
            .collect();
        let exclude = exclude
            .into_iter()
            .filter_map(|pattern| normalize_relative_pattern(pattern.as_ref()))
            .collect();
        Self { include, exclude }
    }

    /// Excludes win even when an include pattern also matches.  A directory is
    /// traversed when an include pattern could still match a file below it.
    pub fn allows_directory(&self, relative: &Path) -> bool {
        let Some(relative) = normalize_relative_path(relative) else {
            return false;
        };
        if self.is_excluded(&relative) {
            return false;
        }
        if self.include.is_empty() {
            return false;
        }
        self.include.iter().any(|pattern| {
            // Strip a trailing file-component glob (e.g. `*.v`) and check the
            // remaining pattern as a directory prefix, so `**/*.v` keeps
            // traversing `src/` while `src/*.sv` still reaches `src`.
            let Some(components) = normalized_pattern_components(pattern) else {
                return false;
            };
            if components.is_empty() {
                return false;
            }
            // A bare `**` (or all-`**`) pattern matches at any depth: every
            // directory must stay traversable.  The stripped-prefix check
            // below would compute an empty pattern and reject everything.
            if components.iter().all(|component| component == "**") {
                return true;
            }
            let pattern_dir = components[..components.len() - 1].join("/");
            pattern_prefix_matches(&relative, &pattern_dir)
        })
    }

    /// Evaluate filters against a source file's relative path.  Include and
    /// exclude patterns are file-path globs (e.g. `**/*.v`, `**/generated/**`)
    /// evaluated from the discovery directory; excludes win.
    pub fn allows_file(&self, relative_file: &Path) -> bool {
        let Some(relative_file) = normalize_relative_path(relative_file) else {
            return false;
        };
        if self.is_excluded(&relative_file) {
            return false;
        }
        !self.include.is_empty()
            && self
                .include
                .iter()
                .any(|pattern| glob_matches(&relative_file, pattern))
    }

    fn is_excluded(&self, relative: &Path) -> bool {
        ancestor_paths(relative).iter().any(|ancestor| {
            self.exclude
                .iter()
                .any(|pattern| glob_matches(ancestor, pattern))
        })
    }
}

/// A regular supported source file discovered below one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    /// Normalized absolute path on disk.
    pub path: PathBuf,
    /// Normalized path relative to the discovery directory.
    pub relative_path: PathBuf,
    pub kind: SourceFileKind,
}

/// Discover supported regular files under one directory.
///
/// Directory entries are inspected with [`fs::DirEntry::file_type`], so
/// symlinked directories and symlinked files are skipped rather than followed.
/// The returned paths are sorted and deduplicated by normalized absolute path.
pub fn discover_files(root: &Path, filters: &DiscoveryFilters) -> io::Result<Vec<DiscoveredFile>> {
    let root = normalize_absolute_path(root).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "workspace discovery root must be an absolute path",
        )
    })?;
    let metadata = fs::symlink_metadata(&root)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!(
                "workspace discovery root is not a directory: {}",
                root.display()
            ),
        ));
    }

    let mut pending = vec![(root.clone(), PathBuf::new())];
    let mut discovered = BTreeMap::new();
    while let Some((directory, relative_directory)) = pending.pop() {
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by(|left, right| {
            left.file_name()
                .to_string_lossy()
                .cmp(&right.file_name().to_string_lossy())
        });

        for entry in entries {
            let file_type = entry.file_type()?;
            let name = entry.file_name();
            let relative = if relative_directory.as_os_str().is_empty() {
                PathBuf::from(&name)
            } else {
                relative_directory.join(&name)
            };

            if file_type.is_dir() {
                if filters.allows_directory(&relative) {
                    pending.push((entry.path(), relative));
                }
                continue;
            }

            // `is_file` is false for symlinks when using file_type(), which is
            // the desired no-follow behavior for both files and directories.
            if !file_type.is_file() || !filters.allows_file(&relative) {
                continue;
            }
            let Some(kind) = SourceFileKind::from_path(&relative) else {
                continue;
            };
            let Some(path) = normalize_absolute_path(&entry.path()) else {
                continue;
            };
            discovered.entry(path).or_insert((relative, kind));
        }
    }

    Ok(discovered
        .into_iter()
        .map(|(path, (relative_path, kind))| DiscoveredFile {
            path,
            relative_path,
            kind,
        })
        .collect())
}

fn relative_components(path: &Path) -> Option<Vec<String>> {
    let path = normalize_relative_path(path)?;
    Some(
        path.components()
            .filter_map(|component| match component {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect(),
    )
}

fn normalized_pattern_components(pattern: &str) -> Option<Vec<String>> {
    let pattern = normalize_relative_pattern(pattern)?;
    Some(if pattern.is_empty() {
        Vec::new()
    } else {
        pattern.split('/').map(str::to_owned).collect()
    })
}

fn glob_components_match(path: &[String], pattern: &[String]) -> bool {
    let mut table = vec![vec![false; pattern.len() + 1]; path.len() + 1];
    table[0][0] = true;
    for path_index in 0..=path.len() {
        for pattern_index in 0..pattern.len() {
            if !table[path_index][pattern_index] {
                continue;
            }
            if pattern[pattern_index] == "**" {
                table[path_index][pattern_index + 1] = true;
                if path_index < path.len() {
                    table[path_index + 1][pattern_index] = true;
                }
            } else if path_index < path.len()
                && component_glob_matches(&path[path_index], &pattern[pattern_index])
            {
                table[path_index + 1][pattern_index + 1] = true;
            }
        }
    }
    table[path.len()][pattern.len()]
}

fn pattern_prefix_matches(path: &Path, pattern: &str) -> bool {
    let Some(path) = relative_components(path) else {
        return false;
    };
    let Some(pattern) = normalized_pattern_components(pattern) else {
        return false;
    };
    let mut table = vec![vec![false; pattern.len() + 1]; path.len() + 1];
    table[0][0] = true;
    for path_index in 0..=path.len() {
        for pattern_index in 0..=pattern.len() {
            if !table[path_index][pattern_index] {
                continue;
            }
            if pattern_index < pattern.len() && pattern[pattern_index] == "**" {
                table[path_index][pattern_index + 1] = true;
                if path_index < path.len() {
                    table[path_index + 1][pattern_index] = true;
                }
            } else if path_index < path.len()
                && pattern_index < pattern.len()
                && component_glob_matches(&path[path_index], &pattern[pattern_index])
            {
                table[path_index + 1][pattern_index + 1] = true;
            }
        }
    }
    table[path.len()].iter().any(|matched| *matched)
}

fn component_glob_matches(value: &str, pattern: &str) -> bool {
    let value: Vec<char> = value.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let mut table = vec![vec![false; pattern.len() + 1]; value.len() + 1];
    table[0][0] = true;
    for value_index in 0..=value.len() {
        for pattern_index in 0..pattern.len() {
            if !table[value_index][pattern_index] {
                continue;
            }
            match pattern[pattern_index] {
                '*' => {
                    table[value_index][pattern_index + 1] = true;
                    if value_index < value.len() {
                        table[value_index + 1][pattern_index] = true;
                    }
                }
                '?' if value_index < value.len() => {
                    table[value_index + 1][pattern_index + 1] = true;
                }
                character if value_index < value.len() && character == value[value_index] => {
                    table[value_index + 1][pattern_index + 1] = true;
                }
                _ => {}
            }
        }
    }
    table[value.len()][pattern.len()]
}

fn ancestor_paths(path: &Path) -> Vec<PathBuf> {
    let Some(path) = normalize_relative_path(path) else {
        return Vec::new();
    };
    let mut ancestors = vec![PathBuf::new()];
    let mut current = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(part) = component {
            current.push(part);
            ancestors.push(current.clone());
        }
    }
    ancestors
}
