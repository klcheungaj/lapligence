use super::run_case_with_stderr;

#[test]
fn intra_assignment_event_lists_keep_edge_qualifier_and_function_dependencies() {
    run_case_with_stderr(
        "intra_assignment_event_sources",
        "sources t=2000 result=1 source=1\n",
        "",
    );
}
