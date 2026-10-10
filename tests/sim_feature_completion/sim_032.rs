//! SIM-032: programs, reactive lifecycles and structural program bind.
//! Oracles are derived by hand in the fixture readme from IEEE 1800-2009
//! §§4.4.2, 4.5, 9.2.3, 23.11 and 24.3-24.7. Order-dependent outputs are
//! only asserted where those clauses fix the order; races are checked against
//! their permitted outcomes.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_032";

/// Check an output that ends with final procedures, which execute in an
/// arbitrary order (SV 9.2.3): the leading lines are exact and the trailing
/// lines are exactly `finals`, once each, in any order.
fn assert_exact_then_finals(
    label: &str,
    output: &std::process::Output,
    leading: &str,
    finals: &[&str],
) {
    {
        assert!(output.status.success(), "{label}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let tail = stdout
            .strip_prefix(leading)
            .unwrap_or_else(|| panic!("{label}: unexpected leading output: {stdout}"));
        let mut got: Vec<&str> = tail.lines().collect();
        got.sort_unstable();
        let mut want = finals.to_vec();
        want.sort_unstable();
        assert_eq!(got, want, "{label}: final procedures: {stdout}");
    }
}

// ── A01: Active versus Reactive and the #0/NBA dual regions ─────────────────

#[test]
fn program_regions_follow_the_reference_algorithm() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "dual_regions",
        include_str!("../fixtures/sim/feature_completion/sim_032/dual_regions.out"),
        &[],
        &[],
    );
}

#[test]
fn module_tasks_inherit_the_calling_threads_region_set() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "task_origin",
        include_str!("../fixtures/sim/feature_completion/sim_032/task_origin.out"),
        &[],
        &[],
    );
}

#[test]
fn program_continuous_assignments_run_in_the_reactive_region() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "program_assign",
        include_str!("../fixtures/sim/feature_completion/sim_032/program_assign.out"),
        &[],
        &[],
    );
}

#[test]
fn racing_programs_produce_only_permitted_outcomes() {
    sim_cli::run_case_checked_matrix(SUITE, "program_race", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Both programs run after the Active region wrote `d`; their order in
        // the Reactive region, and so the surviving write, is unspecified.
        let permitted = [
            "p1 d=5\np2 d=5\nfinal shared=2\n",
            "p2 d=5\np1 d=5\nfinal shared=1\n",
        ];
        assert!(permitted.contains(&stdout.as_ref()), "{label}: {stdout}");
    });
}

#[test]
fn programs_admit_generate_ports_packages_classes_and_anonymous_scopes() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "program_generate",
        include_str!("../fixtures/sim/feature_completion/sim_032/program_generate.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "interface_ports",
        include_str!("../fixtures/sim/feature_completion/sim_032/interface_ports.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "program_scopes",
        include_str!("../fixtures/sim/feature_completion/sim_032/program_scopes.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "anonymous_programs",
        include_str!("../fixtures/sim/feature_completion/sim_032/anonymous_programs.out"),
        &[],
        &[],
    );
}

// ── A02: $exit, completion, implicit finish and final procedures ────────────

#[test]
fn exit_cancels_only_its_program_and_its_detached_descendants() {
    let leading = include_str!("../fixtures/sim/feature_completion/sim_032/exit_detached.out");
    sim_cli::run_case_checked_matrix(SUITE, "exit_detached", &[], &|label, output| {
        assert_exact_then_finals(
            label,
            output,
            leading,
            &["pe final", "po final", "tb final t=30"],
        );
    });
}

#[test]
fn exit_from_a_grandchild_ends_the_whole_program() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "exit_descendant",
        include_str!("../fixtures/sim/feature_completion/sim_032/exit_descendant.out"),
        &[],
        &[],
    );
}

#[test]
fn exit_in_a_module_task_follows_the_calling_thread() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "exit_module_task",
        include_str!("../fixtures/sim/feature_completion/sim_032/exit_module_task.out"),
        &[],
        &[],
    );
}

#[test]
fn all_programs_ending_finishes_while_modules_are_live() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "implicit_finish",
        include_str!("../fixtures/sim/feature_completion/sim_032/implicit_finish.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "multi_initial",
        include_str!("../fixtures/sim/feature_completion/sim_032/multi_initial.out"),
        &[],
        &[],
    );
}

// ── A03: structural program bind and illegal program contents ───────────────

#[test]
fn programs_bind_into_module_types_and_instances() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_032/bind_targets.out");
    sim_cli::run_case_backend_parity(SUITE, "bind_targets", expected, &[], &[]);
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("bind_targets.sv");
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "bind_targets.sv",
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        "",
    );
}

#[test]
fn adopted_program_bind_witness() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_program_bind",
        include_str!("../fixtures/sim/feature_completion/sim_032/witness_program_bind.out"),
        &[],
        &[],
    );
}

#[test]
fn illegal_program_contents_reject_at_their_source() {
    sim_cli::reject_case(
        SUITE,
        "neg_always",
        "neg_always.sv:5:3 member not allowed in program declaration",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_always_in_generate",
        "neg_always_in_generate.sv:6:5 member not allowed in program declaration",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_primitive",
        "neg_primitive.sv:5:3 member not allowed in program declaration",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_module_instance",
        "neg_module_instance.sv:7:3 cannot instantiate a module in a program",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_interface_instance",
        "neg_interface_instance.sv:8:3 cannot instantiate an interface in a program",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_nested_program",
        "neg_nested_program.sv:4:11 cannot instantiate a program in a program",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_bind_module_into_program",
        "neg_bind_module_into_program.sv:15:8 cannot instantiate a module in a program",
    );
}

#[test]
fn design_code_cannot_reach_program_items() {
    sim_cli::reject_case(
        SUITE,
        "neg_module_calls_program_task",
        "neg_module_calls_program_task.sv:12:11 cannot reference program item from outside of a program",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_module_reads_program_var",
        "neg_module_reads_program_var.sv:10:29 cannot reference program item from outside of a program",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_anonymous_from_module",
        "neg_anonymous_from_module.sv:10:27 cannot reference program item from outside of a program",
    );
}
