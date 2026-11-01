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

/// Whether the recorded mode means the archives contain LTO IR.
pub fn is_enabled(mode: &str) -> bool {
    let mode = mode.trim();
    !mode.is_empty() && mode != "off"
}
