//! SIM-040: DPI-C packed, sized and open-array imports. Every positive
//! fixture has a checked-in C companion that is built here into a shared
//! library (a DLL on Windows) and loaded with `--dpi-lib`; the expected
//! outputs are derived by hand in the fixture readme from IEEE 1800-2009
//! 35.5-35.6 and Annex H.
use super::{c_compiler, sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/sim_040";

/// Build `<stem>.c` from the fixture directory into a shared library in
/// `directory` (see [`shared_companion`]).
fn companion(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    companion_with_flags(directory, stem, &[])
}

/// [`companion`] with extra compiler `flags` after `$LLG_CFLAGS`.
fn companion_with_flags(
    directory: &sim_harness::TempDir,
    stem: &str,
    flags: &[&str],
) -> Option<String> {
    shared_companion(SUITE, directory, stem, flags)
}

/// True on a Windows host whose C compiler is not MSVC: DLL companions are
/// built only with MSVC there (the release matrix's Windows toolchain).
pub(super) fn windows_without_msvc() -> bool {
    if cfg!(windows) && !c_compiler::is_msvc(&c_compiler::host_c_compiler()) {
        eprintln!("SKIP: Windows DLL companions are built with MSVC only");
        return true;
    }
    false
}

/// Build `tests/fixtures/sim/<suite>/<stem>.c` into a shared DPI-C library
/// for `--dpi-lib` with the model's C compiler, strict C11 warnings as
/// errors and `$LLG_CFLAGS` (so sanitizer runs instrument the companion
/// too); shared with later DPI suites. On Windows the DLL links against the
/// import library of `<suite>/<stem>.sv` and the returned path is the DLL's
/// import library. `None` only where [`windows_without_msvc`] holds.
pub(super) fn shared_companion(
    suite: &str,
    directory: &sim_harness::TempDir,
    stem: &str,
    flags: &[&str],
) -> Option<String> {
    let library = try_shared_companion(suite, directory, stem, stem, flags)?
        .unwrap_or_else(|error| panic!("{stem}.c: {error}"));
    Some(library.link.to_string_lossy().into_owned())
}

/// Build `<suite>/<stem>.c` as [`shared_companion`] does, for the design
/// `<suite>/<design>.sv`, whose DPI-C exports a Windows DLL may call. `Err`
/// is the failed build's output: a DLL resolves every symbol when it is
/// linked, so on Windows a call of an unprovided routine or an undeclared
/// export fails here rather than at the model link.
pub(super) fn try_shared_companion(
    suite: &str,
    directory: &sim_harness::TempDir,
    stem: &str,
    design: &str,
    flags: &[&str],
) -> Option<Result<c_compiler::SharedLibrary, String>> {
    if windows_without_msvc() {
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join(format!("tests/fixtures/sim/{suite}"));
    let import = cfg!(windows).then(|| {
        sim_cli::model_import_library(
            &fixtures.join(format!("{design}.sv")),
            &[],
            directory.path(),
        )
    });
    Some(c_compiler::shared_library(
        &fixtures.join(format!("{stem}.c")),
        directory.path(),
        stem,
        flags,
        import.as_deref(),
    ))
}

/// Build the VPI application `<suite>/<stem>.c` into a run-time plugin and
/// return the path for `LLG_VPI_PLUGIN`. On Windows the DLL links against
/// the import library of a model with no DPI-C exports: every model exports
/// the same VPI routines, so any design serves. `None` only where
/// [`windows_without_msvc`] holds.
pub(super) fn plugin_library(
    suite: &str,
    directory: &sim_harness::TempDir,
    stem: &str,
    flags: &[&str],
) -> Option<String> {
    if windows_without_msvc() {
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let import = cfg!(windows).then(|| {
        let design = directory.path().join("vpi_import_design.sv");
        std::fs::write(&design, "module tb;\nendmodule\n").expect("write the import design");
        sim_cli::model_import_library(&design, &[], directory.path())
    });
    let library = c_compiler::shared_library(
        &root.join(format!("tests/fixtures/sim/{suite}/{stem}.c")),
        directory.path(),
        stem,
        flags,
        import.as_deref(),
    )
    .unwrap_or_else(|error| panic!("{stem}.c: {error}"));
    Some(library.module.to_string_lossy().into_owned())
}

/// The model C flags for a `-funsigned-char` lane: `$LLG_CFLAGS` plus the
/// host compiler's spelling of the option (MSVC `/J`).
pub(super) fn unsigned_char_cflags() -> String {
    let flag = c_compiler::native_flag(
        "-funsigned-char",
        c_compiler::is_msvc(&c_compiler::host_c_compiler()),
    );
    std::env::var("LLG_CFLAGS").map_or_else(|_| flag.to_owned(), |flags| format!("{flags} {flag}"))
}

/// The `svdpi.h` written beside every model is self-contained strict C11: a
/// DPI-C file that includes nothing before it compiles. `-U__linux` stands in
/// for a host outside the Annex I sample's Linux and Windows branches, whose
/// `<sys/types.h>` fallback lacks `uint8_t`/`uint32_t` (macOS).
#[test]
fn svdpi_header_is_self_contained_strict_c11() {
    let compiler = c_compiler::host_c_compiler();
    if c_compiler::is_msvc(&compiler) {
        eprintln!("SKIP: the strict svdpi.h check uses GCC/Clang options");
        return;
    }
    let directory = sim_harness::TempDir::new("sim040-svdpi").expect("temporary directory");
    std::fs::write(
        directory.path().join("svdpi.h"),
        llg::sim::rt::svdpi_header(),
    )
    .expect("write svdpi.h");
    let source = directory.path().join("only_svdpi.c");
    std::fs::write(
        &source,
        "#include \"svdpi.h\"\n\
         uint64_t only_svdpi_wide(const svBitVecVal *v) { return (uint64_t)v[0]; }\n\
         svLogic only_svdpi_scalar(const svLogicVecVal *v) { return (svLogic)(v->aval & 1u); }\n\
         int32_t only_svdpi_time(const svTimeVal *t) { return t->type; }\n",
    )
    .expect("write probe");
    for extra in [None, Some("-U__linux")] {
        let mut command = std::process::Command::new(&compiler);
        command
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-pedantic",
                "-Werror",
                "-fsyntax-only",
            ])
            .args(extra)
            .arg("-I")
            .arg(directory.path())
            .arg(&source);
        let output = sim_harness::run_command(&mut command, std::time::Duration::from_secs(60))
            .expect("C compiler for the svdpi.h check");
        assert!(
            output.status.success(),
            "svdpi.h {extra:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn packed_widths_cross_chunk_boundaries() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "packed_widths") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "packed_widths",
        include_str!("../fixtures/sim/feature_completion/sim_040/packed_widths.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn open_arrays_take_each_actual_shape() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "open_arrays") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "open_arrays",
        include_str!("../fixtures/sim/feature_completion/sim_040/open_arrays.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn sized_aggregates_and_strings_use_the_c_layout() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "aggregates") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "aggregates",
        include_str!("../fixtures/sim/feature_completion/sim_040/aggregates.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// SV `byte` is C `char` (Table H.1), whose signedness is the ABI's: plain
/// `char` is unsigned on AArch64 Linux, PowerPC and RISC-V. `-funsigned-char`
/// (GCC and Clang; MSVC `/J`) reproduces that ABI here for the companion and
/// the model, which must print the same signed values.
#[test]
fn byte_members_keep_their_sign_when_char_is_unsigned() {
    let directory = sim_harness::TempDir::new("sim040-uchar").expect("temporary directory");
    let Some(library) = companion_with_flags(&directory, "aggregates", &["-funsigned-char"]) else {
        return;
    };
    let cflags = unsigned_char_cflags();
    let output = sim_cli::invoke_with_env(
        SUITE,
        "aggregates",
        true,
        &["--dpi-lib", &library],
        &[("LLG_CFLAGS", &cflags)],
        &[],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        include_str!("../fixtures/sim/feature_completion/sim_040/aggregates.out")
    );
}

#[test]
fn round_trips_keep_unknowns_and_copy_out_order() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "roundtrip_xz") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "roundtrip_xz",
        include_str!("../fixtures/sim/feature_completion/sim_040/roundtrip_xz.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn svdpi_routines_handle_valid_boundary_and_invalid_arguments() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "svdpi_access") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "svdpi_access",
        include_str!("../fixtures/sim/feature_completion/sim_040/svdpi_access.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// A wrong number of element indices ends the run when the import returns;
/// the final procedure runs (S40-D6).
#[test]
fn a_wrong_element_index_count_ends_the_run() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "svdpi_index_count") else {
        return;
    };
    let expected = include_str!("../fixtures/sim/feature_completion/sim_040/svdpi_index_count.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "svdpi_index_count",
        &["--dpi-lib", &library],
        &|label, output| {
            assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
            assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                stderr
                    .matches("llg: DPI-C open array element access")
                    .count(),
                1,
                "{label}: {stderr}"
            );
        },
    );
}

#[test]
fn one_c_function_imported_from_several_scopes() {
    let directory = sim_harness::TempDir::new("sim040").expect("temporary directory");
    let Some(library) = companion(&directory, "scopes_types") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes_types",
        include_str!("../fixtures/sim/feature_completion/sim_040/scopes_types.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn unsupported_actuals_and_formal_types_are_explicit_errors() {
    sim_cli::reject_case(SUITE, "neg_dynamic_actual", "a dynamic array, queue or associative array actual of DPI-C open-array formal `a` is not supported (SIM-040)");
    sim_cli::reject_case(SUITE, "neg_string_array", "DPI-C import `neg_strings` formal 0 (`a`): unpacked aggregates with real, shortreal, string or chandle elements are not supported (SIM-040)");
    sim_cli::reject_case(SUITE, "neg_real_open", "DPI-C import `neg_reals` formal 0 (`a`): unpacked aggregates with real, shortreal, string or chandle elements are not supported (SIM-040)");
    sim_cli::reject_case(SUITE, "neg_wide_aggregate", "DPI-C import `neg_big` formal 0 (`a`): an unpacked aggregate wider than 1048575 payload bits is not supported (SIM-040)");
}

#[test]
fn illegal_signatures_are_frontend_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_packed_result",
        "is not a valid return type for a DPI subroutine",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ref_formal",
        "DPI subroutines cannot have 'ref' arguments",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_event_formal",
        "'event' is not a valid argument type in a DPI subroutine",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_class_formal",
        "'item' is not a valid argument type in a DPI subroutine",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_export_open",
        "is not a valid argument type in a DPI subroutine",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_union",
        "'u_t' is not a valid argument type in a DPI subroutine",
    );
    sim_cli::reject_case(SUITE, "neg_conflicting_signature", "more than one DPI subroutine with C identifier 'neg_twice' declared with mismatching type signatures");
}

#[test]
fn missing_import_symbol_is_named() {
    let directory = sim_harness::TempDir::new("sim040-missing").expect("temporary directory");
    let Some(library) = companion(&directory, "neg_missing_symbol") else {
        return;
    };
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_missing_symbol",
        "DPI-C import symbol `neg_absent` is not defined by any --dpi-lib library",
        &["--dpi-lib", &library],
    );
}

/// A library calling an `svdpi.h` routine llg does not provide is named:
/// by llg at the model link (ELF), before the build (Mach-O, below), or by
/// the DLL's own link on Windows, where the model's import library lacks it.
#[test]
fn unprovided_svdpi_routine_is_named() {
    let directory = sim_harness::TempDir::new("sim040-routine").expect("temporary directory");
    let stem = "neg_unprovided_routine";
    let Some(built) = try_shared_companion(SUITE, &directory, stem, stem, &[]) else {
        return;
    };
    if cfg!(windows) {
        let error = built.expect_err("the DLL link must not resolve svGetTime");
        assert!(error.contains("svGetTime"), "{error}");
        return;
    }
    let library = built
        .unwrap_or_else(|error| panic!("{stem}.c: {error}"))
        .link
        .to_string_lossy()
        .into_owned();
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_unprovided_routine",
        "a DPI-C library calls svdpi.h routine `svGetTime`, which llg does not provide",
        &["--dpi-lib", &library],
    );
}

/// A 64-bit Mach-O dylib whose symbol table imports `names` through
/// `-undefined dynamic_lookup` (library ordinal 0xfe), as the macOS build of
/// `neg_unprovided_routine.c` does. Written by hand so every host can check it.
fn dynamic_lookup_dylib(names: &[&str]) -> Vec<u8> {
    let mut strings = vec![0u8];
    let mut table = Vec::new();
    for name in names {
        table.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        table.extend_from_slice(&[0x01, 0]); // N_UNDF | N_EXT, no section
        table.extend_from_slice(&(0xfeu16 << 8).to_le_bytes());
        table.extend_from_slice(&0u64.to_le_bytes());
        strings.extend_from_slice(format!("_{name}\0").as_bytes());
    }
    let symbols_at = 32 + 24;
    let strings_at = symbols_at + table.len();
    let mut image = Vec::new();
    // mach_header_64 (arm64 MH_DYLIB, one load command), then LC_SYMTAB.
    for word in [0xfeed_facfu32, 0x0100_000c, 0, 6, 1, 24, 0x85, 0] {
        image.extend_from_slice(&word.to_le_bytes());
    }
    for word in [
        0x2u32,
        24,
        symbols_at as u32,
        names.len() as u32,
        strings_at as u32,
        strings.len() as u32,
    ] {
        image.extend_from_slice(&word.to_le_bytes());
    }
    image.extend_from_slice(&table);
    image.extend_from_slice(&strings);
    image
}

/// ld64 links a model against a dylib without checking the dylib's undefined
/// symbols, so on macOS the unprovided routine reached dyld and the model
/// ended by a signal. llg reads a Mach-O library's
/// imports before the build and names the routine as the ELF link does; the
/// check depends only on the file, so it runs on every host.
#[test]
fn unprovided_routine_imported_by_a_mach_o_library_is_named_before_the_build() {
    let directory = sim_harness::TempDir::new("sim040-macho").expect("temporary directory");
    let library = directory.path().join("libneg_unprovided_routine.dylib");
    std::fs::write(
        &library,
        dynamic_lookup_dylib(&["svGetTime", "svGetArrayPtr", "neg_helper"]),
    )
    .expect("write Mach-O library");
    let library = library.to_string_lossy().into_owned();
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_unprovided_routine",
        "a DPI-C library calls svdpi.h routine `svGetTime`, which llg does not provide",
        &["--dpi-lib", &library],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_unprovided_routine",
        "imports svGetTime for dyld to resolve when the model runs",
        &["--dpi-lib", &library],
    );
}

/// `--dpi-import-lib` writes the import library a Windows DLL links against
/// to reach `sim.exe` (an `ar` archive naming the executable). Other
/// toolchains have no use for one and get an explicit error instead.
#[test]
fn model_import_library_is_explicit_about_the_toolchain() {
    let directory = sim_harness::TempDir::new("sim040-import").expect("temporary directory");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/{SUITE}/svdpi_access.sv"));
    let library = directory.path().join("model.lib");
    let output = sim_harness::run_command(
        std::process::Command::new(env!("CARGO_BIN_EXE_llg"))
            .current_dir(directory.path())
            .args(["--top", "tb", "--dpi-import-lib"])
            .arg(&library)
            .arg(&source),
        std::time::Duration::from_secs(300),
    )
    .expect("llg should start");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if c_compiler::is_msvc(&c_compiler::host_c_compiler()) {
        assert!(output.status.success(), "{stderr}");
        let bytes = std::fs::read(&library).expect("import library");
        assert!(bytes.starts_with(b"!<arch>\n"), "not an archive");
        let contains = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(contains(b"sim.exe"), "the import library must name sim.exe");
        assert!(contains(b"svGetArrElemPtr1"), "missing svdpi.h routine");
        assert!(contains(b"vpi_register_systf"), "missing VPI routine");
        assert!(!contains(b"svGetTime"), "unprovided routine exported");
    } else {
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.contains(
                "an import library for the model executable is built only by MSVC toolchains"
            ),
            "{stderr}"
        );
        assert!(!library.exists());
    }
    // The run stops after the import library: the model is never linked.
    let built = directory.path().join("build/sim/tb/build/bin");
    assert!(
        !built
            .join(format!("sim{}", std::env::consts::EXE_SUFFIX))
            .exists(),
        "the model was linked"
    );
}

/// C function definitions in `source` that do not carry `DPI_DLLESPEC`: a
/// signature starting in column 0 (not `static`) followed by a body.
fn unexported_definitions(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut missing = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let starts_declaration = line.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && line.contains('(')
            && !["static", "typedef", "struct", "enum", "union", "extern"]
                .iter()
                .any(|keyword| line.starts_with(keyword));
        if !starts_declaration {
            continue;
        }
        // A function body, not an initializer (`= {`) or a prototype.
        let definition = lines[index..]
            .iter()
            .flat_map(|line| line.chars())
            .find(|c| matches!(c, '{' | ';' | '='))
            == Some('{');
        if definition && !line.contains("DPI_DLLESPEC") {
            missing.push(format!("{}: {line}", index + 1));
        }
    }
    missing
}

/// A Windows DLL exports only what is marked: an unmarked import definition
/// leaves the model's link without the symbol, and a DLL that exports
/// nothing gets no import library for `--dpi-lib` (CI round 6: sim_039's
/// `endpoints_dpi_dll.lib` "is not a regular file"). Every `svdpi.h`
/// companion that the DLL helpers build marks each import definition
/// `DPI_DLLESPEC`, which is empty on ELF and Mach-O hosts. `sim/dpi` is the
/// Unix-only `sim_dpi` suite, which never builds a DLL.
#[test]
fn dll_companions_export_every_import_definition() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let mut pending = vec![root.clone()];
    let mut checked = 0;
    let mut missing = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read fixture directory") {
            let path = entry.expect("fixture entry").path();
            if path.is_dir() {
                if path != root.join("dpi") {
                    pending.push(path);
                }
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("c") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read companion");
            if !source.contains("#include \"svdpi.h\"") {
                continue;
            }
            checked += 1;
            missing.extend(
                unexported_definitions(&source)
                    .into_iter()
                    .map(|line| format!("{}:{line}", path.display())),
            );
        }
    }
    assert!(checked > 20, "found only {checked} svdpi.h companions");
    assert!(
        missing.is_empty(),
        "import definitions without DPI_DLLESPEC:\n{}",
        missing.join("\n")
    );
    assert_eq!(
        unexported_definitions(
            "int f(int a,\n      int b)\n{\n    return a;\n}\nint g(void);\nvoid (*t[])(void) = {0};\n"
        ),
        ["1: int f(int a,"]
    );
}
