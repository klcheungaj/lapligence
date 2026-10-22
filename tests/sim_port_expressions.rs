//! Port expressions in module, interface and program headers: concatenated,
//! part-selected and bit-selected port references (IEEE 1800-2009 23.2.2.1,
//! 23.2.2.2, 23.3.3; IEEE 1364-2001 12.3.2, 12.3.9). See
//! `tests/fixtures/sim/port_expressions/readme.md`.
use crate::sim_cli;

const SUITE: &str = "port_expressions";

#[test]
fn output_concatenations_drive_nets_and_variables() {
    sim_cli::run_case(
        SUITE,
        "concat_output",
        "1 pm=101 pv=101 a=1 b=01 nm=101\n\
         6 pm=010 pv=010 a=0 b=10 nm=010\n",
        "llg: $finish at time 6000 at tb:30:5\n",
        &[],
    );
}

#[test]
fn input_concatenations_split_the_actual() {
    sim_cli::run_case(
        SUITE,
        "concat_input",
        "1 net a=1 b=01 var a=0 b=10\n\
         2 net a=0 b=10 var a=1 b=01\n",
        "llg: $finish at time 2000 at tb:22:5\n",
        &[],
    );
}

#[test]
fn inout_concatenations_connect_both_ways() {
    sim_cli::run_case(
        SUITE,
        "concat_inout",
        "1 w=100 a=1 b=00\n\
         3 w=zzz a=z b=zz\n\
         4 w=011 a=0 b=11\n",
        "llg: $finish at time 4000 at tb:23:5\n",
        &[],
    );
}

#[test]
fn bit_and_part_select_port_references() {
    sim_cli::run_case(
        SUITE,
        "selects",
        "1 w=0110 v=0110 x=1100 pv=10 pw=z101 q1=01 q2=11\n\
         1 si a=1010 b=z1\n\
         3 w=1001 v=1001 pv=01 pw=z011\n\
         5 pv=01 pw=z011 r=0101\n",
        "llg: $finish at time 5000 at tb:53:5\n",
        &[],
    );
}

#[test]
fn inout_select_port_references() {
    sim_cli::run_case(
        SUITE,
        "inout_selects",
        "1 w=01 v=01 a=1001 b=0 y=01 da=1101\n\
         3 w=zz v=zz a=zzzz b=z\n\
         4 w=10 v=zz a=zz10 b=z\n",
        "llg: $finish at time 4000 at tb:32:5\n",
        &[],
    );
}

#[test]
fn indexed_part_select_port_references() {
    sim_cli::run_case(
        SUITE,
        "indexed",
        "w1=01 w2=10 n=1001\n",
        "llg: $finish at time 1000 at tb:18:5\n",
        &[],
    );
}

#[test]
fn mixed_direction_concatenation() {
    sim_cli::run_case(
        SUITE,
        "mixed_direction",
        "1 w=01\n2 w=10\n",
        "llg: $finish at time 2000 at tb:20:5\n",
        &[],
    );
}

#[test]
fn explicit_ansi_port_expressions() {
    sim_cli::run_case(
        SUITE,
        "ansi_explicit",
        "1 lo=0101 hi=0101\n2 lo=1101 hi=1101\n",
        "llg: $finish at time 2000 at tb:20:5\n",
        &[],
    );
}

#[test]
fn interface_and_program_port_expressions() {
    sim_cli::run_case(
        SUITE,
        "interface_program",
        "ifc a=1 b=0 q1=10 q2=10\nprg a=0 b=1\n",
        "",
        &[],
    );
}

#[test]
fn verilog_2001_port_expressions() {
    sim_cli::run_case_with_args(
        SUITE,
        "nonansi.v",
        "pm=101 pn=101 cm=10 e1=z1zz e2=z0zz\n",
        "llg: $finish at time 2000 at tb:22:5\n",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn outside_lvalue_forms_and_shared_inner_nets() {
    sim_cli::run_case(
        SUITE,
        "outside_forms",
        "1 va=10 vb=10 na=10 nb=10 vw=xx1010xx nw=10101010 tw=11000011 x6=1100\n\
         1 s1=1 x=0 y=0 n=z010 v=1xxx\n\
         3 va=01 vb=01 na=01 nb=01 vw=xx0101xx nw=01010101 tw=zzzzzzzz\n",
        "llg: $finish at time 3000 at tb:61:5\n",
        &[],
    );
}

#[test]
fn generate_and_instance_array_port_expressions() {
    sim_cli::run_case(
        SUITE,
        "instance_arrays",
        "w0=10 w1=10 x=1010 y=1010\n",
        "llg: $finish at time 1000 at tb:27:5\n",
        &[],
    );
}

#[test]
fn output_port_expression_width_differences() {
    sim_cli::run_case(
        SUITE,
        "widths",
        "narrow=10 wide=zz110\n",
        "llg: $finish at time 1000 at tb:18:5\n",
        &[],
    );
}

#[test]
fn non_constant_port_reference_select_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_var_select",
        "neg_var_select.sv:2:15 identifier 'i' used before its declaration",
    );
}

#[test]
fn output_port_expression_needs_an_lvalue() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_expression",
        "neg_output_expression.sv:10:10 expression is not assignable",
    );
}

#[test]
fn unnamed_implicit_port_cannot_be_connected_by_name() {
    sim_cli::reject_case(
        SUITE,
        "neg_named_implicit",
        "neg_named_implicit.sv:8:8 port 'a' does not exist in 'm'",
    );
}

#[test]
fn inout_concatenation_with_a_variable_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_inout_variable_part",
        "neg_inout_variable_part.sv:2:13 port concatentation can't contain any 'inout' ports",
    );
}

#[test]
fn inout_port_expression_to_a_variable_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_inout_to_variable",
        "neg_inout_to_variable.sv:8:10 'v' cannot be connected to 'inout' port",
    );
}

#[test]
fn input_port_variable_cannot_be_assigned() {
    sim_cli::reject_case(
        SUITE,
        "neg_input_assigned",
        "neg_input_assigned.sv:5:11 cannot assign to input port 'v'",
    );
}

#[test]
fn inout_port_expression_width_difference_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_inout_width",
        "inout port `p` connects a 2-bit actual to its 3-bit port expression at ",
    );
    sim_cli::reject_case(SUITE, "neg_inout_width", "neg_inout_width.sv:5:15");
}
