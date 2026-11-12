//! Forms a verification library's run reaches once its classes generate:
//! automatic string, class handle, chandle and process locals a subroutine
//! never names again, and randomization through handles of a class with many
//! derived classes. Clauses and oracles are in the fixture readme; the
//! generated-C sweep (`generated_c_frame_lint`) compiles the same fixtures
//! with `-Wall` and accepts no warning.
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_uvm_run";

#[test]
fn unused_locals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unused_locals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/unused_locals.out"),
        &[],
        &[],
    );
}

/// Randomize through base-class handles over 40 derived classes. The model
/// is about 2 MB of C; the dispatch shape, not C optimization, is under test,
/// so the model builds at `-O0`.
#[test]
fn wide_hierarchy_randomize() {
    sim_cli::run_case_with_args(
        SUITE,
        "wide_hierarchy_randomize",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/wide_hierarchy_randomize.out"),
        "INFO: wide_hierarchy_randomize.sv:256:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--model-opt-level", "O0"],
    );
}

/// The randomize walks dispatch on the object's class once per derived
/// class. One C block per class (an `if`/`else if` chain) would nest deeper
/// than the class count; clang stops at 256 brackets and MSVC at 128 blocks,
/// which a library class with hundreds of derived classes exceeds.
#[test]
fn wide_hierarchy_randomize_nests_below_the_class_count() {
    const CLASSES: usize = 40;
    let directory = sim_harness::TempDir::new("ki-uvm-run-nesting").expect("temporary directory");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/ki_uvm_run/wide_hierarchy_randomize.sv");
    for (out, mode) in [("opt", None), ("no_opt", Some("--no-opt"))] {
        let output = sim_harness::run_command(
            Command::new(env!("CARGO_BIN_EXE_llg"))
                .current_dir(directory.path())
                .args(["--top", "tb", "--gen-only", "--out-dir", out])
                .args(mode)
                .arg(&fixture),
            Duration::from_secs(120),
        )
        .expect("llg should start");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let model = std::fs::read_to_string(directory.path().join(out).join("sim/tb/model.c"))
            .expect("model.c");
        let depth = max_block_depth(&model);
        assert!(
            depth < CLASSES,
            "{out}: generated C nests {depth} blocks for {CLASSES} derived classes"
        );
    }
}

/// The deepest `{` nesting of C source, outside string and character
/// literals.
fn max_block_depth(source: &str) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    let mut quote = None;
    let mut escaped = false;
    for byte in source.bytes() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                quote = None;
            }
            continue;
        }
        match byte {
            b'"' | b'\'' => quote = Some(byte),
            b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}
