//! R03: fixed-array reductions through the public CLI, in both optimizer modes.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn fixed_array_reduction_basic() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "basic",
        "sum=9 product=24 and=0 or=7 xor=5 bits=8\nchanged=13\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_widths() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "widths",
        "sum=0 wide=256 named=256 sizes=8,32\nproduct=0 wide=256\nflags=1 widened=3 fill=1\nmapped=7,12,0,7,7\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_signed() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "signed",
        "sum=-7 product=12 wide=-7 enum=-7 record=-7\nmapped=-7 width=64\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_four_state() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "four_state",
        "sum=xxxx product=xxxx and=00x0 or=111x xor=11xx\nsingleton=zzzz,zzzz,zzzz,zzzz,zzzz\nsingle_x=10xz clean=8\nzero_and=0000 unknown_product=xxxx\none_or=1111\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_wide() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "wide",
        "wide=ok\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_nested() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "nested",
        "nested=50 named=50 indexed=52\nbounds=1,-1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_aggregates() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "aggregates",
        "packed=100 field=6 records=-7 lanes=23\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_functions() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "functions",
        "receiver=15 calls=1 captured=27 local=36 skipped=7\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_views() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "views",
        "selected=6 calls=1 slice=30 indices=5\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_ports() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "ports",
        "total=259\ntotal=60\ntotal=6\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_resizable_control() {
    sim_cli::run_case_with_args(
        "fixed_array_reductions",
        "resizable_control",
        "dynamic=6 queue=24 assoc=6 mapped=9\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_rejects_unmapped_row() {
    sim_cli::reject_case_with_args(
        "fixed_array_reductions",
        "unmapped_row",
        "can only be called on unpacked arrays of integral values",
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_rejects_nonintegral_map() {
    sim_cli::reject_case_with_args(
        "fixed_array_reductions",
        "nonintegral_map",
        "can only be called on unpacked arrays of integral values",
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_array_reduction_rejects_iterator_without_with() {
    sim_cli::reject_case_with_args(
        "fixed_array_reductions",
        "iterator_without_with",
        "without corresponding 'with' clause",
        &["--edition", "2009"],
    );
}
