use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_002b";

#[test]
fn descriptor_calls_preserve_modes_lifetimes_and_recursion() {
    sim_cli::run_case(SUITE, "calls", "PASS rtl002b calls\n", "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "calls", "PASS rtl002b calls\n");
}

#[test]
fn selected_views_conditionals_and_nba_snapshots() {
    sim_cli::run_case(SUITE, "views", "PASS rtl002b views\n", "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "views", "PASS rtl002b views\n");
}

#[test]
fn segmented_unaligned_streams_at_both_capacities() {
    for count in ["65537", "16777216"] {
        let define = format!("RTL002B_COUNT={count}");
        sim_cli::run_case_with_args(
            SUITE,
            "streams",
            "PASS rtl002b streams\n",
            "",
            &[],
            &["-D", &define],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "streams", "PASS rtl002b streams\n");
}

#[test]
fn initializers_array_items_and_scatter_at_both_capacities() {
    for count in ["65537", "16777216"] {
        let define = format!("RTL002B_COUNT={count}");
        sim_cli::run_case_with_args(
            SUITE,
            "patterns",
            "PASS rtl002b patterns\n",
            "",
            &[],
            &["-D", &define],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "patterns", "PASS rtl002b patterns\n");
}

#[test]
fn neg_ref_conditional() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_conditional",
        "invalid expression for pass by reference",
    );
}
