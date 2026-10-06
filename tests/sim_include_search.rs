//! Public `llg` acceptance for include directories as definition search
//! directories: only the top file is given and `-I` finds the rest.

use std::path::Path;

use crate::sim_cli;

fn fixture_dir(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/include_search")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn top_file_with_include_dirs_finds_package_child_and_leaf() {
    let rtl = fixture_dir("rtl");
    let lib = fixture_dir("lib");
    sim_cli::run_case_with_args(
        "include_search",
        "top",
        "y=42 width=8\n",
        "llg: $finish at time 1000 at tb:10:5\n",
        &[],
        &["-I", &rtl, "--include-dir", &lib],
    );
}
