//! Remaining practical RTL contexts, tested through both public CLI modes.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn defparam_values_match_parameter_overrides() {
    sim_cli::run_case(
        "rtl_completion",
        "defparam_value",
        "n=f def=ff param=ff DW=8 PW=8 DB=8 PB=8\n",
        "",
        &[],
    );
}

#[test]
fn streaming_with_uses_preceding_unpacked_values() {
    sim_cli::run_case(
        "rtl_completion",
        "stream_sequential_selector",
        "count=2 lanes=11,22\n",
        "",
        &[],
    );
}

#[test]
fn unit_declarations_must_precede_variable_references() {
    sim_cli::reject_case(
        "rtl_completion",
        "unit_forward_reference",
        "compilation-unit forward reference",
    );
    sim_cli::run_case(
        "rtl_completion",
        "unit_prior_declaration",
        "early=7\n",
        "",
        &[],
    );
}

#[test]
fn interface_wired_nets_preserve_instance_identity() {
    sim_cli::run_case(
        "rtl_completion",
        "interface_wired_net",
        "value=1\n",
        "",
        &[],
    );
    sim_cli::run_case(
        "rtl_completion",
        "interface_wired_instances",
        "a=00 b=aa o=ff\na=f0 b=aa o=f0\n",
        "",
        &[],
    );
}

#[test]
fn fixed_values_have_independent_locals_returns_and_copyout() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_value_calls",
        "source=1,2,3,4\nresult=2,3,4,5 changed=15\n",
        "",
        &[],
    );
}

#[test]
fn fixed_references_alias_each_caller_element_through_recursion() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_references",
        "value=1,5,3,4\n",
        "",
        &[],
    );
}

#[test]
fn fixed_structs_preserve_value_copy_and_reference_semantics() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_struct_calls",
        "source=2,9,2,3\nresult=f,1,5,3\n",
        "",
        &[],
    );
}

#[test]
fn fixed_arrays_of_structs_preserve_member_paths_and_formal_shapes() {
    sim_cli::run_case(
        "rtl_completion",
        "struct_array_values",
        "sum=16 data=5a,a5\n",
        "",
        &[],
    );
}

#[test]
fn fixed_defaults_keep_each_leafs_state_domain() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_value_defaults",
        "fixed defaults passed\n",
        "",
        &[],
    );
}

#[test]
fn nested_fixed_views_preserve_aliases_bounds_and_state() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_nested_views",
        "word=125b value=9 states=0,z calls=1\n",
        "",
        &[],
    );
}

#[test]
fn fixed_block_locals_observe_automatic_and_static_lifetimes() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_block_locals",
        "local=26 saved=11\nlocal=26 saved=12\n",
        "",
        &[],
    );
}

#[test]
fn fixed_conversions_apply_to_each_typed_leaf() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_conversions",
        "wide=81,f2\n",
        "",
        &[],
    );
}

#[test]
fn fixed_unions_preserve_common_initial_sequences_across_calls() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_unions",
        "source=7,5a result=9,a5,11\n",
        "",
        &[],
    );
}

#[test]
fn fixed_member_defaults_apply_to_every_storage_lifetime() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_member_defaults",
        "member defaults passed\n",
        "",
        &[],
    );
}

#[test]
fn fixed_members_and_whole_call_inputs_keep_sensitivity_dependencies() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_sensitivity",
        "observed=34 combined=54\nobserved=56 combined=88\nobserved=56 combined=95\n",
        "",
        &[],
    );
}

#[test]
fn inout_arrays_nested_peers_and_selected_ports_share_resolution() {
    sim_cli::run_case(
        "rtl_completion",
        "inout_composition",
        "lane=5a sibling=zz bus=z5a5\nlane=a5 sibling=zz bus=zzz5\n",
        "",
        &[],
    );
}

#[test]
fn net_array_selected_ports_preserve_independent_bits() {
    sim_cli::run_case(
        "rtl_completion",
        "net_array_selections",
        "lane=a5 siblings=zz,zz\nlane=z3 siblings=zz,zz\n",
        "",
        &[],
    );
}

#[test]
fn wired_arrays_resolve_sites_and_keep_pull_defaults() {
    sim_cli::run_case(
        "rtl_completion",
        "wired_arrays",
        "and=a0 other=zz or=f5 defaults=00,1\nand=03 other=zz or=35 defaults=00,1\n",
        "",
        &[],
    );
}

#[test]
fn fixed_initializers_execute_before_initial_processes() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_initialization",
        "calls=3 values=7,9,11\n",
        "",
        &[],
    );
}

#[test]
fn fixed_ports_accept_struct_array_elements() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_ports",
        "count=8 data=5a,ff\ncount=8 data=12,b7\n",
        "",
        &[],
    );
}

#[test]
fn aggregate_nets_preserve_member_drivers_through_ports() {
    sim_cli::run_case(
        "rtl_completion",
        "aggregate_nets",
        "result=5a,a5\nresult=12,a5\n",
        "",
        &[],
    );
}

#[test]
fn fixed_ref_ports_alias_struct_array_members_and_notify_changes() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_ref_ports",
        "before=34\nafter=7,3a observed=3a sibling=1,11\n",
        "",
        &[],
    );
}

#[test]
fn aggregate_inouts_share_whole_and_member_electrical_paths() {
    sim_cli::run_case(
        "rtl_completion",
        "aggregate_inouts",
        "bus=a5,5a\nbus=12,34\n",
        "",
        &[],
    );
}

#[test]
fn fixed_enum_leaves_use_captured_state_domains() {
    sim_cli::run_case(
        "rtl_completion",
        "fixed_enum_states",
        "enum state domains passed\n",
        "",
        &[],
    );
}
