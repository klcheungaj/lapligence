//! Regression coverage for string-producing system formatting tasks/functions.

use crate::sim_cli;

#[test]
fn string_formatting_is_typed_and_retains_results() {
    sim_cli::run_case(
        "string_format",
        "entrypoints",
        "out=<d=17 h=af b=1010 o=017 c=* s=ok r=1.5>\nb=1010\no=017\nh=af\nsformat=<prefix:x:3>\ndynamic=<v=7>\ndynamic-f=<v=8>\nwide=00000000000000000000000000004142\nwide-real=00000000000000000000000000312e35\nnarrow=45\npacked=4243\nfunction=fn=4\nnested=[9]\nempty=<>\ncalls=2 side=1/2\nheld=<hold>\nsegments=left=1 right=0f\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
    );
}

#[test]
fn packed_arguments_print_as_ascii_strings_in_both_editions() {
    // Independent oracle: the standards' `%s` examples (V 2.6.3 / 17.1.1,
    // SV 11.10); leading zero bytes are never printed (V 17.1.1.7).
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "string_format",
            "packed_string_conversion",
            concat!(
                "Hello world is stored as 00000048656c6c6f20776f726c64\n",
                "Hello world!!! is stored as 48656c6c6f20776f726c64212121\n",
                "e is ascii value for 101\n",
                "[A][A]\n",
                "<A>\n",
            ),
            "llg: simulation ended without $finish (no processes remain) at time 0\n",
            &[],
            &["--edition", edition],
        );
    }
}
