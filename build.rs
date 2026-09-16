//! Builds the vendored Slang frontend and its C ABI wrapper.
//!
//! Release builds target fully static musl Linux, static-CRT MSVC Windows,
//! and Apple Silicon macOS.

use std::path::{Path, PathBuf};

#[path = "build_support/vendor_patches.rs"]
mod vendor_patches;

fn emit_rerun_if_changed() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let target_underscored = target.replace('-', "_");
    for variable in [
        "LLG_CCACHE".to_string(),
        "LLG_SYSROOT".to_string(),
        "CC".to_string(),
        "CXX".to_string(),
        "TARGET_CC".to_string(),
        "TARGET_CXX".to_string(),
        format!("CC_{target}"),
        format!("CC_{target_underscored}"),
        format!("CXX_{target}"),
        format!("CXX_{target_underscored}"),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    for path in [
        "Cargo.lock",
        "patches/slang",
        "patches/libaco",
        "src/wrapper/mimalloc_shim.c",
        "src/wrapper/slang/CMakeLists.txt",
        "src/wrapper/slang_c_api.cpp",
        "src/wrapper/slang_c_api.h",
        "vendor/slang/CMakeLists.txt",
        "vendor/slang/cmake",
        "vendor/slang/external",
        "vendor/slang/include",
        "vendor/slang/scripts",
        "vendor/slang/source",
        "vendor/libaco",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn requested_ccache() -> Option<PathBuf> {
    let requested = std::env::var("LLG_CCACHE").unwrap_or_default();
    if !matches!(requested.to_ascii_lowercase().as_str(), "1" | "on" | "true") {
        return None;
    }
    let executable = if cfg!(windows) {
        "ccache.exe"
    } else {
        "ccache"
    };
    let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(executable))
        .find(|path| path.is_file());
    if found.is_none() {
        println!(
            "cargo:warning=LLG_CCACHE requested `{executable}` but it was not found on PATH; \
             continuing without a compiler launcher"
        );
    }
    found
}

/// CMake caches launcher values. Remove only its cache when the requested
/// launcher changes, leaving compiled objects available for reuse.
fn sync_launcher_state(build_dir: &Path, active: bool) {
    let state = if active { "ccache" } else { "none" };
    let marker = build_dir.join(".llg_ccache_state");
    let changed = match std::fs::read_to_string(&marker) {
        Ok(previous) => previous.trim() != state,
        Err(_) => active,
    };
    if !changed {
        return;
    }
    let cache = build_dir.join("build/CMakeCache.txt");
    if cache.exists() {
        std::fs::remove_file(&cache)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", cache.display()));
        println!("cargo:warning=LLG_CCACHE changed to `{state}`; forcing Slang CMake reconfigure");
    }
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
    }
    std::fs::write(&marker, format!("{state}\n"))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", marker.display()));
}

/// Native build caches can survive a workspace move through a shared target
/// volume. Force reconfiguration when the cached source path no longer names
/// this checkout; otherwise CMake rejects the build before it can regenerate.
fn sync_cmake_source(build_dir: &Path, source_dir: &Path) {
    let cache = build_dir.join("build/CMakeCache.txt");
    let Ok(contents) = std::fs::read_to_string(&cache) else {
        return;
    };
    let cached_source = contents.lines().find_map(|line| {
        line.strip_prefix("CMAKE_HOME_DIRECTORY:INTERNAL=")
            .map(PathBuf::from)
    });
    if cached_source.as_deref() == Some(source_dir) {
        return;
    }
    std::fs::remove_file(&cache)
        .unwrap_or_else(|error| panic!("failed to remove {}: {error}", cache.display()));
    println!(
        "cargo:warning=Slang build cache belongs to another workspace; forcing CMake reconfigure"
    );
}

fn emit_native_search(directory: &Path, is_msvc: bool) {
    println!("cargo:rustc-link-search=native={}", directory.display());
    if is_msvc {
        for configuration in ["Release", "Debug"] {
            println!(
                "cargo:rustc-link-search=native={}",
                directory.join(configuration).display()
            );
        }
    }
}

/// Keep CMake's absolute-path cache private to the path visible to this
/// process. The Cargo target directory can be shared by containers that mount
/// the same checkout at different paths, but one CMake build tree cannot.
fn workspace_cache_key(manifest_dir: &Path) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in manifest_dir.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Resolve a native compiler with cc-rs-compatible target precedence. Generic
/// CC/CXX remains useful for native Alpine builds, while target-specific values
/// win for cross compilation.
fn target_tool(executable: &str, target: &str, variable: &str) -> String {
    let target_underscored = target.replace('-', "_");
    let keys = [
        format!("{variable}_{target}"),
        format!("{variable}_{target_underscored}"),
        format!("TARGET_{variable}"),
        variable.to_string(),
    ];
    keys.into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| format!("{}-{executable}", target.replace("-unknown-", "-")))
}

fn build_slang(manifest_dir: &Path) {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "default".to_string());
    let target = std::env::var("TARGET").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_msvc = target_env == "msvc";
    let is_musl = target_env == "musl";

    let project = manifest_dir.join("src/wrapper/slang");
    let repo = manifest_dir.join("vendor/slang");
    let build_dir = manifest_dir
        .join("target/slang")
        .join(&target)
        .join(profile)
        .join(workspace_cache_key(manifest_dir));

    sync_cmake_source(&build_dir, &project);

    let mut config = cmake::Config::new(project);
    config
        .out_dir(&build_dir)
        // Slang debug archives are exceptionally large and expose the same C
        // ABI. Rust remains in the selected Cargo profile.
        .profile("Release")
        .define("LLG_SLANG_SOURCE_DIR", &repo)
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("FETCHCONTENT_TRY_FIND_PACKAGE_MODE", "NEVER");
    if is_msvc {
        config
            .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded")
            .define("SLANG_WARN_FLAGS", "/w");
    } else {
        config.define("SLANG_WARN_FLAGS", "-w");
    }

    let ccache = requested_ccache();
    sync_launcher_state(&build_dir, ccache.is_some());
    let launcher = ccache
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    config.define("CMAKE_CXX_COMPILER_LAUNCHER", launcher);

    if is_musl {
        config
            .define("CMAKE_CXX_COMPILER", target_tool("g++", &target, "CXX"))
            .define("CMAKE_SYSTEM_NAME", "Linux")
            .define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY")
            .define("CMAKE_FIND_LIBRARY_SUFFIXES", ".a")
            .define("LLG_SLANG_MUSL", "ON");
    }

    let install_dir = config.build();
    emit_native_search(&install_dir.join("lib"), is_msvc);
    println!("cargo:rustc-link-lib=static=llg_slang_wrapper");
    println!("cargo:rustc-link-lib=static=svlang");
    println!("cargo:rustc-link-lib=static=fmt");

    if is_musl {
        let mimalloc_archive = build_mimalloc_shim(manifest_dir, &target);
        let drivers = collect_driver_candidates(&target);
        let archives = find_static_archives(&drivers, &["stdc++", "gcc"], &["supc++", "gcc_eh"]);
        for archive in ["stdc++", "supc++", "gcc_eh", "gcc"] {
            if archives.contains(archive) {
                println!("cargo:rustc-link-lib=static={archive}");
            }
        }
        println!("cargo:rustc-link-arg=-static");
        emit_mimalloc_link_args(&mimalloc_archive, &archives);
    } else {
        match target_os.as_str() {
            "macos" => println!("cargo:rustc-link-lib=dylib=c++"),
            "windows" => {}
            _ => {
                println!("cargo:rustc-link-lib=dylib=stdc++");
                println!("cargo:rustc-link-lib=dylib=pthread");
            }
        }
    }
}

fn build_mimalloc_shim(manifest_dir: &Path, target: &str) -> PathBuf {
    let mimalloc = find_libmimalloc_sys_src(manifest_dir);
    let include = mimalloc.join("c_src/mimalloc/v3/include");
    let source_dir = mimalloc.join("c_src/mimalloc/v3/src");
    let source = source_dir.join("static.c");
    println!("cargo:rerun-if-changed={}", source.display());

    cc::Build::new()
        // Keep cc-rs metadata so downstream consumers of the Rust library get
        // the allocator archive. Package tests and binaries can link without
        // using that library, so they also receive its absolute path below.
        .compiler(target_tool("gcc", target, "CC"))
        .file(manifest_dir.join("src/wrapper/mimalloc_shim.c"))
        .file(source)
        .include(include)
        .include(source_dir)
        .define("MI_DEBUG", Some("0"))
        .flag("-Wno-date-time")
        .flag_if_supported("-ftls-model=initial-exec")
        .compile("mimalloc_with_shim");

    let out_dir = PathBuf::from(
        std::env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR for the build script"),
    );
    let archive = out_dir.join("libmimalloc_with_shim.a");
    assert!(
        archive.is_file(),
        "cc did not produce the combined musl allocator archive {}",
        archive.display()
    );
    archive.canonicalize().unwrap_or_else(|error| {
        panic!(
            "failed to canonicalize combined musl allocator archive {}: {error}",
            archive.display()
        )
    })
}

fn emit_mimalloc_link_args(archive: &Path, available: &std::collections::BTreeSet<String>) {
    for symbol in [
        "malloc",
        "calloc",
        "realloc",
        "free",
        "aligned_alloc",
        "posix_memalign",
    ] {
        println!("cargo:rustc-link-arg=-Wl,--wrap={symbol}");
    }

    // `rustc-link-arg` values appear after Rust's standard-library archives.
    // The undefined wrapper symbol extracts the shim and mimalloc objects from
    // the archive without making a repeated metadata link define them twice.
    // Then rescan static libc and the compiler runtime for mimalloc's uses.
    // rustc may have switched to dynamic library lookup before link args are
    // appended; without this, small test binaries can pick libc.so and crash
    // in the static PIE startup code before their test harness runs.
    println!("cargo:rustc-link-arg=-Wl,--undefined=__wrap_malloc");
    println!("cargo:rustc-link-arg=-Wl,-Bstatic");
    println!("cargo:rustc-link-arg=-Wl,--start-group");
    println!("cargo:rustc-link-arg={}", archive.display());
    println!("cargo:rustc-link-arg=-lc");
    if available.contains("gcc_eh") {
        println!("cargo:rustc-link-arg=-lgcc_eh");
    }
    if available.contains("gcc") {
        println!("cargo:rustc-link-arg=-lgcc");
    }
    println!("cargo:rustc-link-arg=-Wl,--end-group");
}

fn collect_driver_candidates(target: &str) -> Vec<String> {
    let mut drivers = vec![
        target_tool("g++", target, "CXX"),
        target_tool("gcc", target, "CC"),
    ];
    let mut seen = std::collections::BTreeSet::new();
    drivers.retain(|driver| seen.insert(driver.clone()));
    drivers
}

fn fallback_search_dirs(target: &str) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if !target.is_empty() {
        directories.push(PathBuf::from(format!(
            "/usr/lib/{}",
            target.replace("-unknown-", "-")
        )));
    }
    if let Ok(sysroot) = std::env::var("LLG_SYSROOT") {
        if !sysroot.is_empty() {
            directories.push(PathBuf::from(sysroot).join("usr/lib"));
        }
    }
    directories
}

fn find_static_archives(
    drivers: &[String],
    required: &[&str],
    tolerated: &[&str],
) -> std::collections::BTreeSet<String> {
    let target = std::env::var("TARGET").unwrap_or_default();
    let fallback = fallback_search_dirs(&target);
    let fallback_list = fallback
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut emitted = std::collections::BTreeSet::new();
    let mut available = std::collections::BTreeSet::new();

    for archive in required.iter().chain(tolerated) {
        let file_name = format!("lib{archive}.a");
        let mut directory = None;
        for driver in drivers {
            let Ok(output) = std::process::Command::new(driver)
                .arg(format!("-print-file-name={file_name}"))
                .output()
            else {
                continue;
            };
            let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            if path.is_absolute() && path.exists() {
                let canonical = path.canonicalize().unwrap_or(path);
                directory = canonical.parent().map(Path::to_path_buf);
                break;
            }
        }
        if directory.is_none() {
            directory = fallback
                .iter()
                .find(|candidate| candidate.join(&file_name).exists())
                .cloned();
        }
        match directory {
            Some(path) => {
                available.insert(archive.to_string());
                if emitted.insert(path.clone()) {
                    println!("cargo:rustc-link-search=native={}", path.display());
                }
            }
            None if required.contains(archive) => panic!(
                "could not locate required static archive `{file_name}` for musl; \
                 target drivers tried: {}; target/sysroot fallback directories: {fallback_list}. \
                 Set CXX_<target>, CXX_<target_with_underscores>, TARGET_CXX, or CXX to the \
                 musl C++ driver; set LLG_SYSROOT when its target libraries are outside the \
                 compiler's search path",
                drivers.join(", ")
            ),
            None => println!(
                "cargo:warning=optional musl archive `{file_name}` was not found by target \
                 drivers ({}) or in target/sysroot directories {fallback_list}",
                drivers.join(", ")
            ),
        }
    }
    available
}

fn find_libmimalloc_sys_src(manifest_dir: &Path) -> PathBuf {
    let lock_path = manifest_dir.join("Cargo.lock");
    let lock = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", lock_path.display()));
    let version = lock
        .split("[[package]]")
        .find(|package| {
            package
                .lines()
                .any(|line| line.trim() == "name = \"libmimalloc-sys\"")
        })
        .and_then(|package| {
            package.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("version = \"")
                    .and_then(|value| value.strip_suffix('"'))
            })
        })
        .unwrap_or_else(|| panic!("libmimalloc-sys version is absent from Cargo.lock"));
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .unwrap_or_else(|| panic!("neither CARGO_HOME nor HOME is set"));
    let registry = cargo_home.join("registry/src");
    for entry in std::fs::read_dir(&registry)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", registry.display()))
    {
        let candidate = entry
            .unwrap_or_else(|error| panic!("cannot read registry entry: {error}"))
            .path()
            .join(format!("libmimalloc-sys-{version}"));
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "libmimalloc-sys-{version} not found under {}",
        registry.display()
    );
}

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    emit_rerun_if_changed();
    vendor_patches::emit_rerun_if_changed(&manifest_dir);
    vendor_patches::apply_all(&manifest_dir)
        .unwrap_or_else(|error| panic!("vendor patch preparation failed: {error}"));
    build_slang(&manifest_dir);
}
