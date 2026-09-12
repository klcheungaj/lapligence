use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct PatchError(String);

impl PatchError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PatchError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchState {
    Clean,
    Applied,
    Mismatch,
}

#[derive(Debug)]
struct PatchSpec {
    path: PathBuf,
    files: Vec<FilePatch>,
}

#[derive(Debug)]
struct FilePatch {
    path: PathBuf,
    hunks: Vec<Hunk>,
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<DiffLine>,
}

#[derive(Debug)]
enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

#[derive(Debug)]
struct SourceText {
    lines: Vec<String>,
    newline: &'static str,
    final_newline: bool,
}

#[derive(Debug)]
struct PatchPlan {
    path: PathBuf,
    state: PatchState,
    writes: Vec<(PathBuf, String)>,
}

/// Emit Cargo dependencies for repository-owned patches and the embedded
/// libaco sources. Directory dependencies cover additions/removals; individual
/// patch files make the relevant inputs explicit in Cargo diagnostics.
pub fn emit_rerun_if_changed(manifest_dir: &Path) {
    for relative in ["patches/slang", "patches/libaco", "vendor/libaco"] {
        println!(
            "cargo:rerun-if-changed={}",
            manifest_dir.join(relative).display()
        );
    }
    for relative in ["patches/slang", "patches/libaco"] {
        let directory = manifest_dir.join(relative);
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("patch") {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

/// Apply every repository-owned vendor patch, accepting only an entirely
/// clean or entirely-applied submodule checkout.
pub fn apply_all(manifest_dir: &Path) -> Result<(), PatchError> {
    apply_directory(
        "Slang",
        &manifest_dir.join("vendor/slang"),
        &manifest_dir.join("patches/slang"),
    )?;
    apply_directory(
        "libaco",
        &manifest_dir.join("vendor/libaco"),
        &manifest_dir.join("patches/libaco"),
    )?;
    Ok(())
}

fn apply_directory(label: &str, repository: &Path, patches_dir: &Path) -> Result<(), PatchError> {
    let entries = fs::read_dir(patches_dir).map_err(|error| {
        PatchError::new(format!(
            "cannot read {label} patch directory {}: {error}",
            patches_dir.display()
        ))
    })?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("patch"))
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        return Err(PatchError::new(format!(
            "no {label} vendor patches found in {}; restore the tracked patch files",
            patches_dir.display()
        )));
    }

    let mut plans = Vec::with_capacity(paths.len());
    let mut targets = BTreeSet::new();
    for path in paths {
        let text = fs::read_to_string(&path).map_err(|error| {
            PatchError::new(format!(
                "cannot read vendor patch {}: {error}",
                path.display()
            ))
        })?;
        let spec = parse_patch(&path, &text)?;
        let plan = plan_patch(repository, spec)?;
        for (target, _) in &plan.writes {
            if !targets.insert(target.clone()) {
                return Err(PatchError::new(format!(
                    "{label} vendor patch {} overlaps another patch at {}",
                    plan.path.display(),
                    target.display()
                )));
            }
        }
        plans.push(plan);
    }

    let all_clean = plans.iter().all(|plan| plan.state == PatchState::Clean);
    let all_applied = plans.iter().all(|plan| plan.state == PatchState::Applied);
    if all_applied {
        return Ok(());
    }
    if !all_clean {
        let states = plans
            .iter()
            .map(|plan| format!("{}={:?}", plan.path.display(), plan.state))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PatchError::new(format!(
            "{label} vendor checkout {} is partially applied or mismatched ({states}); check out the documented base revision, or restore the complete applied state",
            repository.display()
        )));
    }

    for plan in plans {
        for (path, contents) in plan.writes {
            fs::write(&path, contents).map_err(|error| {
                PatchError::new(format!(
                    "cannot apply vendor patch {} to {}: {error}",
                    plan.path.display(),
                    path.display()
                ))
            })?;
        }
    }
    Ok(())
}

fn plan_patch(repository: &Path, spec: PatchSpec) -> Result<PatchPlan, PatchError> {
    let mut states = Vec::with_capacity(spec.files.len());
    let mut writes = Vec::new();
    for file in &spec.files {
        let path = repository.join(&file.path);
        let contents = fs::read_to_string(&path).map_err(|error| {
            PatchError::new(format!(
                "cannot read vendor file {} for patch {}: {error}",
                path.display(),
                spec.path.display()
            ))
        })?;
        let source = SourceText::parse(&contents);
        let clean = file
            .hunks
            .iter()
            .all(|hunk| hunk_matches(&source.lines, hunk.old_start, &hunk.old_lines()));
        let applied = file
            .hunks
            .iter()
            .all(|hunk| hunk_matches(&source.lines, hunk.new_start, &hunk.new_lines()));
        let state = match (clean, applied) {
            (true, false) => PatchState::Clean,
            (false, true) => PatchState::Applied,
            _ => PatchState::Mismatch,
        };
        states.push(state);
        if state == PatchState::Clean {
            writes.push((path, apply_file(&source, file, &spec.path)?));
        }
    }
    let state = if states.iter().all(|state| *state == PatchState::Clean) {
        PatchState::Clean
    } else if states.iter().all(|state| *state == PatchState::Applied) {
        PatchState::Applied
    } else {
        PatchState::Mismatch
    };
    Ok(PatchPlan {
        path: spec.path,
        state,
        writes: if state == PatchState::Clean {
            writes
        } else {
            Vec::new()
        },
    })
}

fn apply_file(
    source: &SourceText,
    file: &FilePatch,
    patch_path: &Path,
) -> Result<String, PatchError> {
    let mut lines = source.lines.clone();
    let mut offset = 0isize;
    for hunk in &file.hunks {
        let index = adjusted_index(hunk.old_start, offset).ok_or_else(|| {
            PatchError::new(format!(
                "patch {} has an invalid line offset for {}",
                patch_path.display(),
                file.path.display()
            ))
        })?;
        let old_lines = hunk.old_lines();
        let new_lines = hunk.new_lines();
        if !hunk_matches(&lines, index.saturating_add(1), &old_lines) {
            return Err(PatchError::new(format!(
                "patch {} changed while applying {} at old line {}",
                patch_path.display(),
                file.path.display(),
                hunk.old_start
            )));
        }
        lines.splice(index..index + old_lines.len(), new_lines);
        offset += hunk.new_count as isize - hunk.old_count as isize;
    }
    Ok(source.render(&lines))
}

fn adjusted_index(start: usize, offset: isize) -> Option<usize> {
    let base = start.checked_sub(1)? as isize;
    base.checked_add(offset)
        .filter(|index| *index >= 0)
        .map(|index| index as usize)
}

fn hunk_matches(lines: &[String], start: usize, expected: &[String]) -> bool {
    let Some(index) = start.checked_sub(1) else {
        return expected.is_empty();
    };
    lines
        .get(index..index.saturating_add(expected.len()))
        .is_some_and(|actual| actual == expected)
}

impl Hunk {
    fn old_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                DiffLine::Context(value) | DiffLine::Removed(value) => Some(value.clone()),
                DiffLine::Added(_) => None,
            })
            .collect()
    }

    fn new_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                DiffLine::Context(value) | DiffLine::Added(value) => Some(value.clone()),
                DiffLine::Removed(_) => None,
            })
            .collect()
    }
}

impl SourceText {
    fn parse(contents: &str) -> Self {
        let final_newline = contents.ends_with('\n');
        let newline = if contents.as_bytes().windows(2).any(|pair| pair == b"\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut lines = if contents.is_empty() {
            Vec::new()
        } else {
            contents
                .split('\n')
                .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
                .collect::<Vec<_>>()
        };
        if final_newline {
            lines.pop();
        }
        Self {
            lines,
            newline,
            final_newline,
        }
    }

    fn render(&self, lines: &[String]) -> String {
        let mut rendered = lines.join(self.newline);
        if self.final_newline {
            rendered.push_str(self.newline);
        }
        rendered
    }
}

fn parse_patch(path: &Path, text: &str) -> Result<PatchSpec, PatchError> {
    let mut files = Vec::new();
    let mut current: Option<FilePatch> = None;
    let mut current_hunk: Option<Hunk> = None;

    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.starts_with("diff --git ") {
            finish_hunk(path, &mut current, &mut current_hunk)?;
            finish_file(path, &mut files, &mut current)?;
            continue;
        }
        if let Some(raw_path) = line.strip_prefix("--- ") {
            finish_hunk(path, &mut current, &mut current_hunk)?;
            finish_file(path, &mut files, &mut current)?;
            current = Some(FilePatch {
                path: parse_diff_path(path, raw_path, "a/")?,
                hunks: Vec::new(),
            });
            continue;
        }
        if let Some(raw_path) = line.strip_prefix("+++ ") {
            if current.is_none() {
                return Err(PatchError::new(format!(
                    "patch {} has a new-file header without an old-file header",
                    path.display()
                )));
            }
            let new_path = parse_diff_path(path, raw_path, "b/")?;
            let old_path = current.as_ref().map(|file| file.path.clone()).unwrap();
            if old_path != new_path {
                return Err(PatchError::new(format!(
                    "patch {} changes paths from {} to {}; renames are unsupported",
                    path.display(),
                    old_path.display(),
                    new_path.display()
                )));
            }
            continue;
        }
        if line.starts_with("@@") {
            finish_hunk(path, &mut current, &mut current_hunk)?;
            let (old_start, old_count, new_start, new_count) = parse_hunk_header(path, line)?;
            if current.is_none() {
                return Err(PatchError::new(format!(
                    "patch {} has a hunk without a file header",
                    path.display()
                )));
            }
            current_hunk = Some(Hunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: Vec::new(),
            });
            continue;
        }
        if let Some(hunk) = current_hunk.as_mut() {
            if line == "\\ No newline at end of file" {
                continue;
            }
            if line.is_empty() {
                continue;
            }
            let Some((marker, value)) = line.split_at_checked(1) else {
                return Err(PatchError::new(format!(
                    "patch {} contains an empty hunk line",
                    path.display()
                )));
            };
            let value = value.to_string();
            match marker {
                " " => hunk.lines.push(DiffLine::Context(value)),
                "+" => hunk.lines.push(DiffLine::Added(value)),
                "-" => hunk.lines.push(DiffLine::Removed(value)),
                _ => {
                    return Err(PatchError::new(format!(
                        "patch {} contains invalid hunk marker {:?}",
                        path.display(),
                        marker
                    )))
                }
            }
        }
    }
    finish_hunk(path, &mut current, &mut current_hunk)?;
    finish_file(path, &mut files, &mut current)?;
    if files.is_empty() {
        return Err(PatchError::new(format!(
            "patch {} contains no unified-diff files",
            path.display()
        )));
    }
    Ok(PatchSpec {
        path: path.to_path_buf(),
        files,
    })
}

fn finish_hunk(
    path: &Path,
    current: &mut Option<FilePatch>,
    hunk: &mut Option<Hunk>,
) -> Result<(), PatchError> {
    let Some(hunk_value) = hunk.take() else {
        return Ok(());
    };
    let old_seen = hunk_value
        .lines
        .iter()
        .filter(|line| !matches!(line, DiffLine::Added(_)))
        .count();
    let new_seen = hunk_value
        .lines
        .iter()
        .filter(|line| !matches!(line, DiffLine::Removed(_)))
        .count();
    if old_seen != hunk_value.old_count || new_seen != hunk_value.new_count {
        return Err(PatchError::new(format!(
            "patch {} hunk line counts disagree (old header {}, saw {}; new header {}, saw {})",
            path.display(),
            hunk_value.old_count,
            old_seen,
            hunk_value.new_count,
            new_seen
        )));
    }
    let Some(file) = current.as_mut() else {
        return Err(PatchError::new(format!(
            "patch {} has a hunk without a file header",
            path.display()
        )));
    };
    file.hunks.push(hunk_value);
    Ok(())
}

fn finish_file(
    path: &Path,
    files: &mut Vec<FilePatch>,
    current: &mut Option<FilePatch>,
) -> Result<(), PatchError> {
    let Some(file) = current.take() else {
        return Ok(());
    };
    if file.hunks.is_empty() {
        return Err(PatchError::new(format!(
            "patch {} has no hunks for {}",
            path.display(),
            file.path.display()
        )));
    }
    files.push(file);
    Ok(())
}

fn parse_diff_path(patch: &Path, raw: &str, prefix: &str) -> Result<PathBuf, PatchError> {
    let token = raw.split_ascii_whitespace().next().unwrap_or_default();
    let Some(relative) = token.strip_prefix(prefix) else {
        return Err(PatchError::new(format!(
            "patch {} has an unsupported path header {raw:?}",
            patch.display()
        )));
    };
    let path = PathBuf::from(relative);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PatchError::new(format!(
            "patch {} contains an unsafe path {relative:?}",
            patch.display()
        )));
    }
    Ok(path)
}

fn parse_hunk_header(patch: &Path, line: &str) -> Result<(usize, usize, usize, usize), PatchError> {
    let end = line[2..].find("@@").map(|index| index + 2).ok_or_else(|| {
        PatchError::new(format!(
            "patch {} has an invalid hunk header {line:?}",
            patch.display()
        ))
    })?;
    let mut ranges = line[2..end].split_ascii_whitespace();
    let old = ranges.next().ok_or_else(|| {
        PatchError::new(format!(
            "patch {} has an incomplete hunk header {line:?}",
            patch.display()
        ))
    })?;
    let new = ranges.next().ok_or_else(|| {
        PatchError::new(format!(
            "patch {} has an incomplete hunk header {line:?}",
            patch.display()
        ))
    })?;
    let (old_start, old_count) = parse_range(patch, old, '-')?;
    let (new_start, new_count) = parse_range(patch, new, '+')?;
    Ok((old_start, old_count, new_start, new_count))
}

fn parse_range(patch: &Path, token: &str, marker: char) -> Result<(usize, usize), PatchError> {
    let Some(range) = token.strip_prefix(marker) else {
        return Err(PatchError::new(format!(
            "patch {} has an invalid hunk range {token:?}",
            patch.display()
        )));
    };
    let (start, count) = range
        .split_once(',')
        .map_or((range, "1"), |(start, count)| (start, count));
    let start = start.parse::<usize>().map_err(|_| {
        PatchError::new(format!(
            "patch {} has an invalid hunk start {token:?}",
            patch.display()
        ))
    })?;
    let count = count.parse::<usize>().map_err(|_| {
        PatchError::new(format!(
            "patch {} has an invalid hunk count {token:?}",
            patch.display()
        ))
    })?;
    Ok((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PATCH: &str = "diff --git a/alpha.txt b/alpha.txt\n--- a/alpha.txt\n+++ b/alpha.txt\n@@ -1,2 +1,2 @@\n one\n-old\n+new\n";

    fn temporary_tree() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "llg-vendor-patches-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("patches")).expect("create temporary patch tree");
        path
    }

    #[test]
    fn clean_checkout_applies_and_applied_checkout_is_accepted() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");

        apply_directory("test", &repository, &patches).expect("clean patch applies");
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nnew\n"
        );
        apply_directory("test", &repository, &patches).expect("applied patch is accepted");

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn mismatched_checkout_is_rejected_without_writing() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nother\n").expect("write source");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");

        let error = apply_directory("test", &repository, &patches).expect_err("mismatch fails");
        assert!(error
            .to_string()
            .contains("partially applied or mismatched"));
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nother\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }
}
