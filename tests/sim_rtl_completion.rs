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
fn unpacked_structure_conditionals_merge_immediate_members() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "syn_004_record_conditional",
        "record conditional passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn unpacked_structure_conditionals_reject_native_members() {
    sim_cli::reject_case_with_args(
        "rtl_completion",
        "syn_004_record_conditional_rejected",
        "conditional structure member has no supported fixed payload",
        &["--edition", "2009"],
    );
}

#[test]
fn whole_array_continuous_assignments_keep_sources_cells_and_rhs_snapshots() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "syn_006_array_continuous",
        concat!(
            "t1 net=a1,X2 var=a1,a2 cond=a1,xx\n",
            "t1 pattern=c1,c2 selected=zz,a2 split=a1,b2 bit=01,02 row=31,32 ",
            "func=e1,e3 calls=1\n",
            "t2 net=X1,X2 var=a1,d2 cond=a1,d2\n",
            "t2 pattern=c1,c2 selected=zz,d2 split=a1,c2 bit=03,02 row=41,32 ",
            "func=e1,e4 calls=2\n",
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_input_ports_capture_values_and_runtime_rows() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "syn_007_array_input_values",
        concat!(
            "t1 cond=a1,b1 func=12,22 selected=33,43 slice=14,24 pattern=17,39 reverse=5b,6c calls=1\n",
            "t2 cond=11,30 func=12,31 selected=e3,d3 slice=14,24 pattern=17,39 reverse=5b,6c calls=2\n",
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_input_port_rank_mismatch_is_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_007_array_input_rank_rejected",
        "cannot be assigned to type 'row_t'",
    );
}

#[test]
fn fixed_array_input_port_element_mismatch_is_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_007_array_input_element_rejected",
        "cannot be assigned to type 'row_t'",
    );
}

#[test]
fn fixed_output_ref_and_aggregate_port_shapes_preserve_storage() {
    sim_cli::run_case(
        "rtl_completion",
        "syn_008_port_shape_matrix",
        concat!(
            "direct=11,22 slice=51,62\n",
            "ref=11,32 observed=23\n",
            "packet=02,1d,d3\n",
            "nested=54 distributed=65,76\n",
        ),
        "",
        &[],
    );
}

#[test]
fn illegal_output_expression_and_ref_shape_are_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_008_output_expression_rejected",
        "expression is not assignable",
    );
    sim_cli::reject_case(
        "rtl_completion",
        "syn_008_ref_shape_rejected",
        "inequivalent type",
    );
}

#[test]
fn whole_array_continuous_variable_conflicts_are_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_006_array_continuous_variable_conflict",
        "multiple continuous assignments to variable storage",
    );
}

#[test]
fn dynamic_fixed_net_array_continuous_targets_are_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_006_array_continuous_dynamic_net",
        "reference to non-constant variable 'index'",
    );
}

#[test]
fn conditional_generate_continuous_assignments_only_keep_active_branches() {
    sim_cli::run_case(
        "rtl_completion",
        "syn_006_generate_continuous",
        "generated=1,0\n",
        "",
        &[],
    );
}

#[test]
fn same_instance_continuous_variable_conflicts_are_rejected() {
    sim_cli::reject_case(
        "rtl_completion",
        "syn_006_generate_continuous_conflict",
        "multiple continuous assignments to variable storage",
    );
}

#[test]
fn fixed_arrays_of_structs_preserve_member_paths_and_formal_shapes() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "struct_array_values",
        "sum=16 data=5a,a5\n",
        "",
        &[],
        &["--edition", "2009"],
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
    sim_cli::run_case_with_args(
        "rtl_completion",
        "fixed_block_locals",
        "local=26 saved=11\nlocal=26 saved=12\n",
        "",
        &[],
        &["--edition", "2009"],
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
    sim_cli::run_case_with_args(
        "rtl_completion",
        "inout_composition",
        "lane=5a sibling=zz bus=z5a5\nlane=a5 sibling=zz bus=zzz5\n",
        "",
        &[],
        &["--edition", "2009"],
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

#[test]
fn array_conditional_values_preserve_element_semantics() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "array_conditional_values",
        "array conditional values passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_nested_elements_use_uninitialized_defaults() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "array_conditional_nested",
        "array conditional nested defaults passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_wide_elements_preserve_state_domains() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "array_conditional_wide",
        "array conditional wide states passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_arms_are_captured_once_and_short_circuited() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "array_conditional_effects",
        "array conditional effects passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
