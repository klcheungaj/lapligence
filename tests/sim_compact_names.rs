use crate::sim_cli;

#[test]
fn compact_c_names_preserve_distinct_source_scopes_and_values() {
    sim_cli::run_case(
        "compact_names",
        "names",
        "scope tb\nchildren 4 5\ndescriptor 19\nclass descriptor 23\nnames 15\ngenerate 11 13\nlong 37 wake 1\n",
        "",
        &[],
    );
}
