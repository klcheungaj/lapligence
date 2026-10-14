//! SIM-022: native initialization, package state and let environments.
//! Oracles come from IEEE 1800-2009 6.8, 6.20.6, 6.21, 8.8, 10.5, 11.13,
//! 13.4.4, 26 and 3.12.1 as quoted in the fixture readme; expected values are
//! hand-derived from those rules, never from the simulator.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_022";

// ── A01: static once, automatic per activation, detached captures ───────────

#[test]
fn static_native_locals_initialize_once_and_automatics_every_entry() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "static_native_locals",
        include_str!("../fixtures/sim/feature_completion/sim_022/static_native_locals.out"),
        &[],
        &[],
    );
}

/// The same model built at the extreme C optimization levels, and generated
/// after the owned database is dropped.
#[test]
fn static_native_locals_survive_model_optimization_and_db_drop() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_022/static_native_locals.out");
    for level in ["O0", "O3"] {
        sim_cli::run_case_with_args(
            SUITE,
            "static_native_locals",
            expected,
            "llg: $finish at time 2000 at tb:100:5\n",
            &[],
            &["--model-opt-level", level],
        );
    }
    let source = format!(
        "{}/tests/fixtures/sim/{SUITE}/static_native_locals.sv",
        env!("CARGO_MANIFEST_DIR")
    );
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "static_native_locals",
        llg::core::compile::CompileOpts {
            files: vec![source],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        "llg: $finish at time 2000 at tb:100:5\n",
    );
}

#[test]
fn initializers_finish_before_processes_and_ignore_first_call_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "init_order",
        include_str!("../fixtures/sim/feature_completion/sim_022/init_order.out"),
        &[],
        &[],
    );
}

#[test]
fn detached_children_keep_automatic_natives_alive_after_return() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fork_capture_lifetime",
        include_str!("../fixtures/sim/feature_completion/sim_022/fork_capture_lifetime.out"),
        &[],
        &[],
    );
}

#[test]
fn static_class_properties_initialize_once_with_any_value() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_static_properties",
        include_str!("../fixtures/sim/feature_completion/sim_022/class_static_properties.out"),
        &[],
        &[],
    );
}

#[test]
fn objects_built_by_static_initializers_seed_from_their_instance_stream() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "instance_object_streams",
        include_str!("../fixtures/sim/feature_completion/sim_022/instance_object_streams.out"),
        &[],
        &[],
    );
}

// ── A02: package and $unit sharing, let environments ───────────────────────

#[test]
fn package_state_is_one_copy_through_import_and_export_chains() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "package_state",
        include_str!("../fixtures/sim/feature_completion/sim_022/package_state.out"),
        &[],
        &[],
    );
}

#[test]
fn merged_compilation_unit_shares_unit_declarations_between_files() {
    sim_cli::run_case_with_source_prefix(
        SUITE,
        "unit_shared_b",
        &["unit_shared_a"],
        include_str!("../fixtures/sim/feature_completion/sim_022/unit_shared_b.out"),
        "llg: $finish at time 1000 at tb:9:5\n",
        &[],
        &["--compilation-units", "merged"],
    );
}

#[test]
fn separate_compilation_units_do_not_see_each_others_declarations() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_source_prefix(
            SUITE,
            "unit_shared_b",
            &["unit_shared_a"],
            optimized,
            &["--compilation-units", "separate"],
        );
        let seen = sim_cli::observed_output(&output);
        assert_eq!(output.status.code(), Some(1), "{seen}");
        assert!(output.stdout.is_empty(), "{seen}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for name in ["unit_text", "unit_queue", "unit_counter"] {
            assert!(
                stderr.contains("unit_shared_b.sv:8:")
                    && stderr.contains(&format!("use of undeclared identifier '{name}'")),
                "{seen}"
            );
        }
    }
}

#[test]
fn separate_compilation_units_keep_same_named_unit_variables_apart() {
    sim_cli::run_case_with_source_prefix(
        SUITE,
        "unit_isolated_b",
        &["unit_shared_a"],
        include_str!("../fixtures/sim/feature_completion/sim_022/unit_isolated_b.out"),
        "llg: $finish at time 1000 at tb:11:5\n",
        &[],
        &["--compilation-units", "separate"],
    );
}

#[test]
fn merged_compilation_unit_rejects_a_unit_name_declared_twice() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_source_prefix(
            SUITE,
            "unit_isolated_b",
            &["unit_shared_a"],
            optimized,
            &["--compilation-units", "merged"],
        );
        let seen = sim_cli::observed_output(&output);
        assert_eq!(output.status.code(), Some(1), "{seen}");
        assert!(output.stdout.is_empty(), "{seen}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("redefinition of `unit_text` in the compilation-unit scope at")
                && stderr.contains("unit_isolated_b.sv:4:8")
                && stderr.contains("unit_shared_a.sv:9:8"),
            "{seen}"
        );
    }
}

#[test]
fn let_names_bind_at_declaration_and_actuals_at_use() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "let_environments",
        include_str!("../fixtures/sim/feature_completion/sim_022/let_environments.out"),
        &[],
        &[],
    );
}

// ── A03: diagnostics that stay located ─────────────────────────────────────

// A static initializer's call reaches a statement that schedules work after
// the call returns: the error names the initializer, the callee, the
// statement and both locations (SV 13.4.4).

#[test]
fn static_initializer_calling_a_forking_function_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_fork",
        "neg_initializer_fork.sv:10:17 calls `watch`, whose fork at ",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_fork",
        "neg_initializer_fork.sv:6:7 schedules work after the call returns; a function may do that only when called from a process (SV 13.4.4)",
    );
}

#[test]
fn static_local_initializer_reaching_a_nonblocking_assignment_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_nba",
        "neg_initializer_nba.sv:14:24 calls `mark`, whose nonblocking assignment at ",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_nba",
        "neg_initializer_nba.sv:7:5 schedules work after the call returns",
    );
}

#[test]
fn handle_initializer_whose_constructor_forks_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_constructor",
        "neg_initializer_constructor.sv:11:21 calls `new`, whose fork at ",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_initializer_constructor",
        "neg_initializer_constructor.sv:7:9 schedules work after the call returns",
    );
}

#[test]
fn the_same_effects_are_legal_from_a_process() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "initializer_effects_from_process",
        include_str!(
            "../fixtures/sim/feature_completion/sim_022/initializer_effects_from_process.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn modifying_methods_on_const_variables_are_rejected_with_their_location() {
    sim_cli::reject_case(
        SUITE,
        "neg_const_queue_method",
        "neg_const_queue_method.sv:6:5: `push_back` modifies `limits`, which is declared const",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_string_method",
        "neg_const_string_method.sv:5:5: `putc` modifies `name`, which is declared const",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_property_method",
        "neg_const_property_method.sv:9:5: `delete` modifies `entries`, which is declared const",
    );
}

#[test]
fn const_reads_and_const_handle_pointees_stay_legal() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "const_reads_and_pointees",
        include_str!("../fixtures/sim/feature_completion/sim_022/const_reads_and_pointees.out"),
        &[],
        &[],
    );
}

#[test]
fn frontend_lifetime_and_const_diagnostics_stay_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_const_handle_write",
        "neg_const_handle_write.sv:7:15 cannot assign to read-only variable 'bag'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_static_reads_automatic",
        "neg_static_reads_automatic.sv:5:27 cannot refer to automatic variable 'text' from static initializer",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_nba_automatic_string",
        "neg_nba_automatic_string.sv:6:10 nonblocking assignment to automatic variable 'text' is not allowed",
    );
}
