//! build.rs — builds the native SystemVerilog frontends and their C ABI
//! wrappers. Surelog remains the default; the `slang` Cargo feature also builds
//! the experimental Slang wrapper and static dependencies.
//!
//! The release matrix targets static-musl Linux on x86_64/aarch64, MSVC
//! Windows on x86_64/aarch64, and Apple Silicon macOS. Vendored libraries are
//! static; platform system libraries remain dynamic on Windows/macOS.

use std::path::{Path, PathBuf};

/// Builds the list of paths Cargo should watch for this build script.
///
/// Why this exists: emitting ANY `cargo:rerun-if-changed=` switches Cargo
/// from its default ("re-run when any file under the package changes") to
/// an exact allow-list.  Every input below therefore feeds either the
/// CMake-built vendor libraries or the locally-compiled wrapper archive;
/// omitting one means edits to it silently keep stale artifacts linked.
///
/// Granularity trade-off: directories are watched as whole trees (Cargo
/// recursively scans them against a dir-mtime fast path), which keeps the
/// per-build scan cheap (~1.4k files total here) at the cost of coarse
/// granularity — ANY submodule checkout/update touching these trees
/// triggers a full rebuild.  That is intentional: the build must always
/// reflect the latest vendored sources, and submodule updates are rare
/// compared to correctness of incremental artifacts.
///
/// Deliberately NOT tracked: `target/` (build outputs), `slpp_all/`
/// (Surelog preprocessor scratch from tests run in the repo cwd),
/// `vendor/libaco` (embedded later in generated models), `docs/`, and `tests/`.
fn emit_rerun_if_changed() {
    // Environment variables read by this script: without these directives a
    // value change alone would NOT re-run the script (Cargo tracks files,
    // not arbitrary env).
    println!("cargo:rerun-if-env-changed=LLG_CCACHE");
    println!("cargo:rerun-if-env-changed=LLG_SYSROOT");

    // Locally compiled sources (the `cc` crate steps below).
    println!("cargo:rerun-if-changed=src/wrapper/surelog_c_api.cpp");
    println!("cargo:rerun-if-changed=src/wrapper/surelog_c_api.h");
    println!("cargo:rerun-if-changed=src/wrapper/mimalloc_shim.c");

    // Patches applied to the vendored submodule.  Per-file directives are
    // also emitted by apply_surelog_patches(); watching the directory
    // itself additionally catches ADDED or REMOVED .patch files (file
    // additions update the directory mtime).
    println!("cargo:rerun-if-changed=patches");

    // Vendored Surelog proper: build logic, its own sources/headers, the
    // ANTLR grammars (parser codegen inputs) and CMake modules.
    println!("cargo:rerun-if-changed=vendor/Surelog/CMakeLists.txt");
    println!("cargo:rerun-if-changed=vendor/Surelog/cmake");
    println!("cargo:rerun-if-changed=vendor/Surelog/src");
    println!("cargo:rerun-if-changed=vendor/Surelog/include");
    println!("cargo:rerun-if-changed=vendor/Surelog/grammar");

    // UHDM (nested third_party of Surelog).  UHDM has no src/ tree: its
    // C++ sources are GENERATED at build time from model/*.yaml through
    // templates/ and the scripts/util/model_gen helpers, so those are the
    // tracked inputs.  tests/images/python are not consumed by the build.
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/CMakeLists.txt");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/cmake");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/include");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/model");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/model_gen");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/scripts");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/templates");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/util");

    // ANTLR4 C++ runtime (static lib + headers included by the wrapper).
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/antlr4/runtime/Cpp/CMakeLists.txt");
    println!(
        "cargo:rerun-if-changed=vendor/Surelog/third_party/antlr4/runtime/Cpp/runtime/CMakeLists.txt"
    );
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/antlr4/runtime/Cpp/runtime/src");
    // ANTLR4 CMake modules (FindANTLR, ExternalAntlr4Cpp) read during
    // Surelog's configure step.
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/antlr4/runtime/Cpp/cmake");

    // Cap'n Proto (built by UHDM; kj/capnp archives linked below).
    println!(
        "cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/third_party/capnproto/c++/CMakeLists.txt"
    );
    println!(
        "cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/third_party/capnproto/c++/src"
    );
    // Package-config templates consumed when UHDM find_package(CapnProto) runs.
    println!(
        "cargo:rerun-if-changed=vendor/Surelog/third_party/UHDM/third_party/capnproto/c++/cmake"
    );

    // nlohmann JSON: only single_include feeds the compile; the rest of
    // that 20MB tree (tests/benchmarks) is deliberately untracked.
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/json/CMakeLists.txt");
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/json/single_include");

    // ANTLR grammar-codegen jars (antlr4_bin).
    println!("cargo:rerun-if-changed=vendor/Surelog/third_party/antlr4_bin");
}

/// Adds the opt-in Slang build inputs to Cargo's exact build-script watch list.
fn emit_slang_rerun_if_changed() {
    println!("cargo:rerun-if-changed=src/wrapper/slang/CMakeLists.txt");
    println!("cargo:rerun-if-changed=src/wrapper/slang_c_api.cpp");
    println!("cargo:rerun-if-changed=src/wrapper/slang_c_api.h");

    // Slang generates headers from scripts at build time. Its public headers,
    // implementation, bundled header dependencies, and CMake modules are all
    // native build inputs; tools, tests, docs, and Python bindings are disabled.
    println!("cargo:rerun-if-changed=vendor/slang/CMakeLists.txt");
    println!("cargo:rerun-if-changed=vendor/slang/cmake");
    println!("cargo:rerun-if-changed=vendor/slang/external");
    println!("cargo:rerun-if-changed=vendor/slang/include");
    println!("cargo:rerun-if-changed=vendor/slang/scripts");
    println!("cargo:rerun-if-changed=vendor/slang/source");
}

/// Returns the ccache executable when the user opted in via LLG_CCACHE
/// and it is present on PATH; None otherwise.  An opt-in without a findable
/// binary emits a cargo warning instead of silently disabling the cache.
fn requested_ccache() -> Option<PathBuf> {
    let v = std::env::var("LLG_CCACHE").unwrap_or_default();
    if v.is_empty() || !matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "true") {
        return None;
    }
    let exe_name = if cfg!(windows) {
        "ccache.exe"
    } else {
        "ccache"
    };
    let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(exe_name))
        .find(|p| p.is_file());
    if found.is_none() {
        println!(
            "cargo:warning=LLG_CCACHE requested `{exe_name}` but it was not \
             found on PATH; continuing without a compiler launcher"
        );
    }
    found
}

/// Keeps the compiler-launcher choice effective across builds.
///
/// The `cmake` crate SKIPS the configure step whenever CMakeCache.txt
/// already exists, so `-D` defines (including *_COMPILER_LAUNCHER) only
/// take effect on the first configure.  We persist the requested state in
/// a marker file next to the build dir and, whenever it changes, delete
/// CMakeCache.txt so the next invocation re-configures with the current
/// defines.  Object files stay valid; only configure re-runs.
fn sync_launcher_state(build_dir: &Path, active: bool, component: &str) {
    let state = if active { "ccache" } else { "none" };
    let marker = build_dir.join(".llg_ccache_state");
    let changed = match std::fs::read_to_string(&marker) {
        Ok(s) => s.trim() != state,
        // An absent marker represents the default launcher-off state.
        Err(_) => active,
    };
    if !changed {
        return;
    }
    let cache = build_dir.join("build").join("CMakeCache.txt");
    if cache.exists() {
        match std::fs::remove_file(&cache) {
            Ok(()) => println!(
                "cargo:warning=LLG_CCACHE setting changed to `{state}`; \
                 forcing {component} CMake reconfigure"
            ),
            Err(e) => panic!(
                "failed to remove {} while switching the compiler launcher: {e}",
                cache.display()
            ),
        }
    }
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "failed to create compiler-launcher state directory {}: {error}",
                parent.display()
            )
        });
    }
    std::fs::write(&marker, format!("{state}\n")).unwrap_or_else(|error| {
        panic!(
            "failed to write compiler-launcher state {}: {error}",
            marker.display()
        )
    });
}

/// Configures and builds Surelog via the `cmake` crate, which integrates
/// cleanly with Cargo's build system.  The CMake build type is derived from
/// the active Cargo profile: `release` profiles map to `Release`, everything
/// else maps to `Debug`.
///
/// When the active Cargo target uses musl (e.g. `x86_64-unknown-linux-musl`)
/// the CMake build is driven through a musl cross-toolchain and Surelog is
/// configured for fully static musl linking.  The C/C++ compiler executables
/// default to `<triple>-gcc` / `<triple>-g++` and can be overridden via the
/// `CC` and `CXX` environment variables.
fn cmake_build_surelog(repo: &Path, build_dir: &Path) {
    let build_type = if std::env::var("PROFILE").as_deref() == Ok("release") {
        "Release"
    } else {
        "Debug"
    };

    // `TARGET` is set by Cargo to the full Rust target triple being compiled.
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_musl = target.contains("musl");
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let mut cfg = cmake::Config::new(repo);
    cfg.out_dir(build_dir)
        .define("CMAKE_BUILD_TYPE", build_type)
        // CMake 4 removed implicit compatibility with policy versions older
        // than 3.5. Vendored Cap'n Proto still declares an older minimum but
        // configures correctly when that compatibility floor is explicit.
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        // Avoid a second allocator in the statically linked frontend.
        .define("SURELOG_WITH_TCMALLOC", "OFF")
        // The Rust binaries consume only static frontend archives. Avoid
        // building unused shared variants and test targets.
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("ANTLR_BUILD_SHARED", "OFF")
        .define("ANTLR_BUILD_STATIC", "ON")
        .define("SURELOG_BUILD_TESTS", "OFF")
        .define("UHDM_BUILD_TESTS", "OFF")
        .define("BUILD_TESTING", "OFF")
        // Surelog's install target expects precompiled package directories
        // that QUICK_COMP omits, so keep its complete install graph enabled.
        .define("QUICK_COMP", "OFF")
        // Ensure headers install under <prefix>/include/antlr4-runtime rather
        // than the bare /antlr4-runtime that results when this is unset.
        .define("CMAKE_INSTALL_INCLUDEDIR", "include")
        .define("CMAKE_INSTALL_PREFIX", "../install");

    if target_env == "msvc" {
        // Keep the complete native dependency graph on the same static MSVC
        // runtime as Rust's +crt-static build. The accompanying Surelog patch
        // permits this command-line cache value to override vendor defaults.
        cfg.define(
            "CMAKE_MSVC_RUNTIME_LIBRARY",
            "MultiThreaded$<$<CONFIG:Debug>:Debug>",
        )
        .define("WITH_STATIC_CRT", "ON")
        // zlib is optional for Surelog's cache files. Disabling it on Windows
        // avoids shipping a third-party DLL or requiring a target-specific
        // vcpkg installation; all core parsing/elaboration remains available.
        .define("SURELOG_WITH_ZLIB", "OFF")
        .define("WITH_ZLIB", "OFF")
        .define("WITH_OPENSSL", "OFF");
    }

    // Silence vendored-third-party compiler chatter with the native spelling.
    if target_env == "msvc" {
        cfg.cxxflag("/w");
    } else {
        cfg.cxxflag("-w");
    }

    // Opt-in compiler cache (LLG_CCACHE=1).
    // The launcher is defined in BOTH states so that switching it off also
    // overrides the cached value (an absent -D would leave the stale
    // CMakeCache entry active).
    let ccache = requested_ccache();
    sync_launcher_state(build_dir, ccache.is_some(), "Surelog");
    let launcher = ccache
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    cfg.define("CMAKE_C_COMPILER_LAUNCHER", &launcher)
        .define("CMAKE_CXX_COMPILER_LAUNCHER", &launcher);

    if is_musl {
        // Derive the GNU triple used by the musl cross-toolchain
        // (e.g. x86_64-unknown-linux-musl → x86_64-linux-musl).
        let musl_triple = target.replace("-unknown-", "-");
        let cc = std::env::var("CC").unwrap_or_else(|_| format!("{musl_triple}-gcc"));
        let cxx = std::env::var("CXX").unwrap_or_else(|_| format!("{musl_triple}-g++"));

        cfg.define("CMAKE_C_COMPILER", cc)
            .define("CMAKE_CXX_COMPILER", cxx)
            .define("SURELOG_USE_MUSL", "ON")
            // zlib is optional for frontend cache compression. Cross-musl
            // toolchains do not consistently include it, so omit that feature
            // instead of accidentally selecting a host/glibc archive.
            .define("SURELOG_WITH_ZLIB", "OFF")
            .define("WITH_ZLIB", "OFF")
            .define("WITH_OPENSSL", "OFF")
            .define("CMAKE_FIND_LIBRARY_SUFFIXES", ".a")
            // crtbeginT.o (used for -static) cannot build a shared library.
            // Disable the ANTLR4 shared-library target; only the static
            // archive (antlr4_static / antlr4-runtime) is needed.
            .define("ANTLR_BUILD_SHARED", "OFF")
            .define("ANTLR_BUILD_STATIC", "ON");
    } else if target_os == "macos" {
        // Use the SDK's zlib. It is a platform library on macOS and remains
        // dynamically linked along with libc++ and system frameworks.
        cfg.define("ZLIB_USE_STATIC_LIBS", "OFF");
    }

    cfg.build();
}

/// Builds and links the experimental Slang C ABI shim when its Cargo feature
/// is enabled. The Slang source remains an ordinary vendored checkout; CMake
/// places all generated files and fetched dependency sources under the
/// target-specific build directory.
fn build_slang_wrapper(manifest_dir: &Path) {
    emit_slang_rerun_if_changed();

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "default".to_string());
    // Slang's debug library is exceptionally large and exports no different C
    // ABI. Keep the native dependency optimized in every Cargo profile; Rust
    // code and the boundary checks retain the active Cargo profile.
    let build_type = "Release";
    let target = std::env::var("TARGET").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_msvc = target_env == "msvc";
    let is_musl = target.contains("musl");

    let project = manifest_dir.join("src/wrapper/slang");
    let repo = manifest_dir.join("vendor/slang");
    apply_vendor_patches(&repo, &manifest_dir.join("patches/slang"));
    let build_dir = manifest_dir
        .join("target/slang")
        .join(&target)
        .join(&profile);

    let mut cfg = cmake::Config::new(project);
    cfg.out_dir(&build_dir)
        .profile(build_type)
        .define("LLG_SLANG_SOURCE_DIR", &repo)
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        // Slang v11 pins fmt 12.1.0. Always build that static dependency so
        // Cargo never accidentally links an incompatible host fmt package.
        .define("FETCHCONTENT_TRY_FIND_PACKAGE_MODE", "NEVER");

    if is_msvc {
        cfg.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded")
            .define("SLANG_WARN_FLAGS", "/w");
    } else {
        cfg.define("SLANG_WARN_FLAGS", "-w");
    }

    let ccache = requested_ccache();
    sync_launcher_state(&build_dir, ccache.is_some(), "Slang");
    let launcher = ccache
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    cfg.define("CMAKE_CXX_COMPILER_LAUNCHER", &launcher);

    if is_musl {
        let musl_triple = target.replace("-unknown-", "-");
        let cxx = std::env::var("CXX").unwrap_or_else(|_| format!("{musl_triple}-g++"));
        cfg.define("CMAKE_CXX_COMPILER", cxx)
            .define("CMAKE_FIND_LIBRARY_SUFFIXES", ".a");
    }

    let install_dir = cfg.build();
    emit_native_search(&install_dir.join("lib"), is_msvc);
    println!("cargo:rustc-link-lib=static=llg_slang_wrapper");
    println!("cargo:rustc-link-lib=static=svlang");
    println!("cargo:rustc-link-lib=static=fmt");

    // Slang's public library enables threads. Keep the platform runtime
    // dependencies explicit because the Rust library can be consumed by bins
    // that do not otherwise link C++.
    match target_os.as_str() {
        "macos" => println!("cargo:rustc-link-lib=dylib=c++"),
        "windows" => {}
        _ if is_musl => {
            let drivers = collect_driver_candidates();
            ensure_static_archives(&drivers, &["stdc++"], &["supc++", "gcc_eh", "gcc"]);
            println!("cargo:rustc-link-lib=static=stdc++");
            println!("cargo:rustc-link-lib=static=supc++");
            println!("cargo:rustc-link-lib=static=gcc_eh");
            println!("cargo:rustc-link-lib=static=gcc");
            println!("cargo:rustc-link-arg=-static");
        }
        _ => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
        }
    }
}

/// Applies every `.patch` file from `patches_dir` to a vendored repository.
/// POSIX/GNU `patch(1)` is preferred; `git apply` is the fallback on hosts such
/// as Windows. Each patch is skipped if it is already applied.
/// Patches are applied in lexicographic order for deterministic sequencing.
///
/// `patch(1)` remains the primary path because container bind mounts can make
/// the nested submodule's relative gitdir unreachable. Normal GitHub checkouts
/// have a valid gitdir and can use the Git-for-Windows fallback. Both paths are
/// checked before they mutate the source tree so a rejected later hunk cannot
/// leave an earlier hunk applied and poison the fallback.
///
/// Flags (all standard in GNU patch ≥ 2.7): `-p1` strips the leading path
/// component of the patch headers, `-R` reverses the patch for applied-
/// detection, `--dry-run` makes that probe side-effect free, `-s` keeps the
/// run silent on success, and `-N` defensively skips hunks that were already
/// applied when the reverse probe could not detect them.
fn apply_vendor_patches(repo: &Path, patches_dir: &Path) {
    if !patches_dir.exists() {
        return;
    }

    let mut patches: Vec<_> = std::fs::read_dir(patches_dir)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read patch directory {}: {error}",
                patches_dir.display()
            )
        })
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("patch"))
        .collect();
    patches.sort();

    for patch in patches {
        println!("cargo:rerun-if-changed={}", patch.display());

        // A successful reverse dry-run means the patch is already applied.
        // The probe is fully captured (.output()) so neither its silence nor
        // a mismatch report leaks into the Cargo build log.
        let reverse_patch = std::process::Command::new("patch")
            .args(["-p1", "-R", "-s", "--dry-run"])
            .stdin(std::process::Stdio::from(
                std::fs::File::open(&patch)
                    .unwrap_or_else(|e| panic!("failed to open patch {}: {e}", patch.display())),
            ))
            .current_dir(repo)
            .output();
        let already_applied = reverse_patch
            .as_ref()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if already_applied {
            continue;
        }

        let forward_patch = std::process::Command::new("patch")
            .args(["-p1", "-s", "-N", "--dry-run"])
            .stdin(std::process::Stdio::from(
                std::fs::File::open(&patch)
                    .unwrap_or_else(|e| panic!("failed to open patch {}: {e}", patch.display())),
            ))
            .current_dir(repo)
            .output();

        if forward_patch
            .as_ref()
            .is_ok_and(|output| output.status.success())
        {
            let patch_status = std::process::Command::new("patch")
                // The dry-run above established that every hunk can be
                // applied, so patch will not create mismatch backup files.
                .args(["-p1", "-s", "-N"])
                .stdin(std::process::Stdio::from(
                    std::fs::File::open(&patch).unwrap_or_else(|e| {
                        panic!("failed to open patch {}: {e}", patch.display())
                    }),
                ))
                .current_dir(repo)
                .status();

            if patch_status.as_ref().is_ok_and(|status| status.success()) {
                continue;
            }

            let patch_error = patch_status
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "patch(1) failed after a successful dry-run".to_string());
            panic!(
                "failed to apply {} with patch(1): {patch_error}",
                patch.display()
            );
        }

        // Git for Windows is installed on GitHub-hosted MSVC runners even
        // when a standalone patch(1) is unavailable. A normal checkout has a
        // valid submodule gitdir, so git-apply provides a portable fallback.
        let git_reverse = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            // Git for Windows may check out the vendored CMake files with
            // CRLF while repository patches are deliberately LF-only.
            .args([
                "apply",
                "--ignore-space-change",
                "--unidiff-zero",
                "--reverse",
                "--check",
            ])
            .arg(&patch)
            .output();
        if git_reverse
            .as_ref()
            .is_ok_and(|output| output.status.success())
        {
            continue;
        }

        let git_check = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "apply",
                "--ignore-space-change",
                "--unidiff-zero",
                "--check",
            ])
            .arg(&patch)
            .output();

        if git_check
            .as_ref()
            .is_ok_and(|output| output.status.success())
        {
            let git_status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["apply", "--ignore-space-change", "--unidiff-zero"])
                .arg(&patch)
                .status();
            if git_status.as_ref().is_ok_and(|status| status.success()) {
                continue;
            }

            let git_error = git_status
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "git apply failed after a successful check".to_string());
            panic!("failed to apply {} with git: {git_error}", patch.display());
        } else {
            let patch_error = forward_patch
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "patch(1) dry-run rejected the patch".to_string());
            let git_error = git_check
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "git apply --check rejected the patch".to_string());
            panic!(
                "failed to apply {} with patch(1) ({patch_error}) or git ({git_error})",
                patch.display()
            );
        }
    }
}

/// Resolves and validates all paths that depend on the Surelog CMake build
/// output, then compiles the C++ wrapper and emits linker directives.
fn build_surelog_wrapper(manifest_dir: &Path) {
    let build_type = std::env::var("PROFILE").unwrap_or_else(|_| "default".to_string());
    let repo = manifest_dir.join("vendor").join("Surelog");
    let target = std::env::var("TARGET").unwrap_or_default();
    let surelog_temp = manifest_dir
        .join("target")
        .join("surelog")
        .join(&target)
        .join(build_type);
    let surelog_build = surelog_temp.join("build");
    let uhdm_gen = surelog_build.join("third_party/UHDM/generated");

    // Rebuild whenever any input to the vendor build or the wrapper archive
    // changes (see emit_rerun_if_changed for the full rationale).
    emit_rerun_if_changed();

    // ── Apply patches to the Surelog submodule ───────────────────────────────
    // Patches are applied before CMake runs so that the patched CMakeLists.txt
    // is in place on a freshly-initialised submodule.  Each patch is idempotent
    // (skipped when already applied) so repeated builds are safe.
    apply_vendor_patches(&repo, &manifest_dir.join("patches"));

    // ── Build Surelog via CMake ───────────────────────────────────────────────
    // Always invoke cmake_build_surelog so that -D overrides (e.g. ZLIB_LIBRARY)
    // are passed on every configure run.  CMake's own incremental build logic
    // makes this a no-op when nothing has changed.
    let sl_config = surelog_build.join("generated/include/Surelog/config.h");
    let uhdm_config = uhdm_gen.join("uhdm/config.h");
    cmake_build_surelog(&repo, &surelog_temp);

    // ── Compile the C++ wrapper ──────────────────────────────────────────────
    let wrapper_dir = manifest_dir.join("src/wrapper");
    let capnp_src = repo.join("third_party/UHDM/third_party/capnproto/c++/src");

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_musl = target_env == "musl";
    // MSVC's cl.exe/link.exe use different option spellings from GNU
    // toolchains (see the branches below).
    let is_msvc = target_env == "msvc";

    let mut wrapper = cc::Build::new();
    wrapper
        .cpp(true)
        .file(wrapper_dir.join("surelog_c_api.cpp"))
        // Wrapper header
        .include(&wrapper_dir)
        // Main include dirs
        .include(surelog_build.join("generated/include"))
        .include(surelog_build.join("generated/src"))
        .include(repo.join("include"))
        // ANTLR runtime
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/atn"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/dfa"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/internal"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/misc"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/support"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/tree"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/tree/pattern"))
        .include(repo.join("third_party/antlr4/runtime/Cpp/runtime/src/tree/xpath"))
        // UHDM generated
        .include(uhdm_gen.join("src"))
        .include(&uhdm_gen)
        // Cap'n Proto
        .include(&capnp_src)
        // nlohmann JSON
        .include(repo.join("third_party/json/single_include"));

    // Force-include config headers (same as CMake build) + language mode.
    if is_msvc {
        // cl.exe spells force-include as /FI<file> (single token) and wants
        // /std:c++17 instead of -std=c++17.
        wrapper
            .flag(format!("/FI{}", sl_config.to_str().unwrap()))
            .flag(format!("/FI{}", uhdm_config.to_str().unwrap()))
            .flag("/std:c++17");
    } else {
        wrapper
            .flag("-include")
            .flag(sl_config.to_str().unwrap())
            .flag("-include")
            .flag(uhdm_config.to_str().unwrap())
            .flag("-std=c++17");
    }

    // Surelog/UHDM's generated listener interface intentionally provides
    // no-op virtual methods whose parameters are unused.  The wrapper must
    // include those headers, so suppress only the corresponding warning for
    // this translation unit rather than mutating generated/vendor sources.
    if is_msvc {
        wrapper.flag("/wd4100");
    } else {
        wrapper.flag("-Wno-unused-parameter");
    }

    // Preprocessor defines: `.define` renders per-toolchain (-D vs /D),
    // unlike raw `-D` flags.
    wrapper
        .define("PLI_DLLESPEC", Some(""))
        .define("PLI_DLLISPEC", Some(""));

    wrapper.compile("surelog_c_wrapper");

    // ── Link pre-built static libraries ─────────────────────────────────────
    // Surelog
    emit_native_search(&surelog_build.join("lib"), is_msvc);
    println!("cargo:rustc-link-lib=static=surelog");

    // UHDM
    emit_native_search(&surelog_build.join("third_party/UHDM/lib"), is_msvc);
    println!("cargo:rustc-link-lib=static=uhdm");

    // ANTLR4 runtime
    emit_native_search(
        &surelog_build.join("third_party/antlr4/runtime/Cpp/runtime"),
        is_msvc,
    );
    let antlr_library = if is_msvc {
        "antlr4-runtime-static"
    } else {
        "antlr4-runtime"
    };
    println!("cargo:rustc-link-lib=static={antlr_library}");

    // Cap'n Proto & kj
    let capnp_build = surelog_build.join("third_party/UHDM/third_party/capnproto/c++/src");
    emit_native_search(&capnp_build.join("capnp"), is_msvc);
    println!("cargo:rustc-link-lib=static=capnp");
    emit_native_search(&capnp_build.join("kj"), is_msvc);
    println!("cargo:rustc-link-lib=static=kj-async");
    println!("cargo:rustc-link-lib=static=kj");

    // ── mimalloc shim: redirect C allocators via --wrap ────────────────────
    // mimalloc_shim.c defines __wrap_malloc / __wrap_free / … which forward
    // to mi_malloc / mi_free.  The --wrap=<sym> linker flags rename every
    // unresolved `malloc` reference in every input archive to `__wrap_malloc`,
    // so all C allocations (Surelog, UHDM, ANTLR, Cap'n Proto) go through
    // mimalloc without touching operator new/delete.
    // operator new in static libstdc++ calls malloc → __wrap_malloc → mi_malloc.
    // Rust allocations that reach malloc use the same wrapped symbols.
    // No MI_MALLOC_OVERRIDE means no operator new/delete conflict with stdc++.
    // The final binary is fully static — portable across all Linux distros.
    //
    // Linker-ordering note: if the shim and libmimalloc.a are separate archives
    // the linker may scan libmimalloc.a before the shim demands mi_malloc,
    // leaving mi_malloc unresolved.  The fix: compile mimalloc's static.c
    // together with mimalloc_shim.c into ONE archive so __wrap_malloc and
    // mi_malloc are co-located and the linker resolves them in a single pass.
    // [target.x86_64-unknown-linux-musl.mimalloc] in .cargo/config.toml
    // suppresses libmimalloc-sys's separate archive to avoid duplicate symbols.
    // This whole archive exists only on musl: non-musl targets (including
    // MSVC) compile neither the shim nor mimalloc here, so their C-side
    // allocations go straight to the CRT/system allocator.
    if is_musl {
        let mi_crate = find_libmimalloc_sys_src(manifest_dir);
        let mi_include = mi_crate.join("c_src/mimalloc/v3/include");
        let mi_src = mi_crate.join("c_src/mimalloc/v3/src");
        let mi_static = mi_src.join("static.c");
        println!("cargo:rerun-if-changed={}", mi_static.display());

        cc::Build::new()
            .file(wrapper_dir.join("mimalloc_shim.c"))
            .file(&mi_static)
            .include(&mi_include)
            .include(&mi_src)
            // No MI_MALLOC_OVERRIDE: avoids operator new/delete conflict with
            // static libstdc++.  C malloc is redirected via --wrap instead.
            .define("MI_DEBUG", Some("0"))
            .flag("-Wno-date-time")
            // Static binary: initial-exec TLS is safe and efficient.
            .flag_if_supported("-ftls-model=initial-exec")
            .compile("mimalloc_with_shim");

        for sym in &[
            "malloc",
            "calloc",
            "realloc",
            "free",
            "aligned_alloc",
            "posix_memalign",
        ] {
            // `--wrap` is a GNU/LLD linker option (this branch is musl-only).
            println!("cargo:rustc-link-arg=-Wl,--wrap={sym}");
        }
    }

    // ── Static C++ runtime ───────────────────────────────────────────────────
    // Link libstdc++, libsupc++ (C++ ABI/RTTI, __dynamic_cast) and libgcc
    // statically so the final binary has no glibc dependency.
    // rustc-link-lib=static is used because -static-libstdc++ / -static-libgcc
    // are GCC driver flags that Cargo's cc linker wrapper may silently ignore.
    if is_musl {
        // rustc must be able to FIND every static archive below: it searches
        // only the `-L` paths emitted via cargo:rustc-link-search and never
        // falls back to the linker's default directories (see
        // ensure_static_archives).
        let drivers = collect_driver_candidates();
        ensure_static_archives(&drivers, &["stdc++"], &["supc++", "gcc_eh", "gcc"]);
        println!("cargo:rustc-link-lib=static=stdc++");
        println!("cargo:rustc-link-lib=static=supc++");
        println!("cargo:rustc-link-lib=static=gcc_eh");
        println!("cargo:rustc-link-lib=static=gcc");
        // musl libc bundles pthreads; optional zlib support is disabled above.
        // Fully static executable — portable to any Linux distro, no runtime
        // .so dependencies.
        println!("cargo:rustc-link-arg=-static");
    } else {
        // Vendored dependencies above remain static. Only target system
        // libraries and runtimes are dynamically linked on macOS/Windows.
        match target_os.as_str() {
            "macos" => {
                // Darwin: libstdc++ is long gone — link libc++ instead;
                // pthread lives in libSystem (no separate -lpthread).
                println!("cargo:rustc-link-lib=dylib=c++");
                println!("cargo:rustc-link-lib=dylib=z");
                println!("cargo:rustc-link-lib=framework=CoreFoundation");
            }
            "windows" => {
                // The CRT is embedded by +crt-static. kj-async uses Winsock;
                // all remaining imports are Windows system DLLs.
                println!("cargo:rustc-link-lib=dylib=ws2_32");
            }
            _ => {
                println!("cargo:rustc-link-lib=dylib=stdc++");
                println!("cargo:rustc-link-lib=dylib=z");
                println!("cargo:rustc-link-lib=dylib=pthread");
            }
        }
    }
}

/// Emits the archive directory and, for multi-config MSVC generators, its
/// configuration-specific child directory.
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

/// Candidate compiler drivers used to locate static archives, in priority
/// order and deduped (first occurrence wins): `$CXX`, `$CC` (when set), the
/// target-derived cross-toolchain pair, then plain `g++`/`gcc`.
fn collect_driver_candidates() -> Vec<String> {
    // Mirror cmake_build_surelog's env handling: $CXX/$CC win, else derive
    // from $TARGET (x86_64-unknown-linux-musl → x86_64-linux-musl-{g,g++}).
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut drivers = Vec::new();
    if let Ok(cxx) = std::env::var("CXX") {
        drivers.push(cxx);
    }
    if let Ok(cc) = std::env::var("CC") {
        drivers.push(cc);
    }
    if !target.is_empty() {
        let triple = target.replace("-unknown-", "-");
        drivers.push(format!("{triple}-g++"));
        drivers.push(format!("{triple}-gcc"));
    }
    drivers.push("g++".to_string());
    drivers.push("gcc".to_string());
    let mut seen = std::collections::BTreeSet::new();
    drivers.retain(|driver| seen.insert(driver.clone()));
    drivers
}

/// Directories probed for an archive when no queried driver resolves it.
fn fallback_search_dirs() -> Vec<PathBuf> {
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut dirs = vec![PathBuf::from("/usr/lib"), PathBuf::from("/usr/local/lib")];
    if !target.is_empty() {
        dirs.push(PathBuf::from(format!(
            "/usr/lib/{}",
            target.replace("-unknown-", "-")
        )));
    }
    if let Ok(sysroot) = std::env::var("LLG_SYSROOT") {
        if !sysroot.is_empty() {
            dirs.push(PathBuf::from(sysroot).join("usr/lib"));
        }
    }
    dirs
}

/// Actionable install hint carried in the failure message of a required
/// archive that could not be located.
fn remedy_hint(archive: &str) -> &'static str {
    match archive {
        "stdc++" => "`apk add g++ libstdc++-dev`",
        _ => "install the package providing this static archive",
    }
}

/// Locates static archives for the musl link step and emits
/// `cargo:rustc-link-search=native=` entries for their directories.
///
/// Why this is needed: the musl branch below asks rustc to link
/// libstdc++/libsupc++/libgcc statically, but rustc resolves every
/// `cargo:rustc-link-lib=static=<name>` by searching ONLY the `-L` paths
/// previously emitted via `cargo:rustc-link-search` — it never falls back to
/// the linker's own default directories.  Two classes of archives therefore
/// need explicit coverage here:
///
/// 1. GCC's own archives do not live on any default library path — they sit
///    in a compiler-PRIVATE, version/triple-specific directory (Alpine 3.20
///    e.g. `/usr/lib/gcc/x86_64-alpine-linux-musl/13.2.1/`).  Querying the
///    driver with `<driver> -print-file-name=<archive>` is the portable way
///    to discover those directories: it works across gcc versions, target
///    triples and Alpine-native-vs-cross toolchain layouts, where hardcoding
///    paths would not.  For cross toolchains the query also doubles as
///    sysroot awareness — the driver answers with archives from ITS OWN
///    sysroot, not the build host's.
/// 2. Some toolchains place runtime archives in ordinary system directories
///    which are linker defaults but not part of Cargo's emitted `-L` set.
///    When every queried driver echoes the bare name back (its way of saying
///    "unknown"), we probe those directories directly.
///
/// The driver prints an absolute path when it knows the archive, and merely
/// echoes the bare name back when it does not — that distinction drives the
/// handling below.  REQUIRED archives must resolve or the build panics;
/// TOLERATED ones may be missing (warning + continue): some toolchains fold
/// libsupc++ into libstdc++, and the unwinder may ship as libgcc_s instead
/// of libgcc_eh.
fn ensure_static_archives(drivers: &[String], required: &[&str], tolerated: &[&str]) {
    // One search entry per unique directory across ALL archives.
    let mut dirs = std::collections::BTreeSet::<PathBuf>::new();
    let fallback_dirs = fallback_search_dirs();
    let fallback_list = fallback_dirs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    for archive in required.iter().chain(tolerated.iter()).copied() {
        let file_name = format!("lib{archive}.a");

        // Ask each driver where its copy of the archive lives; the first
        // absolute-and-existing answer wins.  A driver that cannot be spawned
        // simply does not contribute an answer.
        let mut resolved = None;
        for driver in drivers {
            let Ok(output) = std::process::Command::new(driver)
                .arg(format!("-print-file-name={file_name}"))
                .output()
            else {
                continue;
            };
            let printed = String::from_utf8_lossy(&output.stdout);
            let path = PathBuf::from(printed.trim());
            if path.is_absolute() && path.exists() {
                // Drivers may answer with unnormalized paths containing `..`
                // runs; collapse them so the emitted -L is clean.
                let canonical = path.canonicalize().unwrap_or(path);
                resolved = canonical.parent().map(|p| p.to_path_buf());
                break;
            }
        }

        // Every driver echoed the bare name back (unresolved) → probe the
        // well-known system directories directly.
        if resolved.is_none() {
            resolved = fallback_dirs
                .iter()
                .find(|dir| dir.join(&file_name).exists())
                .cloned();
        }

        match resolved {
            Some(dir) => {
                if dirs.insert(dir.clone()) {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
            }
            None if required.contains(&archive) => panic!(
                "could not locate static archive `{file_name}`, which the \
                 fully-static musl link requires.\n  drivers tried: {}\n  \
                 fallback directories probed: {fallback_list}\n  remedy: {}",
                drivers.join(", "),
                remedy_hint(archive),
            ),
            None => println!(
                "cargo:warning=`{file_name}` was neither resolved by any \
                 queried driver ({}) nor found in the fallback directories \
                 ({fallback_list}); continuing without a search path for it",
                drivers.join(", "),
            ),
        }
    }
}

/// Scan CARGO_HOME for the libmimalloc-sys source directory selected by the
/// current lockfile.  Reading the version from Cargo.lock keeps this build
/// input aligned when the transitive dependency is updated.
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
        .unwrap_or_else(|| {
            panic!(
                "libmimalloc-sys package/version not found in {}",
                lock_path.display()
            )
        });
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .unwrap_or_else(|| panic!("neither CARGO_HOME nor HOME is set"));
    let src_root = cargo_home.join("registry").join("src");
    for entry in std::fs::read_dir(&src_root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", src_root.display()))
    {
        let crate_dir = entry
            .unwrap()
            .path()
            .join(format!("libmimalloc-sys-{version}"));
        if crate_dir.exists() {
            return crate_dir;
        }
    }
    panic!(
        "libmimalloc-sys-{version} not found under {}",
        src_root.display()
    );
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    if std::env::var_os("CARGO_FEATURE_SLANG").is_some() {
        build_slang_wrapper(&manifest_dir);
    }
    build_surelog_wrapper(&manifest_dir);
}
