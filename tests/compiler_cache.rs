#[path = "../build_support/compiler_cache.rs"]
mod compiler_cache;
#[path = "../build_support/host_platform.rs"]
mod host_platform;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "llg-compiler-cache-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn executable(&self, name: &str) -> PathBuf {
        let name = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        };
        let path = self.0.join(name);
        std::fs::write(&path, "tool").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn default_and_disabled_cache_need_no_tools() {
    let empty = Scratch::new();
    for setting in ["", "0", "off", "false"] {
        assert_eq!(
            compiler_cache::requested_launcher(&empty.0, setting, empty.0.as_os_str()).unwrap(),
            None
        );
    }
}

#[test]
fn requested_cache_must_exist_and_invalid_values_fail() {
    let empty = Scratch::new();
    for setting in ["1", "ccache", "sccache"] {
        let error =
            compiler_cache::requested_launcher(&empty.0, setting, empty.0.as_os_str()).unwrap_err();
        assert!(error.contains("LLG_CCACHE"));
        assert!(error.contains("PATH"));
    }
    assert!(compiler_cache::requested_launcher(&empty.0, "typo", empty.0.as_os_str()).is_err());
}

#[test]
fn named_cache_and_legacy_aliases_select_the_requested_tool() {
    let tools = Scratch::new();
    let ccache = tools.executable("ccache");
    let sccache = tools.executable("sccache");
    for setting in ["1", "on", "true", "ccache", "CCACHE"] {
        assert_eq!(
            compiler_cache::requested_launcher(&tools.0, setting, tools.0.as_os_str()).unwrap(),
            Some(ccache.clone())
        );
    }
    let expected = if cfg!(unix) {
        tools.0.join("scripts/sccache.sh")
    } else {
        sccache
    };
    assert_eq!(
        compiler_cache::requested_launcher(&tools.0, "sccache", tools.0.as_os_str()).unwrap(),
        Some(expected)
    );
}

#[cfg(unix)]
#[test]
fn non_executable_cache_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let tools = Scratch::new();
    let cache = tools.executable("ccache");
    std::fs::set_permissions(cache, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(compiler_cache::requested_launcher(&tools.0, "1", tools.0.as_os_str()).is_err());
}

#[test]
fn launcher_changes_remove_only_cmake_cache_and_unchanged_launcher_reuses_it() {
    let scratch = Scratch::new();
    let build = scratch.0.join("build");
    std::fs::create_dir(&build).unwrap();
    let cache = build.join("CMakeCache.txt");
    let object = build.join("keep.o");
    std::fs::write(&object, "object").unwrap();
    compiler_cache::sync_launcher_state(&scratch.0, None).unwrap();
    assert!(!scratch.0.join(".llg_ccache_state").exists());
    for launcher in [
        Some(Path::new("/ccache")),
        Some(Path::new("/sccache")),
        None,
    ] {
        std::fs::write(&cache, "configuration").unwrap();
        compiler_cache::sync_launcher_state(&scratch.0, launcher).unwrap();
        assert!(!cache.exists());
        assert!(object.exists());
        std::fs::write(&cache, "configuration").unwrap();
        compiler_cache::sync_launcher_state(&scratch.0, launcher).unwrap();
        assert!(cache.exists());
    }
}

#[test]
fn legacy_ccache_marker_migrates_without_removing_objects() {
    let scratch = Scratch::new();
    let build = scratch.0.join("build");
    std::fs::create_dir(&build).unwrap();
    std::fs::write(scratch.0.join(".llg_ccache_state"), "ccache\n").unwrap();
    std::fs::write(build.join("CMakeCache.txt"), "old").unwrap();
    compiler_cache::sync_launcher_state(&scratch.0, Some(Path::new("/usr/bin/ccache"))).unwrap();
    assert!(!build.join("CMakeCache.txt").exists());
    assert_eq!(
        std::fs::read_to_string(scratch.0.join(".llg_ccache_state")).unwrap(),
        "/usr/bin/ccache\n"
    );
}
