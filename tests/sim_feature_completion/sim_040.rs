//! SIM-040: DPI-C packed, sized and open-array imports. Every positive
//! fixture has a checked-in C companion that is built here into a shared
//! library and loaded with `--dpi-lib`; the expected outputs are derived by
//! hand in the fixture readme from IEEE 1800-2009 35.5-35.6 and Annex H.
use super::{c_compiler, sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/sim_040";

/// Build `<stem>.c` from the fixture directory into a shared library in
/// `directory` with the model's C compiler, strict C11 warnings as errors and
/// `$LLG_CFLAGS` (so sanitizer runs instrument the companion too). `None`
/// on hosts where shared DPI fixtures are not built (Windows DLLs cannot
/// import the svdpi.h routines from the model executable).
fn companion(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    companion_with_flags(directory, stem, &[])
}

/// [`companion`] with extra compiler `flags` after `$LLG_CFLAGS`.
fn companion_with_flags(
    directory: &sim_harness::TempDir,
    stem: &str,
    flags: &[&str],
) -> Option<String> {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/{SUITE}/{stem}.c"));
    let library = directory.path().join(format!("lib{stem}.so"));
    let mut command = std::process::Command::new(c_compiler::host_c_compiler());
    command
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
        ])
        .arg("-I")
        .arg(root.join("src/sim/rt"));
    if cfg!(target_os = "macos") {
        command.args(["-undefined", "dynamic_lookup"]);
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command.args(flags).arg(&source).arg("-o").arg(&library);
    let output = sim_harness::run_command(&mut command, std::time::Duration::from_secs(60))
        .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{stem}.c: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(library.to_string_lossy().into_owned())
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
/// (GCC and Clang, the Unix hosts that build these companions) reproduces
/// that ABI here for the companion and the model, which must print the same
/// signed values.
#[test]
fn byte_members_keep_their_sign_when_char_is_unsigned() {
    let directory = sim_harness::TempDir::new("sim040-uchar").expect("temporary directory");
    let Some(library) = companion_with_flags(&directory, "aggregates", &["-funsigned-char"]) else {
        return;
    };
    let cflags = std::env::var("LLG_CFLAGS").map_or_else(
        |_| "-funsigned-char".to_owned(),
        |flags| format!("{flags} -funsigned-char"),
    );
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

#[test]
fn unprovided_svdpi_routine_is_named() {
    let directory = sim_harness::TempDir::new("sim040-routine").expect("temporary directory");
    let Some(library) = companion(&directory, "neg_unprovided_routine") else {
        return;
    };
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
/// ended by a signal with nothing on stderr. llg reads a Mach-O library's
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
