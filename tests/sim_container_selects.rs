//! End-to-end coverage for selects of packed resizable-container elements.
//!
//! Every positive fixture runs on the legacy and compact value backends in
//! both optimizer modes. Expected values were derived by hand from the bit
//! patterns each fixture writes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

fn parity(fixture: &str, expected: &str) {
    sim_cli::run_case_backend_parity("container_selects", fixture, expected, &[], &[]);
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
