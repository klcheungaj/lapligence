use super::run_case;

#[test]
fn port_defaults_and_generate_actuals_use_their_elaborated_scope() {
    run_case("port_parameter_scopes", "55 aa 04 05\n");
}

#[test]
fn reference_aliases_preserve_initialization_and_edge_visibility() {
    run_case("reference_initialization", "initial a\nedge 1 b b\n");
}

#[test]
fn reference_ports_share_storage_immediately_through_nested_instances() {
    run_case("reference_ports", "parent 10000000000000001 10000000000000001\nchild 1fedcba9876543210 1fedcba9876543210\nupdated 1fedcba987654321f 1fedcba987654321f\n");
}

#[test]
fn constant_and_default_input_ports() {
    run_case("port_constants", "5a 93 xx ff 10xz01zx\n");
}

#[test]
fn input_port_expressions_follow_all_operands_and_selects() {
    run_case("port_expressions", "b7 23 01\nc3 24 01\n55 44 00\n");
}

#[test]
fn input_port_assignment_width_signedness_and_state_conversion() {
    run_case("port_conversion", "fffe 10001000 100\nff80 00000000 100\n");
}

#[test]
fn output_port_selected_targets_preserve_other_bits() {
    run_case("port_output_selects", "az5\n3zc\n");
}
