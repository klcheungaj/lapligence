//! Queue and dynamic-array `sort`/`rsort` through the public CLI: result order,
//! stable ties, X/Z-key barriers, wide and signed keys, `with` evaluation count
//! and a size that a quadratic sort cannot finish quickly. Expected text is
//! written by hand or recomputed here with Rust integer arithmetic.

use crate::sim_cli;

const SUITE: &str = "container_sort";

fn run_both_backends(fixture: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, fixture, expected, &[], &[]);
}

#[test]
fn sort_and_rsort_order_ties_keys_and_boundaries() {
    let expected = "\
q.sort -4 0 1 3 3 5 9 9
q.rsort 9 9 5 3 3 1 0 -4
d.sort -4 0 1 3 3 5 9 9
d.rsort 9 9 5 3 3 1 0 -4
tq.sort 12 11 13 21 23 22
tq.rsort 21 23 22 12 11 13
td.sort 12 11 13 21 23 22
td.rsort 21 23 22 12 11 13
u8.sort 5 127 128 130 200
u8.signed_sort 128 130 200 5 127
u8.signed_rsort 127 5 200 130 128
xq.sort 2 5 x 0 1 z 3 9
xq.rsort 5 2 x 1 0 z 9 3
xd.sort 0010 0101 1x00 0000 0001 0z11 0011 1001
xd.rsort 0101 0010 1x00 0001 0000 0z11 1001 0011
kq.xkey_sort 4 6 5 2 9 1 0 8
kq.xkey_rsort 6 4 5 2 9 1 8 0
wide_q.sort -16 -1 0 0 16 | 0 249 0 3 0
wide_u.rsort 8000000000000000000000001 8000000000000000000000000 1000000000000000000000000 0000000000000000000000005
wide_u.sort 0000000000000000000000005 1000000000000000000000000 8000000000000000000000000 8000000000000000000000001
q.rsort_index 40 30 20 10
q.sort_index 40 30 20 10
q.sort_index_mix 20 30 10 40
empty 0
single 7
";
    run_both_backends("order", expected);
}

#[test]
fn function_call_keys_order_queue_and_dynamic_array() {
    // The permutation i * 37 mod 100 has distinct keys; element v has key
    // (v % 10) * 10 + v / 10, so the element at sorted position k is its inverse.
    let ascending: Vec<u32> = (0..100).map(|key| (key % 10) * 10 + key / 10).collect();
    let join = |values: Vec<u32>| {
        values
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let expected = format!(
        "q.sort {}\nd.rsort {}\n",
        join(ascending.clone()),
        join(ascending.into_iter().rev().collect())
    );
    run_both_backends("key_function", &expected);
}

fn summary(label: &str, values: &[u64], descending: bool, key_order: bool) -> String {
    let checksum = values
        .iter()
        .fold(0u64, |sum, value| sum.wrapping_mul(31).wrapping_add(*value));
    let ordered = values.windows(2).all(|pair| {
        if key_order {
            pair[0] % 100 < pair[1] % 100 || (pair[0] % 100 == pair[1] % 100 && pair[0] <= pair[1])
        } else if descending {
            pair[0] >= pair[1]
        } else {
            pair[0] <= pair[1]
        }
    });
    format!(
        "{label} ordered={} first={} last={} checksum={checksum}\n",
        u8::from(ordered),
        values[0],
        values[values.len() - 1]
    )
}

fn next_value(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 33) % 1000
}

#[test]
fn twenty_thousand_element_sorts_finish_quickly() {
    let mut state = 12345u64;
    let mut values: Vec<u64> = (0..20000).map(|_| next_value(&mut state)).collect();
    let mut expected = String::new();
    values.sort_unstable();
    expected += &summary("q.sort", &values, false, false);
    values.reverse();
    expected += &summary("q.rsort", &values, true, false);

    let mut state = 777u64;
    let mut keyed: Vec<u64> = (0..20000u64)
        .map(|index| index * 1000 + next_value(&mut state) % 100)
        .collect();
    keyed.sort_by_key(|value| value % 100);
    expected += &summary("d.sort_key", &keyed, false, true);

    run_both_backends("large", &expected);
}
