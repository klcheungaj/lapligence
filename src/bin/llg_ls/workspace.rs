//! Workspace discovery primitives for the language server.
//!
//! This module deliberately owns no LSP state.  It provides the pure path
//! helpers, glob matching and discovery walks the backend uses, plus the
//! [`RootDescriptor`] that carries per-root configuration-derived state.
//! Discovery is driven by each root's `llg.toml` (see `crate::config`), not
//! by client globs.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::config::{self, LlgConfig};

/// Source extensions accepted by workspace discovery.
#[allow(dead_code)] // retained as the documented discovery extension set
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

/// Classification useful to document event handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFileEvent {
    /// A regular Verilog/SystemVerilog source file.
    Source(SourceFileKind),
    /// A `llg.toml` configuration change.
    Config,
    /// A path in the generated shadow tree; it must not be indexed as source.
    ShadowTree,
    /// Any other file or path.
    Other,
}

/// Classify a path without touching the filesystem.
pub fn classify_source_file_event(path: &Path) -> SourceFileEvent {
    if is_shadow_tree_path(path) {
        return SourceFileEvent::ShadowTree;
    }
    if path.file_name() == Some(OsStr::new(config::CONFIG_FILE)) {
        return SourceFileEvent::Config;
    }
    SourceFileKind::from_path(path).map_or(SourceFileEvent::Other, SourceFileEvent::Source)
}

/// Whether `path` is exactly the effective `llg.toml` for `root`.
///
/// This lexical path check intentionally does not consult discovery filters:
/// the config file controls source discovery and must remain observable even
/// when the root excludes its directory from source scans.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_root_config_path(root: &Path, path: &Path) -> bool {
    let Some(root) = normalize_absolute_path(root) else {
        return false;
    };
    let Some(path) = normalize_absolute_path(path) else {
        return false;
    };
    path == root.join(config::CONFIG_FILE)
}

/// Whether `path` is the effective config file of ANY of `roots`.
///
/// Roots may point at arbitrary config paths through initialization overrides
/// (e.g. `custom.toml` via the client's `configFiles`), so a basename check
/// against `llg.toml` alone misses watched-file reload events for overridden
/// roots.  Callers should keep the cheap basename fast-path first and use
/// this predicate for events it rejects.
pub fn is_effective_config_path(path: &Path, roots: &[RootDescriptor]) -> bool {
    let Some(path) = normalize_absolute_path(path) else {
        return false;
    };
    roots
        .iter()
        .any(|root| normalize_absolute_path(&root.config_path).as_deref() == Some(path.as_path()))
}

/// Whether `path` is a source file supported by discovery.
#[allow(dead_code)] // retained as a public path-classification helper
pub fn is_source_file(path: &Path) -> bool {
    SourceFileKind::from_path(path).is_some()
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

/// A normalized LSP workspace root carrying its configuration-derived state.
///
/// `config` is the last-valid `llg.toml` (or `None` before a valid config has
/// ever loaded).  Callers use [`RootDescriptor::effective_config`] to get the
/// safe-default config when none has loaded yet.
#[derive(Debug, Clone, PartialEq)]
pub struct RootDescriptor {
    /// Client-provided workspace-folder identity, when available.
    pub id: String,
    /// Normalized absolute root path (the directory holding `llg.toml`).
    pub root: PathBuf,
    /// Effective `llg.toml` path (defaults to `<root>/llg.toml`).
    pub config_path: PathBuf,
    /// Last-valid config, or `None` when no valid config has ever loaded.
    pub config: Option<Arc<LlgConfig>>,
}

impl RootDescriptor {
    /// Make a descriptor from an absolute root without filesystem access.
    pub fn from_absolute(path: impl AsRef<Path>) -> io::Result<Self> {
        let root = normalize_absolute_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "workspace root must be an absolute path",
            )
        })?;
        let config_path = root.join(config::CONFIG_FILE);
        Ok(Self {
            id: root.to_string_lossy().into_owned(),
            root,
            config_path,
            config: None,
        })
    }

    /// Set a stable client workspace-folder identity.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the effective config path (an override supplied by the client).
    pub fn with_config_path(mut self, config_path: impl Into<PathBuf>) -> Self {
        self.config_path = config_path.into();
        self
    }

    /// Attach the last-valid config.
    pub fn with_config(mut self, config: Option<Arc<LlgConfig>>) -> Self {
        self.config = config;
        self
    }

    /// The effective config: the last-valid one, or safe defaults when none
    /// has ever loaded.
    pub fn effective_config(&self) -> LlgConfig {
        self.config
            .as_deref()
            .cloned()
            .unwrap_or_else(|| config::default_config(&self.root))
    }

    /// Discovery filters derived from the effective config.
    pub fn discovery_filters(&self) -> DiscoveryFilters {
        let cfg = self.effective_config();
        DiscoveryFilters::new(cfg.sources.include, cfg.sources.exclude)
    }

    /// Whether this root's filters include a source file at a root-relative
    /// path within one of its source directories.
    pub fn allows_file(&self, relative_file: &Path) -> bool {
        self.discovery_filters().allows_file(relative_file)
    }
}

/// Longest-prefix ownership of an absolute path over a root's include dirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootOwnership {
    pub root_index: usize,
    pub root: PathBuf,
    pub relative_path: PathBuf,
}

/// All include-search directories for a root (source dirs + compile include
/// dirs), derived from its effective config.
fn root_include_dirs(descriptor: &RootDescriptor) -> Vec<PathBuf> {
    let cfg = descriptor.effective_config();
    config::include_dirs(&cfg)
}

fn longest_prefix_owner(path: &Path, roots: &[RootDescriptor]) -> Option<RootOwnership> {
    let path = normalize_absolute_path(path)?;
    roots
        .iter()
        .enumerate()
        .filter_map(|(root_index, descriptor)| {
            // A file may sit under several of a root's include dirs (source
            // dirs are implicit include dirs and `compile.include_dirs` can
            // nest inside them).  The DEEPEST match wins within a root so the
            // ownership relative path stays as short as possible.
            let mut best: Option<(usize, RootOwnership)> = None;
            for dir in root_include_dirs(descriptor) {
                let Some(stripped) = path.strip_prefix(&dir).ok() else {
                    continue;
                };
                let Some(relative_path) = normalize_relative_path(stripped) else {
                    continue;
                };
                let depth = dir.components().count();
                if best
                    .as_ref()
                    .is_none_or(|(best_depth, _)| depth > *best_depth)
                {
                    best = Some((
                        depth,
                        RootOwnership {
                            root_index,
                            root: descriptor.root.clone(),
                            relative_path,
                        },
                    ));
                }
            }
            best
        })
        .max_by(|(depth_a, a), (depth_b, b)| {
            depth_a
                .cmp(depth_b)
                .then_with(|| b.root_index.cmp(&a.root_index))
        })
        .map(|(_, ownership)| ownership)
}

/// Find the owner of `path` by longest normalized absolute include-dir prefix.
pub fn owner_for_path(path: &Path, roots: &[RootDescriptor]) -> Option<RootOwnership> {
    longest_prefix_owner(path, roots)
}

/// Find the owner of `path` by longest prefix without applying filters.
///
/// Ownership is structural over a root's configured include directories.
pub fn owning_root_unfiltered(path: &Path, roots: &[RootDescriptor]) -> Option<RootOwnership> {
    longest_prefix_owner(path, roots)
}

/// Alias for callers that use the noun form.
#[allow(dead_code)] // retained as a compatibility alias for workspace callers
pub fn owning_root(path: &Path, roots: &[RootDescriptor]) -> Option<RootOwnership> {
    owner_for_path(path, roots)
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

/// Discover the `.v`/`.sv` compilation units for a root from its effective
/// config: walk each configured source directory applying the root-relative
/// include/exclude filters, keeping only `.v`/`.sv` files.  Deduplicated and
/// sorted by normalized path.
pub fn discover_units(descriptor: &RootDescriptor) -> io::Result<Vec<PathBuf>> {
    let cfg = descriptor.effective_config();
    let filters = DiscoveryFilters::new(cfg.sources.include, cfg.sources.exclude);
    let mut units = BTreeMap::new();
    for dir in &cfg.sources.directories {
        for file in discover_files(dir, &filters)? {
            if config::is_compilation_unit(&file.path) {
                units.entry(file.path).or_insert(());
            }
        }
    }
    Ok(units.into_keys().collect())
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

/// Whether `path` is inside the process shadow tree (under the OS temp dir).
///
/// The check is robust to symlinked temp dirs (e.g. macOS `/tmp`→`/private/tmp`,
/// or a `TMPDIR` whose path resolves through a symlink): a watched-file event
/// for a staged copy may arrive with a canonically-resolved path that does not
/// lexically share the recorded base prefix.  The lexical fast path covers the
/// common case; when it misses, the canonicalized forms of both sides are
/// compared so the server's OWN staging writes (which it must never react to)
/// are still recognized regardless of symlink layout.
pub fn is_shadow_tree_path(path: &Path) -> bool {
    path_is_under(path, &crate::features::process_shadow_base())
}

/// Whether `path` lies under `base`, tolerating symlink resolution on either
/// side.  `path` must be absolute; the lexical check is cheap and handles the
/// normal layout, while the canonical fallback catches symlinked bases.
///
/// The canonical fallback only applies when `base` actually exists: staged
/// files only ever appear after the server created the base directory, so an
/// event for a staged copy always arrives with the base already present.
/// Requiring the base to exist also prevents a real file under the temp dir
/// (e.g. a project living there) from being mistaken for a shadow file while
/// the base has not been created yet.
pub(crate) fn path_is_under(path: &Path, base: &Path) -> bool {
    let Some(normalized) = normalize_absolute_path(path) else {
        return false;
    };
    if normalized.starts_with(base) {
        return true;
    }
    // The lexical prefix missed; canonicalize both sides so a symlinked base
    // (e.g. `TMPDIR` resolving through a symlink) is still recognized.
    let Ok(canonical_base) = std::fs::canonicalize(base) else {
        return false;
    };
    let Ok(canonical_path) = std::fs::canonicalize(&normalized) else {
        return false;
    };
    canonical_path.starts_with(&canonical_base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("llg_workspace_{}_{}", std::process::id(), id));
            fs::create_dir_all(&path).expect("create workspace test root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn descriptor_with_sources(
        root: &Path,
        directories: Vec<&str>,
        include: Vec<&str>,
        exclude: Vec<&str>,
    ) -> RootDescriptor {
        let dirs = if directories.is_empty() {
            vec![root.to_path_buf()]
        } else {
            directories.iter().map(|d| root.join(d)).collect()
        };
        let cfg = LlgConfig {
            schema_version: config::SCHEMA_VERSION,
            base_dir: root.to_path_buf(),
            sources: config::SourcesConfig {
                directories: dirs,
                include: include.iter().map(|s| s.to_string()).collect(),
                exclude: exclude.iter().map(|s| s.to_string()).collect(),
            },
            compile: config::CompileConfig {
                top: None,
                include_dirs: Vec::new(),
                defines: Vec::new(),
                param_overrides: Default::default(),
            },
            analysis: Default::default(),
            lint: Default::default(),
        };
        RootDescriptor::from_absolute(root)
            .expect("descriptor")
            .with_config(Some(Arc::new(cfg)))
    }

    #[test]
    fn root_relative_globs_support_components_and_double_star() {
        assert!(glob_matches(Path::new("src/a/top.sv"), "src/**"));
        assert!(glob_matches(
            Path::new("generated/top.sv"),
            "**/generated/**"
        ));
        assert!(glob_matches(Path::new("src/top.sv"), "src/*.sv"));
        assert!(!glob_matches(Path::new("srcx/top.sv"), "src/**"));
        assert!(!glob_matches(Path::new("../top.sv"), "**"));
        assert_eq!(
            normalize_relative_pattern(r"./src\\rtl//**/"),
            Some("src/rtl/**".to_owned())
        );
    }

    #[test]
    fn discovery_units_only_include_v_and_sv() {
        let root = TempRoot::new();
        fs::create_dir_all(root.path().join("src/nested")).expect("create source dirs");
        fs::write(root.path().join("src/z.sv"), "").expect("write source");
        fs::write(root.path().join("src/a.v"), "").expect("write source");
        fs::write(root.path().join("src/nested/h.svh"), "").expect("write header");
        fs::write(root.path().join("src/readme.txt"), "").expect("write non-source");
        fs::write(root.path().join("src/nested.inc"), "").expect("write include-only");

        let descriptor = descriptor_with_sources(
            root.path(),
            vec!["src"],
            vec!["**/*.v", "**/*.sv"],
            vec![".git/**", "target/**"],
        );
        let units = discover_units(&descriptor).expect("discover units");
        let paths: Vec<_> = units
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            vec![
                root.path().join("src/a.v").to_string_lossy().into_owned(),
                root.path().join("src/z.sv").to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn excludes_win_over_includes() {
        let root = TempRoot::new();
        fs::create_dir_all(root.path().join("src/generated")).expect("create dirs");
        fs::write(root.path().join("src/kept.sv"), "").expect("write kept");
        fs::write(root.path().join("src/generated/gen.sv"), "").expect("write generated");

        let descriptor = descriptor_with_sources(
            root.path(),
            vec!["src"],
            vec!["**/*.sv"],
            vec!["**/generated/**"],
        );
        let units = discover_units(&descriptor).expect("discover units");
        let paths: Vec<_> = units
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            vec![root
                .path()
                .join("src/kept.sv")
                .to_string_lossy()
                .into_owned()]
        );
    }

    #[test]
    fn external_source_dirs_are_owned_by_configured_roots() {
        let root = TempRoot::new();
        let external = TempRoot::new();
        fs::write(external.path().join("shared.sv"), "").expect("write shared source");

        let a = descriptor_with_sources(root.path(), vec![], vec!["**/*.sv"], vec![]);
        let b = descriptor_with_sources(external.path(), vec![], vec!["**/*.sv"], vec![]);
        // Note: directories empty -> default "." resolves to the config root.
        let roots = vec![a.clone(), b.clone()];

        // The external file is owned by root b (whose root is the external dir).
        let owner = owner_for_path(&external.path().join("shared.sv"), &roots).expect("owner");
        assert_eq!(owner.root, external.path());
        let _ = std::fs::remove_dir_all(root.path());
        let _ = std::fs::remove_dir_all(external.path());
    }

    #[test]
    fn shared_external_file_is_member_of_multiple_roots_and_owned_by_longest_prefix() {
        let outer = TempRoot::new();
        let shared = TempRoot::new();
        let nested = shared.path().join("nested");
        fs::create_dir_all(&nested).expect("create nested shared dir");
        fs::write(nested.join("shared.sv"), "").expect("write shared source");

        let mk = |root: &Path, dirs: Vec<PathBuf>| {
            let cfg = LlgConfig {
                schema_version: config::SCHEMA_VERSION,
                base_dir: root.to_path_buf(),
                sources: config::SourcesConfig {
                    directories: dirs,
                    include: vec!["**/*.sv".to_owned()],
                    exclude: Vec::new(),
                },
                compile: config::CompileConfig {
                    top: None,
                    include_dirs: Vec::new(),
                    defines: Vec::new(),
                    param_overrides: Default::default(),
                },
                analysis: Default::default(),
                lint: Default::default(),
            };
            RootDescriptor::from_absolute(root)
                .expect("descriptor")
                .with_config(Some(Arc::new(cfg)))
        };
        // The outer root declares the shared tree as a source directory; the
        // nested root (itself under the shared tree) declares `nested/`.
        let outer_descriptor = mk(outer.path(), vec![shared.path().to_path_buf()]);
        let nested_descriptor = mk(nested.as_path(), vec![nested.clone()]);
        let roots = vec![outer_descriptor, nested_descriptor];
        let file = nested.join("shared.sv");

        // The file is under both roots' include dirs, so both claim membership.
        let owner = owner_for_path(&file, &roots).expect("owner");
        // The nested root's include dir is the longest prefix and wins.
        assert_eq!(owner.root, nested);
        assert_eq!(owner.root_index, 1);
        assert_eq!(owner.relative_path, PathBuf::from("shared.sv"));
        let _ = std::fs::remove_dir_all(outer.path());
        let _ = std::fs::remove_dir_all(shared.path());
    }

    #[test]
    fn config_events_distinguish_shadow_and_config_files() {
        assert_eq!(
            classify_source_file_event(Path::new("src/top.SV")),
            SourceFileEvent::Source(SourceFileKind::SystemVerilog)
        );
        assert_eq!(
            classify_source_file_event(Path::new("llg.toml")),
            SourceFileEvent::Config
        );
        assert_eq!(
            classify_source_file_event(Path::new("src/readme.txt")),
            SourceFileEvent::Other
        );
    }

    #[test]
    fn shadow_under_predicate_lexical_and_symlink_robust() {
        let root = TempRoot::new();
        let real = root.path().join("real");
        fs::create_dir_all(&real).expect("create real dir");
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");

        let nested = real.join("a").join("b");
        fs::create_dir_all(&nested).expect("create nested");
        let staged = nested.join("top.sv");
        fs::write(&staged, "module top;\nendmodule\n").expect("write staged copy");

        // Lexical match through the direct base.
        assert!(
            path_is_under(&real.join("a").join("b").join("top.sv"), &real),
            "direct base prefix must match lexically"
        );
        // A path reached through the symlink resolves to the same inode, so it
        // must be recognized even though its lexical prefix differs from `real`.
        assert!(
            path_is_under(&link.join("a").join("b").join("top.sv"), &real),
            "symlinked base must match through canonicalization"
        );
        // The inverse: base given as the symlink path, path via the real dir.
        assert!(
            path_is_under(&real.join("a").join("b").join("top.sv"), &link),
            "canonicalized base must match the real path too"
        );
        // A sibling file outside the tree is never treated as shadow.
        let outside = root.path().join("outside.sv");
        fs::write(&outside, "module out;\nendmodule\n").expect("write outside");
        assert!(
            !path_is_under(&outside, &real),
            "outside path must not match"
        );
        // Relative paths are rejected outright.
        assert!(
            !path_is_under(Path::new("a/b/top.sv"), &real),
            "relative path must not match"
        );
        let _ = std::fs::remove_dir_all(root.path());
    }

    #[test]
    fn shadow_under_predicate_requires_existing_base_for_canonical_match() {
        let root = TempRoot::new();
        let real = root.path().join("real");
        fs::create_dir_all(&real).expect("create real dir");
        // The base has not been created yet: nothing can have been staged
        // under it, so a real file under the temp dir must NOT be treated as
        // shadow (that would wrongly swallow legitimate watched events for
        // projects living under the OS temp dir).
        let unborn_base = real.join("llg-pid-rand");
        let real_file = real.join("top.sv");
        fs::write(&real_file, "module top;\nendmodule\n").expect("write real file");
        assert!(
            !path_is_under(&real_file, &unborn_base),
            "unborn base must not canonicalize-match unrelated files"
        );
        // Once the base exists, its staged copies match through symlinks.
        fs::create_dir_all(unborn_base.join("a")).expect("create base");
        let staged = unborn_base.join("a").join("top.sv");
        fs::write(&staged, "module top;\nendmodule\n").expect("write staged copy");
        let link = real.join("link");
        std::os::unix::fs::symlink(&unborn_base, &link).expect("create symlink");
        assert!(
            path_is_under(&link.join("a").join("top.sv"), &unborn_base),
            "existing base must match through canonicalization"
        );
        let _ = std::fs::remove_dir_all(root.path());
    }

    #[test]
    fn owner_uses_deepest_matching_include_dir_within_a_root() {
        let root = TempRoot::new();
        let nested = root.path().join("lib").join("nested");
        fs::create_dir_all(&nested).expect("create nested include dir");
        let file = nested.join("deep.sv");
        fs::write(&file, "").expect("write deep source");

        // Source dir "." implies the root itself as an include dir; the
        // explicit include dir nests inside it.
        let cfg = LlgConfig {
            schema_version: config::SCHEMA_VERSION,
            base_dir: root.path().to_path_buf(),
            sources: config::SourcesConfig {
                directories: vec![root.path().to_path_buf()],
                include: vec!["**/*.sv".to_owned()],
                exclude: Vec::new(),
            },
            compile: config::CompileConfig {
                top: None,
                include_dirs: vec![nested.clone()],
                defines: Vec::new(),
                param_overrides: Default::default(),
            },
            analysis: Default::default(),
            lint: Default::default(),
        };
        let descriptor = RootDescriptor::from_absolute(root.path())
            .expect("descriptor")
            .with_config(Some(Arc::new(cfg)));

        // The DEEPEST matching include dir owns the path: the relative path is
        // taken from the deeper directory, not from the shallower root.
        let owner = owner_for_path(&file, std::slice::from_ref(&descriptor)).expect("owner");
        assert_eq!(owner.root, root.path());
        assert_eq!(owner.relative_path, PathBuf::from("deep.sv"));
    }

    #[test]
    fn root_config_predicate_ignores_discovery_filters() {
        let root = Path::new("/workspace/project");
        let config_path = root.join(config::CONFIG_FILE);

        assert!(is_root_config_path(root, &config_path));
        assert!(!is_root_config_path(root, &root.join("nested/llg.toml")));
        assert!(!is_root_config_path(root, Path::new("llg.toml")));
    }

    #[test]
    fn effective_config_predicate_matches_override_paths() {
        let root_a = TempRoot::new();
        let override_config = root_a.path().join("custom.toml");
        let descriptor = RootDescriptor::from_absolute(root_a.path())
            .expect("descriptor")
            .with_config_path(override_config.clone());
        let roots = vec![descriptor];

        // An override config with a non-default basename must be recognized
        // as that root's effective config (watch routing), while unrelated
        // files and other roots' defaults are not.
        assert!(is_effective_config_path(&override_config, &roots));
        assert!(is_effective_config_path(
            &root_a.path().join("./nested/../custom.toml"),
            &roots
        ));
        assert!(!is_effective_config_path(
            &root_a.path().join(config::CONFIG_FILE),
            &roots
        ));
        assert!(!is_effective_config_path(Path::new("custom.toml"), &roots));

        // Default descriptors point at `<root>/llg.toml`.
        let default_roots = vec![RootDescriptor::from_absolute(root_a.path()).expect("descriptor")];
        assert!(is_effective_config_path(
            &root_a.path().join(config::CONFIG_FILE),
            &default_roots
        ));
        assert!(!is_effective_config_path(&override_config, &default_roots));
    }

    #[test]
    fn allows_directory_traverses_under_bare_double_star() {
        // `**` matches files at any depth, so every directory must remain
        // traversable; the stripped-prefix heuristic previously reduced this
        // to an empty pattern and rejected everything below the top level.
        let filters = DiscoveryFilters {
            include: vec!["**".to_owned()],
            exclude: Vec::new(),
        };
        assert!(filters.allows_directory(Path::new("src")));
        assert!(filters.allows_directory(Path::new("a/b/c")));
        assert!(filters.allows_file(Path::new("top.sv")));
        assert!(filters.allows_file(Path::new("src/deep/top.sv")));

        let deep = DiscoveryFilters {
            include: vec!["**/**/*.sv".to_owned()],
            exclude: Vec::new(),
        };
        assert!(
            deep.allows_directory(Path::new("src")),
            "all-`**` prefixes must keep directories traversable"
        );
    }
}
