//! Output variable port declaration initializers (IEEE 1800-2009 A.1.3,
//! A.2.1.2, 6.8; IEEE 1364-2001 12.3.3, 6.2.1).
use crate::sim_cli;

const SUITE: &str = "output_port_init";

#[test]
fn ansi_output_port_initializers_set_the_variable_before_procedures() {
    sim_cli::run_case(
        SUITE,
        "ansi",
        "tb t0 u2.o=1 u2.n=5 u2.q=21 u3.q=8\n\
         tb t1 po=1 pn=5 pq=21 pb=a pe=10 changed=1\n\
         tb t1 io=1 in=3 qo=0 qn=4\n\
         program po=0 pn=4\n",
        "",
        &[],
    );
}

#[test]
fn ansi_output_port_initializers_of_every_variable_type() {
    sim_cli::run_case(
        SUITE,
        "ansi_types",
        "r=2.500000 e=C s=34 k=111111 str=hi i=-3 ua=1,2\n",
        "llg: $finish at time 1000 at tb:24:5\n",
        &[],
    );
}

#[test]
fn non_ansi_output_port_initializers() {
    sim_cli::run_case(
        SUITE,
        "nonansi",
        "po=1 pq=9 pr=6\n",
        "llg: $finish at time 1000 at tb:15:5\n",
        &[],
    );
}

#[test]
fn verilog_2001_output_reg_port_initializers() {
    sim_cli::run_case_with_args(
        SUITE,
        "nonansi.v",
        "child o=1 q=9\ntb po=1 pq=9\n",
        "llg: $finish at time 3000 at tb:13:14\n",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn inout_port_initializer_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_inout",
        "neg_inout.sv:2:28 only 'output' variable port declarations may have an initializer expression",
    );
}

#[test]
fn output_net_port_initializer_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_net",
        "neg_output_net.sv:2:28 only 'output' variable port declarations may have an initializer expression",
    );
}
