//! C++ options that build.rs gives the native frontend (Slang, fmt and the
//! wrapper) beyond those of the `cc` crate.
//!
//! The `cmake` crate passes the `cc` crate's compiler arguments as
//! `CMAKE_CXX_FLAGS`, which replaces CMake's defaults for the compiler. For
//! MSVC and clang-cl those defaults are `/DWIN32 /D_WINDOWS /GR /EHsc`, and
//! `cc` adds none of them. Without `/EHsc` neither compiler defines
//! `__cpp_exceptions`, so Slang builds its mode without exceptions
//! (`slang/util/Util.h`): a source nested past its limit then aborts the
//! process in `slang::assert::handleThrow` instead of throwing a
//! `RecursionException` that the wrapper catches (CI: "internal compiler
//! error: 'language constructs are too deeply nested'", exit 0xC0000409), and
//! the wrapper's own exceptions unwind no destructors. GCC and Clang enable
//! exceptions and RTTI by default. `src/wrapper/slang_platform.hpp` stops a
//! build without exceptions on every compiler.

/// The options added to the frontend's `CMAKE_CXX_FLAGS` for Cargo target
/// environment `target_env`: exception handling (`/EHsc`) and RTTI (`/GR`,
/// already the compilers' default) for MSVC and clang-cl, which restores
/// CMake's defaults (Slang defines `WIN32` and `_WINDOWS` itself); nothing
/// elsewhere.
pub fn cxx_flags(target_env: &str) -> &'static [&'static str] {
    match target_env {
        "msvc" => &["/EHsc", "/GR"],
        _ => &[],
    }
}
