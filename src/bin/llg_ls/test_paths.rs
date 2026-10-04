//! Host-absolute spellings of the Unix-style file names unit tests use
//! (`/x/top.sv`).
//!
//! The LSP turns analysis file names into `file:` URIs, which needs an
//! absolute path on the host. `/x/top.sv` is absolute on Unix but has no
//! drive on Windows, where it becomes `C:\x\top.sv` (the drive of the
//! temporary directory). On Unix every name is returned unchanged.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The host-absolute form of `path` when it starts with `/`; other strings
/// are returned unchanged. Results are interned so callers keep `&str`.
pub(crate) fn host_path(path: &'static str) -> &'static str {
    if !path.starts_with('/') {
        return path;
    }
    static CACHE: OnceLock<Mutex<HashMap<&'static str, &'static str>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(found) = cache.get(path) {
        return found;
    }
    let temp = std::env::temp_dir();
    let mut built: PathBuf = temp.ancestors().last().unwrap_or(&temp).to_path_buf();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        built.push(segment);
    }
    let mut text = built.to_string_lossy().into_owned();
    if path.len() > 1 && path.ends_with('/') && !text.ends_with(std::path::MAIN_SEPARATOR) {
        text.push(std::path::MAIN_SEPARATOR);
    }
    let interned: &'static str = Box::leak(text.into_boxed_str());
    cache.insert(path, interned);
    interned
}

#[cfg(test)]
mod tests {
    use super::host_path;

    #[test]
    fn unix_style_names_become_absolute_host_paths() {
        let path = host_path("/x/top.sv");
        assert!(std::path::Path::new(path).is_absolute(), "{path}");
        assert!(std::path::Path::new(path).ends_with("x/top.sv"), "{path}");
        assert!(std::ptr::eq(path, host_path("/x/top.sv")));
        assert_eq!(host_path("rel/top.sv"), "rel/top.sv");
        if cfg!(unix) {
            assert_eq!(path, "/x/top.sv");
            assert_eq!(host_path("/x/"), "/x/");
        }
    }
}
