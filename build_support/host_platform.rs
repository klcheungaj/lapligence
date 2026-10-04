//! Host-platform primitives for the build scripts.
//!
//! Build scripts cannot depend on the crate's `ffi::platform`, so this module
//! is their single home for `cfg(unix)`, `cfg(windows)` and `target_os`
//! branches; `vendor_patches.rs` and `compiler_cache.rs` call the neutral
//! functions below. Every includer (`build.rs` and the tests that include the
//! build-support sources) declares it as the sibling module `host_platform`.

use std::fs;
use std::io;
use std::path::Path;

use cap_std::fs::{Dir, OpenOptions as CapOpenOptions};

#[cfg(any(target_os = "linux", windows))]
use cap_std::fs::OpenOptionsExt as CapOpenOptionsExt;

/// File name of executable `tool` on this host (`tool.exe` on Windows).
pub fn executable_name(tool: &str) -> String {
    if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    }
}

/// Whether `path` is a regular file the host would execute: any regular file
/// on Windows, one with an execute permission bit elsewhere.
pub fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Whether sccache must be started through the repository's socket-safe Unix
/// wrapper script rather than directly.
pub fn sccache_needs_unix_wrapper() -> bool {
    cfg!(unix)
}

/// Flushes a directory's entries after a rename inside it.
#[cfg(unix)]
pub fn sync_directory(directory: &Dir) -> io::Result<()> {
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
pub fn sync_directory(_directory: &Dir) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub fn sync_directory(directory: &Dir) -> io::Result<()> {
    directory.try_clone()?.into_std_file().sync_all()
}

/// Opens a writable file in `parent` that has no directory entry yet, or
/// returns `None` when the host has no anonymous files and the caller must
/// stage under a fresh name instead.
///
/// Linux's O_TMPFILE creates an inode without a directory entry; keeping the
/// written inode anonymous closes the hard-link window between an identity
/// check and write(2). It is published later with [`publish_anonymous_file`].
pub fn open_anonymous_file(parent: &Dir) -> Option<io::Result<cap_std::fs::File>> {
    #[cfg(target_os = "linux")]
    {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
        use rustix::fs::OFlags;

        let mut options = CapOpenOptions::new();
        options
            .read(true)
            .write(true)
            .follow(FollowSymlinks::No)
            .custom_flags((OFlags::TMPFILE | OFlags::DIRECTORY).bits() as i32);
        Some(parent.open_with(".", &options))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = parent;
        None
    }
}

/// Makes a named staging file exclusive where the host supports it: Windows
/// share mode 0 blocks every other open, rename and delete of the name while
/// the handle is open.
pub fn lock_named_staging_file(options: &mut CapOpenOptions) {
    #[cfg(windows)]
    options.share_mode(0);
    #[cfg(not(windows))]
    let _ = options;
}

/// Whether [`lock_named_staging_file`] keeps the staged name from being
/// renamed or replaced while its handle is open (then re-opening it by path
/// is unnecessary and would itself fail with a sharing violation).
pub fn named_staging_file_is_locked() -> bool {
    cfg!(windows)
}

/// Why [`publish_anonymous_file`] did not link the file.
pub enum PublishError {
    /// The destination name exists; it is never replaced.
    AlreadyExists,
    /// The link operation failed.
    Failed(io::Error),
    /// The host has no safe operation that links an open file.
    #[cfg_attr(unix, allow(dead_code))]
    Unsupported,
}

/// Gives the anonymous `file` the directory entry `name` in `parent`.
#[cfg(unix)]
pub fn publish_anonymous_file(
    parent: &Dir,
    file: &cap_std::fs::File,
    name: &Path,
) -> Result<(), PublishError> {
    use std::os::fd::AsRawFd;
    use std::path::PathBuf;

    use rustix::fs::{linkat, AtFlags, CWD};

    // Linux and the BSDs which expose AT_EMPTY_PATH can link the open inode
    // directly. Some kernels/filesystems reject that form without the
    // CAP_DAC_READ_SEARCH capability, so retain the descriptor-relative procfs
    // form as a safe fallback.
    #[cfg(any(target_os = "freebsd", target_os = "fuchsia", target_os = "linux"))]
    match linkat(file, "", parent, name, AtFlags::EMPTY_PATH) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(PublishError::AlreadyExists)
        }
        Err(_) => {}
    }

    let descriptor_path = if cfg!(any(target_os = "macos", target_os = "ios")) {
        PathBuf::from("/dev/fd").join(file.as_raw_fd().to_string())
    } else {
        PathBuf::from("/proc/self/fd").join(file.as_raw_fd().to_string())
    };
    linkat(CWD, &descriptor_path, parent, name, AtFlags::SYMLINK_FOLLOW)
        .map_err(|error| PublishError::Failed(error.into()))
}

#[cfg(not(unix))]
pub fn publish_anonymous_file(
    _parent: &Dir,
    _file: &cap_std::fs::File,
    _name: &Path,
) -> Result<(), PublishError> {
    Err(PublishError::Unsupported)
}

/// Whether `metadata` (from a no-follow lookup) describes a Windows reparse
/// point; other hosts have none.
pub fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

/// The number of directory entries naming the file at `path`, when the host
/// can report it.
pub fn hard_link_count(path: &Path, metadata: &fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        let _ = path;
        Some(std::os::unix::fs::MetadataExt::nlink(metadata))
    }
    #[cfg(windows)]
    {
        use cap_fs_ext::MetadataExt as CapMetadataExt;
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
