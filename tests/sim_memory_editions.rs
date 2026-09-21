//! Public CLI coverage for edition-specific memory-file addressing.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn omitted_memory_range_keeps_the_selected_edition_order() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "default_order",
        "m1=11 m0=22\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2001"],
        &[("words.mem", "11\n22\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "default_order",
        "m1=22 m0=11\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "11\n22\n")],
    );
}

#[test]
fn start_only_memory_range_uses_the_edition_default_finish() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "start_only",
        "m3=00 m2=aa m1=bb m0=00\n",
        concat!(
            "llg: memory file `words.mem`: memory file contains too few words for the selected range\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2001"],
        &[("words.mem", "aa\nbb\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "start_only",
        "m3=bb m2=aa m1=00 m0=00\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "aa\nbb\n")],
    );
}

#[test]
fn explicit_descending_range_and_address_jumps_preserve_source_direction() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "explicit_descending",
        "m0=00 m1=33 m2=22 m3=11\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("words.mem", "11\n22\n33\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "address_jump",
        "m0=c3 m1=b2 m2=d4 m3=a1\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
        &[("jumps.mem", "@3\na1\n@1\nb2\nc3\n@2\nd4\n")],
    );
}

#[test]
fn truncated_data_and_two_state_conversion_keep_prior_values_and_diagnose() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "truncated",
        "m0=7f m1=b1\n",
        concat!(
            "llg: memory file `short.mem`: memory file contains too few words for the selected range\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("short.mem", "7f\n")],
    );
    sim_cli::run_case_with_files(
        "memory_editions",
        "two_state",
        "m0=1000 m1=1010\n",
        concat!(
            "llg: memory file `unknown.mem`: X/Z memory data converted to a two-state element\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("unknown.mem", "1x0z\n1010\n")],
    );
}

#[test]
fn invalid_address_terminates_after_already_loaded_values() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "invalid_address",
        "m0=00 m1=aa m2=02 m3=03\n",
        concat!(
            "llg: memory file `bad.mem`: address jump is outside the destination memory or selected range; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("bad.mem", "@1\naa\n@3\nbb\n")],
    );
}

#[test]
fn enum_memory_data_stops_at_the_first_non_member() {
    sim_cli::run_case_with_files(
        "memory_editions",
        "enum_invalid",
        "m0=00 m1=01\n",
        concat!(
            "llg: memory file `enum.mem`: memory data value is not a member of the enum; load terminated\n",
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
        ),
        &[],
        &["--edition", "2009"],
        &[("enum.mem", "00\n02\n")],
    );
}

#[test]
fn memory_file_tasks_reject_multidimensional_arrays() {
    sim_cli::reject_case_with_args(
        "memory_editions",
        "reject_multidim",
        "requires a one-dimensional memory",
        &["--edition", "2009"],
    );
}
