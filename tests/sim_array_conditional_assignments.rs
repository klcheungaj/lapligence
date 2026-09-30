//! R04: module-procedural fixed-array expressions in both public CLI modes.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn array_conditional_assignment_comb() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "comb",
        "false=a6,3c\ntrue=a5,3c\nunknown=xx,3c\nhighz=xx,3c\nequal=a5,3c\nchanged=a5,xx\nknown_one=a5,f0\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_effects() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "effects",
        "effects=6,4,2 result=11,22,33\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_overlap() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "overlap",
        "overlap=11,22,11,22\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_nba() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "nba",
        "nba=33,ee row=xx,5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_clocked() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "clocked",
        "clock0=a6,5a\nclock1=a5,5a\nclockx=xx,5a\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_views() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "views",
        "views=33,44 calls=2,2\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_shapes() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "shapes",
        "shapes passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_expressions() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "expressions",
        "expressions=12,ab,55,55\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_rejects_incompatible_rank() {
    sim_cli::reject_case_with_args(
        "array_conditional_assignments",
        "wrong_shape",
        "no implicit conversion",
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_rejects_incompatible_elements() {
    sim_cli::reject_case_with_args(
        "array_conditional_assignments",
        "wrong_elements",
        "cannot be assigned to type",
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_nested_defaults_preserve_bound_element_order() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "nested_defaults",
        "nested_defaults passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_typed_defaults_preserve_byte_values() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "typed_defaults",
        "typed_defaults passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_deep_defaults_preserve_shared_wide_values() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "deep_defaults",
        "deep_defaults passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn array_conditional_assignment_repeated_values_preserve_effects_and_resumes() {
    sim_cli::run_case_with_args(
        "array_conditional_assignments",
        "repeated_values",
        "repeated_values passed\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
