//! R05: dissimilar port-net resolution, separate from true alias legality.
use crate::sim_cli;

#[test]
fn port_net_type_wire_wired() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "wire_wired",
        "known=01/01\nunknown=0x/0x\none=11/11\nreleased=zz/zz\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_hierarchy() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "hierarchy",
        "chain=000\none=111\nforce=000\nrelease=111\nfloat=zzz\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_bias_strengths() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "bias_strengths",
        "zero=01001/011\none=01101\nunknown=01x01\nfloat=01101\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_selected() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "selected",
        "selected=f0/f0\nforce=f4/f4\nrelease=f0/f0\nunknown=f0/f0\nfloat=zz/zz\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_arrays() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "arrays",
        "arrays=0z/0z selected=zz1z/zzzz\nchanged=5/5\nfloat=z/z\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_aliases() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "aliases",
        "aliases=1000/1000/0010\nchanged=1010/1010/1010\nzero_parent=0000/0000/0000\nzero_child=0000/0000/0000\nfloat=zzzz/zzzz/zzzz\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_delays() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "delays",
        "t3=zz/11\nt5=11/11\nt10=11/00\nt12=00/00\nt14=00/zz\nt16=zz/zz\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_selected_delays() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "selected_delays",
        "t2=z1\nt5=11\n",
        "",
        &[],
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_ascending_disjoint_selections() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "ascending",
        "wired=11 split=1001 cell=11/1\nzero=00 cell=00/0\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

fn assert_collapse_warnings(output: std::process::Output, expected: &str, count: usize) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected,
        "{stderr}"
    );
    let warnings = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("llg: warning: "))
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), count, "{stderr}");
    for warning in warnings {
        assert!(warning.starts_with("dissimilar inout port `"), "{warning}");
        assert!(warning.contains("Table 23-1) at "), "{warning}");
        assert!(
            warning.contains(".sv:"),
            "warning must retain source location: {warning}"
        );
    }
    for line in stderr.lines() {
        assert!(
            line.starts_with("llg: warning: ") || crate::sim_harness::is_compile_report_line(line),
            "{stderr}"
        );
    }
}

#[test]
fn port_net_type_warning_pairs_use_the_external_type() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "port_net_types",
            "warnings",
            optimized,
            &["--edition", "v2001"],
            &[],
            &[],
        );
        assert_collapse_warnings(output, "conflicts=010101\n", 6);
    }
}

#[test]
fn port_net_type_sibling_tie_order_is_deterministic() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "port_net_types",
            "siblings",
            optimized,
            &["--edition", "v2001"],
            &[],
            &[],
        );
        assert_collapse_warnings(output, "siblings=000\n", 1);
    }
}

#[test]
fn port_net_type_uwire_formal_keeps_one_collapsed_driver() {
    // RTL-105 admits `inout uwire` formals; the collapsed net still allows
    // one driver (IEEE 1800-2009 6.6.2).
    sim_cli::reject_case_with_args(
        "port_net_types",
        "bad_uwire",
        "a collapsed uwire net has 2 drivers",
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_does_not_relax_alias_legality() {
    sim_cli::reject_case_with_args(
        "port_net_types",
        "bad_alias",
        "all nets in a net alias statement must have a common nettype",
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_trireg_remains_explicitly_unsupported() {
    sim_cli::reject_case_with_args(
        "port_net_types",
        "bad_trireg",
        "unsupported: `trireg` net `",
        &["--edition", "v2001"],
    );
}

#[test]
fn port_net_type_admits_undriven_uwire_actual() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "uwire_external",
        "uwire=z/z\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_keeps_unconnected_same_type_uwire_aliases() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "uwire_alias",
        "alias=11\nalias=00\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn port_net_type_concat_actual_without_high_declaration_is_connected() {
    sim_cli::run_case_with_args(
        "port_net_types",
        "concat_actual",
        "concat=10/00/1000\nchanged=01/01/0101\nparent_only=11/00/1100\nfloat=zz/zz/zzzz\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
