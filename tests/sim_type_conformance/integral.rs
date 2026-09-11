use std::fmt::Write;

use super::run_case;

#[derive(Clone, Copy)]
struct IntegralType {
    width: usize,
    signed: bool,
    two_state: bool,
    explicit_assignment: bool,
}

const fn integral(width: usize, signed: bool, two_state: bool) -> IntegralType {
    IntegralType {
        width,
        signed,
        two_state,
        explicit_assignment: false,
    }
}

const TYPES: &[IntegralType] = &[
    integral(1, false, false),
    integral(1, false, false),
    integral(1, false, true),
    integral(8, true, false),
    integral(16, false, false),
    integral(8, true, true),
    integral(8, true, true),
    integral(8, false, true),
    integral(16, true, true),
    integral(16, false, true),
    integral(32, true, true),
    integral(32, false, true),
    integral(64, true, true),
    integral(64, false, true),
    integral(32, true, false),
    integral(32, false, false),
    integral(64, false, false),
    integral(64, true, false),
    integral(65, true, false),
    IntegralType {
        explicit_assignment: true,
        ..integral(8, true, false)
    },
    IntegralType {
        explicit_assignment: true,
        ..integral(16, false, true)
    },
    integral(8, true, false),
    integral(16, false, true),
];

fn mask(width: usize) -> u128 {
    (1u128 << width) - 1
}

fn signed_value(bits: u128, width: usize, signed: bool) -> i128 {
    if signed && bits & (1 << (width - 1)) != 0 {
        bits as i128 - (1i128 << width)
    } else {
        bits as i128
    }
}

fn binary(bits: u128, width: usize) -> String {
    format!("{:0width$b}", bits & mask(width))
}

fn sample(ty: IntegralType, iteration: usize) -> u128 {
    match iteration {
        0 => mask(ty.width).wrapping_sub(2) & mask(ty.width),
        1 => (1 << (ty.width - 1)) | (5 & mask(ty.width)),
        _ => 0,
    }
}

// §§11.6–11.8: binary integral operands share the maximum width; a single
// unsigned operand makes both operands unsigned before widening.
fn binary_expected(left: IntegralType, right: IntegralType, a: u128, b: u128, op: &str) -> String {
    let width = left.width.max(right.width);
    let signed = left.signed && right.signed;
    let a = signed_value(a, left.width, signed);
    let b = signed_value(b, right.width, signed);
    let bits = match op {
        "+" => a.wrapping_add(b) as u128,
        "-" => a.wrapping_sub(b) as u128,
        "*" => a.wrapping_mul(b) as u128,
        "/" if b != 0 => (a / b) as u128,
        "%" if b != 0 => (a % b) as u128,
        "/" | "%" => return "x".repeat(width),
        "&" => (a & b) as u128,
        "|" => (a | b) as u128,
        "^" => (a ^ b) as u128,
        "==" | "===" => return u8::from(a == b).to_string(),
        "!=" | "!==" => return u8::from(a != b).to_string(),
        "<" => return u8::from(a < b).to_string(),
        "<=" => return u8::from(a <= b).to_string(),
        ">" => return u8::from(a > b).to_string(),
        ">=" => return u8::from(a >= b).to_string(),
        "&&" => return u8::from(a != 0 && b != 0).to_string(),
        "||" => return u8::from(a != 0 || b != 0).to_string(),
        _ => panic!("missing independent operator oracle: {op}"),
    };
    binary(bits, width)
}

fn mixed_operations(left_index: usize) {
    let left = TYPES[left_index];
    let mut expected = String::new();
    for (right_index, right) in TYPES.iter().copied().enumerate() {
        for iteration in 0..3 {
            let a = sample(left, iteration);
            let b = sample(right, (iteration + 1) % 3);
            for op in [
                "+", "-", "*", "/", "%", "&", "|", "^", "==", "!=", "===", "!==", "<", "<=", ">",
                ">=", "&&", "||",
            ] {
                let label = format!(
                    "{left_index}/{right_index}/{iteration}/{}",
                    op.replace('%', "mod")
                );
                writeln!(
                    expected,
                    "{label}={}",
                    binary_expected(left, right, a, b, op)
                )
                .unwrap();
            }
        }
    }
    run_case(&format!("mixed-types-{left_index}"), &expected);
}

macro_rules! operation_cases {
    ($($name:ident: $index:expr),* $(,)?) => { $(
        #[test] fn $name() { mixed_operations($index); }
    )* };
}

operation_cases! {
    reg_operations: 0, logic_operations: 1, bit_operations: 2,
    signed_reg_operations: 3, logic_vector_operations: 4, signed_bit_operations: 5,
    byte_operations: 6, unsigned_byte_operations: 7,
    shortint_operations: 8, unsigned_shortint_operations: 9,
    int_operations: 10, unsigned_int_operations: 11,
    longint_operations: 12, unsigned_longint_operations: 13,
    integer_operations: 14, unsigned_integer_operations: 15,
    time_operations: 16, signed_time_operations: 17,
    wide_signed_operations: 18, four_state_enum_operations: 19,
    two_state_enum_operations: 20, packed_struct_operations: 21,
    packed_union_operations: 22,
}

// §§6.11.2, 6.24.1, 10.8: retain low bits, extend according to the source's
// sign, then discard X/Z in two-state destinations. Bit strings avoid sharing
// either the runtime limb layout or its conversion implementation.
fn converted(source: &str, signed: bool, target: IntegralType) -> String {
    let mut bits = if source.len() >= target.width {
        source[source.len() - target.width..].to_owned()
    } else {
        let fill = if signed {
            source.as_bytes()[0] as char
        } else {
            '0'
        };
        fill.to_string().repeat(target.width - source.len()) + source
    };
    if target.two_state {
        bits = bits.replace(['x', 'z'], "0");
    }
    bits
}

fn conversion_matrix(width: usize) {
    let mut expected = String::new();
    for (index, target) in TYPES.iter().enumerate() {
        writeln!(
            expected,
            "default/{index}={}",
            if target.two_state { "0" } else { "x" }.repeat(target.width)
        )
        .unwrap();
    }
    for (pattern_index, pattern) in ["x", "z", "10xz01zx", "x10z01", "z01x10", "011001"]
        .iter()
        .enumerate()
    {
        let bits: String = pattern.chars().cycle().take(width).collect();
        for signed in [false, true] {
            for (index, target) in TYPES.iter().copied().enumerate() {
                let label = format!("{pattern_index}/{signed}/{index}");
                let result = converted(&bits, signed, target);
                writeln!(expected, "cast/{label}={result}").unwrap();
                let fill = if target.signed { &result[..1] } else { "0" };
                writeln!(
                    expected,
                    "shift/{label}={fill}{}",
                    &result[..result.len() - 1]
                )
                .unwrap();
                if !target.explicit_assignment {
                    writeln!(expected, "assign/{label}={result}").unwrap();
                }
            }
        }
    }
    run_case(&format!("conversions-{width}"), &expected);
}

macro_rules! conversion_cases {
    ($($name:ident: $width:expr),* $(,)?) => { $(
        #[test] fn $name() { conversion_matrix($width); }
    )* };
}

conversion_cases! {
    conversions_1: 1, conversions_7: 7, conversions_8: 8, conversions_9: 9,
    conversions_31: 31, conversions_32: 32, conversions_33: 33,
    conversions_63: 63, conversions_64: 64, conversions_65: 65,
    conversions_127: 127, conversions_129: 129,
}

fn truth_index(bit: u8) -> usize {
    match bit {
        b'0' => 0,
        b'1' => 1,
        b'x' => 2,
        b'z' => 3,
        _ => panic!("invalid oracle bit"),
    }
}

fn truth_value(bits: &str) -> usize {
    if bits.contains('1') {
        1
    } else if bits.contains(['x', 'z']) {
        2
    } else {
        0
    }
}

// IEEE 1800-2009 §§11.4.5, 11.4.7–11.4.8. Z is unknown in
// operators, but remains distinct from X in case equality and copying.
fn state_operation(a: &str, b: &str, op: &str) -> String {
    const AND: [&[u8; 4]; 4] = [b"0000", b"01xx", b"0xxx", b"0xxx"];
    const OR: [&[u8; 4]; 4] = [b"01xx", b"1111", b"x1xx", b"x1xx"];
    const XOR: [&[u8; 4]; 4] = [b"01xx", b"10xx", b"xxxx", b"xxxx"];
    match op {
        "&" | "|" | "^" => {
            let table = match op {
                "&" => AND,
                "|" => OR,
                _ => XOR,
            };
            a.bytes()
                .zip(b.bytes())
                .map(|(a, b)| table[truth_index(a)][truth_index(b)] as char)
                .collect()
        }
        "&&" | "||" => {
            let table = if op == "&&" { AND } else { OR };
            (table[truth_value(a)][truth_value(b)] as char).to_string()
        }
        "===" | "!==" => u8::from((a == b) == (op == "===")).to_string(),
        "==" | "!=" => {
            let mismatch = a
                .bytes()
                .zip(b.bytes())
                .any(|(a, b)| a <= b'1' && b <= b'1' && a != b);
            if !mismatch && (a.contains(['x', 'z']) || b.contains(['x', 'z'])) {
                "x".to_owned()
            } else {
                u8::from(mismatch == (op == "!=")).to_string()
            }
        }
        "+" => {
            if a.contains(['x', 'z']) || b.contains(['x', 'z']) {
                "x".repeat(a.len())
            } else {
                binary(
                    u128::from_str_radix(a, 2).unwrap() + u128::from_str_radix(b, 2).unwrap(),
                    a.len(),
                )
            }
        }
        _ => panic!("missing four-state oracle for {op}"),
    }
}

fn mixed_net_operations(kind: &str) {
    let operators = ["&", "|", "^", "&&", "||", "==", "!=", "===", "!==", "+"];
    let mut expected = String::new();
    for (a, state) in ['0', '1', 'x', 'z'].into_iter().enumerate() {
        for b in 0..4 {
            let net_bits = super::nets::resolve(kind, &[b]).to_string().repeat(65);
            for (index, ty) in TYPES.iter().copied().enumerate() {
                let bit = if ty.two_state && matches!(state, 'x' | 'z') {
                    '0'
                } else {
                    state
                };
                // The unsigned net makes the common expression unsigned;
                // the narrower variable is zero-extended even when signed.
                let variable_bits = "0".repeat(65 - ty.width) + &bit.to_string().repeat(ty.width);
                for op in operators {
                    writeln!(
                        expected,
                        "{a}/{b}/{index}/{op}={}",
                        state_operation(&variable_bits, &net_bits, op)
                    )
                    .unwrap();
                }
                writeln!(
                    expected,
                    "{a}/{b}/{index}/cast={}",
                    converted(&net_bits, false, ty)
                )
                .unwrap();
            }
        }
    }
    run_case(&format!("data-net-{kind}"), &expected);
}

macro_rules! net_operation_cases {
    ($($name:ident: $kind:literal),* $(,)?) => { $(
        #[test] fn $name() { mixed_net_operations($kind); }
    )* };
}

net_operation_cases! {
    data_with_wire: "wire", data_with_tri: "tri", data_with_uwire: "uwire",
    data_with_wand: "wand", data_with_triand: "triand", data_with_wor: "wor",
    data_with_trior: "trior", data_with_tri0: "tri0", data_with_tri1: "tri1",
    data_with_supply0: "supply0", data_with_supply1: "supply1",
}
