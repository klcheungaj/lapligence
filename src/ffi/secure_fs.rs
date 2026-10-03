//! Safe handles for reading admitted source files and traversing map roots.
//!
//! Core input admission must not canonicalize a path and then reopen that
//! pathname after a parent directory has been replaced.  This module keeps
//! the platform calls and their `unsafe` details here.  Callers receive an
//! owned `File` handle together with the target path reported by that handle;
//! no path is read until the handle has been checked for regular-file type,
//! expected identity, and (when requested) containment below an admitted
//! directory.

#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{OsStr, OsString};
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use std::fs::ReadDir;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// An opened filesystem object whose target was obtained from the handle.
///
/// The file is kept open for the lifetime of this value.  `actual_path` is
/// derived from the open handle rather than from a second path lookup.
#[derive(Debug)]
pub struct OpenedPath {
    file: File,
    metadata: Metadata,
    identity: FileIdentity,
    actual_path: PathBuf,
    root: PathBuf,
}

/// Stable identity captured from an opened filesystem object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    marker: (),
}

/// Handle-derived identity retained after the admission handle is closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedTarget {
    actual_path: PathBuf,
    identity: FileIdentity,
}

impl AdmittedTarget {
    /// Path reported by the admission handle.
    pub fn actual_path(&self) -> &Path {
        &self.actual_path
    }
}

impl OpenedPath {
    /// Path reported by the open handle.
    pub fn actual_path(&self) -> &Path {
        &self.actual_path
    }

    /// Metadata captured from the open handle.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Stable identity of the object held by this handle.
    pub fn identity(&self) -> FileIdentity {
        self.identity
    }

    /// Capture the handle-derived path and stable object identity for a later
    /// reopen.  The later open must match both fields before any read.
    pub fn admitted_target(&self) -> AdmittedTarget {
        AdmittedTarget {
            actual_path: self.actual_path.clone(),
            identity: self.identity,
        }
    }

    /// Duplicate the underlying descriptor and revalidate its target.
    pub fn try_clone(&self) -> io::Result<Self> {
        let file = self.file.try_clone()?;
        let target = self.admitted_target();
        opened_from_file(file, Some(&self.root), Some(&target))
    }

    /// Re-read metadata through the same open handle after a bounded read.
    pub fn refresh_metadata(&mut self) -> io::Result<Metadata> {
        let metadata = self.file.metadata()?;
        self.metadata = metadata.clone();
        Ok(metadata)
    }

    /// Whether this handle refers to a regular file.
    pub fn is_file(&self) -> bool {
        self.metadata.is_file()
    }

    /// Whether this handle refers to a directory.
    pub fn is_dir(&self) -> bool {
        self.metadata.is_dir()
    }

    /// Read bytes from the already validated regular-file handle.
    pub fn read_bytes(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }

    /// Open a child using the current admitted root as the containment root.
    /// The returned handle is checked before it is exposed to the caller.
    pub fn open_child(&self, name: &OsStr) -> io::Result<Self> {
        if !self.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "secure filesystem child requires a directory",
            ));
        }
        #[cfg(unix)]
        {
            let file = platform::open_child(&self.file, name)?;
            opened_from_file(file, Some(&self.root), None)
        }
        #[cfg(not(unix))]
        {
            let path = self.actual_path.join(name);
            open_path_under(&path, self)
        }
    }

    /// Enumerate names from this opened directory.
    ///
    /// Directory names are only candidates. Every candidate is opened below
    /// the admitted root through the platform handle wrapper before the
    /// caller can inspect or read it.
    pub fn read_dir(&self) -> io::Result<SecureReadDir> {
        if !self.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "secure filesystem read_dir requires a directory",
            ));
        }
        platform::read_dir(&self.file, &self.actual_path)
    }
}

/// Iterator over names in an already opened directory.
pub struct SecureReadDir {
    inner: SecureReadDirInner,
}

impl Iterator for SecureReadDir {
    type Item = io::Result<OsString>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

enum SecureReadDirInner {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Descriptor(platform::DirectoryStream),
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Standard(ReadDir),
}

impl SecureReadDirInner {
    fn next(&mut self) -> Option<io::Result<OsString>> {
        match self {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Descriptor(stream) => stream.next(),
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            Self::Standard(read_dir) => read_dir
                .next()
                .map(|entry| entry.map(|entry| entry.file_name())),
        }
    }
}

/// Open an existing path and report its handle-derived target.
pub fn open_path(path: &Path) -> io::Result<OpenedPath> {
    open_path_internal(path, None, None)
}

/// Open a path while requiring the handle-derived target to remain below an
/// already admitted root target.
pub fn open_path_under(path: &Path, root: &OpenedPath) -> io::Result<OpenedPath> {
    #[cfg(unix)]
    {
        let relative = path.strip_prefix(root.actual_path()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "secure filesystem path is outside the admitted root",
            )
        })?;
        let file = platform::open_relative(&root.file, relative, root.actual_path())?;
        opened_from_file(file, Some(root.actual_path()), None)
    }
    #[cfg(not(unix))]
    {
        if !platform::path_is_within(path, root.actual_path()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "secure filesystem path is outside the admitted root",
            ));
        }
        // Windows lacks a standard descriptor-relative open primitive. Check
        // the directory handle identity immediately before and after opening
        // the child, and validate the child handle itself before exposure.
        let _before = open_path_internal(root.actual_path(), None, Some(&root.admitted_target()))?;
        let file = platform::path_options().open(path)?;
        let opened = opened_from_file(file, Some(root.actual_path()), None)?;
        let _after = open_path_internal(root.actual_path(), None, Some(&root.admitted_target()))?;
        Ok(opened)
    }
}

/// Reopen the parent directory of an admitted target and verify the target
/// still names the same filesystem object.  Callers use the returned handle
/// as the descriptor-relative anchor for later wildcard traversal.
pub fn open_parent_of_target(expected: &AdmittedTarget) -> io::Result<OpenedPath> {
    let parent_path = expected.actual_path().parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "secure filesystem target has no parent directory",
        )
    })?;
    let parent = open_path(parent_path)?;
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "secure filesystem target parent is not a directory",
        ));
    }
    let child = open_path_under(expected.actual_path(), &parent)?;
    if child.admitted_target() != *expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "secure filesystem target parent changed after admission",
        ));
    }
    Ok(parent)
}

/// Open a regular file and reject a target that is not the expected handle
/// target.  This is used when a path was admitted earlier and is being read
/// after a possible directory replacement.
pub fn open_regular_file_exact(path: &Path, expected: &AdmittedTarget) -> io::Result<OpenedPath> {
    open_path_internal(path, None, Some(expected)).and_then(|opened| {
        if opened.is_file() {
            Ok(opened)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "secure filesystem path is not a regular file",
            ))
        }
    })
}

fn open_path_internal(
    path: &Path,
    confined_to: Option<&Path>,
    expected: Option<&AdmittedTarget>,
) -> io::Result<OpenedPath> {
    let options = platform::path_options();
    let file = options.open(path)?;
    opened_from_file(file, confined_to, expected)
}

fn opened_from_file(
    file: File,
    confined_to: Option<&Path>,
    expected: Option<&AdmittedTarget>,
) -> io::Result<OpenedPath> {
    let actual_path = platform::path_from_handle(&file)?;
    if let Some(root) = confined_to {
        if !platform::path_is_within(&actual_path, root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "secure filesystem path {} escapes admitted root {}",
                    actual_path.display(),
                    root.display()
                ),
            ));
        }
    }
    let metadata = file.metadata()?;
    let identity = platform::identity(&file, &metadata)?;
    if let Some(expected) = expected {
        if !platform::same_path(&actual_path, expected.actual_path())
            || identity != expected.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "secure filesystem path {} changed from admitted target {}",
                    actual_path.display(),
                    expected.actual_path().display()
                ),
            ));
        }
    }
    let root = confined_to
        .map(Path::to_path_buf)
        .unwrap_or_else(|| actual_path.clone());
    Ok(OpenedPath {
        file,
        metadata,
        identity,
        actual_path,
        root,
    })
}

#[cfg(unix)]
mod platform {
    use super::{
        File, FileIdentity, Metadata, OsString, Path, PathBuf, SecureReadDir, SecureReadDirInner,
    };
    use std::ffi::OsStr;
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::FromRawFd;

    pub(super) fn identity(_file: &File, metadata: &Metadata) -> io::Result<FileIdentity> {
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub(super) fn path_options() -> std::fs::OpenOptions {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = std::fs::OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK);
        options
    }

    pub(super) fn path_from_handle(file: &File) -> io::Result<PathBuf> {
        #[cfg(target_os = "linux")]
        let path = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
        #[cfg(target_os = "macos")]
        let path = descriptor_path(file)?;
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let path: PathBuf = return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure handle paths are unavailable on this Unix target",
        ));
        // Linux appends ` (deleted)` to the procfs link for an unlinked
        // inode.  A live file may have those exact bytes in its name, so the
        // link spelling alone cannot identify deletion.  An unlinked inode
        // has no directory links; use that handle metadata instead.
        if file.metadata()?.nlink() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "secure filesystem target was deleted while opening",
            ));
        }
        Ok(path)
    }

    /// macOS `/dev/fd` entries are fdesc device nodes rather than symlinks,
    /// so `readlink` fails with EINVAL. `F_GETPATH` asks the kernel for the
    /// descriptor's current path instead, with symlinks resolved.
    #[cfg(target_os = "macos")]
    fn descriptor_path(file: &File) -> io::Result<PathBuf> {
        use std::os::unix::ffi::OsStringExt;

        // F_GETPATH requires a MAXPATHLEN (== PATH_MAX) byte buffer.
        let mut buffer = vec![0_u8; libc::PATH_MAX as usize];
        let result = unsafe {
            // SAFETY: `file` is a live descriptor and `buffer` is an owned,
            // writable PATH_MAX-byte allocation that outlives the call;
            // F_GETPATH writes at most that many bytes, NUL included.
            libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buffer.as_mut_ptr())
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        let length = buffer.iter().position(|&byte| byte == 0).ok_or_else(|| {
            io::Error::other("F_GETPATH returned an unterminated descriptor path")
        })?;
        buffer.truncate(length);
        Ok(PathBuf::from(std::ffi::OsString::from_vec(buffer)))
    }

    pub(super) fn path_is_within(path: &Path, root: &Path) -> bool {
        path == root || path.starts_with(root)
    }

    pub(super) fn same_path(path: &Path, expected: &Path) -> bool {
        path == expected
    }

    pub(super) fn open_relative(parent: &File, relative: &Path, root: &Path) -> io::Result<File> {
        let mut ancestors = vec![parent.try_clone()?];
        let components = relative.components().collect::<Vec<_>>();
        if components.is_empty() {
            return ancestors
                .pop()
                .ok_or_else(|| io::Error::other("secure filesystem root clone missing"));
        }
        for (index, component) in components.iter().enumerate() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if ancestors.len() == 1 {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "secure filesystem path escapes admitted root",
                        ));
                    }
                    ancestors.pop();
                }
                std::path::Component::Normal(name) => {
                    let child = open_child(
                        ancestors.last().ok_or_else(|| {
                            io::Error::other("secure filesystem ancestor missing")
                        })?,
                        name,
                    )?;
                    let child_path = path_from_handle(&child)?;
                    if !path_is_within(&child_path, root) {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "secure filesystem path component escapes admitted root",
                        ));
                    }
                    if index + 1 == components.len() {
                        return Ok(child);
                    }
                    if !std::fs::File::metadata(&child)?.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotADirectory,
                            "secure filesystem path component is not a directory",
                        ));
                    }
                    ancestors.push(child);
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "secure filesystem relative path has a root or prefix",
                    ));
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "secure filesystem relative path has no final component",
        ))
    }

    pub(super) fn open_child(parent: &File, name: &OsStr) -> io::Result<File> {
        let name = std::ffi::CString::new(name.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "secure filesystem child name contains a NUL byte",
            )
        })?;
        let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK;
        let descriptor = unsafe {
            // SAFETY: `parent` is a live directory `File`; `name` is a
            // NUL-terminated child name owned for the duration of this call;
            // and the flags request one new descriptor with no borrowed
            // output pointers.
            libc::openat(parent.as_raw_fd(), name.as_ptr(), flags)
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `descriptor` is the unique file descriptor returned by
        // `openat`; `File` takes ownership and closes it exactly once.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    /// List a fresh `openat(".")` descriptor for the admitted directory, so
    /// the admitted handle keeps its own offset. Path-based reopening through
    /// `/dev/fd/N` is not portable: macOS checks the fdesc node itself and
    /// rejects it as not a directory.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn read_dir(file: &File, _actual_path: &Path) -> io::Result<SecureReadDir> {
        use std::os::fd::IntoRawFd;

        let descriptor = open_child(file, OsStr::new("."))?.into_raw_fd();
        let stream = unsafe {
            // SAFETY: `descriptor` is an open directory descriptor owned here;
            // on success the returned stream takes ownership of it.
            libc::fdopendir(descriptor)
        };
        let Some(stream) = std::ptr::NonNull::new(stream) else {
            let error = io::Error::last_os_error();
            // SAFETY: fdopendir failed, so `descriptor` is still owned here;
            // the `File` closes it exactly once.
            drop(unsafe { File::from_raw_fd(descriptor) });
            return Err(error);
        };
        Ok(SecureReadDir {
            inner: SecureReadDirInner::Descriptor(DirectoryStream {
                stream,
                finished: false,
            }),
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(super) fn read_dir(_file: &File, _actual_path: &Path) -> io::Result<SecureReadDir> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure directory handles are unavailable on this Unix target",
        ))
    }

    /// Owned `DIR*` from `fdopendir`, closed exactly once on drop.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) struct DirectoryStream {
        stream: std::ptr::NonNull<libc::DIR>,
        finished: bool,
    }

    // SAFETY: the stream is owned exclusively by this value and is only used
    // through `&mut self` or on drop; a DIR stream may move between threads.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    unsafe impl Send for DirectoryStream {}

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    impl DirectoryStream {
        /// Next entry name, skipping `.` and `..` as `std::fs::ReadDir` does.
        pub(super) fn next(&mut self) -> Option<io::Result<OsString>> {
            while !self.finished {
                // readdir signals both end and failure with NULL; only errno
                // distinguishes them, so clear it first.
                clear_errno();
                let entry = unsafe {
                    // SAFETY: `stream` is a live DIR* owned by `self`.
                    libc::readdir(self.stream.as_ptr())
                };
                if entry.is_null() {
                    self.finished = true;
                    let error = io::Error::last_os_error();
                    return match error.raw_os_error() {
                        Some(0) | None => None,
                        Some(_) => Some(Err(error)),
                    };
                }
                let name = unsafe {
                    // SAFETY: a non-null readdir result points to a dirent
                    // whose `d_name` is NUL-terminated and stays valid until
                    // the next readdir/closedir on this stream; the bytes are
                    // copied before either happens.
                    std::ffi::CStr::from_ptr((*entry).d_name.as_ptr())
                }
                .to_bytes();
                if name != b"." && name != b".." {
                    return Some(Ok(OsStr::from_bytes(name).to_os_string()));
                }
            }
            None
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    impl Drop for DirectoryStream {
        fn drop(&mut self) {
            // SAFETY: `stream` came from fdopendir, is owned by `self` and is
            // closed only here, which also closes its descriptor.
            unsafe {
                libc::closedir(self.stream.as_ptr());
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn clear_errno() {
        // SAFETY: the errno accessor returns this thread's errno location,
        // which is always valid to write.
        unsafe {
            #[cfg(target_os = "linux")]
            {
                *libc::__errno_location() = 0;
            }
            #[cfg(target_os = "macos")]
            {
                *libc::__error() = 0;
            }
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory(label: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "llg-secure-fs-{label}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create secure filesystem test directory");
        // Handle paths are resolved (macOS reports /var/... as /private/var/...).
        path.canonicalize()
            .expect("canonicalize secure filesystem test directory")
    }

    #[test]
    fn live_filename_with_deleted_suffix_is_admitted() {
        let root = temporary_directory("live-deleted-suffix");
        let path = root.join("source (deleted)");
        std::fs::write(&path, b"module source; endmodule\n")
            .expect("write live deleted-suffix source");

        let opened = open_path(&path).expect("live deleted-suffix source should open");
        assert_eq!(opened.actual_path(), path);
        assert!(opened.is_file());

        std::fs::remove_dir_all(root).expect("remove live deleted-suffix directory");
    }

    #[test]
    fn directory_listing_skips_dot_entries_and_leaves_admitted_handle_reusable() {
        fn assert_send<T: Send>() {}
        assert_send::<SecureReadDir>();

        let root = temporary_directory("read-dir");
        std::fs::create_dir(root.join("rtl")).expect("create listed directory");
        std::fs::write(root.join("cell.sv"), b"").expect("write listed file");
        let opened = open_path(&root).expect("open listed directory");
        // Each listing uses its own descriptor, so repeating it on the same
        // admitted handle yields the same names.
        for _ in 0..2 {
            let mut names = opened
                .read_dir()
                .expect("list admitted directory")
                .collect::<std::io::Result<Vec<_>>>()
                .expect("read directory entries");
            names.sort();
            assert_eq!(names, [OsString::from("cell.sv"), OsString::from("rtl")]);
        }

        std::fs::remove_dir_all(root).expect("remove listed directory");
    }

    #[test]
    fn unlinked_handle_is_rejected() {
        let root = temporary_directory("unlinked");
        let path = root.join("source.sv");
        std::fs::write(&path, b"module source; endmodule\n").expect("write unlinked source");
        let file = std::fs::File::open(&path).expect("open unlinked source");
        std::fs::remove_file(&path).expect("unlink source while handle remains open");

        let error = opened_from_file(file, None, None)
            .expect_err("an unlinked handle must not be admitted");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(error.to_string().contains("deleted"));

        std::fs::remove_dir_all(root).expect("remove unlinked directory");
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        File, FileIdentity, Metadata, OsString, Path, PathBuf, SecureReadDir, SecureReadDirInner,
    };
    use std::io;
    use std::mem::MaybeUninit;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, GetFinalPathNameByHandleW, BY_HANDLE_FILE_INFORMATION,
        FILE_FLAG_BACKUP_SEMANTICS,
    };

    pub(super) fn identity(file: &File, _metadata: &Metadata) -> io::Result<FileIdentity> {
        let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        let result = unsafe {
            // SAFETY: `file` is a live handle borrowed for this call and the
            // API writes one complete `BY_HANDLE_FILE_INFORMATION` value to
            // the valid out pointer when it reports success.
            GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr())
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful Win32 call initialized every field of the
        // `BY_HANDLE_FILE_INFORMATION` output structure.
        let information = unsafe { information.assume_init() };
        Ok(FileIdentity {
            volume_serial: information.dwVolumeSerialNumber,
            file_index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }

    pub(super) fn path_options() -> std::fs::OpenOptions {
        use std::os::windows::fs::OpenOptionsExt;
        let mut options = std::fs::OpenOptions::new();
        options.read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        options
    }

    pub(super) fn path_from_handle(file: &File) -> io::Result<PathBuf> {
        let handle = file.as_raw_handle();
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u16; 512];
        loop {
            let length = unsafe {
                // SAFETY: `handle` is borrowed from a live `File`; `buffer`
                // is writable storage whose capacity is passed accurately.
                GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
            };
            if length == 0 {
                return Err(io::Error::last_os_error());
            }
            if (length as usize) < buffer.len() {
                let mut path = OsString::from_wide(&buffer[..length as usize]);
                let value = path.to_string_lossy();
                if let Some(stripped) = value.strip_prefix("\\\\?\\") {
                    path = if let Some(unc) = stripped.strip_prefix("UNC\\") {
                        OsString::from(format!(r"\\{unc}"))
                    } else {
                        OsString::from(stripped)
                    };
                }
                return Ok(PathBuf::from(path));
            }
            buffer.resize(buffer.len().saturating_mul(2), 0);
            if buffer.len() > 32 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "secure filesystem handle path is too long",
                ));
            }
        }
    }

    pub(super) fn path_is_within(path: &Path, root: &Path) -> bool {
        let path = normalized(path);
        let root = normalized(root);
        let path = Path::new(&path);
        let root = Path::new(&root);
        path == root || path.starts_with(root)
    }

    pub(super) fn same_path(path: &Path, expected: &Path) -> bool {
        normalized(path) == normalized(expected)
    }

    pub(super) fn read_dir(_file: &File, actual_path: &Path) -> io::Result<SecureReadDir> {
        Ok(SecureReadDir {
            inner: SecureReadDirInner::Standard(std::fs::read_dir(actual_path)?),
        })
    }

    fn normalized(path: &Path) -> String {
        let mut value = path.to_string_lossy().replace('/', r"\");
        if let Some(stripped) = value.strip_prefix("\\\\?\\") {
            value = stripped.to_owned();
        }
        if let Some(unc) = value.strip_prefix("UNC\\") {
            value = format!(r"\\{unc}");
        }
        value.to_ascii_lowercase()
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::{File, FileIdentity, Metadata, Path, PathBuf, SecureReadDir};
    use std::io;

    pub(super) fn identity(_file: &File, _metadata: &Metadata) -> io::Result<FileIdentity> {
        Ok(FileIdentity { marker: () })
    }

    pub(super) fn path_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options
    }

    pub(super) fn path_from_handle(_file: &File) -> io::Result<PathBuf> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure handle paths are unavailable on this platform",
        ))
    }

    pub(super) fn path_is_within(_path: &Path, _root: &Path) -> bool {
        false
    }

    pub(super) fn same_path(_path: &Path, _expected: &Path) -> bool {
        false
    }

    pub(super) fn read_dir(_file: &File, _actual_path: &Path) -> io::Result<SecureReadDir> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure directory traversal is unavailable on this platform",
        ))
    }
}
