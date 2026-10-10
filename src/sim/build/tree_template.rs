//! Configured-tree templates for fresh model build trees (opt-in).
//!
//! A seeded configure of a fresh model tree still starts CMake (and, through
//! it, `ninja --version`, linker feature checks and `ninja -t restat`) and
//! writes a handful of generated files. For one toolchain seed, generated
//! `CMakeLists.txt` and configure arguments, that output differs between two
//! trees only in the trees' own paths. A template stores one configured tree
//! with those paths replaced by placeholders; a later fresh tree with the same
//! key is created by writing the template back with its own paths, and its
//! configure is skipped. `cmake --build` then runs on it as on any configured
//! tree; a later re-configure treats it as an existing tree.
//!
//! Life cycle, all under `<cache root>/`[`TEMPLATE_DIR`]`/<hash>`:
//! 1. The first fresh tree for a key configures normally; its configured tree
//!    is captured as a `staged` template.
//! 2. A later fresh tree in a different directory also configures normally,
//!    then compares its configured tree with the staged template written for
//!    its own paths. Only an exact match (every file, byte for byte, and every
//!    directory) publishes the template as `ready`; a mismatch records a
//!    `rejected` marker that names the first differences, and the key never
//!    uses a template.
//! 3. Fresh trees for a ready key are written from the template. When the
//!    build of such a tree fails, the caller rebuilds it with a real configure
//!    and rejects the template if that build succeeds.
//!
//! Only trees whose paths use a conservative character set (no spaces, `$`,
//! `@` or non-ASCII, a drive colon only on Windows) are templated, because
//! CMake's quoting and Ninja's escaping of other characters are not plain
//! substitutions. Every spelling CMake may use for the source directory is
//! covered: as given and canonical, with native and forward separators, with
//! the drive colon plain or Ninja-escaped (`D$:`). A template is used only for
//! a tree whose spellings coincide the same way as the captured tree's (for
//! example, canonical equal to given). Seeds stay in force: a template exists
//! only for a seeded configure, and its key includes the seed's exact key.
//!
//! `LLG_CMAKE_TREE_TEMPLATES=1` ([`TEMPLATE_ENV`]) enables templates; they are
//! off by default and whenever `LLG_CMAKE_PROFILE_DIR` asks for configure
//! profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::RuntimeCacheLock;

/// Environment switch: `1`, `on`, `true` or `yes` enables templates.
pub const TEMPLATE_ENV: &str = "LLG_CMAKE_TREE_TEMPLATES";

/// Subdirectory of the runtime cache root that holds every template entry
/// (`<root>/cmake-tree/<hash>/`) and its lock file.
pub const TEMPLATE_DIR: &str = "cmake-tree";

/// Bumped whenever the layout or the check changes.
const FORMAT: &str = "llg-cmake-tree-template-v1";

/// Placeholder for spelling `index`; `@` never occurs in an accepted path.
fn placeholder(index: usize) -> String {
    format!("@LLG_TREE_{index}@")
}

const PLACEHOLDER_PREFIX: &str = "@LLG_TREE_";

/// Whether `value` (the environment variable) enables templates.
pub(super) fn enabled_by(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "on" | "true" | "yes"
        )
    })
}

/// The template state of one key, looked up before a fresh configure.
#[derive(Debug)]
pub(super) struct Plan {
    entry: PathBuf,
    key: String,
    spellings: Spellings,
    state: State,
}

#[derive(Debug)]
enum State {
    /// Nothing published yet: capture after the configure.
    Absent,
    /// Captured but unchecked: check after the configure.
    Staged(Template),
    /// Checked: write it instead of configuring.
    Ready(Template),
}

/// What [`Plan::after_configure`] did, for the timing record.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Captured,
    Verified,
    Rejected,
    Unchanged,
}

impl Outcome {
    pub(super) fn note(&self) -> &'static str {
        match self {
            Outcome::Captured => "captured",
            Outcome::Verified => "verified",
            Outcome::Rejected => "rejected",
            Outcome::Unchanged => "staged",
        }
    }
}

/// Look up the template for a fresh tree `source`/`build` configured with
/// `seed_key` and `arguments` (every configure argument except `-S`/`-B` and
/// their values). `None` when templates are off, the paths are not eligible,
/// or the key was rejected.
pub(super) fn plan(
    root: &Path,
    seed_key: &str,
    cmake: &str,
    source: &Path,
    build: &Path,
    arguments: &[String],
) -> Option<Plan> {
    if !enabled_by(std::env::var(TEMPLATE_ENV).ok().as_deref())
        || std::env::var_os(super::timings::PROFILE_DIR_ENV).is_some_and(|dir| !dir.is_empty())
    {
        return None;
    }
    if build != source.join("build") {
        return None;
    }
    let spellings = Spellings::of(source)?;
    let cmakelists = std::fs::read(source.join("CMakeLists.txt")).ok()?;
    let key = key_text(seed_key, cmake, arguments, &cmakelists);
    let entry = root
        .join(TEMPLATE_DIR)
        .join(super::toolchain_seed::entry_name(&key));
    if marker_matches(&entry, "rejected", &key) {
        return None;
    }
    let state = if marker_matches(&entry, "ready", &key) {
        State::Ready(Template::load(&entry)?)
    } else if marker_matches(&entry, "staged", &key) {
        Template::load(&entry).map_or(State::Absent, State::Staged)
    } else {
        State::Absent
    };
    Some(Plan {
        entry,
        key,
        spellings,
        state,
    })
}

impl Plan {
    /// Whether the template is ready to replace the configure.
    pub(super) fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    /// Write the ready template into the fresh `build` directory. On failure
    /// the caller removes the partial tree and configures normally.
    pub(super) fn instantiate(&self, build: &Path) -> std::io::Result<()> {
        let State::Ready(template) = &self.state else {
            return Err(std::io::Error::other("template is not ready"));
        };
        let tree = template
            .tree_for(&self.spellings)
            .ok_or_else(|| std::io::Error::other("template does not fit this tree"))?;
        write_tree(build, &tree)
    }

    /// After a successful real configure of `build`: capture the tree when no
    /// template exists, or check a staged one against it. Never waits for
    /// another process; failures leave the entry as it was.
    pub(super) fn after_configure(&self, build: &Path) -> Outcome {
        let Some(parent) = self.entry.parent() else {
            return Outcome::Unchanged;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return Outcome::Unchanged;
        }
        let Ok(Some(_lock)) = RuntimeCacheLock::try_acquire(&self.entry) else {
            return Outcome::Unchanged;
        };
        if marker_matches(&self.entry, "ready", &self.key)
            || marker_matches(&self.entry, "rejected", &self.key)
        {
            return Outcome::Unchanged;
        }
        let Some(actual) = read_tree(build) else {
            return Outcome::Unchanged;
        };
        match &self.state {
            State::Absent | State::Ready(_) => {
                if marker_matches(&self.entry, "staged", &self.key) {
                    return Outcome::Unchanged;
                }
                match Template::capture(&actual, &self.spellings) {
                    Some(template) if template.store(&self.entry).is_ok() => {
                        write_marker(&self.entry, "staged", &self.key);
                        Outcome::Captured
                    }
                    Some(_) => Outcome::Unchanged,
                    None => {
                        write_marker(
                            &self.entry,
                            "rejected",
                            &format!("tree files are not plain text\n{}", self.key),
                        );
                        Outcome::Rejected
                    }
                }
            }
            State::Staged(template) => {
                // The same directory again proves nothing about substitution.
                if template.donor == self.spellings.texts[0] {
                    return Outcome::Unchanged;
                }
                let differences = match template.tree_for(&self.spellings) {
                    Some(expected) => tree_differences(&expected, &actual),
                    None => vec!["placeholders do not fit this tree".to_owned()],
                };
                if differences.is_empty() {
                    write_marker(&self.entry, "ready", &self.key);
                    Outcome::Verified
                } else {
                    write_marker(
                        &self.entry,
                        "rejected",
                        &format!(
                            "template differs from a real configure: {}\n{}",
                            differences.join("; "),
                            self.key
                        ),
                    );
                    Outcome::Rejected
                }
            }
        }
    }

    /// Stop using the template: a build from it failed where a configured
    /// tree built.
    pub(super) fn reject(&self, reason: &str) {
        write_marker(&self.entry, "rejected", &format!("{reason}\n{}", self.key));
    }
}

fn key_text(seed_key: &str, cmake: &str, arguments: &[String], cmakelists: &[u8]) -> String {
    let mut key = format!("{FORMAT}\ncmake={cmake}\n");
    for argument in arguments {
        key.push_str(&format!("arg={argument}\n"));
    }
    key.push_str("seed:\n");
    key.push_str(seed_key);
    key.push_str("\nCMakeLists.txt:\n");
    key.push_str(&String::from_utf8_lossy(cmakelists));
    key
}

/// Whether `<entry>/<name>` exists and stores exactly `key` after its first
/// (detail) line, as the seed markers do.
fn marker_matches(entry: &Path, name: &str, key: &str) -> bool {
    std::fs::read_to_string(entry.join(name)).is_ok_and(|text| {
        text.split_once('\n')
            .is_some_and(|(_, stored)| stored == key)
    })
}

/// Write `<entry>/<name>` through a rename so readers see all or nothing.
/// `body` is either the key or a detail line followed by the key.
fn write_marker(entry: &Path, name: &str, body: &str) {
    let body = if name == "rejected" {
        body.to_owned()
    } else {
        format!("{name}\n{body}")
    };
    let temporary = entry.join(format!("{name}.tmp-{}", std::process::id()));
    if std::fs::create_dir_all(entry).is_ok()
        && std::fs::write(&temporary, body).is_ok()
        && std::fs::rename(&temporary, entry.join(name)).is_err()
    {
        let _ = std::fs::remove_file(&temporary);
    }
}

/// The spellings of one source directory, in a fixed order of transforms,
/// plus which earlier transform each one coincides with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spellings {
    texts: Vec<String>,
    pattern: Vec<usize>,
}

impl Spellings {
    fn of(source: &Path) -> Option<Self> {
        let given = plain_absolute(source)?;
        let canonical = crate::ffi::platform::canonicalize(source).ok()?;
        let canonical = plain_absolute(&canonical)?;
        let mut texts = Vec::new();
        for base in [given, canonical] {
            for separators in [base.clone(), base.replace('\\', "/")] {
                texts.push(separators.clone());
                texts.push(separators.replace(':', "$:"));
            }
        }
        Some(Self::from_texts(texts))
    }

    fn from_texts(texts: Vec<String>) -> Self {
        let pattern = texts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                texts[..index]
                    .iter()
                    .position(|earlier| earlier == text)
                    .unwrap_or(index)
            })
            .collect();
        Self { texts, pattern }
    }

    /// Distinct spellings with their placeholder index, longest first, so a
    /// shorter spelling never splits a longer one that contains it.
    fn distinct_longest_first(&self) -> Vec<(usize, &str)> {
        let mut distinct: Vec<(usize, &str)> = self
            .pattern
            .iter()
            .enumerate()
            .filter(|(index, first)| index == *first)
            .map(|(index, _)| (index, self.texts[index].as_str()))
            .collect();
        distinct.sort_by_key(|(_, text)| std::cmp::Reverse(text.len()));
        distinct
    }
}

/// `path` as text when it is absolute, has no `.`/`..` components and uses
/// only characters whose CMake and Ninja spellings are plain substitutions.
fn plain_absolute(path: &Path) -> Option<String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return None;
    }
    let text = path.to_str()?;
    let plain = text.char_indices().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || matches!(character, '_' | '-' | '.' | '/' | '~' | '+')
            || (cfg!(windows) && (character == '\\' || (character == ':' && index == 1)))
    });
    plain.then(|| text.to_owned())
}

/// A configured tree: directories and files relative to the build directory,
/// with `/` separators.
#[derive(Debug, Default, PartialEq, Eq)]
struct Tree {
    directories: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
}

fn read_tree(build: &Path) -> Option<Tree> {
    let mut paths = Vec::new();
    super::collect_paths(build, &mut paths);
    let mut tree = Tree::default();
    for path in paths {
        let relative = path
            .strip_prefix(build)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        // Ninja's own state records modification times, not configuration.
        if matches!(relative.as_str(), ".ninja_log" | ".ninja_deps") {
            continue;
        }
        if path.is_dir() {
            tree.directories.insert(relative);
        } else if path.is_file() {
            tree.files.insert(relative, std::fs::read(&path).ok()?);
        } else {
            return None;
        }
    }
    Some(tree)
}

/// Directories first, then files in name order, `build.ninja` last: Ninja
/// re-runs CMake when a generator input is newer than `build.ninja`.
fn write_tree(build: &Path, tree: &Tree) -> std::io::Result<()> {
    std::fs::create_dir_all(build)?;
    for directory in &tree.directories {
        std::fs::create_dir_all(build.join(directory))?;
    }
    let (manifest, others): (Vec<_>, Vec<_>) = tree
        .files
        .iter()
        .partition(|(name, _)| name.as_str() == "build.ninja");
    for (name, bytes) in others.into_iter().chain(manifest) {
        std::fs::write(build.join(name), bytes)?;
    }
    Ok(())
}

/// A stored template: the tree with placeholders and the spellings pattern
/// of the tree it was captured from.
#[derive(Debug)]
struct Template {
    donor: String,
    pattern: Vec<usize>,
    tree: Tree,
}

impl Template {
    /// `None` when a file is not UTF-8 or already contains a placeholder.
    fn capture(actual: &Tree, spellings: &Spellings) -> Option<Self> {
        let distinct = spellings.distinct_longest_first();
        let mut files = BTreeMap::new();
        for (name, bytes) in &actual.files {
            let mut text = String::from_utf8(bytes.clone()).ok()?;
            if text.contains(PLACEHOLDER_PREFIX) {
                return None;
            }
            for (index, spelling) in &distinct {
                text = text.replace(spelling, &placeholder(*index));
            }
            files.insert(name.clone(), text.into_bytes());
        }
        Some(Self {
            donor: spellings.texts[0].clone(),
            pattern: spellings.pattern.clone(),
            tree: Tree {
                directories: actual.directories.clone(),
                files,
            },
        })
    }

    /// The tree for a target with `spellings`, or `None` when its spellings
    /// coincide differently from the captured tree's.
    fn tree_for(&self, spellings: &Spellings) -> Option<Tree> {
        if spellings.pattern != self.pattern {
            return None;
        }
        let mut files = BTreeMap::new();
        for (name, bytes) in &self.tree.files {
            let mut text = String::from_utf8(bytes.clone()).ok()?;
            for (index, spelling) in spellings.distinct_longest_first() {
                text = text.replace(&placeholder(index), spelling);
            }
            if text.contains(PLACEHOLDER_PREFIX) {
                return None;
            }
            files.insert(name.clone(), text.into_bytes());
        }
        Some(Tree {
            directories: self.tree.directories.clone(),
            files,
        })
    }

    /// One `template` file: a header, then `D <dir>` lines and
    /// `F <len> <name>` records followed by `<len>` bytes and a newline.
    fn encode(&self) -> Vec<u8> {
        let pattern: Vec<String> = self.pattern.iter().map(usize::to_string).collect();
        let mut out = format!(
            "{FORMAT}\ndonor {}\npattern {}\n",
            self.donor,
            pattern.join(",")
        )
        .into_bytes();
        for directory in &self.tree.directories {
            out.extend_from_slice(format!("D {directory}\n").as_bytes());
        }
        for (name, bytes) in &self.tree.files {
            out.extend_from_slice(format!("F {} {name}\n", bytes.len()).as_bytes());
            out.extend_from_slice(bytes);
            out.push(b'\n');
        }
        out
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let mut rest = bytes;
        if next_line(&mut rest)? != FORMAT {
            return None;
        }
        let donor = next_line(&mut rest)?.strip_prefix("donor ")?.to_owned();
        let pattern = next_line(&mut rest)?
            .strip_prefix("pattern ")?
            .split(',')
            .map(|index| index.parse().ok())
            .collect::<Option<Vec<usize>>>()?;
        let mut tree = Tree::default();
        while !rest.is_empty() {
            let header = next_line(&mut rest)?;
            if let Some(directory) = header.strip_prefix("D ") {
                tree.directories.insert(directory.to_owned());
                continue;
            }
            let (length, name) = header.strip_prefix("F ")?.split_once(' ')?;
            let length: usize = length.parse().ok()?;
            if rest.len() < length + 1 || rest[length] != b'\n' {
                return None;
            }
            tree.files.insert(name.to_owned(), rest[..length].to_vec());
            rest = &rest[length + 1..];
        }
        Some(Self {
            donor,
            pattern,
            tree,
        })
    }

    fn load(entry: &Path) -> Option<Self> {
        Self::decode(&std::fs::read(entry.join("template")).ok()?)
    }

    fn store(&self, entry: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(entry)?;
        let temporary = entry.join(format!("template.tmp-{}", std::process::id()));
        std::fs::write(&temporary, self.encode())?;
        std::fs::rename(&temporary, entry.join("template")).inspect_err(|_| {
            let _ = std::fs::remove_file(&temporary);
        })
    }
}

/// The text up to the next newline of `rest`, which then starts after it.
fn next_line(rest: &mut &[u8]) -> Option<String> {
    let end = rest.iter().position(|&byte| byte == b'\n')?;
    let text = std::str::from_utf8(&rest[..end]).ok()?.to_owned();
    *rest = &rest[end + 1..];
    Some(text)
}

/// At most this many differences are named in a rejection.
const MAX_REPORTED_DIFFERENCES: usize = 8;

fn tree_differences(expected: &Tree, actual: &Tree) -> Vec<String> {
    let mut found = Vec::new();
    for directory in expected
        .directories
        .symmetric_difference(&actual.directories)
    {
        found.push(format!("directory {directory} differs"));
    }
    for (name, bytes) in &expected.files {
        match actual.files.get(name) {
            None => found.push(format!("{name} missing")),
            Some(other) if other != bytes => {
                found.push(format!("{name}: {}", first_differing_line(bytes, other)))
            }
            Some(_) => {}
        }
    }
    found.extend(
        actual
            .files
            .keys()
            .filter(|name| !expected.files.contains_key(*name))
            .map(|name| format!("{name} unexpected")),
    );
    if found.len() > MAX_REPORTED_DIFFERENCES {
        let rest = found.len() - MAX_REPORTED_DIFFERENCES;
        found.truncate(MAX_REPORTED_DIFFERENCES);
        found.push(format!("{rest} more"));
    }
    found
}

fn first_differing_line(expected: &[u8], actual: &[u8]) -> String {
    let expected = String::from_utf8_lossy(expected);
    let actual = String::from_utf8_lossy(actual);
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut number = 1;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (None, None) => return "same lines, different line endings".to_owned(),
            (left, right) if left != right => {
                return format!(
                    "line {number}: `{}` vs `{}`",
                    left.unwrap_or("<no line>"),
                    right.unwrap_or("<no line>")
                );
            }
            _ => number += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> Tree {
        Tree {
            directories: ["CMakeFiles".to_owned(), "CMakeFiles/sim.dir".to_owned()]
                .into_iter()
                .collect(),
            files: files
                .iter()
                .map(|(name, text)| ((*name).to_owned(), text.as_bytes().to_vec()))
                .collect(),
        }
    }

    fn spellings(source: &str) -> Spellings {
        let texts = (0..2)
            .flat_map(|_| {
                let forward = source.replace('\\', "/");
                [
                    source.to_owned(),
                    source.replace(':', "$:"),
                    forward.clone(),
                    forward.replace(':', "$:"),
                ]
            })
            .collect();
        Spellings::from_texts(texts)
    }

    #[test]
    fn switch_enables_only_on_explicit_on_values() {
        for value in ["1", "on", "TRUE", " yes "] {
            assert!(enabled_by(Some(value)), "{value}");
        }
        for value in [None, Some(""), Some("0"), Some("off"), Some("2")] {
            assert!(!enabled_by(value), "{value:?}");
        }
    }

    #[test]
    fn captured_trees_are_rewritten_for_another_directory() {
        let donor = spellings("/work/a/sim/tb");
        let actual = tree(&[
            ("CMakeCache.txt", "CMAKE_CACHEFILE_DIR:INTERNAL=/work/a/sim/tb/build\nCMAKE_HOME_DIRECTORY:INTERNAL=/work/a/sim/tb\n"),
            ("build.ninja", "cmake_ninja_workdir = /work/a/sim/tb/build/\nbuild x: C_COMPILER /work/a/sim/tb/model.c\n"),
        ]);
        let template = Template::capture(&actual, &donor).expect("plain text");
        for bytes in template.tree.files.values() {
            assert!(!String::from_utf8_lossy(bytes).contains("/work/a/"));
        }
        let target = spellings("/scratch/run-7/sim/tb");
        let rewritten = template.tree_for(&target).expect("same pattern");
        assert_eq!(
            rewritten,
            tree(&[
                ("CMakeCache.txt", "CMAKE_CACHEFILE_DIR:INTERNAL=/scratch/run-7/sim/tb/build\nCMAKE_HOME_DIRECTORY:INTERNAL=/scratch/run-7/sim/tb\n"),
                ("build.ninja", "cmake_ninja_workdir = /scratch/run-7/sim/tb/build/\nbuild x: C_COMPILER /scratch/run-7/sim/tb/model.c\n"),
            ])
        );
        assert!(tree_differences(&rewritten, &rewritten).is_empty());
    }

    #[test]
    fn windows_spellings_keep_their_form() {
        let donor = spellings(r"D:\a\llg-1-0\sim\tb");
        let actual = tree(&[(
            "build.ninja",
            "cmake_ninja_workdir = D$:\\a\\llg-1-0\\sim\\tb\\build\\\ninclude D:/a/llg-1-0/sim/tb/x.cmake\ncd /D D:\\a\\llg-1-0\\sim\\tb\\build\n",
        )]);
        let template = Template::capture(&actual, &donor).expect("plain text");
        let target = spellings(r"C:\t\llg-2f-3\sim\tb");
        let rewritten = template.tree_for(&target).expect("same pattern");
        assert_eq!(
            rewritten.files["build.ninja"],
            b"cmake_ninja_workdir = C$:\\t\\llg-2f-3\\sim\\tb\\build\\\ninclude C:/t/llg-2f-3/sim/tb/x.cmake\ncd /D C:\\t\\llg-2f-3\\sim\\tb\\build\n"
        );
    }

    #[test]
    fn templates_round_trip_through_their_file_format() {
        let donor = spellings("/work/a/sim/tb");
        let actual = tree(&[
            ("CMakeFiles/rules.ninja", "rule C\n  command = cc\n"),
            ("empty", ""),
            ("no-final-newline", "x"),
        ]);
        let template = Template::capture(&actual, &donor).expect("plain text");
        let decoded = Template::decode(&template.encode()).expect("decodes");
        assert_eq!(decoded.donor, template.donor);
        assert_eq!(decoded.pattern, template.pattern);
        assert_eq!(decoded.tree, template.tree);
        assert!(Template::decode(b"llg-cmake-tree-template-v0\n").is_none());
        let mut truncated = template.encode();
        truncated.truncate(truncated.len() - 3);
        assert!(Template::decode(&truncated).is_none());
    }

    #[test]
    fn differing_coincidence_patterns_refuse_the_template() {
        let donor =
            Spellings::from_texts(vec!["/a/x".into(), "/a/x".into(), "/private/a/x".into()]);
        let target = Spellings::from_texts(vec!["/b/x".into(), "/b/x".into(), "/b/x".into()]);
        let template = Template::capture(&tree(&[("f", "/a/x /private/a/x")]), &donor).unwrap();
        assert!(template.tree_for(&target).is_none());
    }

    #[test]
    fn non_text_or_placeholder_files_are_not_captured() {
        let donor = spellings("/work/a/sim/tb");
        let mut binary = tree(&[]);
        binary.files.insert("bin".to_owned(), vec![0xff, 0xfe]);
        assert!(Template::capture(&binary, &donor).is_none());
        assert!(Template::capture(&tree(&[("f", "@LLG_TREE_0@")]), &donor).is_none());
    }

    #[test]
    fn only_plain_absolute_paths_are_eligible() {
        assert!(plain_absolute(Path::new("relative/sim/tb")).is_none());
        if cfg!(windows) {
            assert!(plain_absolute(Path::new(r"C:\a\RUNNER~1\llg-1-0\sim\tb")).is_some());
            assert!(plain_absolute(Path::new(r"C:\a b\sim")).is_none());
        } else {
            assert!(plain_absolute(Path::new("/tmp/llg-x-1/sim/tb")).is_some());
            for path in [
                "/tmp/a b/sim",
                "/tmp/a$b",
                "/tmp/a@b",
                "/tmp/a:b",
                "/tmp/../x",
            ] {
                assert!(plain_absolute(Path::new(path)).is_none(), "{path}");
            }
        }
    }

    #[test]
    fn differences_name_files_and_directories() {
        let expected = tree(&[("a", "1\n2\n"), ("b", "x")]);
        let mut actual = tree(&[("a", "1\n3\n"), ("c", "y")]);
        actual.directories.insert("bin".to_owned());
        let found = tree_differences(&expected, &actual);
        assert_eq!(
            found,
            vec![
                "directory bin differs".to_owned(),
                "a: line 2: `2` vs `3`".to_owned(),
                "b missing".to_owned(),
                "c unexpected".to_owned(),
            ]
        );
    }

    #[test]
    fn markers_admit_only_the_exact_key() {
        let dir =
            std::env::temp_dir().join(format!("llg-tree-template-marker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_marker(&dir, "ready", "key one");
        assert!(marker_matches(&dir, "ready", "key one"));
        assert!(!marker_matches(&dir, "ready", "key two"));
        write_marker(&dir, "rejected", "reason\nkey one");
        assert!(marker_matches(&dir, "rejected", "key one"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
