use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt as CapMetadataExt, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions as CapOpenOptions};

#[cfg(target_os = "linux")]
use cap_std::fs::OpenOptionsExt as CapOpenOptionsExt;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
use cap_std::fs::OpenOptionsExt as CapOpenOptionsExt;

const SLANG_BASE_REVISION: &str = "7ddf4059f79eff508dd486eb42fd650cdf320d52";
const FILE_MANIFEST: &str = "files.sha256";
const RETIRED_MANIFEST: &str = "retired-files.sha256";
const SLANG_RETIRED_ENTRIES: &[(&str, &str)] = &[(
    "source/numeric/SVInt.cpp",
    "54af15a814a7982fd5e07d0e15af8c27a52bd17069cf1abb6c54452a4a398cfb",
)];
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicReplacePoint {
    AfterRepositoryOpen,
    AfterParentOpen,
    AfterTemporaryOpen,
    AfterTemporaryCheck,
    BeforeRename,
}

type FileIdentity = (u64, u64);

struct DirectoryAnchor {
    directory: Dir,
    path: PathBuf,
    identity: FileIdentity,
    parent: Option<Arc<Self>>,
    name: Option<PathBuf>,
}

struct StagedFile {
    file: Option<cap_std::fs::File>,
    name: PathBuf,
    anonymous: bool,
}

fn staged_file<'a>(
    staged: &'a StagedFile,
    patch_path: &Path,
) -> Result<&'a cap_std::fs::File, PatchError> {
    staged.file.as_ref().ok_or_else(|| {
        PatchError::new(format!(
            "vendor patch {} lost its staging handle before writing",
            patch_path.display()
        ))
    })
}

fn staged_file_mut<'a>(
    staged: &'a mut StagedFile,
    patch_path: &Path,
) -> Result<&'a mut cap_std::fs::File, PatchError> {
    staged.file.as_mut().ok_or_else(|| {
        PatchError::new(format!(
            "vendor patch {} lost its staging handle before writing",
            patch_path.display()
        ))
    })
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
    writes: Vec<WritePlan>,
}

#[derive(Debug)]
struct WritePlan {
    relative: PathBuf,
    path: PathBuf,
    contents: String,
    base: [u8; 32],
    applied: [u8; 32],
}

#[derive(Debug)]
struct FileManifest {
    entries: BTreeMap<PathBuf, FileExpectation>,
}

#[derive(Debug, Clone, Copy)]
struct FileExpectation {
    base: [u8; 32],
    applied: [u8; 32],
}

#[derive(Debug)]
struct RetiredManifest {
    entries: BTreeMap<PathBuf, [u8; 32]>,
}

/// Emit Cargo dependencies for repository-owned patches. Directory dependencies
/// cover additions/removals; individual patch files make the relevant inputs
/// explicit in Cargo diagnostics.
pub fn emit_rerun_if_changed(manifest_dir: &Path) {
    let directory = manifest_dir.join("patches/slang");
    println!("cargo:rerun-if-changed={}", directory.display());
    for metadata in [FILE_MANIFEST, RETIRED_MANIFEST] {
        let path = directory.join(metadata);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let Ok(entries) = fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("patch") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Apply every repository-owned vendor patch, accepting only an entirely
/// clean or entirely-applied submodule checkout.
pub fn apply_all(manifest_dir: &Path) -> Result<(), PatchError> {
    apply_directory_with_base(
        "Slang",
        manifest_dir,
        &manifest_dir.join("vendor/slang"),
        &manifest_dir.join("patches/slang"),
        Some(SLANG_BASE_REVISION),
    )?;
    Ok(())
}

#[cfg(test)]
fn apply_directory(label: &str, repository: &Path, patches_dir: &Path) -> Result<(), PatchError> {
    let project_root = repository.parent().unwrap_or(repository);
    apply_directory_with_base(label, project_root, repository, patches_dir, None)
}

fn apply_directory_with_base(
    label: &str,
    project_root: &Path,
    repository: &Path,
    patches_dir: &Path,
    expected_base: Option<&str>,
) -> Result<(), PatchError> {
    let repository = canonical_repository(label, project_root, repository)?;
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

    let manifest = FileManifest::read(&patches_dir.join(FILE_MANIFEST), label)?;
    let retired_path = patches_dir.join(RETIRED_MANIFEST);
    let expected_retired = expected_retired_entries(label);
    let retired = RetiredManifest::read(&retired_path, label, expected_retired.is_some())?;
    if let Some(expected) = expected_retired {
        retired.validate_exact(&retired_path, label, expected)?;
    }

    let mut specs = Vec::with_capacity(paths.len());
    let mut expected_paths = BTreeSet::new();
    for path in paths {
        validate_regular_file(&path, label, "vendor patch input")?;
        let text = fs::read_to_string(&path).map_err(|error| {
            PatchError::new(format!(
                "cannot read vendor patch {}: {error}",
                path.display()
            ))
        })?;
        let spec = parse_patch(&path, &text)?;
        expected_paths.extend(spec.files.iter().map(|file| file.path.clone()));
        specs.push(spec);
    }
    manifest.validate_paths(label, &expected_paths, &retired.entries)?;
    validate_retired_files(label, &repository, &retired)?;
    validate_git_checkout(label, &repository, expected_base, &expected_paths)?;

    let mut plans = Vec::with_capacity(specs.len());
    let mut targets = BTreeSet::new();
    for spec in specs {
        let plan = plan_patch(label, &repository, spec, &manifest)?;
        for write in &plan.writes {
            if !targets.insert(write.path.clone()) {
                return Err(PatchError::new(format!(
                    "{label} vendor patch {} overlaps another patch at {}",
                    plan.path.display(),
                    write.path.display()
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
        for write in plan.writes {
            apply_write(label, &repository, &plan.path, write)?;
        }
    }
    Ok(())
}

fn expected_retired_entries(label: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match label {
        "Slang" => Some(SLANG_RETIRED_ENTRIES),
        _ => None,
    }
}

fn apply_write(
    label: &str,
    repository: &Path,
    patch_path: &Path,
    write: WritePlan,
) -> Result<(), PatchError> {
    let path = safe_vendor_target_path(label, repository, &write.relative, "patch target")?;
    if path != write.path {
        return Err(PatchError::new(format!(
            "{label} vendor patch {} target {} changed identity while preparing the atomic write; restore the vendor checkout",
            patch_path.display(),
            write.relative.display()
        )));
    }
    if canonical_digest(write.contents.as_bytes()) != write.applied {
        return Err(PatchError::new(format!(
            "vendor patch {} rendered {} with a digest different from the authenticated applied manifest; the patch input was modified",
            patch_path.display(),
            write.relative.display()
        )));
    }
    atomic_replace(
        label,
        repository,
        &path,
        &write.contents,
        write.base,
        write.applied,
        patch_path,
    )
}

fn atomic_replace(
    label: &str,
    repository: &Path,
    path: &Path,
    contents: &str,
    base: [u8; 32],
    applied: [u8; 32],
    patch_path: &Path,
) -> Result<(), PatchError> {
    atomic_replace_with_hook(
        label,
        repository,
        path,
        contents,
        base,
        applied,
        patch_path,
        &mut |_, _| {},
    )
}

// The hook keeps the security-sensitive points deterministic in regression
// tests while the production wrapper supplies a no-op callback.
#[allow(clippy::too_many_arguments)]
fn atomic_replace_with_hook<F>(
    label: &str,
    repository: &Path,
    path: &Path,
    contents: &str,
    base: [u8; 32],
    applied: [u8; 32],
    patch_path: &Path,
    hook: &mut F,
) -> Result<(), PatchError>
where
    F: FnMut(AtomicReplacePoint, PathBuf),
{
    let relative = path.strip_prefix(repository).map_err(|_| {
        PatchError::new(format!(
            "{label} vendor patch target {} is outside repository {}; restore the checkout",
            path.display(),
            repository.display()
        ))
    })?;
    let file_name = relative.file_name().ok_or_else(|| {
        PatchError::new(format!(
            "{label} vendor patch {} has no file name for atomic replacement",
            relative.display()
        ))
    })?;

    // Open the repository and every target parent as anchored capabilities.
    // The attachment checks below make a moved capability unusable: an open
    // directory remains valid after its path is renamed, but it must not be
    // used as a write authority once it is no longer attached to the vendor
    // tree.
    let repository_dir = open_verified_directory(label, repository, "vendor repository")?;
    hook(
        AtomicReplacePoint::AfterRepositoryOpen,
        repository.to_path_buf(),
    );
    repository_dir.verify_attached(label, "vendor repository")?;
    let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
    let parent_dir = open_relative_directory(
        label,
        &repository_dir,
        parent_relative,
        "atomic replacement parent",
    )?;
    hook(AtomicReplacePoint::AfterParentOpen, parent_dir.path.clone());
    parent_dir.verify_attached(label, "atomic replacement parent")?;
    let target_name = Path::new(file_name);
    let mut target_file = open_regular_file(
        &parent_dir.directory,
        target_name,
        label,
        "atomic replacement target",
    )?;
    let target_metadata = target_file.metadata().map_err(|error| {
        PatchError::new(format!(
            "cannot inspect vendor target {} before patch {}: {error}",
            path.display(),
            patch_path.display()
        ))
    })?;
    let target_identity = file_identity(&target_metadata);
    if target_metadata.nlink() > 1 {
        return Err(PatchError::new(format!(
            "{label} atomic replacement target {} is a hard link; refusing to modify a shared vendor file",
            path.display()
        )));
    }
    let current = read_cap_file(&mut target_file, path, patch_path)?;
    if canonical_digest(&current) != base {
        return Err(PatchError::new(format!(
            "vendor file {} changed after planning patch {}; refusing to replace it",
            path.display(),
            patch_path.display()
        )));
    }

    let mut staged = create_staged_file(&parent_dir.directory, label, patch_path)?;
    hook(AtomicReplacePoint::AfterTemporaryOpen, staged.name.clone());

    let result = (|| {
        parent_dir.verify_attached(label, "atomic replacement parent")?;
        let staged_handle = staged_file(&staged, patch_path)?;
        if staged.anonymous {
            ensure_anonymous_staging_file(staged_handle, label, &staged.name)?;
        } else {
            ensure_private_staging_file(
                &parent_dir.directory,
                &staged.name,
                staged_handle,
                label,
                "before updating staged atomic replacement permissions",
            )?;
        }
        staged_handle
            .set_permissions(target_metadata.permissions())
            .map_err(|error| {
                PatchError::new(format!(
                    "cannot preserve permissions for vendor patch {} in {}: {error}",
                    patch_path.display(),
                    staged.name.display()
                ))
            })?;
        if staged.anonymous {
            ensure_anonymous_staging_file(staged_handle, label, &staged.name)?;
        } else {
            ensure_private_staging_file(
                &parent_dir.directory,
                &staged.name,
                staged_handle,
                label,
                "before writing staged atomic replacement",
            )?;
        }
        hook(AtomicReplacePoint::AfterTemporaryCheck, staged.name.clone());
        if staged.anonymous {
            ensure_anonymous_staging_file(staged_file(&staged, patch_path)?, label, &staged.name)?;
        } else {
            ensure_private_staging_file(
                &parent_dir.directory,
                &staged.name,
                staged_file(&staged, patch_path)?,
                label,
                "after checking staged atomic replacement",
            )?;
        }
        staged_file_mut(&mut staged, patch_path)?
            .write_all(contents.as_bytes())
            .map_err(|error| {
                PatchError::new(format!(
                    "cannot stage vendor patch {} in {}: {error}",
                    patch_path.display(),
                    staged.name.display()
                ))
            })?;
        staged_file(&staged, patch_path)?
            .sync_all()
            .map_err(|error| {
                PatchError::new(format!(
                    "cannot flush staged vendor patch {} in {}: {error}",
                    patch_path.display(),
                    staged.name.display()
                ))
            })?;

        if !staged.anonymous {
            let staged_handle = staged_file(&staged, patch_path)?;
            ensure_private_staging_file(
                &parent_dir.directory,
                &staged.name,
                staged_handle,
                label,
                "after writing staged atomic replacement",
            )?;
        }

        // Keep the staging inode anonymous until the repository and target
        // parent are re-opened from the anchored repository. If a parent was
        // moved out of the vendor tree while the contents were being staged,
        // this fails before the inode is linked into that external tree.
        hook(AtomicReplacePoint::BeforeRename, staged.name.clone());
        repository_dir.verify_attached(label, "vendor repository")?;
        let attached_parent = open_relative_directory(
            label,
            &repository_dir,
            parent_relative,
            "atomic replacement parent before rename",
        )?;
        if attached_parent.identity != parent_dir.identity {
            return Err(PatchError::new(format!(
                "{label} atomic replacement parent {} changed before rename; refusing to write",
                parent_dir.path.display()
            )));
        }
        attached_parent.verify_attached(label, "atomic replacement parent before rename")?;

        let staged_identity = file_identity(
            &staged_file(&staged, patch_path)?
                .metadata()
                .map_err(|error| {
                    PatchError::new(format!(
                        "{label} cannot inspect staged vendor patch {}: {error}",
                        staged.name.display()
                    ))
                })?,
        );
        if staged.anonymous {
            publish_anonymous_staging_file(
                &attached_parent.directory,
                staged_file(&staged, patch_path)?,
                &staged.name,
                label,
            )?;
        } else {
            // Windows keeps the named staging handle closed before rename so
            // the exclusive share mode used during staging does not block the
            // final directory operation.
            staged.file.take();
        }
        ensure_published_staging_file(
            &attached_parent.directory,
            &staged.name,
            staged_identity,
            label,
        )?;

        // Re-open the target through the freshly attached parent handle
        // immediately before the rename. A replacement target is safe to
        // overwrite; a symlink or hard link is rejected, and a changed clean
        // digest fails closed.
        let mut current_target = open_regular_file(
            &attached_parent.directory,
            target_name,
            label,
            "atomic replacement target",
        )?;
        let current_metadata = current_target.metadata().map_err(|error| {
            PatchError::new(format!(
                "cannot inspect vendor target {} before patch {}: {error}",
                path.display(),
                patch_path.display()
            ))
        })?;
        if current_metadata.nlink() > 1 {
            return Err(PatchError::new(format!(
                "{label} atomic replacement target {} became a hard link; refusing to modify a shared vendor file",
                path.display(),
            )));
        }
        if file_identity(&current_metadata) != target_identity {
            return Err(PatchError::new(format!(
                "vendor file {} changed before atomic replacement for patch {}; refusing to overwrite it",
                path.display(),
                patch_path.display()
            )));
        }
        let current = read_cap_file(&mut current_target, path, patch_path)?;
        if canonical_digest(&current) != base {
            return Err(PatchError::new(format!(
                "vendor file {} changed before atomic replacement for patch {}; refusing to overwrite it",
                path.display(),
                patch_path.display()
            )));
        }
        if canonical_digest(&current) == applied {
            return Err(PatchError::new(format!(
                "vendor file {} became applied before patch {}; refusing a duplicate write",
                path.display(),
                patch_path.display()
            )));
        }

        attached_parent
            .directory
            .rename(&staged.name, &attached_parent.directory, target_name)
            .map_err(|error| {
                PatchError::new(format!(
                    "cannot atomically replace vendor file {} for patch {}: {error}; the vendor checkout was not updated",
                    path.display(),
                    patch_path.display()
                ))
            })?;

        let mut final_target = open_regular_file(
            &attached_parent.directory,
            target_name,
            label,
            "applied vendor target",
        )?;
        let final_metadata = final_target.metadata().map_err(|error| {
            PatchError::new(format!(
                "cannot inspect applied vendor file {}: {error}",
                path.display(),
            ))
        })?;
        if final_metadata.nlink() > 1 {
            return Err(PatchError::new(format!(
                "vendor file {} became a hard link after patch {}; refusing to accept the replacement",
                path.display(),
                patch_path.display()
            )));
        }
        let final_contents = read_cap_file(&mut final_target, path, patch_path)?;
        if canonical_digest(&final_contents) != applied {
            return Err(PatchError::new(format!(
                "vendor file {} did not retain its authenticated applied digest after patch {}",
                path.display(),
                patch_path.display()
            )));
        }

        // The file is durable before the directory entry is changed. Flush a
        // cloned parent handle after rename so the replacement survives a
        // crash on filesystems which require directory metadata syncing.
        sync_directory(&attached_parent.directory).map_err(|error| {
            PatchError::new(format!(
                "cannot flush vendor parent directory after patch {}: {error}",
                patch_path.display()
            ))
        })?;
        Ok(())
    })();
    if result.is_err() {
        // Do not remove a staging path through a detached capability. An
        // attacker can move that capability outside the repository; leaving
        // our private temporary entry behind is safer than mutating the
        // external directory during cleanup.
        if parent_dir
            .verify_attached(label, "atomic replacement cleanup")
            .is_ok()
        {
            let _ = parent_dir.directory.remove_file(&staged.name);
        }
    }
    result
}

#[cfg(unix)]
fn sync_directory(directory: &Dir) -> io::Result<()> {
    use rustix::fs::{fsync, openat, Mode, OFlags};

    // cap-std uses O_PATH for directory capabilities on Linux. Re-open the
    // directory through that capability with a normal directory descriptor so
    // fsync is available, without resolving the path through ambient state.
    let readable = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    )?;
    Ok(fsync(&readable)?)
}

// FlushFileBuffers needs a writable handle, but cap-std opens directories
// read-only. NTFS journals the rename itself and the staged contents were
// flushed before it, so there is no separate directory flush on Windows.
#[cfg(windows)]
fn sync_directory(_directory: &Dir) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn sync_directory(directory: &Dir) -> io::Result<()> {
    directory.try_clone()?.into_std_file().sync_all()
}

fn temporary_name() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from(format!(
        ".llg-vendor-patch-{}-{timestamp}-{counter}.tmp",
        std::process::id()
    ))
}

#[cfg(target_os = "linux")]
fn create_staged_file(
    parent: &Dir,
    label: &str,
    patch_path: &Path,
) -> Result<StagedFile, PatchError> {
    let name = temporary_name();

    // Linux's O_TMPFILE creates an inode without a directory entry. This is
    // the only point at which the staged contents are written, so keeping the
    // inode anonymous closes the hard-link window between an identity check
    // and write(2). It is published later with linkat(AT_EMPTY_PATH) after
    // the repository capabilities have been checked again.
    use rustix::fs::OFlags;

    let mut options = CapOpenOptions::new();
    options
        .read(true)
        .write(true)
        .follow(FollowSymlinks::No)
        .custom_flags((OFlags::TMPFILE | OFlags::DIRECTORY).bits() as i32);
    let file = parent.open_with(".", &options).map_err(|error| {
        PatchError::new(format!(
            "{label} cannot create an anonymous staging file for vendor patch {}: {error}; the filesystem must support anonymous temporary files",
            patch_path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} cannot inspect anonymous staging file for vendor patch {}: {error}",
            patch_path.display()
        ))
    })?;
    if metadata.nlink() != 0 {
        return Err(PatchError::new(format!(
            "{label} anonymous staging file {} has an unexpected directory link; refusing to write",
            name.display()
        )));
    }
    Ok(StagedFile {
        file: Some(file),
        name,
        anonymous: true,
    })
}

#[cfg(not(target_os = "linux"))]
fn create_staged_file(
    parent: &Dir,
    label: &str,
    patch_path: &Path,
) -> Result<StagedFile, PatchError> {
    let name = temporary_name();
    let mut options = CapOpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(windows)]
    options.share_mode(0);
    let file = parent.open_with(&name, &options).map_err(|error| {
        PatchError::new(format!(
            "{label} cannot stage vendor patch {} in {}: {error}",
            patch_path.display(),
            name.display()
        ))
    })?;

    Ok(StagedFile {
        file: Some(file),
        name,
        anonymous: false,
    })
}

fn ensure_anonymous_staging_file(
    file: &cap_std::fs::File,
    label: &str,
    name: &Path,
) -> Result<(), PatchError> {
    let metadata = file.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} cannot inspect detached staging file {}: {error}",
            name.display()
        ))
    })?;
    if metadata.nlink() != 0 {
        return Err(PatchError::new(format!(
            "{label} staging file {} gained a hard link; refusing to write",
            name.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn publish_anonymous_staging_file(
    parent: &Dir,
    file: &cap_std::fs::File,
    name: &Path,
    label: &str,
) -> Result<(), PatchError> {
    use std::os::fd::AsRawFd;

    use rustix::fs::{linkat, AtFlags, CWD};

    // Linux and the BSDs which expose AT_EMPTY_PATH can link the open inode
    // directly. Some kernels/filesystems reject that form without the
    // CAP_DAC_READ_SEARCH capability, so retain the descriptor-relative procfs
    // form as a safe fallback.
    #[cfg(any(target_os = "freebsd", target_os = "fuchsia", target_os = "linux"))]
    match linkat(file, "", parent, name, AtFlags::EMPTY_PATH) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(PatchError::new(format!(
                "{label} staging destination {} already exists; refusing to replace it",
                name.display()
            )))
        }
        Err(_) => {}
    }

    let descriptor_path = if cfg!(any(target_os = "macos", target_os = "ios")) {
        PathBuf::from("/dev/fd").join(file.as_raw_fd().to_string())
    } else {
        PathBuf::from("/proc/self/fd").join(file.as_raw_fd().to_string())
    };
    linkat(CWD, &descriptor_path, parent, name, AtFlags::SYMLINK_FOLLOW).map_err(|error| {
        PatchError::new(format!(
            "{label} cannot publish detached staging file {}: {error}",
            name.display()
        ))
    })
}

#[cfg(not(unix))]
fn publish_anonymous_staging_file(
    _parent: &Dir,
    _file: &cap_std::fs::File,
    name: &Path,
    label: &str,
) -> Result<(), PatchError> {
    Err(PatchError::new(format!(
        "{label} cannot publish anonymous staging file {}; platform does not provide a safe link operation",
        name.display()
    )))
}

fn ensure_published_staging_file(
    parent: &Dir,
    name: &Path,
    expected: FileIdentity,
    label: &str,
) -> Result<(), PatchError> {
    let file = open_regular_file(parent, name, label, "published staging file")?;
    let metadata = file.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} cannot inspect published staging file {}: {error}",
            name.display()
        ))
    })?;
    if metadata.nlink() != 1 || file_identity(&metadata) != expected {
        return Err(PatchError::new(format!(
            "{label} published staging file {} changed identity or gained a hard link; refusing to rename",
            name.display()
        )));
    }
    Ok(())
}

fn safe_vendor_target_path(
    label: &str,
    repository: &Path,
    relative: &Path,
    context: &str,
) -> Result<PathBuf, PatchError> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PatchError::new(format!(
            "{label} {context} {} is not a safe relative vendor path",
            relative.display()
        )));
    }
    let mut candidate = repository.to_path_buf();
    let mut components = relative.components().peekable();
    let mut saw_component = false;
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            continue;
        };
        saw_component = true;
        candidate.push(name);
        if components.peek().is_some() {
            validate_directory_path(repository, &candidate, label, context)?;
        }
    }
    if !saw_component {
        return Err(PatchError::new(format!(
            "{label} {context} has an empty vendor path",
        )));
    }
    validate_readable_regular_file(&candidate, label, context)?;
    let canonical = candidate.canonicalize().map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} cannot be canonicalized: {error}",
            relative.display()
        ))
    })?;
    // Compare canonical forms: Windows canonicalization yields verbatim
    // `\\?\D:\...` paths whose prefix never matches a plain `D:\...` root.
    let canonical_repository = repository.canonicalize().map_err(|error| {
        PatchError::new(format!(
            "{label} vendor repository {} cannot be canonicalized: {error}",
            repository.display()
        ))
    })?;
    let Ok(contained) = canonical.strip_prefix(&canonical_repository) else {
        return Err(PatchError::new(format!(
            "{label} {context} {} resolves outside vendor tree {}; restore the checkout",
            relative.display(),
            repository.display()
        )));
    };
    validate_readable_regular_file(&canonical, label, context)?;
    // Callers strip `repository` from the result, so keep its spelling.
    Ok(repository.join(contained))
}

fn canonical_repository(
    label: &str,
    project_root: &Path,
    repository: &Path,
) -> Result<PathBuf, PatchError> {
    let project_root = absolute_path(project_root, label, "project root")?;
    let repository = absolute_path(repository, label, "vendor repository")?;
    let relative = repository.strip_prefix(&project_root).map_err(|_| {
        PatchError::new(format!(
            "{label} vendor repository {} is outside intended project vendor root {}; restore the checkout",
            repository.display(),
            project_root.display()
        ))
    })?;
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(PatchError::new(format!(
            "{label} vendor repository {} is not a descendant of intended project vendor root {}; restore the checkout",
            repository.display(),
            project_root.display()
        )));
    }
    // Validate the complete path by opening each component relative to an
    // already-open directory. A separate metadata walk followed by an ambient
    // open leaves a window in which an ancestor symlink can be installed.
    let project_anchor = open_verified_directory(label, &project_root, "project root")?;
    let repository_anchor = open_verified_directory(label, &repository, "vendor repository")?;
    project_anchor.verify_attached(label, "project root")?;
    repository_anchor.verify_attached(label, "vendor repository")?;
    Ok(repository)
}

fn absolute_path(path: &Path, label: &str, context: &str) -> Result<PathBuf, PatchError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                PatchError::new(format!(
                    "{label} {context} cannot resolve current directory: {error}"
                ))
            })?
            .join(path)
    };
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PatchError::new(format!(
            "{label} {context} {} contains a parent component; restore the checkout",
            path.display()
        )));
    }
    Ok(path)
}

impl DirectoryAnchor {
    fn root(
        directory: Dir,
        path: PathBuf,
        label: &str,
        context: &str,
    ) -> Result<Arc<Self>, PatchError> {
        let metadata = directory.dir_metadata().map_err(|error| {
            PatchError::new(format!(
                "{label} {context} {} cannot be inspected through its handle: {error}",
                path.display()
            ))
        })?;
        Ok(Arc::new(Self {
            directory,
            path,
            identity: file_identity(&metadata),
            parent: None,
            name: None,
        }))
    }

    fn child(
        parent: &Arc<Self>,
        name: &Path,
        label: &str,
        context: &str,
    ) -> Result<Arc<Self>, PatchError> {
        let directory = parent.directory.open_dir_nofollow(name).map_err(|error| {
            PatchError::new(format!(
                "{label} {context} component {} cannot be opened safely: {error}",
                name.to_string_lossy()
            ))
        })?;
        let path = parent.path.join(name);
        let metadata = directory.dir_metadata().map_err(|error| {
            PatchError::new(format!(
                "{label} {context} {} cannot be inspected through its handle: {error}",
                path.display()
            ))
        })?;
        Ok(Arc::new(Self {
            directory,
            path,
            identity: file_identity(&metadata),
            parent: Some(Arc::clone(parent)),
            name: Some(name.to_path_buf()),
        }))
    }

    fn verify_attached(&self, label: &str, context: &str) -> Result<(), PatchError> {
        let Some(parent) = &self.parent else {
            return Ok(());
        };
        parent.verify_attached(label, context)?;
        let Some(name) = self.name.as_ref() else {
            return Err(PatchError::new(format!(
                "{label} {context} {} has no attached path component",
                self.path.display()
            )));
        };
        let current = parent.directory.open_dir_nofollow(name).map_err(|error| {
            PatchError::new(format!(
                "{label} {context} {} is no longer attached to the vendor tree: {error}",
                self.path.display()
            ))
        })?;
        let metadata = current.dir_metadata().map_err(|error| {
            PatchError::new(format!(
                "{label} {context} {} cannot be inspected while checking attachment: {error}",
                self.path.display()
            ))
        })?;
        if file_identity(&metadata) != self.identity {
            return Err(PatchError::new(format!(
                "{label} {context} {} changed identity after its capability was opened; refusing to write",
                self.path.display()
            )));
        }
        Ok(())
    }
}

fn open_verified_directory(
    label: &str,
    path: &Path,
    context: &str,
) -> Result<Arc<DirectoryAnchor>, PatchError> {
    let mut components = path.components().peekable();
    let first = components.next().ok_or_else(|| {
        PatchError::new(format!(
            "{label} {context} {} is not an absolute directory",
            path.display()
        ))
    })?;
    let root_path = match first {
        Component::RootDir => PathBuf::from(std::path::MAIN_SEPARATOR_STR),
        Component::Prefix(prefix) => {
            let mut root = PathBuf::from(prefix.as_os_str());
            if matches!(components.peek(), Some(Component::RootDir)) {
                components.next();
                root.push(Path::new(std::path::MAIN_SEPARATOR_STR));
            } else {
                return Err(PatchError::new(format!(
                    "{label} {context} {} is not an absolute directory",
                    path.display()
                )));
            }
            root
        }
        _ => {
            return Err(PatchError::new(format!(
                "{label} {context} {} is not an absolute directory",
                path.display()
            )))
        }
    };
    let root_dir = Dir::open_ambient_dir(&root_path, ambient_authority()).map_err(|error| {
        PatchError::new(format!(
            "{label} {context} root {} cannot be opened safely: {error}",
            root_path.display()
        ))
    })?;
    let mut current = DirectoryAnchor::root(root_dir, root_path, label, context)?;
    for component in components {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => {
                current = DirectoryAnchor::child(&current, Path::new(name), label, context)?;
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PatchError::new(format!(
                    "{label} {context} {} is not a safe absolute directory",
                    path.display()
                )))
            }
        }
    }
    current.verify_attached(label, context)?;
    Ok(current)
}

fn open_relative_directory(
    label: &str,
    root: &Arc<DirectoryAnchor>,
    relative: &Path,
    context: &str,
) -> Result<Arc<DirectoryAnchor>, PatchError> {
    root.verify_attached(label, context)?;
    let mut current = Arc::clone(root);
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(PatchError::new(format!(
                "{label} {context} {} is not a safe relative directory",
                relative.display()
            )));
        };
        current = DirectoryAnchor::child(&current, Path::new(name), label, context)?;
    }
    Ok(current)
}

fn open_regular_file(
    directory: &Dir,
    name: &Path,
    label: &str,
    context: &str,
) -> Result<cap_std::fs::File, PatchError> {
    let mut options = CapOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = directory.open_with(name, &options).map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} cannot be opened safely: {error}",
            name.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} cannot be inspected: {error}",
            name.display()
        ))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(PatchError::new(format!(
            "{label} {context} {} is not a regular file",
            name.display()
        )));
    }
    Ok(file)
}

fn read_cap_file(
    file: &mut cap_std::fs::File,
    path: &Path,
    patch_path: &Path,
) -> Result<Vec<u8>, PatchError> {
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).map_err(|error| {
        PatchError::new(format!(
            "cannot read vendor file {} for patch {}: {error}",
            path.display(),
            patch_path.display()
        ))
    })?;
    Ok(contents)
}

fn ensure_private_staging_file(
    directory: &Dir,
    name: &Path,
    temporary_file: &cap_std::fs::File,
    label: &str,
    context: &str,
) -> Result<(), PatchError> {
    let temporary_metadata = temporary_file.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} cannot inspect the staging handle: {error}",
            name.display()
        ))
    })?;
    if temporary_metadata.nlink() != 1 {
        return Err(PatchError::new(format!(
            "{label} {context} {} is a hard link; refusing to write through a shared staging file",
            name.display()
        )));
    }
    // Windows stages with share mode 0: while the staging handle is open no
    // other handle can be opened for data access or deletion, so the name
    // cannot be renamed or replaced. Re-opening it by path would itself fail
    // with a sharing violation.
    if cfg!(windows) {
        return Ok(());
    }
    let observed = open_regular_file(directory, name, label, context)?;
    let observed_metadata = observed.metadata().map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} cannot inspect the staging path: {error}",
            name.display()
        ))
    })?;
    if observed_metadata.nlink() != 1
        || file_identity(&temporary_metadata) != file_identity(&observed_metadata)
    {
        return Err(PatchError::new(format!(
            "{label} {context} {} was replaced by a hard link; refusing to write",
            name.display()
        )));
    }
    Ok(())
}

fn file_identity(metadata: &cap_std::fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn validate_directory_path(
    repository: &Path,
    path: &Path,
    label: &str,
    context: &str,
) -> Result<(), PatchError> {
    if !path.starts_with(repository) {
        return Err(PatchError::new(format!(
            "{label} {context} {} is outside vendor tree {}; restore the checkout",
            path.display(),
            repository.display()
        )));
    }
    let relative = path.strip_prefix(repository).map_err(|_| {
        PatchError::new(format!(
            "{label} {context} {} is outside vendor tree {}; restore the checkout",
            path.display(),
            repository.display()
        ))
    })?;
    let mut current = repository.to_path_buf();
    validate_directory_component(&current, label, context)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        validate_directory_component(&current, label, context)?;
    }
    Ok(())
}

fn validate_directory_component(path: &Path, label: &str, context: &str) -> Result<(), PatchError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} is missing or unreadable: {error}",
            path.display()
        ))
    })?;
    reject_link_metadata(&metadata, label, path, context)?;
    if !metadata.is_dir() {
        return Err(PatchError::new(format!(
            "{label} {context} {} is not a directory",
            path.display()
        )));
    }
    Ok(())
}

fn validate_regular_file(
    path: &Path,
    label: &str,
    context: &str,
) -> Result<fs::Metadata, PatchError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} is missing or unreadable: {error}",
            path.display()
        ))
    })?;
    reject_link_metadata(&metadata, label, path, context)?;
    if !metadata.is_file() {
        return Err(PatchError::new(format!(
            "{label} {context} {} is not a regular file",
            path.display()
        )));
    }
    if hard_link_count(path, &metadata).is_some_and(|count| count > 1) {
        return Err(PatchError::new(format!(
            "{label} {context} {} is a hard link; refusing to modify a shared vendor file",
            path.display()
        )));
    }
    Ok(metadata)
}

fn validate_readable_regular_file(
    path: &Path,
    label: &str,
    context: &str,
) -> Result<fs::Metadata, PatchError> {
    // Applied vendor trees may be copied with hard-linked files. Reads are
    // safe; the handle-relative replacement rejects a shared target before
    // it can write anything.
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PatchError::new(format!(
            "{label} {context} {} is missing or unreadable: {error}",
            path.display()
        ))
    })?;
    reject_link_metadata(&metadata, label, path, context)?;
    if !metadata.is_file() {
        return Err(PatchError::new(format!(
            "{label} {context} {} is not a regular file",
            path.display()
        )));
    }
    Ok(metadata)
}

fn reject_link_metadata(
    metadata: &fs::Metadata,
    label: &str,
    path: &Path,
    context: &str,
) -> Result<(), PatchError> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(PatchError::new(format!(
            "{label} {context} {} is a symlink or reparse point; refusing to follow it",
            path.display()
        )));
    }
    Ok(())
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn hard_link_count(path: &Path, metadata: &fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        let _ = path;
        Some(std::os::unix::fs::MetadataExt::nlink(metadata))
    }
    #[cfg(windows)]
    {
        // Path metadata carries no link count on Windows, and std's
        // `number_of_links` is unstable (`windows_by_handle`). cap-std reads
        // it from an open handle. An unopenable file reports no count here;
        // the handle-relative replacement still rejects shared targets.
        let _ = metadata;
        let file = fs::File::open(path).ok()?;
        let metadata = cap_std::fs::Metadata::from_file(&file).ok()?;
        Some(CapMetadataExt::nlink(&metadata))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, metadata);
        None
    }
}

fn validate_git_checkout(
    label: &str,
    repository: &Path,
    expected_base: Option<&str>,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<(), PatchError> {
    if !git_available() {
        return Ok(());
    }

    let Some(git_top_level) = git_output(repository, &["rev-parse", "--show-toplevel"]) else {
        // A source archive nested below an unrelated outer Git checkout has no
        // vendor repository metadata. Preserve the archive path and rely on
        // the authenticated file manifests below.
        return Ok(());
    };
    let git_top_level = PathBuf::from(git_top_level.trim());
    let git_top_level = git_top_level.canonicalize().ok();
    let repository_top_level = repository.canonicalize().ok();
    if git_top_level.is_none() || git_top_level != repository_top_level {
        // `git -C` searches parent directories. Only a Git worktree whose
        // canonical top level is exactly the vendor repository is a checkout;
        // an archive inside an outer project remains a supported no-Git tree.
        return Ok(());
    }

    let Some(git_dir) = git_output(repository, &["rev-parse", "--git-dir"]) else {
        // Source archives and vendor trees copied without their VCS metadata
        // are supported by the exact file manifests below. Git is an optional
        // strengthening check, never a prerequisite for the native build.
        return Ok(());
    };
    if git_dir.trim().is_empty() {
        return Ok(());
    }

    if let Some(expected_base) = expected_base {
        let Some(actual_base) = git_output(repository, &["rev-parse", "--verify", "HEAD"]) else {
            // A checkout whose metadata Git cannot inspect (for example a
            // source tree outside safe.directory) still has the portable
            // manifest checks available. Do not require a user Git setting.
            return Ok(());
        };
        let actual_base = actual_base.trim();
        if actual_base != expected_base {
            return Err(PatchError::new(format!(
                "{label} vendor checkout {} is at unreviewed revision {actual_base}, expected documented base {expected_base}; check out the pinned gitlink revision",
                repository.display()
            )));
        }
    }

    if let Some(output) = git_output_bytes(
        repository,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            "--no-renames",
            "--diff-filter=ACDMRTUXB",
            "-z",
            "HEAD",
            "--",
        ],
    ) {
        let unexpected = output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
            .filter(|path| !expected_paths.contains(path))
            .collect::<BTreeSet<_>>();
        if !unexpected.is_empty() {
            let paths = unexpected
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PatchError::new(format!(
                concat!(
                    "{} vendor checkout {} contains tracked changes outside the current patch set ",
                    "({}); restore the documented base revision or remove the stale vendor patch"
                ),
                label,
                repository.display(),
                paths
            )));
        }
    }

    if let Some(output) = git_output_bytes(repository, &["ls-files", "--others", "-z"]) {
        let unexpected = output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
            .filter(|path| !is_generated_vendor_artifact(path))
            .collect::<BTreeSet<_>>();
        if !unexpected.is_empty() {
            let paths = unexpected
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PatchError::new(format!(
                concat!(
                    "{} vendor checkout {} contains untracked source/build inputs ",
                    "({}); remove them or restore the documented vendor checkout"
                ),
                label,
                repository.display(),
                paths
            )));
        }
    }
    Ok(())
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git_output(repository: &Path, arguments: &[&str]) -> Option<String> {
    String::from_utf8(git_output_bytes(repository, arguments)?).ok()
}

fn git_output_bytes(repository: &Path, arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn is_generated_vendor_artifact(path: &Path) -> bool {
    let components = path.components().filter_map(|component| match component {
        Component::Normal(value) => value.to_str(),
        _ => None,
    });
    if components.clone().any(|component| {
        component == "build"
            || component == "CMakeFiles"
            || component == "_build"
            || component.starts_with("cmake-build")
    }) {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name,
        "CMakeCache.txt"
            | "cmake_install.cmake"
            | "Makefile"
            | "build.ninja"
            | ".ninja_deps"
            | ".ninja_log"
            | "compile_commands.json"
    ) || ["o", "obj", "a", "so", "dylib", "dll", "pdb", "ilk"]
        .iter()
        .any(|extension| path.extension().and_then(|value| value.to_str()) == Some(extension))
}

fn validate_retired_files(
    label: &str,
    repository: &Path,
    retired: &RetiredManifest,
) -> Result<(), PatchError> {
    for (relative, expected) in &retired.entries {
        let path = safe_vendor_target_path(label, repository, relative, "retired vendor file")?;
        let contents = fs::read(&path).map_err(|error| {
            PatchError::new(format!(
                "{label} retired vendor file {} is missing or unreadable: {error}; restore the pinned base checkout",
                path.display()
            ))
        })?;
        if canonical_digest(&contents) != *expected {
            return Err(PatchError::new(format!(
                "{label} vendor file {} contains edits from a retired patch; restore the pinned base content before applying current patches",
                relative.display()
            )));
        }
    }
    Ok(())
}

impl FileManifest {
    fn read(path: &Path, label: &str) -> Result<Self, PatchError> {
        validate_regular_file(path, label, "vendor file manifest")?;
        let text = fs::read_to_string(path).map_err(|error| {
            PatchError::new(format!(
                "cannot read {label} vendor file manifest {}: {error}; restore the tracked manifest",
                path.display()
            ))
        })?;
        let mut entries = BTreeMap::new();
        for (line_number, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
            if fields.len() != 3 {
                return Err(PatchError::new(format!(
                    "{label} vendor file manifest {} line {} must contain path, base SHA-256, and applied SHA-256",
                    path.display(),
                    line_number + 1
                )));
            }
            let relative = manifest_path(path, fields[0])?;
            let base = parse_digest(path, line_number + 1, fields[1])?;
            let applied = parse_digest(path, line_number + 1, fields[2])?;
            if entries
                .insert(relative.clone(), FileExpectation { base, applied })
                .is_some()
            {
                return Err(PatchError::new(format!(
                    "vendor file manifest {} contains duplicate path {}",
                    path.display(),
                    relative.display()
                )));
            }
        }
        if entries.is_empty() {
            return Err(PatchError::new(format!(
                "{label} vendor file manifest {} contains no entries",
                path.display()
            )));
        }
        Ok(Self { entries })
    }

    fn validate_paths(
        &self,
        label: &str,
        active: &BTreeSet<PathBuf>,
        retired: &BTreeMap<PathBuf, [u8; 32]>,
    ) -> Result<(), PatchError> {
        let manifest_paths = self.entries.keys().collect::<BTreeSet<_>>();
        let active_paths = active.iter().collect::<BTreeSet<_>>();
        if manifest_paths != active_paths {
            let missing = active_paths
                .difference(&manifest_paths)
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>();
            let extra = manifest_paths
                .difference(&active_paths)
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>();
            return Err(PatchError::new(format!(
                "{label} vendor file manifest does not exactly match active patch files (missing: {}; extra: {}); regenerate {}",
                if missing.is_empty() {
                    "none".to_string()
                } else {
                    missing.join(", ")
                },
                if extra.is_empty() {
                    "none".to_string()
                } else {
                    extra.join(", ")
                },
                FILE_MANIFEST
            )));
        }
        if manifest_paths
            .iter()
            .any(|path| retired.contains_key(*path))
        {
            return Err(PatchError::new(format!(
                "{label} vendor file manifests overlap; an active patch target cannot also be retired"
            )));
        }
        Ok(())
    }
}

impl RetiredManifest {
    fn read(path: &Path, label: &str, required: bool) -> Result<Self, PatchError> {
        if let Err(error) = fs::symlink_metadata(path) {
            if error.kind() == std::io::ErrorKind::NotFound && !required {
                return Ok(Self {
                    entries: BTreeMap::new(),
                });
            }
            return Err(PatchError::new(format!(
                "cannot read required {label} retired vendor file manifest {}: {error}; restore the tracked stale-patch inventory",
                path.display(),
            )));
        }
        validate_regular_file(path, label, "retired vendor file manifest")?;
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                return Err(PatchError::new(format!(
                    "cannot read required {label} retired vendor file manifest {}: {error}; restore the tracked stale-patch inventory",
                    path.display(),
                )))
            }
        };
        let mut entries = BTreeMap::new();
        for (line_number, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
            if fields.len() != 2 {
                return Err(PatchError::new(format!(
                    "{label} retired vendor file manifest {} line {} must contain path and base SHA-256",
                    path.display(),
                    line_number + 1
                )));
            }
            let relative = manifest_path(path, fields[0])?;
            let digest = parse_digest(path, line_number + 1, fields[1])?;
            if entries.insert(relative.clone(), digest).is_some() {
                return Err(PatchError::new(format!(
                    "retired vendor file manifest {} contains duplicate path {}",
                    path.display(),
                    relative.display()
                )));
            }
        }
        Ok(Self { entries })
    }

    fn validate_exact(
        &self,
        path: &Path,
        label: &str,
        expected: &[(&str, &str)],
    ) -> Result<(), PatchError> {
        let expected_paths = expected
            .iter()
            .map(|(relative, _)| PathBuf::from(relative))
            .collect::<BTreeSet<_>>();
        let actual_paths = self.entries.keys().cloned().collect::<BTreeSet<_>>();
        if expected_paths != actual_paths {
            let missing = expected_paths
                .difference(&actual_paths)
                .map(|entry| entry.display().to_string())
                .collect::<Vec<_>>();
            let extra = actual_paths
                .difference(&expected_paths)
                .map(|entry| entry.display().to_string())
                .collect::<Vec<_>>();
            let missing_text = if missing.is_empty() {
                "none".to_string()
            } else {
                missing.join(", ")
            };
            let extra_text = if extra.is_empty() {
                "none".to_string()
            } else {
                extra.join(", ")
            };
            return Err(PatchError::new(format!(
                "{label} retired vendor manifest {} does not exactly match the expected stale-patch inventory (missing: {}; extra: {}); restore the tracked manifest",
                path.display(),
                missing_text,
                extra_text,
            )));
        }
        for (relative, expected_digest) in expected {
            let expected_digest = parse_digest(path, 0, expected_digest).map_err(|error| {
                PatchError::new(format!(
                    "invalid built-in {label} retired vendor manifest expectation: {error}"
                ))
            })?;
            if self.entries.get(Path::new(relative)) != Some(&expected_digest) {
                return Err(PatchError::new(format!(
                    "{label} retired vendor manifest {} has an incorrect digest for {}; restore the tracked manifest",
                    path.display(),
                    relative
                )));
            }
        }
        Ok(())
    }
}

fn manifest_path(manifest: &Path, raw: &str) -> Result<PathBuf, PatchError> {
    let path = PathBuf::from(raw);
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
            "vendor file manifest {} contains an unsafe path {raw:?}",
            manifest.display()
        )));
    }
    Ok(path)
}

fn parse_digest(manifest: &Path, line: usize, text: &str) -> Result<[u8; 32], PatchError> {
    if text.len() != 64 {
        return Err(PatchError::new(format!(
            "vendor file manifest {} line {} contains a non-SHA-256 digest",
            manifest.display(),
            line
        )));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = hex_digit(pair[0]).map_err(|()| {
            PatchError::new(format!(
                "vendor file manifest {} line {} contains a non-hex SHA-256 digest",
                manifest.display(),
                line
            ))
        })?;
        let low = hex_digit(pair[1]).map_err(|()| {
            PatchError::new(format!(
                "vendor file manifest {} line {} contains a non-hex SHA-256 digest",
                manifest.display(),
                line
            ))
        })?;
        digest[index] = high * 16 + low;
    }
    Ok(digest)
}

fn hex_digit(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut schedule = [0_u32; 64];
        for (index, word) in schedule[..16].iter_mut().enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let mut working = state;
        for index in 0..64 {
            let s1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let choose = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
            let temp1 = working[7]
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let s0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let temp2 = s0.wrapping_add(majority);
            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3].wrapping_add(temp1);
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip(working) {
            *slot = (*slot).wrapping_add(value);
        }
    }

    let mut digest = [0_u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn plan_patch(
    label: &str,
    repository: &Path,
    spec: PatchSpec,
    manifest: &FileManifest,
) -> Result<PatchPlan, PatchError> {
    let mut states = Vec::with_capacity(spec.files.len());
    let mut writes = Vec::new();
    for file in &spec.files {
        let path = safe_vendor_target_path(label, repository, &file.path, "patch target")?;
        let contents = fs::read(&path).map_err(|error| {
            PatchError::new(format!(
                "cannot read vendor file {} for patch {}: {error}",
                path.display(),
                spec.path.display()
            ))
        })?;
        let expectation = manifest.entries.get(&file.path).ok_or_else(|| {
            PatchError::new(format!(
                "vendor patch {} has no exact file expectation for {}; regenerate {}",
                spec.path.display(),
                file.path.display(),
                FILE_MANIFEST
            ))
        })?;
        let digest = canonical_digest(&contents);
        if digest != expectation.base && digest != expectation.applied {
            return Err(PatchError::new(format!(
                "vendor file {} does not match its exact clean or applied content for patch {}; restore the pinned base checkout",
                path.display(),
                spec.path.display()
            )));
        }
        let contents = String::from_utf8(contents).map_err(|_| {
            PatchError::new(format!(
                "vendor file {} for patch {} is not valid UTF-8",
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
        let state = match (
            digest == expectation.base,
            digest == expectation.applied,
            clean,
            applied,
        ) {
            (true, false, true, false) => PatchState::Clean,
            (false, true, false, true) => PatchState::Applied,
            _ => PatchState::Mismatch,
        };
        states.push(state);
        if state == PatchState::Clean {
            let rendered = apply_file(&source, file, &spec.path)?;
            if canonical_digest(rendered.as_bytes()) != expectation.applied {
                return Err(PatchError::new(format!(
                    "vendor patch {} renders {} with a digest different from the authenticated applied manifest; the patch input was modified",
                    spec.path.display(),
                    file.path.display()
                )));
            }
            writes.push(WritePlan {
                relative: file.path.clone(),
                path,
                contents: rendered,
                base: expectation.base,
                applied: expectation.applied,
            });
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

fn canonical_digest(bytes: &[u8]) -> [u8; 32] {
    let mut canonical = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'\r' {
            canonical.push(b'\n');
            index += usize::from(bytes.get(index + 1) == Some(&b'\n')) + 1;
        } else {
            canonical.push(byte);
            index += 1;
        }
    }
    sha256(&canonical)
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
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PATCH: &str = "diff --git a/alpha.txt b/alpha.txt\n--- a/alpha.txt\n+++ b/alpha.txt\n@@ -1,2 +1,2 @@\n one\n-old\n+new\n";

    fn temporary_tree() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        // Keep fixtures on the checkout filesystem. Linux's anonymous
        // staging primitive is filesystem-specific, while some CI images
        // mount the system temporary directory on overlayfs without it.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/vendor-patch-tests")
            .join(format!(
                "llg-vendor-patches-{}-{suffix}",
                std::process::id()
            ));
        fs::create_dir_all(path.join("patches")).expect("create temporary patch tree");
        path
    }

    fn digest_text(text: &str) -> String {
        canonical_digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn sha256_matches_standard_empty_vector() {
        assert_eq!(
            digest_text(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest_text("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn write_manifest(patches: &Path, relative: &str, base: &str, applied: &str) {
        fs::write(
            patches.join(FILE_MANIFEST),
            format!(
                "{relative} {} {}\n",
                digest_text(base),
                digest_text(applied)
            ),
        )
        .expect("write exact file manifest");
    }

    fn write_retired_manifest(patches: &Path, relative: &str, base: &str) {
        fs::write(
            patches.join(RETIRED_MANIFEST),
            format!("{relative} {}\n", digest_text(base)),
        )
        .expect("write retired file manifest");
    }

    fn initialize_repository(repository: &Path) {
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .arg(repository)
            .status()
            .expect("git should be available for vendor patch tests");
        assert!(status.success(), "git init should succeed");
        for (key, value) in [
            ("user.email", "vendor-patches-tests@example.invalid"),
            ("user.name", "vendor-patches-tests"),
        ] {
            let status = Command::new("git")
                .arg("-C")
                .arg(repository)
                .args(["config", key, value])
                .status()
                .expect("git config should run");
            assert!(status.success(), "git config should succeed");
        }
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["add", "."])
            .status()
            .expect("git add should run");
        assert!(status.success(), "git add should succeed");
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["commit", "--quiet", "-m", "base"])
            .status()
            .expect("git commit should run");
        assert!(status.success(), "git commit should succeed");
    }

    #[test]
    fn clean_checkout_applies_and_applied_checkout_is_accepted() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

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
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches).expect_err("mismatch fails");
        assert!(error
            .to_string()
            .contains("does not match its exact clean or applied content"));
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nother\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn stale_removed_patch_is_rejected() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(repository.join("source/numeric")).expect("create source tree");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write active source");
        fs::write(
            repository.join("source/numeric/SVInt.cpp"),
            "return base;\n",
        )
        .expect("write stale patch source");
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write current patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");
        write_retired_manifest(&patches, "source/numeric/SVInt.cpp", "return base;\n");
        apply_directory("test", &repository, &patches).expect("current patch should apply");
        fs::write(
            repository.join("source/numeric/SVInt.cpp"),
            "return stale;\n",
        )
        .expect("apply stale removed patch");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("stale removed patch should be rejected");
        let message = error.to_string();
        assert!(message.contains("retired patch"));
        assert!(message.contains("source/numeric/SVInt.cpp"));
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nnew\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn checkout_without_git_metadata_applies_and_accepts_applied_files() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        apply_directory("test", &repository, &patches).expect("plain source tree applies");
        apply_directory("test", &repository, &patches).expect("plain source tree is accepted");
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nnew\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn archive_nested_in_outer_git_repository_uses_manifest_validation() {
        let root = temporary_tree();
        let repository = root.join("vendor/archive");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create archive vendor tree");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        initialize_repository(&root);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        apply_directory("test", &repository, &patches)
            .expect("archive inside an outer Git repository applies");
        apply_directory("test", &repository, &patches)
            .expect("archive inside an outer Git repository is accepted applied");
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nnew\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn extra_same_file_edit_is_rejected() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(repository.join(".gitignore"), "extra.cpp\n").expect("write ignore file");
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        apply_directory("test", &repository, &patches).expect("clean patch applies");
        fs::write(repository.join("alpha.txt"), "one\nnew\nextra\n").expect("add same-file edit");
        let error = apply_directory("test", &repository, &patches)
            .expect_err("same-file edit should be rejected");
        assert!(error
            .to_string()
            .contains("does not match its exact clean or applied content"));

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn wrong_clean_commit_is_rejected_when_git_metadata_is_available() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory_with_base(
            "test",
            &root,
            &repository,
            &patches,
            Some("0000000000000000000000000000000000000000"),
        )
        .expect_err("unreviewed clean commit should be rejected");
        let message = error.to_string();
        assert!(message.contains("unreviewed revision"));
        assert!(message.contains("expected documented base"));
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn untracked_source_input_is_rejected_but_build_artifact_is_ignored() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        initialize_repository(&repository);
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");
        fs::create_dir_all(repository.join("build")).expect("create build directory");
        fs::write(repository.join("build/generated.o"), "object")
            .expect("write generated artifact");
        apply_directory("test", &repository, &patches)
            .expect("generated build artifact should be ignored");

        fs::write(repository.join("extra.cpp"), "int extra;\n").expect("write source input");
        let error = apply_directory("test", &repository, &patches)
            .expect_err("untracked source input should be rejected");
        let message = error.to_string();
        assert!(message.contains("untracked source/build inputs"));
        assert!(message.contains("extra.cpp"));

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn crlf_checkout_uses_lf_manifest_digests_and_preserves_crlf_output() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\r\nold\r\n").expect("write CRLF source");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write LF patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        apply_directory("test", &repository, &patches).expect("CRLF patch applies");
        assert_eq!(
            fs::read(repository.join("alpha.txt")).unwrap(),
            b"one\r\nnew\r\n"
        );
        apply_directory("test", &repository, &patches).expect("CRLF applied state is accepted");

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn modified_patch_output_is_rejected_before_any_write() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(
            patches.join("alpha.patch"),
            PATCH.replace("+new", "+tampered"),
        )
        .expect("write modified patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("modified rendered output should be rejected");
        assert!(error.to_string().contains("authenticated applied manifest"));
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_is_rejected_without_writing_outside_vendor_tree() {
        use std::os::unix::fs::symlink;

        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        let outside = root.join("outside.txt");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(&outside, "one\nold\n").expect("write outside source");
        symlink(&outside, repository.join("alpha.txt")).expect("link target");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("symlink target should be rejected");
        assert!(error.to_string().contains("symlink or reparse"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "one\nold\n");

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_parent_is_rejected_without_writing_outside_vendor_tree() {
        use std::os::unix::fs::symlink;

        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        let outside = root.join("outside");
        fs::create_dir_all(&repository).expect("create repository");
        fs::create_dir_all(&outside).expect("create outside directory");
        fs::write(outside.join("alpha.txt"), "one\nold\n").expect("write outside source");
        symlink(&outside, repository.join("escape")).expect("link parent");
        let patch = PATCH.replace("alpha.txt", "escape/alpha.txt");
        fs::write(patches.join("alpha.patch"), patch).expect("write nested patch");
        write_manifest(&patches, "escape/alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("symlink parent should be rejected");
        assert!(error.to_string().contains("symlink or reparse"));
        assert_eq!(
            fs::read_to_string(outside.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_is_rejected_before_vendor_write() {
        use std::os::unix::fs::symlink;

        let root = temporary_tree();
        let real_project = root.join("real-project");
        let project_link = root.join("project-link");
        let repository = project_link.join("vendor");
        let patches = root.join("patches");
        fs::create_dir_all(real_project.join("vendor")).expect("create real vendor tree");
        fs::write(real_project.join("vendor/alpha.txt"), "one\nold\n").expect("write source");
        symlink(&real_project, &project_link).expect("link project ancestor");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("ancestor symlink should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("symlink or reparse") || message.contains("cannot be opened safely"),
            "unexpected ancestor rejection: {message}"
        );
        assert_eq!(
            fs::read_to_string(real_project.join("vendor/alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn parent_replacement_between_capability_checks_is_rejected_without_outside_write() {
        use std::os::unix::fs::symlink;

        let root = temporary_tree();
        let repository = root.join("repo");
        let parent = repository.join("nested");
        let moved_parent = root.join("moved-nested");
        let outside = root.join("outside");
        fs::create_dir_all(&parent).expect("create nested vendor directory");
        fs::create_dir_all(&outside).expect("create outside directory");
        fs::write(parent.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(outside.join("alpha.txt"), "one\nold\n").expect("write outside source");
        let repository = canonical_repository("test", &root, &repository).expect("canonical repo");
        let target = repository.join("nested/alpha.txt");
        let base = canonical_digest(b"one\nold\n");
        let applied = canonical_digest(b"one\nnew\n");
        let mut replaced = false;
        let mut hook = |point, _| {
            if point == AtomicReplacePoint::AfterRepositoryOpen {
                fs::rename(&parent, &moved_parent).expect("move nested vendor directory");
                symlink(&outside, &parent).expect("replace parent with symlink");
                replaced = true;
            }
        };

        let error = atomic_replace_with_hook(
            "test",
            &repository,
            &target,
            "one\nnew\n",
            base,
            applied,
            &root.join("alpha.patch"),
            &mut hook,
        )
        .expect_err("replaced parent should fail closed");
        assert!(replaced);
        assert!(error.to_string().contains("cannot be opened safely"));
        assert_eq!(
            fs::read_to_string(moved_parent.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );
        assert_eq!(
            fs::read_to_string(outside.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn parent_capability_replacement_after_open_is_rejected_without_outside_write() {
        use std::os::unix::fs::symlink;

        let root = temporary_tree();
        let repository = root.join("repo");
        let parent = repository.join("nested");
        let moved_parent = root.join("moved-nested");
        let outside = root.join("outside");
        fs::create_dir_all(&parent).expect("create nested vendor directory");
        fs::create_dir_all(&outside).expect("create outside directory");
        fs::write(parent.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(outside.join("alpha.txt"), "one\nold\n").expect("write outside source");
        let repository = canonical_repository("test", &root, &repository).expect("canonical repo");
        let target = repository.join("nested/alpha.txt");
        let base = canonical_digest(b"one\nold\n");
        let applied = canonical_digest(b"one\nnew\n");
        let mut replaced = false;
        let mut hook = |point, _| {
            if point == AtomicReplacePoint::AfterParentOpen {
                fs::rename(&parent, &moved_parent).expect("move opened nested directory");
                symlink(&outside, &parent).expect("replace parent with symlink");
                replaced = true;
            }
        };

        let error = atomic_replace_with_hook(
            "test",
            &repository,
            &target,
            "one\nnew\n",
            base,
            applied,
            &root.join("alpha.patch"),
            &mut hook,
        )
        .expect_err("replaced opened parent should fail closed");
        assert!(replaced);
        assert!(error.to_string().contains("no longer attached"));
        assert_eq!(
            fs::read_to_string(moved_parent.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );
        assert_eq!(
            fs::read_to_string(outside.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn temporary_hardlink_insertion_is_rejected_before_staging_write() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let outside = root.join("outside.txt");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::write(&outside, "outside\n").expect("write outside sentinel");
        let repository = canonical_repository("test", &root, &repository).expect("canonical repo");
        let target = repository.join("alpha.txt");
        let base = canonical_digest(b"one\nold\n");
        let applied = canonical_digest(b"one\nnew\n");
        let parent = repository.clone();
        let mut hook = |point, temporary_name: PathBuf| {
            if point == AtomicReplacePoint::AfterTemporaryCheck {
                // Anonymous Linux staging has no pathname to link. The
                // fallback platforms still expose a name, and must reject
                // this insertion before writing through the staging handle.
                let result = fs::hard_link(parent.join(temporary_name), &outside);
                #[cfg(target_os = "linux")]
                assert!(result.is_err(), "anonymous staging unexpectedly had a name");
            }
        };

        let result = atomic_replace_with_hook(
            "test",
            &repository,
            &target,
            "one\nnew\n",
            base,
            applied,
            &root.join("alpha.patch"),
            &mut hook,
        );
        #[cfg(target_os = "linux")]
        {
            result.expect("anonymous staging should not be externally linkable");
            assert_eq!(fs::read_to_string(&outside).unwrap(), "outside\n");
            assert_eq!(
                fs::read_to_string(repository.join("alpha.txt")).unwrap(),
                "one\nnew\n"
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
            let error = result.expect_err("temporary hard link should fail closed");
            assert!(error.to_string().contains("hard link"));
            assert_eq!(fs::read_to_string(&outside).unwrap(), "outside\n");
            assert_eq!(
                fs::read_to_string(repository.join("alpha.txt")).unwrap(),
                "one\nold\n"
            );
        }

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[cfg(unix)]
    #[test]
    fn hard_link_target_is_rejected_without_modifying_the_shared_file() {
        let root = temporary_tree();
        let repository = root.join("repo");
        let patches = root.join("patches");
        let outside = root.join("outside.txt");
        fs::create_dir_all(&repository).expect("create repository");
        fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
        fs::hard_link(repository.join("alpha.txt"), &outside).expect("create hard link");
        fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
        write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");

        let error = apply_directory("test", &repository, &patches)
            .expect_err("hard-linked target should be rejected");
        assert!(error.to_string().contains("hard link"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "one\nold\n");
        assert_eq!(
            fs::read_to_string(repository.join("alpha.txt")).unwrap(),
            "one\nold\n"
        );

        fs::remove_dir_all(root).expect("remove temporary patch tree");
    }

    #[test]
    fn slang_retired_manifest_is_required_and_exact_without_git() {
        let cases = [
            ("missing", None, "required"),
            ("empty", Some("# path base-content-sha256\n"), "exactly match"),
            (
                "incorrect",
                Some("source/numeric/SVInt.cpp 0000000000000000000000000000000000000000000000000000000000000000\n"),
                "incorrect digest",
            ),
        ];
        for (name, retired, expected_message) in cases {
            let root = temporary_tree();
            let repository = root.join("repo");
            let patches = root.join("patches");
            fs::create_dir_all(repository.join("source/numeric")).expect("create source tree");
            fs::write(repository.join("alpha.txt"), "one\nold\n").expect("write source");
            fs::write(
                repository.join("source/numeric/SVInt.cpp"),
                "return base;\n",
            )
            .expect("write retired source");
            fs::write(patches.join("alpha.patch"), PATCH).expect("write patch");
            write_manifest(&patches, "alpha.txt", "one\nold\n", "one\nnew\n");
            if let Some(retired) = retired {
                fs::write(patches.join(RETIRED_MANIFEST), retired)
                    .expect("write retired manifest variant");
            }

            let error = apply_directory("Slang", &repository, &patches)
                .expect_err("retired manifest variant should fail");
            assert!(
                error.to_string().contains(expected_message),
                "{name} retired manifest error: {error}"
            );
            assert_eq!(
                fs::read_to_string(repository.join("alpha.txt")).unwrap(),
                "one\nold\n"
            );
            fs::remove_dir_all(root).expect("remove temporary patch tree");
        }
    }
}
