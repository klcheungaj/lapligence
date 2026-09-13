//! CLI coverage for typed `ref` and `const ref` subroutine aliases.

#[path = "support/sim.rs"]
mod sim_harness;
#[path = "support/sim_cli.rs"]
mod sim_cli;

#[test]
fn reference_argument_aliases_run_in_both_optimizer_modes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_cli::run_case(
        "function",
        "reference_argument_alias",
        "alias-inside=9\n\
         alias-after=9\n\
         nested-after=10\n\
         bit-after=4\n\
         array-after=170\n\
         dynamic-array-after=170 calls=1\n\
         recursive-after=11\n\
         const-after=42\n\
         done\n",
        "",
        &[],
    );
}

#[test]
fn reference_argument_rejects_a_packed_bit_select_in_2009() {
    sim_cli::reject_case(
        "function",
        "reference_packed_select_rejected",
        "invalid expression for pass by reference",
    );
}

#[test]
fn reference_argument_rejects_const_mutation() {
    sim_cli::reject_case(
        "function",
        "reference_const_mutation_rejected",
        "cannot assign to read-only variable",
    );
}

#[test]
fn reference_argument_rejects_automatic_ref_static_storage() {
    sim_cli::reject_case(
        "function",
        "reference_automatic_ref_static_rejected",
        "cannot pass automatic variables",
    );
}
