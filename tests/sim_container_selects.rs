//! End-to-end coverage for selects of packed resizable-container elements.
//!
//! Every positive fixture runs on the legacy and compact value backends in
//! both optimizer modes. Expected values were derived by hand from the bit
//! patterns each fixture writes.

use crate::sim_cli;

fn parity(fixture: &str, expected: &str) {
    sim_cli::run_case_backend_parity("container_selects", fixture, expected, &[], &[]);
}

/// [`parity`] with index checks off (S07-D15), for a fixture that tests the
/// result IEEE 1800 gives an invalid index.
fn parity_without_index_checks(fixture: &str, expected: &str) {
    sim_cli::run_case_backend_parity(
        "container_selects",
        fixture,
        expected,
        &["--no-index-checks"],
        &[],
    );
}

#[test]
fn dynamic_array_element_selects_write_only_the_selected_bits() {
    parity(
        "dynamic_selects",
        concat!(
            "d0=3ffffffffffffffff0000000000000000\n",
            "d1=x000000000000000z0000000000000001\n",
            "d2=0 0000000000000000000000000000a50\n",
            "reads=xz f a5 z10x\n",
            "size=3 d0=3ffffffffffffffff0000000000000000\n",
            "asc=3c0000000000000009\n",
            "pk=905a0001 byte2=5a\n",
            "two=4ffffffffffffffffffffffff\n",
            "s=-590295810358705651712\n",
        ),
    );
}

#[test]
fn queue_element_selects_follow_queue_write_index_rules() {
    let expected = concat!(
        "q0=2aaaabbbbccccdddd0000000000000000\n",
        "q1=3fffff00ffffffffeffffffffffffff xz01xxxx\n",
        "n=3 q2=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx low=xxx1\n",
        "n=3\n",
        "reads=a 0 xz01\n",
    );
    parity("queue_selects", expected);
    sim_cli::run_case(
        "container_selects",
        "queue_selects",
        expected,
        concat!(
            "llg container warning: invalid queue write index\n",
            "llg: $finish at time 0 at tb:26:9\n",
        ),
        &[],
    );
}

#[test]
fn associative_element_selects_create_missing_entries_from_the_default() {
    parity(
        "assoc_selects",
        concat!(
            "a7=xz 00000000000000000000000000000001\n",
            "a-3=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx xx10xxxx x n=2 e=1\n",
            "a4=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxffx n=3\n",
            "reads=1 x ff\n",
            "b1=280000000000000000\n",
            "d9=3d d10=3c n=1\n",
            "sk=000000000000000008 sm=0101xx n=2\n",
        ),
    );
}

#[test]
fn nested_dynamic_array_element_selects_use_every_container_index() {
    parity(
        "nested_selects",
        concat!(
            "e2=280000000000000zz0 e1=000000000000000001\n",
            "reads=00 a 0zzzzzzzz0\n",
            "row=000000000000000000\n",
        ),
    );
}

#[test]
fn compound_assignment_to_container_element_select_is_rejected() {
    sim_cli::reject_case(
        "container_selects",
        "compound_select",
        "compound assignment to resizable container element in `tb` is not supported",
    );
}

#[test]
fn queue_indices_beyond_32_bits_are_invalid_not_truncated() {
    parity_without_index_checks(
        "wide_index_queue",
        concat!(
            "int  0 0 0 0 0 0 0\n",
            "logic xxxx xxxx xxxx xxxx\n",
            "string [] [] []\n",
            "real 0.0 0.0\n",
            "record 0 []\n",
            "struct 0 class 0\n",
            "valid 8 5 8\n",
            "q='{5, 6, 7, 8}\n",
            "lq='{17, 34, 51, 68} sq='{\"a\", \"b\", \"c\", \"d\"}\n",
            "rq='{1.500000, 2.500000, 3.500000, 4.500000}\n",
            "rr='{'{a:1, s:\"x\"}, '{a:2, s:\"y\"}, '{a:3, s:\"z\"}, '{a:4, s:\"w\"}}\n",
            "h='{10, 20, 30, 40} dq='{50, 60, 70, 80}\n",
            "after q='{5, 106, 7, 8}\n",
            "not appended: size=4\n",
            "appended: '{5, 106, 7, 8, 201, 202}\n",
            "last=202 before=201\n",
            "'{5, 106, 7, 8, 203, 202}\n",
            "wide offset from $: 0\n",
            "methods ignored '{5, 106, 7, 8, 203, 202}\n",
            "methods valid '{5, 58, 106, 7, 8, 203}\n",
            "slice '{106, 7}\n",
            "clipped high 6\n",
            "clipped low '{5, 106}\n",
            "wide high 6\n",
            "wide high 65 6\n",
            "unsigned high '{106, 7, 8, 203, 202}\n",
            "wide low 64 '{5, 106}\n",
            "wide low 65 '{5, 106}\n",
            "wide left beyond $ 0\n",
            "wide both 0\n",
            "wide both ends 6\n",
            "x bound 0\n",
            "x bound right 0\n",
        ),
    );
}

#[test]
fn string_indices_beyond_32_bits_are_invalid_not_truncated() {
    parity(
        "wide_index_strings",
        concat!(
            "read 100 0 0 0 0\n",
            "ignored [abcdef]\n",
            "valid [abcZef]\n",
            "getc 90 0 0 0\n",
            "putc ignored [abcZef]\n",
            "putc valid [abcWef]\n",
            "substr [bc] [] [] [] []\n",
        ),
    );
}
