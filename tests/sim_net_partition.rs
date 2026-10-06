//! Independent bit-string oracles for electrical range partitioning.
use crate::sim_cli;

fn bits(width: usize, bit: impl Fn(usize) -> char) -> String {
    (0..width).rev().map(bit).collect()
}

#[test]
fn electrical_ranges_preserve_values_and_hierarchy_in_both_modes() {
    let widths = [1, 7, 64, 65, 129];
    let mut expected = String::new();
    for width in widths {
        let zero = "0".repeat(width);
        let one = "1".repeat(width);
        let unknown = "x".repeat(width);
        let overlap = bits(width, |bit| match (bit >= width / 3, bit <= width / 2) {
            (true, true) => 'x',
            (true, false) => '0',
            (false, true) => '1',
            (false, false) => 'z',
        });
        let disjoint = bits(width, |bit| if bit >= width / 2 { '0' } else { '1' });
        expected.push_str(&format!("tb.w{width} {zero} {unknown} {overlap} {disjoint} {zero} {zero} {unknown} {zero} {one} {zero} {unknown}\n"));
    }
    for width in widths {
        let forced = bits(width, |bit| if bit == width / 2 { '1' } else { 'x' });
        expected.push_str(&format!("tb.w{width} bit {forced}\n"));
    }
    for width in widths {
        let forced = bits(width, |bit| if bit >= width / 2 { '0' } else { '1' });
        expected.push_str(&format!(
            "tb.w{width} range {forced} {}\n",
            "1".repeat(width)
        ));
    }
    for width in widths {
        expected.push_str(&format!("tb.w{width} release {0} {0}\n", "1".repeat(width)));
    }
    sim_cli::run_case_with_args(
        "net_partition",
        "ranges",
        &expected,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn wide_net_array_runtime_has_independent_expected_values() {
    sim_cli::run_case_with_args(
        "net_partition",
        "runtime",
        "WIDE_NET_RUNTIME_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn electrical_partition_keeps_alias_width_legality() {
    sim_cli::reject_case_with_args(
        "net_partition",
        "invalid_alias_width",
        "all aliased nets must have the same width",
        &["--edition", "sv2009"],
    );
}
