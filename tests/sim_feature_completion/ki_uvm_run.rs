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

/// A type reaches the specializations of a generic class through two
/// spellings, as a library's `uvm_typeid #(CB)` and `uvm_typeid #(T)` do;
/// both name one specialization. `generated_c_determinism` generates the same
/// fixture repeatedly: the spelling the frontend keeps varies between runs
/// and must not reorder the classes.
#[test]
fn specialization_spellings() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "specialization_spellings",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/specialization_spellings.out"),
        &[],
        &[],
    );
}

/// `%t` in `$swrite`, `$sformat` and `$sformatf` (literal and run-time
/// formats) reads its value in the calling scope's time unit, as `$display`
/// does: a library's report server stamps every message this way.
#[test]
fn string_format_time_unit() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_format_time_unit",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/string_format_time_unit.out"),
        &[],
        &[],
    );
}

/// A string-returning function with packed output and inout formals, called
/// inside expressions (a library's array-index parser returns the index and
/// sets a wildcard flag this way): each actual receives its formal at the
/// return, for automatic and static functions and class properties.
#[test]
fn string_function_packed_outputs() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_function_packed_outputs",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/string_function_packed_outputs.out"
        ),
        &[],
        &[],
    );
}

/// A virtual interface type first reached through a class type parameter
/// names the same interface type as its own spelling: a library's resource
/// and configuration classes take the virtual interface as `T`.
#[test]
fn virtual_interface_type_parameter() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "virtual_interface_type_parameter",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/virtual_interface_type_parameter.out"
        ),
        &[],
        &[],
    );
}

/// A function returns a virtual interface, and a static method copies one
/// out through an `inout` formal: a library's configuration database reads
/// a stored virtual interface this way.
#[test]
fn virtual_interface_function_return() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "virtual_interface_function_return",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/virtual_interface_function_return.out"
        ),
        &[],
        &[],
    );
}

/// Forward class typedefs in a module only name classes declared later: a
/// register model included into a testbench module declares them.
#[test]
fn forward_class_typedef_in_module() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "forward_class_typedef_in_module",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/forward_class_typedef_in_module.out"
        ),
        &[],
        &[],
    );
}

/// Class handles whose classes declare embedded covergroups print in an
/// assignment pattern without the covergroup instances: a library's pool of
/// monitors prints its entries this way.
#[test]
fn class_pattern_covergroup_member() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_pattern_covergroup_member",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/class_pattern_covergroup_member.out"
        ),
        &[],
        &[],
    );
}

/// Fixed arrays of class handles and strings assigned to dynamic arrays
/// copy element by element: a register model hands a broadcast callback a
/// fixed array of its target fields.
#[test]
fn fixed_array_to_dynamic_handles() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fixed_array_to_dynamic_handles",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/fixed_array_to_dynamic_handles.out"
        ),
        &[],
        &[],
    );
}

/// `$countones`, `$onehot`, `$onehot0` and `$isunknown` in concurrent
/// assertions, from a procedure of an interface and at module level, read
/// their arguments' sampled values: a bus interface checks its grant and
/// address lines this way.
#[test]
fn assertion_bit_vector_functions() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "assertion_bit_vector_functions",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/assertion_bit_vector_functions.out"
        ),
        &[],
        &[],
    );
}

/// Inline constraints naming the randomized object through the receiver's
/// own path (`req.randomize() with { req.size == 4; }`, as a sequence's
/// `uvm_do_with` writes them) constrain its random variables; a path to
/// another object reads that object's values.
#[test]
fn inline_constraint_receiver_path() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "inline_constraint_receiver_path",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/inline_constraint_receiver_path.out"
        ),
        &[],
        &[],
    );
}

/// Interface nets driven by a clocking block and read in the select of a
/// nonblocking assignment's target: a register file indexed by a bus
/// address writes the register file, not the address net.
#[test]
fn net_read_in_target_select() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "net_read_in_target_select",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/net_read_in_target_select.out"),
        &[],
        &[],
    );
}

/// A package defined twice, by a guarded header compiled on its own and
/// included again by the testbench in another compilation unit (a
/// verification library's package file is): every reference names the
/// first definition, and the design holds one package of that name.
#[test]
fn duplicate_package_definition() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "duplicate_package_definition",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/duplicate_package_definition.out"
        ),
        &[concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sim/feature_completion/ki_uvm_run/duplicate_package_definition_pkg.svh"
        )],
        &[],
    );
}

/// `process::self()` in static declaration initializers returns the process
/// running static initialization (S13-D7), as the industry-standard simulator
/// does; a library's configuration database calls `get_randstate` on it.
#[test]
fn process_self_static_initializer() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "process_self_static_initializer",
        include_str!(
            "../fixtures/sim/feature_completion/ki_uvm_run/process_self_static_initializer.out"
        ),
        &[],
        &[],
    );
}

/// `kill()` of the process running static initialization is a run-time
/// error that does nothing (S13-D7, llg limitation).
#[test]
fn process_self_static_kill() {
    sim_cli::run_case_failing_at_runtime(
        SUITE,
        "process_self_static_kill",
        &[],
        "x=1\n",
        "process::kill of the process running static initialization is not supported (llg limitation)",
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

/// A recursive `rand` handle over a hierarchy of 14 classes: every object
/// the chain links is randomized with its own class's constraints. The model
/// is about 2 MB of C, so it builds at `-O0`.
#[test]
fn recursive_rand_handle() {
    sim_cli::run_case_with_args(
        SUITE,
        "recursive_rand_handle",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/recursive_rand_handle.out"),
        "INFO: recursive_rand_handle.sv:115:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--model-opt-level", "O0"],
    );
}

/// A randomize call through a handle whose class has a recursive `rand`
/// handle builds one problem per class the object can have, each with one
/// unit per class the handle can reach. Every unit links the handle once,
/// whatever the child's class: a class test per derived class in every unit
/// made the C cubic in the class count, and 34 classes did not generate in
/// ten minutes. The bound on the generation time is the test's timeout.
#[test]
fn recursive_rand_handle_links_once_per_unit() {
    const CLASSES: usize = 14;
    let directory = sim_harness::TempDir::new("ki-uvm-run-recursive").expect("temporary directory");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/ki_uvm_run/recursive_rand_handle.sv");
    for (out, mode) in [("opt", None), ("no_opt", Some("--no-opt"))] {
        let output = sim_harness::run_command(
            Command::new(env!("CARGO_BIN_EXE_llg"))
                .current_dir(directory.path())
                .args(["--top", "tb", "--gen-only", "--out-dir", out])
                .args(mode)
                .arg(&fixture),
            Duration::from_secs(60),
        )
        .expect("llg should generate within the time bound");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let model = std::fs::read_to_string(directory.path().join(out).join("sim/tb/model.c"))
            .expect("model.c");
        let links = model.matches("llg_cr_link(").count();
        assert!(
            links <= CLASSES * (CLASSES + 1),
            "{out}: {links} link sites for {CLASSES} problems of {} units",
            CLASSES + 1
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
