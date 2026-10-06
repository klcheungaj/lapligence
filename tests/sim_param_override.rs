//! Public `llg` acceptance for type-correct top-level parameter overrides
//! (`-G/--param-override NAME=VALUE` and llg.toml `compile.param_overrides`).
//! Expected values follow IEEE 1800-2009 assignment conversion (6.24.1,
//! 10.7) of the given value to each parameter's declared type (23.10).

use std::path::Path;

use crate::sim_cli;

const SUITE: &str = "param_override";
const NO_FINISH: &str = "llg: simulation ended without $finish (no processes remain) at time 0\n";

fn config_path() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/param_override/overrides.toml")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn defaults_without_overrides() {
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[default] len=7\n\
         COUNT=1 OFFSET=0\n\
         WIDE=0\n\
         PATTERN=00000000 FLAG=0\n\
         RATE=0.000000 GAIN=0.000000 DELAY=0.000000\n\
         ANY=0 bits=32\n\
         MODE=SLOW PAIR=00\n\
         ELEM bits=32 elem=-1 FIXED=3\n",
        NO_FINISH,
        &[],
        &[],
    );
}

#[test]
fn command_line_overrides_follow_each_parameter_type() {
    // A string parameter takes bare text verbatim; a negative value converts
    // to signed types; an unsized decimal wider than 64 bits keeps its value
    // (and, untyped, its own width plus a sign bit: 4294967296 needs 33 bits);
    // X/Z digits survive; a packed struct takes an assignment pattern; an enum
    // member is found by name although its package is not imported; a type
    // parameter takes a data type.
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[hello world] len=11\n\
         COUNT=-5 OFFSET=-3\n\
         WIDE=123456789012345678901234567890\n\
         PATTERN=x1z01010 FLAG=1\n\
         RATE=2.500000 GAIN=1.000000 DELAY=3.250000\n\
         ANY=4294967296 bits=34\n\
         MODE=FAST PAIR=12\n\
         ELEM bits=12 elem=4095 FIXED=3\n",
        NO_FINISH,
        &[],
        &[
            "-G",
            "MSG=hello world",
            "-G",
            "COUNT=-5",
            "--param-override",
            "OFFSET=-3",
            "-G",
            "WIDE=123456789012345678901234567890",
            "-G",
            "PATTERN=8'bx1z0_1010",
            "-G",
            "FLAG=1",
            "-G",
            "RATE=2.5",
            "-G",
            "GAIN=1",
            "-G",
            "DELAY=3.25",
            "-G",
            "ANY=4294967296",
            "-G",
            "MODE=FAST",
            "-G",
            "PAIR='{hi:4'h1,lo:4'h2}",
            "-G",
            "ELEM=logic [11:0]",
        ],
    );
}

#[test]
fn command_line_overrides_keep_exact_text_and_wide_values() {
    // `a\b"c` is five bytes, not an escape sequence; 100'hF...F is 2^100-1;
    // -123456789012 converts to int modulo 2^32 (1097262572); 'z fills every
    // bit; a qualified enum member and a keyword type also work.
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[a\\b\"c] len=5\n\
         COUNT=1097262572 OFFSET=0\n\
         WIDE=1267650600228229401496703205375\n\
         PATTERN=zzzzzzzz FLAG=0\n\
         RATE=0.000000 GAIN=0.000000 DELAY=0.000000\n\
         ANY=7 bits=8\n\
         MODE=MID PAIR=00\n\
         ELEM bits=8 elem=-1 FIXED=3\n",
        NO_FINISH,
        &[],
        &[
            "-G",
            "MSG=a\\b\"c",
            "-G",
            "WIDE=100'hF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF",
            "-G",
            "ANY=8'd7",
            "-G",
            "MODE=po_pkg::MID",
            "-G",
            "ELEM=byte",
            "-G",
            "COUNT=-123456789012",
            "-G",
            "PATTERN='z",
        ],
    );
}

#[test]
fn quoted_string_overrides_use_string_literal_escapes() {
    // A whole double-quoted value is a string literal: `\t` is a tab.
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[tab\there] len=8\n\
         COUNT=1 OFFSET=127\n\
         WIDE=1267650600228229401496703205375\n\
         PATTERN=00000000 FLAG=0\n\
         RATE=0.000000 GAIN=0.000000 DELAY=0.000000\n\
         ANY=0 bits=32\n\
         MODE=SLOW PAIR=00\n\
         ELEM bits=32 elem=-1 FIXED=3\n",
        NO_FINISH,
        &[],
        &[
            "-G",
            "MSG=\"tab\\there\"",
            "-G",
            "WIDE=-1",
            "-G",
            "OFFSET=127",
        ],
    );
}

#[test]
fn config_overrides_follow_each_parameter_type() {
    // overrides.toml: strings are VALUE texts, -7 an integer, 0.125/-2.5e3/1.5
    // floats and `true` a boolean (1'b1). 36893488147419103232 is 2^65.
    let config = config_path();
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[from toml, with \"quotes\"] len=24\n\
         COUNT=-7 OFFSET=-100\n\
         WIDE=36893488147419103232\n\
         PATTERN=1x0z1x0z FLAG=1\n\
         RATE=0.125000 GAIN=-2500.000000 DELAY=1.500000\n\
         ANY=48879 bits=16\n\
         MODE=MID PAIR=93\n\
         ELEM bits=3 elem=7 FIXED=3\n",
        NO_FINISH,
        &[],
        &["--config", &config],
    );
}

#[test]
fn appended_command_line_override_replaces_the_config_value_by_name() {
    // `-G` replaces the configured list; `--append-param-override` adds to it
    // and a later entry for the same name wins.
    let config = config_path();
    sim_cli::run_case_with_args(
        SUITE,
        "overrides",
        "MSG=[cli] len=3\n\
         COUNT=-7 OFFSET=-100\n\
         WIDE=36893488147419103232\n\
         PATTERN=1x0z1x0z FLAG=1\n\
         RATE=0.125000 GAIN=-2500.000000 DELAY=1.500000\n\
         ANY=48879 bits=16\n\
         MODE=FAST PAIR=93\n\
         ELEM bits=3 elem=7 FIXED=3\n",
        NO_FINISH,
        &[],
        &[
            "--config",
            &config,
            "--append-param-override",
            "MSG=cli",
            "--append-param-override",
            "MODE=FAST",
        ],
    );
}

#[test]
fn type_parameter_without_default_takes_the_override() {
    sim_cli::run_case_with_args(
        SUITE,
        "type_required",
        "bits=5 V=21 copy=10101\n",
        NO_FINISH,
        &[],
        &["-G", "T=logic [4:0]", "-G", "V=21"],
    );
}

#[test]
fn local_parameter_override_is_rejected() {
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "parameter override 'FIXED' targets local parameter 'FIXED' of 'tb', which cannot be overridden",
        &["-G", "FIXED=9"],
    );
}

#[test]
fn unknown_parameter_override_is_rejected() {
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "unknown top-level parameter override 'MISSING'",
        &["-G", "MISSING=1"],
    );
}

#[test]
fn unparsable_override_value_is_rejected_as_written() {
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "'COUNT=1+' is not a valid form of parameter override",
        &["-G", "COUNT=1+"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "'ELEM=[3:0]' is not a valid form of parameter override",
        &["-G", "ELEM=[3:0]"],
    );
}

#[test]
fn unconvertible_override_value_names_the_value() {
    // An int does not convert implicitly to an enum (6.19.3).
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "parameter override value `1`: no implicit conversion from 'int' to 'mode_t'",
        &["-G", "MODE=1"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "overrides",
        "parameter override value `nowhere`: use of undeclared identifier 'nowhere'",
        &["-G", "COUNT=nowhere"],
    );
}
