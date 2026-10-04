//! RTL-020: combinational UDP contexts (IEEE 1364-2001 8.1-8.2, 8.6;
//! IEEE 1800-2009 29.3-29.4, 29.8). Expected outputs are hand-derived (see the
//! fixture readme) or, for the exhaustive table sweep, computed by the
//! independent all-matching-rows oracle below.

use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_020";

/// One definition as written in `exhaustive_tables.v`: input symbols per row
/// and the output symbol.
type Table = &'static [(&'static str, char)];

const MUX3: Table = &[
    ("00?", '0'),
    ("01?", '1'),
    ("1?0", '0'),
    ("1?1", '1'),
    ("x00", '0'),
    ("x11", '1'),
];

const PARITY3: Table = &[
    ("000", '0'),
    ("001", '1'),
    ("010", '1'),
    ("011", '0'),
    ("100", '1'),
    ("101", '0'),
    ("110", '0'),
    ("111", '1'),
];

const SYMBOLS: Table = &[
    ("0b?", '0'),
    ("0B0", '0'),
    ("10?", '1'),
    ("11b", 'X'),
    ("1X?", '1'),
    ("x00", '1'),
    ("X1B", '0'),
    ("xx?", '0'),
    ("x01", 'x'),
];

/// IEEE 1364-2001 8.1.6 / 1800-2009 29.3.5: a Z input is treated as X, `b`
/// covers 0 and 1, `?` covers 0, 1 and X, and an unmatched combination gives
/// X. Every matching row is collected: legal tables never disagree.
fn table_output(table: Table, inputs: &[char]) -> char {
    let matches = |symbols: &str| {
        symbols.chars().zip(inputs).all(|(symbol, input)| {
            let input = if *input == 'z' { 'x' } else { *input };
            match symbol.to_ascii_lowercase() {
                '?' => true,
                'b' => input != 'x',
                exact => exact == input,
            }
        })
    };
    let mut outputs = table
        .iter()
        .filter(|(symbols, _)| matches(symbols))
        .map(|(_, output)| output.to_ascii_lowercase());
    let first = outputs.next().unwrap_or('x');
    assert!(
        outputs.all(|output| output == first),
        "conflicting oracle rows"
    );
    first
}

fn exhaustive_expected() -> String {
    const VALUES: [char; 4] = ['0', '1', 'x', 'z'];
    let mut expected = String::new();
    for pass in 0..2 {
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    let pick = |index: usize| VALUES[if pass == 0 { index } else { 3 - index }];
                    let inputs = [pick(a), pick(b), pick(c)];
                    expected.push_str(&format!(
                        "{}{}{} m={} p={} s={}\n",
                        inputs[0],
                        inputs[1],
                        inputs[2],
                        table_output(MUX3, &inputs),
                        table_output(PARITY3, &inputs),
                        table_output(SYMBOLS, &inputs),
                    ));
                }
            }
        }
    }
    expected
}

#[test]
fn component_oracle_follows_the_clause_examples() {
    // IEEE 1364-2001 8.1.6: an unknown select with equal data inputs is known.
    assert_eq!(table_output(MUX3, &['x', '1', '1']), '1');
    assert_eq!(table_output(MUX3, &['z', '0', '0']), '0');
    assert_eq!(table_output(MUX3, &['x', '0', '1']), 'x');
    assert_eq!(table_output(PARITY3, &['1', 'z', '0']), 'x');
    assert_eq!(table_output(SYMBOLS, &['0', '1', 'z']), '0');
    assert_eq!(table_output(SYMBOLS, &['0', 'x', '0']), 'x');
    assert_eq!(exhaustive_expected().lines().count(), 128);
}

#[test]
fn exhaustive_tables_cover_every_symbol_and_state_in_both_editions() {
    let expected = exhaustive_expected();
    sim_cli::run_case_backend_parity(
        SUITE,
        "exhaustive_tables.v",
        &expected,
        &["--edition", "2009"],
        &[],
    );
    sim_cli::run_case_with_args(
        SUITE,
        "exhaustive_tables.v",
        &expected,
        "",
        &[],
        &["--edition", "2001"],
    );
}

#[test]
fn selected_scalar_terminals_drive_and_read_larger_objects() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_020/selected_terminals.out");
    sim_cli::run_case(SUITE, "selected_terminals", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "selected_terminals", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "selected_terminals", expected);
}

#[test]
fn instance_arrays_slice_every_connection_form_in_both_editions() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_020/instance_arrays.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "instance_arrays.v",
        expected,
        &["--edition", "2009"],
        &[],
    );
    sim_cli::run_case_with_args(
        SUITE,
        "instance_arrays.v",
        expected,
        "",
        &[],
        &["--edition", "2001"],
    );
}

#[test]
fn multidimensional_and_unpacked_instance_arrays_match_dimensions() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_020/instance_arrays_sv.out");
    sim_cli::run_case(SUITE, "instance_arrays_sv", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "instance_arrays_sv", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "instance_arrays_sv", expected);
}

#[test]
fn competing_drivers_strengths_and_delays_in_both_editions() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_020/drivers_delays.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "drivers_delays.v",
        expected,
        &["--edition", "2009"],
        &[],
    );
    sim_cli::run_case_with_args(
        SUITE,
        "drivers_delays.v",
        expected,
        "",
        &[],
        &["--edition", "2001"],
    );
}

#[test]
fn udp_instance_arrays_compose_into_a_ripple_adder() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_020/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "composition", expected);
}

#[test]
fn neg_definition_and_instance_shape_boundaries_reject() {
    // Adopted FND-002 witnesses: L-F08-09-02 (vector definition port) and
    // L-F08-09-01 (row width).
    sim_cli::reject_case(
        SUITE,
        "neg_vector_definition",
        "port 'a' is missing a corresponding body declaration",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_width",
        "incorrect number of input fields in table row; have 2 but expect 1",
    );
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_conflicting_rows.v",
            "primitive table row duplicates a set of inputs with a different specified output value",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_terminal_count.v",
            "wrong number of port connections for 'p' instance (4 given, expected 3)",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_empty_terminal.v",
            "combinational UDPs take one scalar output followed by scalar input terminals",
            &["--edition", edition],
        );
    }
}

#[test]
fn neg_vector_and_aggregate_terminals_remain_rejected() {
    // Accepting a selected scalar is not accepting a vector terminal.
    sim_cli::reject_case(
        SUITE,
        "neg_vector_input",
        "input terminal 1 of combinational UDP `u` in `tb` must be scalar",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_vector_output",
        "output terminal 0 of combinational UDP `u` in `tb` must be scalar",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_aggregate_input",
        "input terminal 1 of combinational UDP `u` in `tb` must be scalar",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_input",
        "value of type 'logic$[2]' cannot be assigned to type 'logic'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_array_width",
        "cannot connect 'logic[2:0]' to each port of type 'logic' in 'u'",
    );
}
