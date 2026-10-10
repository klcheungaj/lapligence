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
        .arg(root.join("vendor/slang/external/ieee1800"));
    if cfg!(target_os = "macos") {
        command.args(["-undefined", "dynamic_lookup"]);
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command.arg(&source).arg("-o").arg(&library);
    let output = sim_harness::run_command(&mut command, std::time::Duration::from_secs(60))
        .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{stem}.c: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(library.to_string_lossy().into_owned())
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
        "a DPI-C library calls svdpi.h routine `svGetScope`, which llg does not provide",
        &["--dpi-lib", &library],
    );
}
