use super::host_platform;
use std::path::{Path, PathBuf};

/// Resolve an explicitly requested native cache; Unix sccache uses our socket-safe wrapper.
pub fn requested_launcher(
    manifest_dir: &Path,
    requested: &str,
    search_path: &std::ffi::OsStr,
) -> Result<Option<PathBuf>, String> {
    let tool = match requested.to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "ccache" => "ccache",
        "sccache" => "sccache",
        "" | "0" | "off" | "false" => return Ok(None),
        _ => return Err("LLG_CCACHE must be 0, 1, ccache or sccache".to_owned()),
    };
    let executable = host_platform::executable_name(tool);
    let found = std::env::split_paths(search_path)
        .map(|directory| directory.join(&executable))
        .find(|path| host_platform::is_executable(path));
    let found = found.ok_or_else(|| {
        format!("LLG_CCACHE requested `{executable}` but it was not found executable on PATH")
    })?;
    if tool == "sccache" && host_platform::sccache_needs_unix_wrapper() {
        Ok(Some(manifest_dir.join("scripts/sccache.sh")))
    } else {
        Ok(Some(found))
    }
}

/// CMake caches launcher values. Remove only its cache when the requested
/// launcher changes, leaving compiled objects available for reuse.
pub fn sync_launcher_state(build_dir: &Path, launcher: Option<&Path>) -> std::io::Result<()> {
    let state = launcher
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "none".to_owned());
    let marker = build_dir.join(".llg_ccache_state");
    let changed = match std::fs::read_to_string(&marker) {
        Ok(previous) => previous.trim() != state,
        Err(_) => launcher.is_some(),
    };
    if !changed {
        return Ok(());
    }
    let cache = build_dir.join("build/CMakeCache.txt");
    if cache.exists() {
        std::fs::remove_file(&cache)?;
        println!("cargo:warning=LLG_CCACHE changed to `{state}`; forcing Slang CMake reconfigure");
    }
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, format!("{state}\n"))
}
