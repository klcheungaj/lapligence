//! Host-platform facts and path spellings behind a platform-neutral API.
//!
//! Rust code outside `src/ffi/` must not branch on `cfg(windows)`,
//! `cfg(unix)` or `target_os`; it calls these helpers (or the handle-based
//! [`super::secure_fs`] and [`super::process_memory`] services) instead.
//! Tests may still use platform `cfg`s where they assert platform-specific
//! behaviour such as symlinks.

use std::io;
use std::path::{Path, PathBuf};

/// Characters Windows rejects in file and directory names (in addition to
/// control characters). POSIX hosts accept all of them.
const WINDOWS_RESERVED_NAME_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Removes the Windows verbatim prefix (`\\?\C:\...`, `\\?\UNC\server\...`)
/// that `std::fs::canonicalize` and handle queries return, giving the
/// ordinary spelling users, compilers and other tools expect (`C:\...`,
/// `\\server\...`). Other paths, and every path on other hosts, are returned
/// unchanged.
pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    host::strip_verbatim_prefix(path)
}

/// `std::fs::canonicalize` with the result in ordinary spelling (see
/// [`strip_verbatim_prefix`]), so it compares equal to handle-derived paths
/// and can be handed to CMake, compilers and LSP clients.
pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(path).map(strip_verbatim_prefix)
}

/// Whether the host accepts `name` as one file or directory name component.
/// Windows rejects `<>:"/\|?*` and control characters; POSIX hosts reject
/// only `/` and NUL.
pub fn is_valid_file_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || name.contains('/') {
        return false;
    }
    !cfg!(windows)
        || !name
            .chars()
            .any(|c| c.is_control() || WINDOWS_RESERVED_NAME_CHARS.contains(&c))
}

/// Replaces every character the host rejects in a file name with `_`, for
/// scratch and temporary names built from arbitrary text.
pub fn portable_file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '/'
                || c == '\0'
                || (cfg!(windows) && (c.is_control() || WINDOWS_RESERVED_NAME_CHARS.contains(&c)))
            {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Whether paths should be resolved through symlinks before they are compared
/// with handle-derived spellings. POSIX temporary directories may be symlinks
/// (macOS `/var` -> `/private/var`); Windows handle paths keep the requested
/// spelling apart from the verbatim prefix, so they are compared as given.
pub fn resolves_symlinked_paths() -> bool {
    cfg!(unix)
}

#[cfg(windows)]
mod host {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;

    // Works on UTF-16 units so unpaired surrogates survive unchanged.
    pub(super) fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
        let Some(rest) = wide.strip_prefix(verbatim.as_slice()) else {
            return path;
        };
        let unc: Vec<u16> = r"UNC\".encode_utf16().collect();
        let stripped = match rest.strip_prefix(unc.as_slice()) {
            Some(share) => r"\\".encode_utf16().chain(share.iter().copied()).collect(),
            None => rest.to_vec(),
        };
        PathBuf::from(OsString::from_wide(&stripped))
    }
}

#[cfg(not(windows))]
mod host {
    use std::path::PathBuf;

    pub(super) fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefixes_are_stripped_only_on_windows() {
        let drive = PathBuf::from(r"\\?\C:\work\top.sv");
        let unc = PathBuf::from(r"\\?\UNC\server\share\top.sv");
        if cfg!(windows) {
            assert_eq!(
                strip_verbatim_prefix(drive),
                PathBuf::from(r"C:\work\top.sv")
            );
            assert_eq!(
                strip_verbatim_prefix(unc),
                PathBuf::from(r"\\server\share\top.sv")
            );
        } else {
            assert_eq!(strip_verbatim_prefix(drive.clone()), drive);
            assert_eq!(strip_verbatim_prefix(unc.clone()), unc);
        }
        let plain = PathBuf::from("rtl/top.sv");
        assert_eq!(strip_verbatim_prefix(plain.clone()), plain);
    }

    #[test]
    fn canonical_paths_have_no_verbatim_prefix_and_match_the_file() {
        let path = canonicalize(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("canonical root");
        assert!(path.is_absolute());
        assert!(!path.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(
            canonicalize(&path.join("Cargo.toml")).expect("canonical manifest"),
            path.join("Cargo.toml")
        );
    }

    #[test]
    fn file_name_rules_follow_the_host() {
        assert!(is_valid_file_name("cell.sv"));
        assert!(is_valid_file_name("é.sv"));
        assert!(!is_valid_file_name(""));
        assert!(!is_valid_file_name("a/b"));
        assert_eq!(is_valid_file_name("lib*"), !cfg!(windows));
        assert_eq!(is_valid_file_name("a:b"), !cfg!(windows));
        let portable = portable_file_name("x*?/y");
        assert!(is_valid_file_name(&portable), "{portable}");
        assert_eq!(portable_file_name("plain-name_1"), "plain-name_1");
    }
}
