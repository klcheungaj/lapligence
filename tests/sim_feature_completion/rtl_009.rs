use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_009";

#[test]
fn module_interface_generate_and_instance_array_ports() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_009/module_interface_generate.out");
    sim_cli::run_case(SUITE, "module_interface_generate", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "module_interface_generate", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "module_interface_generate", expected);
}

#[test]
fn computed_array_inputs_track_contents_and_selectors() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_009/computed_inputs.out");
    sim_cli::run_case(SUITE, "computed_inputs", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "computed_inputs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "computed_inputs", expected);
}

#[test]
fn fixed_outputs_publish_nested_member_destinations() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_009/nested_output_members.out");
    sim_cli::run_case(SUITE, "nested_output_members", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "nested_output_members", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "nested_output_members", expected);
}

#[test]
fn two_state_cast_before_four_state_formal_stays_visible() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_009/state_conversions.out");
    sim_cli::run_case(SUITE, "state_conversions", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "state_conversions", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "state_conversions", expected);
}

#[test]
fn ref_ports_alias_and_value_inputs_copy() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_009/ref_value_ports.out");
    sim_cli::run_case(SUITE, "ref_value_ports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "ref_value_ports", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "ref_value_ports", expected);
}

#[test]
fn runtime_selected_variable_outputs_are_continuous_assignments() {
    let witness =
        include_str!("../fixtures/sim/feature_completion/rtl_009/output_runtime_select.out");
    sim_cli::run_case(SUITE, "output_runtime_select", witness, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "output_runtime_select", witness, &[], &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_009/runtime_output_targets.out");
    sim_cli::run_case(SUITE, "runtime_output_targets", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "runtime_output_targets", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "runtime_output_targets", expected);
}

#[test]
fn net_array_ports_resolve_inout_cells() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_009/net_array_ports.out");
    sim_cli::run_case(SUITE, "net_array_ports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "net_array_ports", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "net_array_ports", expected);
}

#[test]
fn descriptor_arrays_cross_ports_without_flattening() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected = include_str!("../fixtures/sim/feature_completion/rtl_009/descriptor_ports.out");
    sim_cli::run_case(SUITE, "descriptor_ports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_ports", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_ports", expected);

    // Per-cell port links once emitted code proportional to the 65,537-cell
    // extent (over 100 MB); descriptor copies keep the model bounded.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl009-descriptor-ports").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_009/descriptor_ports.sv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .arg(source)
            .args(["--top", "tb", "--gen-only", "--out-dir"]);
        command.arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .expect("generate descriptor model");
        assert!(output.status.success(), "{output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(
            model.len() < 200_000,
            "unexpected model size: {}",
            model.len()
        );
    }
}

#[test]
fn neg_output_expression() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_expression",
        "expression is not assignable",
    );
}

#[test]
fn neg_chandle_port() {
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_port",
        "'chandle' is not a valid type for a port",
    );
}

#[test]
fn neg_ref_shape() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_shape",
        "connects to 'ref' port of inequivalent type",
    );
}

#[test]
fn neg_variable_inout() {
    sim_cli::reject_case(
        SUITE,
        "neg_variable_inout",
        "cannot be connected to 'inout' port (only nets are allowed)",
    );
}

#[test]
fn neg_runtime_ref() {
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_ref",
        "runtime reference rewiring is not supported",
    );
}

#[test]
fn neg_runtime_net_and_inout_selects() {
    const CONSTANT_SELECT: &str =
        "reference to non-constant variable 'i' is not allowed in a constant expression";
    sim_cli::reject_case(SUITE, "neg_runtime_net_output", CONSTANT_SELECT);
    sim_cli::reject_case(SUITE, "neg_runtime_inout", CONSTANT_SELECT);
}

#[test]
fn neg_output_target_writers() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_procedural_write",
        "has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_output_multiple_drivers",
        "multiple continuous assignments to variable storage `tb.a[1]`",
    );
}
