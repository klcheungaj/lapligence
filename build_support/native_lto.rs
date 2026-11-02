//! Link-time optimization of the native Slang frontend (Slang, fmt and the C++
//! wrapper). `src/wrapper/slang/CMakeLists.txt` compiles the archives with
//! CMake's IPO flags and records which compiler family did; the final link is
//! rustc's, so build.rs adds the link arguments that family's LTO needs.

/// What the CMake project is asked to do (its `LLG_SLANG_LTO` cache value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Ordinary objects.
    Off,
    /// LTO when the compiler supports it, ordinary objects with a warning
    /// otherwise.
    Auto,
    /// LTO or a configure error.
    On,
}

impl Request {
    /// The `LLG_SLANG_LTO` value understood by the CMake project.
    pub fn cmake_value(self) -> &'static str {
        match self {
            Request::Off => "OFF",
            Request::Auto => "AUTO",
            Request::On => "ON",
        }
    }

    /// Suffix of the Slang build directory, so LTO and ordinary archives never
    /// share one tree when the setting changes within a Cargo profile.
    pub fn build_dir_suffix(self) -> &'static str {
        match self {
            Request::Off => "",
            Request::Auto | Request::On => "-lto",
        }
    }
}

/// Resolve `LLG_SLANG_LTO` (unset or `auto`: LTO in release builds only;
/// `1`/`on`/`true`: required in every profile; `0`/`off`/`false`: never).
/// `cargo_profile` is Cargo's `PROFILE` build-script value, `release` for every
/// profile that inherits from release and `debug` otherwise.
pub fn request(setting: &str, cargo_profile: &str) -> Result<Request, String> {
    match setting.trim().to_ascii_lowercase().as_str() {
        "" | "auto" if cargo_profile == "release" => Ok(Request::Auto),
        "" | "auto" => Ok(Request::Off),
        "1" | "on" | "true" => Ok(Request::On),
        "0" | "off" | "false" => Ok(Request::Off),
        _ => Err("LLG_SLANG_LTO must be auto, 0, 1, on, off, true or false".to_owned()),
    }
}

/// Final-link arguments for the compiler family CMake recorded
/// (`CMAKE_CXX_COMPILER_ID`, or `off`) on a target with Cargo's
/// `CARGO_CFG_TARGET_OS`/`CARGO_CFG_TARGET_ENV` values.
///
/// - GNU: slim GIMPLE objects need GCC's linker plugin, which the `cc` driver
///   hands to GNU ld or mold. `-flto=auto` states the parallel partitioned
///   backend on the link line instead of relying on the options recorded in
///   the objects. On Linux GNU targets rustc links through its bundled lld by
///   default, which ignores the plugin and leaves every Slang symbol undefined,
///   so GNU ld is selected; a later `-fuse-ld` (the mold wrapper's) still wins.
/// - MSVC: `/LTCG` tells link.exe up front that `/GL` objects follow, instead
///   of restarting the link when it meets the first one.
/// - Clang and AppleClang: the linker reads ThinLTO bitcode itself (ld64
///   through libLTO, lld natively) and the `cc` driver may be GCC, which
///   rejects `-flto=thin`, so nothing is added.
pub fn link_args(mode: &str, target_os: &str, target_env: &str) -> &'static [&'static str] {
    match mode.trim() {
        "GNU" if target_os == "linux" && target_env == "gnu" => &["-flto=auto", "-fuse-ld=bfd"],
        "GNU" => &["-flto=auto"],
        "MSVC" => &["/LTCG"],
        _ => &[],
    }
}

/// Compile options the CMake project adds to Slang, fmt and the wrapper only
/// when GCC builds them for LTO, for a target with Cargo's
/// `CARGO_CFG_TARGET_ENV` value (passed as `LLG_SLANG_GNU_LTO_OPTIONS`).
///
/// - musl: Alpine's GCC defines `_FORTIFY_SOURCE` whenever it optimizes, and
///   its fortify-headers replace libc functions with `always_inline`
///   `gnu_inline` wrappers of the same name. Under LTO the static libc's
///   definition prevails at link time, so an LTRANS unit that turns an
///   indirect call into a direct one (libstdc++'s `std::to_string` passes
///   `vsnprintf` by pointer) cannot inline the wrapper and the link fails:
///   "inlining failed in call to 'always_inline' 'vsnprintf': function body
///   can be overwritten at link time". Fortification is turned off for the
///   frontend on musl only, which then calls libc without the fortify bounds
///   checks. `-U` removes a definition made before the user options and
///   `=0` also satisfies a driver default that only yields to an explicit
///   `-D_FORTIFY_SOURCE`; fortify-headers and glibc both test `> 0`.
/// - Other targets: glibc's fortify wrappers call `__*_chk` builtins and link
///   under GCC LTO, so nothing is added.
pub fn gnu_lto_compile_options(target_env: &str) -> &'static [&'static str] {
    match target_env {
        "musl" => &["-U_FORTIFY_SOURCE", "-D_FORTIFY_SOURCE=0"],
        _ => &[],
    }
}

/// The static C++ runtime archives of a fully static (musl) link, in the
/// order they must be scanned.
const STATIC_CXX_RUNTIME: [&str; 4] = ["stdc++", "supc++", "gcc_eh", "gcc"];

/// `cargo:rustc-link-lib` values for the static C++ runtime archives in
/// `available`, in `STATIC_CXX_RUNTIME` order.
///
/// They are unbundled for the same reason as the frontend archives and so
/// that they follow them on the link line. rustc places a crate's unbundled
/// static libraries right after its rlib, the `#[link]` ones (wrapper, svlang,
/// fmt) first and the build-script ones after. A bundled runtime would sit
/// inside the rlib, before `-lsvlang`. GNU ld does not return to an archive
/// it has passed (outside `--start-group`), so the libstdc++ members that the
/// frontend's LTRANS objects need would never be extracted, while members
/// pulled from the rlib would keep references that no later archive resolves
/// (CI: `basic_string::_M_mutate`, `system_error` undefined in `eh_alloc.o`,
/// `fs_path.o`). glibc builds link libstdc++ dynamically and never meet this.
pub fn static_cxx_runtime_link_libs(available: impl Fn(&str) -> bool) -> Vec<String> {
    STATIC_CXX_RUNTIME
        .iter()
        .filter(|archive| available(archive))
        .map(|archive| format!("static:-bundle={archive}"))
        .collect()
}

/// Whether the recorded mode means the archives contain LTO IR.
pub fn is_enabled(mode: &str) -> bool {
    let mode = mode.trim();
    !mode.is_empty() && mode != "off"
}
