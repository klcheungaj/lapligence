use super::{reject_case, run_case};
use std::fmt::Write;

const NETS: &[&str] = &[
    "wire", "tri", "wand", "triand", "wor", "trior", "tri0", "tri1", "supply0", "supply1",
];
const WIRE: [&str; 4] = ["0xx0", "x1x1", "xxxx", "01xz"];
const WAND: [&str; 4] = ["0000", "01x1", "0xxx", "01xz"];
const WOR: [&str; 4] = ["01x0", "1111", "x1xx", "01xz"];

pub(super) fn resolve(kind: &str, drivers: &[usize]) -> char {
    let table = match kind {
        "wand" | "triand" => WAND,
        "wor" | "trior" => WOR,
        _ => WIRE,
    };
    let mut value = 3;
    for driver in drivers {
        value = match table[value].as_bytes()[*driver] {
            b'0' => 0,
            b'1' => 1,
            b'x' => 2,
            b'z' => 3,
            _ => unreachable!(),
        };
    }
    match kind {
        "supply0" => '0',
        "supply1" => '1',
        "tri0" if value == 3 => '0',
        "tri1" if value == 3 => '1',
        _ => ['0', '1', 'x', 'z'][value],
    }
}

fn net_truth_tables(width: usize) {
    let mut expected = String::new();
    for kind in NETS {
        writeln!(
            expected,
            "empty/{kind}={}",
            resolve(kind, &[]).to_string().repeat(width)
        )
        .unwrap();
    }
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for kind in NETS {
                    writeln!(
                        expected,
                        "{a}{b}{c}/{kind}={}",
                        resolve(kind, &[a, b, c]).to_string().repeat(width)
                    )
                    .unwrap();
                }
            }
        }
    }
    for kind in NETS {
        let bits: String = (0..width)
            .rev()
            .map(|bit| resolve(kind, &[bit % 4, (bit / 4) % 4, (bit / 16) % 4]))
            .collect();
        writeln!(expected, "mixed/{kind}={bits}").unwrap();
    }
    run_case(&format!("net-tables-{width}"), &expected);
}

#[test]
fn scalar_three_driver_truth_tables() {
    net_truth_tables(1);
}
#[test]
fn partial_limb_three_driver_truth_tables() {
    net_truth_tables(65);
}
#[test]
fn multi_limb_three_driver_truth_tables() {
    net_truth_tables(129);
}
#[test]
fn wide_three_driver_truth_tables() {
    net_truth_tables(2048);
}

#[test]
fn uwire_defaults_selects_declaration_drivers_and_release() {
    run_case("uwire", "PASS uwire\n");
}

#[test]
fn ordinary_net_defaults_survive_ports_interfaces_and_arrays() {
    run_case("net-storage-defaults", "PASS net defaults\n");
}

#[test]
fn ordinary_net_arrays_preserve_four_state_drivers() {
    run_case("net-array-drivers", "PASS net arrays\n");
}

#[test]
fn uwire_rejects_overlapping_drivers_but_accepts_disjoint_bits() {
    reject_case("uwire-overlapping", "cannot have multiple drivers");
    reject_case("uwire-multiple", "cannot have multiple drivers");
    run_case("uwire-disjoint", "10\n");
}

#[test]
fn unsupported_charge_storage_is_always_rejected() {
    for index in 0..5 {
        reject_case(&format!("trireg-{index}"), "trireg charge storage");
    }
}

#[test]
fn pending_gate_drivers_are_x_while_undriven_nets_are_z() {
    run_case("pending-gate-defaults", "PASS pending gates\n");
}

#[test]
fn explicit_net_datatypes_preserve_width_signedness_and_unknowns() {
    run_case("net-data-types", "PASS net datatypes\n");
}

#[test]
fn two_state_net_declarations_are_frontend_errors() {
    for ty in ["bit", "byte", "shortint", "int", "longint"] {
        reject_case(&format!("invalid-net-{ty}"), "not a valid type for a net");
    }
}
