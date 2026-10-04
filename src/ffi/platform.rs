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
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::fs::canonicalize(path).map(strip_verbatim_prefix)
}

/// Method form of [`canonicalize`] for path chains:
/// `dir.join("a.sv").canonical()`.
pub trait CanonicalPath {
    /// [`canonicalize`] applied to this path.
    fn canonical(&self) -> io::Result<PathBuf>;
}

impl CanonicalPath for Path {
    fn canonical(&self) -> io::Result<PathBuf> {
        canonicalize(self)
    }
}

/// A relative path that encodes absolute `path` one-to-one, for mirroring
/// files under a private directory (`base.join(mirror_relative(p))`).
///
/// Unix drops the leading `/`. Windows turns the prefix into ordinary
/// components (`C:\x` -> `C\x`, `\\server\share\x` -> `UNC\server\share\x`),
/// so joining the result can never replace `base` the way joining an
/// absolute Windows path does. [`mirror_absolute`] is the inverse.
pub fn mirror_relative(path: &Path) -> PathBuf {
    host::mirror_relative(path)
}

/// Inverse of [`mirror_relative`]; `None` when `relative` does not start
/// with an encoded root.
pub fn mirror_absolute(relative: &Path) -> Option<PathBuf> {
    host::mirror_absolute(relative)
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

/// `path` spelled for a glob pattern (LSP `GlobPattern`, file watchers):
/// `/` separates components on every host, because glob syntax reserves `\`
/// as an escape. Windows separators are rewritten; `None` when the path is
/// not valid Unicode. Glob metacharacters inside names are not escaped.
pub fn glob_spelling(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    Some(if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.to_owned()
    })
}

/// Whether paths should be resolved through symlinks before they are compared
/// with handle-derived spellings. POSIX temporary directories may be symlinks
/// (macOS `/var` -> `/private/var`); Windows handle paths keep the requested
/// spelling apart from the verbatim prefix, so they are compared as given.
pub fn resolves_symlinked_paths() -> bool {
    cfg!(unix)
}

/// Rewrites every CRLF pair as LF; a lone CR is kept.
pub fn crlf_to_lf(bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.windows(2).any(|pair| pair == b"\r\n") {
        return bytes;
    }
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter().copied().peekable();
    while let Some(byte) = iter.next() {
        if byte == b'\r' && iter.peek() == Some(&b'\n') {
            continue;
        }
        normalized.push(byte);
    }
    normalized
}

/// Text a generated model or `llg` wrote to its console or to a text-mode
/// file, with the host's native line ending rewritten to LF. Simulators keep
/// the OS-native newline, so Windows output ends lines with CRLF; this gives
/// tests and tools one LF spelling to compare against. Other hosts already
/// write LF, and their bytes are returned unchanged.
pub fn native_text_to_lf(bytes: Vec<u8>) -> Vec<u8> {
    if cfg!(windows) {
        crlf_to_lf(bytes)
    } else {
        bytes
    }
}

#[cfg(windows)]
mod host {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Component, Path, PathBuf, Prefix};

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

    // Prefix encodings; drive letters are one character, so they cannot
    // collide with these names.
    const UNC: &str = "UNC";
    const DEVICE: &str = "DEVICE";
    const VERBATIM: &str = "VERBATIM";

    pub(super) fn mirror_relative(path: &Path) -> PathBuf {
        let mut relative = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => match prefix.kind() {
                    Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                        relative.push(char::from(letter).to_ascii_uppercase().to_string())
                    }
                    Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                        relative.push(UNC);
                        relative.push(server);
                        relative.push(share);
                    }
                    Prefix::DeviceNS(name) => {
                        relative.push(DEVICE);
                        relative.push(name);
                    }
                    Prefix::Verbatim(name) => {
                        relative.push(VERBATIM);
                        relative.push(name);
                    }
                },
                Component::RootDir => {}
                other => relative.push(other.as_os_str()),
            }
        }
        relative
    }

    pub(super) fn mirror_absolute(relative: &Path) -> Option<PathBuf> {
        let mut components = relative.components();
        let Some(Component::Normal(first)) = components.next() else {
            return None;
        };
        let first = first.to_str()?;
        let mut root = match first {
            UNC => {
                let server = components.next()?.as_os_str().to_str()?.to_owned();
                let share = components.next()?.as_os_str().to_str()?.to_owned();
                PathBuf::from(format!(r"\\{server}\{share}\"))
            }
            DEVICE => {
                let name = components.next()?.as_os_str().to_str()?.to_owned();
                PathBuf::from(format!(r"\\.\{name}\"))
            }
            VERBATIM => {
                let name = components.next()?.as_os_str().to_str()?.to_owned();
                PathBuf::from(format!(r"\\?\{name}\"))
            }
            letter if letter.len() == 1 && letter.as_bytes()[0].is_ascii_alphabetic() => {
                PathBuf::from(format!(r"{letter}:\"))
            }
            _ => return None,
        };
        root.push(components.as_path());
        Some(root)
    }
}

#[cfg(not(windows))]
mod host {
    use std::path::{Component, Path, PathBuf};

    pub(super) fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
        path
    }

    pub(super) fn mirror_relative(path: &Path) -> PathBuf {
        path.strip_prefix("/").unwrap_or(path).to_path_buf()
    }

    pub(super) fn mirror_absolute(relative: &Path) -> Option<PathBuf> {
        match relative.components().next() {
            Some(Component::Normal(_)) => Some(Path::new("/").join(relative)),
            _ => None,
        }
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
            canonicalize(path.join("Cargo.toml")).expect("canonical manifest"),
            path.join("Cargo.toml")
        );
    }

    #[test]
    fn glob_spellings_use_forward_slashes() {
        assert_eq!(
            glob_spelling(Path::new("rtl/top.sv")).as_deref(),
            Some("rtl/top.sv")
        );
        let windows = glob_spelling(Path::new(r"C:\work\llg.toml")).expect("unicode path");
        if cfg!(windows) {
            assert_eq!(windows, "C:/work/llg.toml");
        } else {
            // A backslash is an ordinary name character on POSIX hosts.
            assert_eq!(windows, r"C:\work\llg.toml");
        }
    }

    #[test]
    fn crlf_pairs_become_lf_and_lone_carriage_returns_stay() {
        assert_eq!(crlf_to_lf(b"a\r\nb\r\n".to_vec()), b"a\nb\n");
        assert_eq!(crlf_to_lf(b"a\rb\r\r\n\n".to_vec()), b"a\rb\r\n\n");
        assert_eq!(crlf_to_lf(b"plain\n".to_vec()), b"plain\n");
        assert_eq!(crlf_to_lf(Vec::new()), b"");
        let native = native_text_to_lf(b"x\r\n".to_vec());
        if cfg!(windows) {
            assert_eq!(native, b"x\n");
        } else {
            assert_eq!(native, b"x\r\n");
        }
    }

    #[test]
    fn mirrored_paths_stay_relative_and_round_trip() {
        let absolute = canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("canonical root")
            .join("src")
            .join("lib.rs");
        let relative = mirror_relative(&absolute);
        assert!(relative.is_relative(), "{}", relative.display());
        let base = Path::new("base");
        assert!(base.join(&relative).starts_with(base));
        assert_eq!(mirror_absolute(&relative), Some(absolute));
        assert_eq!(mirror_absolute(Path::new("")), None);
        if cfg!(unix) {
            assert_eq!(
                mirror_relative(Path::new("/x/top.sv")),
                Path::new("x/top.sv")
            );
        }
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
