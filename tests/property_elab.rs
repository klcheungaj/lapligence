//! Property tests for `core::elab` 4-state value math.
//!
//! The properties pin `elab::Value` semantics against naive per-bit reference
//! implementations.  Every property is deterministic: inputs are drawn from
//! proptest strategies through a fixed-seed `TestRunner`, and each property is
//! executed twice over the same generated inputs, asserting that both runs
//! produce identical result fingerprints.
//!
//! The C runtime twin (`sv4_t` in `src/sim/rt/llg_rt.c`) is cross-checked
//! against the same `elab::Value` semantics by the deterministic vector table
//! in `src/sim/rt/llg_rt_selftest.c`.  Regenerate that table with:
//!
//! ```text
//! cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
//! ```
//!
//! then replace the `VECTORS[]` body in `llg_rt_selftest.c`.

use llg::core::elab::{
    add, bit_and, bit_or, bit_xor, casex_eq, casez_eq, concat, cond, div, eq, ge, gt, le, lt, mul,
    neq, power, rem, shl, shr, sub, Bit, Value,
};
use proptest::prelude::*;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

// ── Deterministic proptest driver ─────────────────────────────────────────────

/// Fixed base seed; each property derives its own seed from this.
const SEED: [u8; 32] = [
    0x5e, 0xed, 0x51, 0x7c, 0x9c, 0x8f, 0x2a, 0x6b, //
    0x3d, 0x4e, 0x1a, 0x2b, 0x7f, 0x0c, 0x9d, 0x3e, //
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, //
    0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01,
];

fn per_op_seed(name: &str) -> [u8; 32] {
    let mut seed = SEED;
    for (i, b) in name.bytes().enumerate() {
        seed[i % 32] = seed[i % 32].wrapping_add(b);
    }
    seed
}

/// Run `check` over `cases` inputs drawn from `strategy` with a fixed seed,
/// returning one fingerprint string per case.  A second run with the same seed
/// sees the same inputs, so a property asserts determinism by comparing two
/// runs' fingerprints.
fn run_prop<A: Strategy>(
    seed: [u8; 32],
    cases: u32,
    strategy: &A,
    mut check: impl FnMut(A::Value) -> String,
) -> Vec<String> {
    let config = Config {
        cases,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &seed));
    let mut out = Vec::with_capacity(cases as usize);
    for _ in 0..cases {
        let tree = strategy
            .new_tree(&mut runner)
            .expect("strategy must generate a value");
        out.push(check(tree.current()));
    }
    out
}

fn assert_deterministic<A: Strategy>(
    name: &str,
    seed: [u8; 32],
    cases: u32,
    strategy: A,
    mut check: impl FnMut(A::Value) -> String,
) {
    let once = run_prop(seed, cases, &strategy, &mut check);
    let twice = run_prop(seed, cases, &strategy, &mut check);
    assert_eq!(once.len(), cases as usize);
    assert_eq!(twice.len(), cases as usize);
    assert_eq!(once, twice, "{name}: same-seed rerun must be deterministic");
}

// ── Strategies ────────────────────────────────────────────────────────────────

fn any_bit() -> impl Strategy<Value = Bit> {
    prop_oneof![Just(Bit::Zero), Just(Bit::One), Just(Bit::X), Just(Bit::Z)]
}

fn known_bit() -> impl Strategy<Value = Bit> {
    prop_oneof![Just(Bit::Zero), Just(Bit::One)]
}

/// Random 4-state value, width 1..=64, random signedness.
fn random_value_strategy() -> impl Strategy<Value = Value> {
    (1usize..=64, any::<bool>()).prop_flat_map(|(w, signed)| {
        prop::collection::vec(any_bit(), w).prop_map(move |bits| Value::from_bits(bits, signed))
    })
}

/// Known value, always signed (for signed-compare consistency).
fn signed_known_value_strategy() -> impl Strategy<Value = Value> {
    (1usize..=64).prop_flat_map(|w| {
        prop::collection::vec(known_bit(), w).prop_map(|bits| Value::from_bits(bits, true))
    })
}

/// Small random 4-state value, width 1..=8 (casez/casex truth tables).
fn small_value_strategy() -> impl Strategy<Value = Value> {
    (1usize..=8).prop_flat_map(|w| {
        prop::collection::vec(any_bit(), w).prop_map(|bits| Value::from_bits(bits, false))
    })
}

// ── X-propagation monotonicity ────────────────────────────────────────────────
//
// Word-level arithmetic (add/sub/mul) produces all-X as soon as either
// operand has an X or Z bit (LRM 11.4.5: Z behaves as X in expression
// contexts).  Shifts are different: only an unknown shift amount produces
// all-X; unknown LHS bits are shifted positionally and covered by focused
// unit/runtime vectors below.
type BinaryValueOp = fn(&Value, &Value) -> Value;

const ALL_X_OPS: [(&str, BinaryValueOp); 3] = [("add", add), ("sub", sub), ("mul", mul)];

fn check_all_x(name: &str, op: fn(&Value, &Value) -> Value, a: &Value, b: &Value) -> String {
    let r = op(a, b);
    if a.is_unknown() || b.is_unknown() {
        assert!(
            r.bits.iter().all(|&b| b == Bit::X),
            "{name}: op with an X/Z operand must yield all-X; a={a:?} b={b:?} r={r:?}"
        );
    } else {
        assert!(
            !r.is_unknown(),
            "{name}: fully known operands must yield a known result; a={a:?} b={b:?} r={r:?}"
        );
    }
    format!("{:?}", r.bits)
}

#[test]
fn prop_x_propagation_all_x() {
    for (name, op) in ALL_X_OPS {
        let strategy = (random_value_strategy(), random_value_strategy());
        let seed = per_op_seed(name);
        assert_deterministic(name, seed, 256, strategy, move |(a, b)| {
            check_all_x(name, op, &a, &b)
        });
    }
}

// ── bitwise XOR: X/Z propagates per bit ───────────────────────────────────────
//
// XOR is a per-bit op: a result bit is X only where an operand bit is X/Z
// (after resize to max width); known bits XOR normally.

fn check_bit_xor(a: &Value, b: &Value) -> String {
    let r = bit_xor(a, b);
    let w = a.width().max(b.width());
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    for i in 0..w {
        let (x, y) = (ra.bit_lsb(i), rb.bit_lsb(i));
        let exp = match (x, y) {
            (Bit::X, _) | (Bit::Z, _) | (_, Bit::X) | (_, Bit::Z) => Bit::X,
            (Bit::One, Bit::One) | (Bit::Zero, Bit::Zero) => Bit::Zero,
            _ => Bit::One,
        };
        assert_eq!(
            r.bit_lsb(i),
            exp,
            "bit_xor bit {i}: a={a:?} b={b:?} r={r:?}"
        );
    }
    format!("{:?}", r.bits)
}

#[test]
fn prop_bit_xor_x_propagation() {
    let strategy = (random_value_strategy(), random_value_strategy());
    assert_deterministic("xor", per_op_seed("xor"), 256, strategy, |(a, b)| {
        check_bit_xor(&a, &b)
    });
}

// ── bitwise AND/OR: X/Z propagate unless dominated ────────────────────────────
//
// AND/OR are per-bit ops with LRM domination rules (0 dominates AND, 1
// dominates OR).  The properties compare against a per-bit reference.

fn check_bit_and(a: &Value, b: &Value) -> String {
    let r = bit_and(a, b);
    let w = a.width().max(b.width());
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    for i in 0..w {
        let (x, y) = (ra.bit_lsb(i), rb.bit_lsb(i));
        let exp = match (x, y) {
            (Bit::Zero, _) | (_, Bit::Zero) => Bit::Zero,
            (Bit::One, Bit::One) => Bit::One,
            _ => Bit::X,
        };
        assert_eq!(
            r.bit_lsb(i),
            exp,
            "bit_and bit {i}: a={a:?} b={b:?} r={r:?}"
        );
    }
    format!("{:?}", r.bits)
}

#[test]
fn prop_bit_and_x_domination() {
    let strategy = (random_value_strategy(), random_value_strategy());
    assert_deterministic("and", per_op_seed("and"), 256, strategy, |(a, b)| {
        check_bit_and(&a, &b)
    });
}

fn check_bit_or(a: &Value, b: &Value) -> String {
    let r = bit_or(a, b);
    let w = a.width().max(b.width());
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    for i in 0..w {
        let (x, y) = (ra.bit_lsb(i), rb.bit_lsb(i));
        let exp = match (x, y) {
            (Bit::One, _) | (_, Bit::One) => Bit::One,
            (Bit::Zero, Bit::Zero) => Bit::Zero,
            _ => Bit::X,
        };
        assert_eq!(r.bit_lsb(i), exp, "bit_or bit {i}: a={a:?} b={b:?} r={r:?}");
    }
    format!("{:?}", r.bits)
}

#[test]
fn prop_bit_or_x_domination() {
    let strategy = (random_value_strategy(), random_value_strategy());
    assert_deterministic("or", per_op_seed("or"), 256, strategy, |(a, b)| {
        check_bit_or(&a, &b)
    });
}

// ── Resize preserves low bits ─────────────────────────────────────────────────

fn resize_strategy() -> impl Strategy<Value = (Value, usize, bool)> {
    (1usize..=64, any::<bool>()).prop_flat_map(|(w, signed)| {
        (
            prop::collection::vec(known_bit(), w)
                .prop_map(move |bits| Value::from_bits(bits, signed)),
            w..=64,
            any::<bool>(),
        )
    })
}

fn check_resize(v: &Value, new_w: usize, new_s: bool) -> String {
    assert!(new_w >= v.width());
    let r = v.resize(new_w, new_s);
    for i in 0..v.width() {
        assert_eq!(
            r.bit_lsb(i),
            v.bit_lsb(i),
            "resize must preserve low bit {i}: v={v:?} r={r:?}"
        );
    }
    assert!(
        !r.is_unknown(),
        "known value resized must stay known; v={v:?} r={r:?}"
    );
    format!("{:?}", r.bits)
}

#[test]
fn prop_resize_preserves_low_bits() {
    assert_deterministic(
        "resize",
        per_op_seed("resize"),
        256,
        resize_strategy(),
        |(v, new_w, new_s)| check_resize(&v, new_w, new_s),
    );
}

// ── Concat/split round-trip ───────────────────────────────────────────────────

fn concat_split_strategy() -> impl Strategy<Value = (Value, Value)> {
    (1usize..=32, 1usize..=32).prop_flat_map(|(hw, lw)| {
        (
            prop::collection::vec(any_bit(), hw).prop_map(|bits| Value::from_bits(bits, false)),
            prop::collection::vec(any_bit(), lw).prop_map(|bits| Value::from_bits(bits, false)),
        )
    })
}

/// MSB-first part select `v[left..=right]` with `left >= right`.
fn part_select(v: &Value, left: usize, right: usize) -> Value {
    let mut bits = Vec::new();
    for i in (right..=left).rev() {
        bits.push(v.bit_lsb(i));
    }
    Value::from_bits(bits, false)
}

fn check_concat_split(hi: &Value, lo: &Value) -> String {
    let c = concat(&[hi.clone(), lo.clone()]);
    assert_eq!(c.width(), hi.width() + lo.width());
    let hi_sel = part_select(&c, hi.width() + lo.width() - 1, lo.width());
    let lo_sel = part_select(&c, lo.width() - 1, 0);
    assert_eq!(
        hi_sel.bits, hi.bits,
        "hi split: hi={hi:?} lo={lo:?} c={c:?}"
    );
    assert_eq!(
        lo_sel.bits, lo.bits,
        "lo split: hi={hi:?} lo={lo:?} c={c:?}"
    );
    format!("{:?}", c.bits)
}

#[test]
fn prop_concat_split_round_trip() {
    assert_deterministic(
        "concat",
        per_op_seed("concat"),
        256,
        concat_split_strategy(),
        |(hi, lo)| check_concat_split(&hi, &lo),
    );
}

// ── casez/casex wildcard truth tables ─────────────────────────────────────────

fn naive_casez(sel: &Value, item: &Value) -> bool {
    let w = sel.width().max(item.width());
    let rs = sel.resize(w, false);
    let ri = item.resize(w, false);
    for i in 0..w {
        let ib = ri.bit_lsb(i);
        if ib == Bit::Z {
            continue; // item z/? -> don't-care
        }
        let sb = rs.bit_lsb(i);
        if ib == Bit::X {
            if sb != Bit::X {
                return false; // item x matches selector x only
            }
        } else if sb != ib {
            return false; // known item: selector must equal it
        }
    }
    true
}

fn naive_casex(sel: &Value, item: &Value) -> bool {
    let w = sel.width().max(item.width());
    let rs = sel.resize(w, false);
    let ri = item.resize(w, false);
    for i in 0..w {
        let ib = ri.bit_lsb(i);
        if matches!(ib, Bit::X | Bit::Z) {
            continue; // item x/z -> don't-care
        }
        let sb = rs.bit_lsb(i);
        if sb == ib {
            continue; // equal known bits match
        }
        if matches!(sb, Bit::X | Bit::Z) {
            continue; // selector x/z is a don't-care in casex
        }
        return false; // opposite known bit -> no match
    }
    true
}

fn check_casez(sel: &Value, item: &Value) -> String {
    let r = casez_eq(sel, item);
    assert_eq!(
        r.width(),
        1,
        "casez_eq must be 1 bit: sel={sel:?} item={item:?}"
    );
    assert!(
        !r.is_unknown(),
        "casez_eq never X: sel={sel:?} item={item:?}"
    );
    assert_eq!(
        r.to_u64(),
        Some(naive_casez(sel, item) as u64),
        "casez_eq({sel:?}, {item:?})"
    );
    format!("{:?}", r.bits)
}

#[test]
fn prop_casez_matches_naive_reference() {
    let strategy = (small_value_strategy(), small_value_strategy());
    assert_deterministic("casez", per_op_seed("casez"), 256, strategy, |(s, i)| {
        check_casez(&s, &i)
    });
}

fn check_casex(sel: &Value, item: &Value) -> String {
    let r = casex_eq(sel, item);
    assert_eq!(
        r.width(),
        1,
        "casex_eq must be 1 bit: sel={sel:?} item={item:?}"
    );
    assert!(
        !r.is_unknown(),
        "casex_eq never X: sel={sel:?} item={item:?}"
    );
    assert_eq!(
        r.to_u64(),
        Some(naive_casex(sel, item) as u64),
        "casex_eq({sel:?}, {item:?})"
    );
    format!("{:?}", r.bits)
}

#[test]
fn prop_casex_matches_naive_reference() {
    let strategy = (small_value_strategy(), small_value_strategy());
    assert_deterministic("casex", per_op_seed("casex"), 256, strategy, |(s, i)| {
        check_casex(&s, &i)
    });
}

// ── Signed compare consistency ────────────────────────────────────────────────

fn check_signed_compare(a: &Value, b: &Value) -> String {
    let (ia, ib) = (a.to_i64().unwrap(), b.to_i64().unwrap());
    assert_eq!(
        lt(a, b).to_u64(),
        Some((ia < ib) as u64),
        "lt: a={a:?} b={b:?}"
    );
    assert_eq!(
        le(a, b).to_u64(),
        Some((ia <= ib) as u64),
        "le: a={a:?} b={b:?}"
    );
    assert_eq!(
        gt(a, b).to_u64(),
        Some((ia > ib) as u64),
        "gt: a={a:?} b={b:?}"
    );
    assert_eq!(
        ge(a, b).to_u64(),
        Some((ia >= ib) as u64),
        "ge: a={a:?} b={b:?}"
    );
    assert_eq!(
        eq(a, b).to_u64(),
        Some((ia == ib) as u64),
        "eq: a={a:?} b={b:?}"
    );
    assert_eq!(
        neq(a, b).to_u64(),
        Some((ia != ib) as u64),
        "neq: a={a:?} b={b:?}"
    );
    format!("{:?}", lt(a, b).bits)
}

#[test]
fn prop_signed_compare_consistency() {
    let strategy = (signed_known_value_strategy(), signed_known_value_strategy());
    assert_deterministic(
        "cmp-signed",
        per_op_seed("cmp-signed"),
        256,
        strategy,
        |(a, b)| check_signed_compare(&a, &b),
    );
}

// ── C vector-table generator (ignored; run manually) ──────────────────────────
//
// Regenerate `VECTORS[]` in src/sim/rt/llg_rt_selftest.c with:
//   cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc

#[derive(Clone, Copy)]
struct Masks {
    bits: u64,
    x: u64,
    z: u64,
    width: u16,
    signed: i8,
}

const EMPTY: Masks = Masks {
    bits: 0,
    x: 0,
    z: 0,
    width: 0,
    signed: 0,
};

struct VecSpec {
    op: &'static str,
    a: Masks,
    b: Masks,
    c: Masks,
    e: Masks,
}

impl VecSpec {
    fn new(op: &'static str, a: Masks, b: Masks, c: Masks) -> VecSpec {
        let e = masks_of(&expected_value(op, &a, &b, &c));
        VecSpec { op, a, b, c, e }
    }

    fn to_c(&self) -> String {
        format!(
            "    {{ {}, 0x{:x}ULL, 0x{:x}ULL, 0x{:x}ULL, {}, {}, 0x{:x}ULL, 0x{:x}ULL, 0x{:x}ULL, {}, {}, 0x{:x}ULL, 0x{:x}ULL, 0x{:x}ULL, {}, {}, 0x{:x}ULL, 0x{:x}ULL, 0x{:x}ULL, {}, {} }},",
            self.op,
            self.a.bits, self.a.x, self.a.z, self.a.width, self.a.signed,
            self.b.bits, self.b.x, self.b.z, self.b.width, self.b.signed,
            self.c.bits, self.c.x, self.c.z, self.c.width, self.c.signed,
            self.e.bits, self.e.x, self.e.z, self.e.width, self.e.signed,
        )
    }
}

fn all_mask(width: u16) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// MSB-first 0/1/x/z string -> unsigned masks.
fn bstr(s: &str) -> Masks {
    let mut bits = 0u64;
    let mut x = 0u64;
    let mut z = 0u64;
    let n = s.len();
    for (i, c) in s.chars().enumerate() {
        let lsb = n - 1 - i;
        match c {
            '0' => {}
            '1' => bits |= 1u64 << lsb,
            'x' | 'X' => x |= 1u64 << lsb,
            'z' | 'Z' => z |= 1u64 << lsb,
            _ => panic!("bad test bit: {c}"),
        }
    }
    Masks {
        bits,
        x,
        z,
        width: n as u16,
        signed: 0,
    }
}

/// `bstr` with signedness set.
fn sstr(s: &str) -> Masks {
    let mut m = bstr(s);
    m.signed = 1;
    m
}

/// Known unsigned value from raw bits.
fn um(v: u64, width: u16) -> Masks {
    Masks {
        bits: v & all_mask(width),
        x: 0,
        z: 0,
        width,
        signed: 0,
    }
}

/// Known value from raw bits with signedness set.
fn sm(v: u64, width: u16) -> Masks {
    Masks {
        bits: v & all_mask(width),
        x: 0,
        z: 0,
        width,
        signed: 1,
    }
}

/// Resize target: width + signedness packed into the `c` operand.
fn resize_target(width: u16, signed: bool) -> Masks {
    Masks {
        bits: 0,
        x: 0,
        z: 0,
        width,
        signed: signed as i8,
    }
}

fn value_of(m: &Masks) -> Value {
    let mut bits = Vec::with_capacity(m.width as usize);
    for lsb in (0..m.width).rev() {
        let b = if m.x & (1u64 << lsb) != 0 {
            Bit::X
        } else if m.z & (1u64 << lsb) != 0 {
            Bit::Z
        } else if m.bits & (1u64 << lsb) != 0 {
            Bit::One
        } else {
            Bit::Zero
        };
        bits.push(b);
    }
    Value::from_bits(bits, m.signed != 0)
}

fn masks_of(v: &Value) -> Masks {
    let mut bits = 0u64;
    let mut x = 0u64;
    let mut z = 0u64;
    for i in 0..v.width() {
        match v.bit_lsb(i) {
            Bit::One => bits |= 1u64 << i,
            Bit::X => x |= 1u64 << i,
            Bit::Z => z |= 1u64 << i,
            Bit::Zero => {}
        }
    }
    Masks {
        bits,
        x,
        z,
        width: v.width() as u16,
        signed: v.signed as i8,
    }
}

fn expected_value(op: &str, a: &Masks, b: &Masks, c: &Masks) -> Value {
    match op {
        "V_ADD" => add(&value_of(a), &value_of(b)),
        "V_SUB" => sub(&value_of(a), &value_of(b)),
        "V_MUL" => mul(&value_of(a), &value_of(b)),
        "V_DIV" => div(&value_of(a), &value_of(b)),
        "V_MOD" => rem(&value_of(a), &value_of(b)),
        "V_POW" => power(&value_of(a), &value_of(b)),
        "V_SHL" => shl(&value_of(a), &value_of(b)),
        "V_SHR" => shr(&value_of(a), &value_of(b)),
        "V_EQ" => eq(&value_of(a), &value_of(b)),
        "V_LT" => lt(&value_of(a), &value_of(b)),
        "V_MUX" => cond(&value_of(c), &value_of(a), &value_of(b)),
        "V_RESIZE" => value_of(a).resize(c.width as usize, c.signed != 0),
        "V_CAST" => value_of(a).cast(c.width as usize, c.signed != 0),
        "V_CONCAT" => concat(&[value_of(a), value_of(b)]),
        "V_CASEZ" => casez_eq(&value_of(a), &value_of(b)),
        "V_CASEX" => casex_eq(&value_of(a), &value_of(b)),
        other => panic!("unknown op {other}"),
    }
}

fn vector_specs() -> Vec<VecSpec> {
    vec![
        // add
        VecSpec::new("V_ADD", bstr("1001"), bstr("0001"), EMPTY),
        VecSpec::new("V_ADD", bstr("10x1"), bstr("0001"), EMPTY),
        VecSpec::new("V_ADD", sstr("1111"), sstr("0001"), EMPTY),
        VecSpec::new("V_ADD", bstr("11111111"), bstr("00000001"), EMPTY),
        VecSpec::new("V_ADD", bstr("1111"), bstr("00000001"), EMPTY),
        // sub
        VecSpec::new("V_SUB", bstr("1010"), bstr("0011"), EMPTY),
        VecSpec::new("V_SUB", bstr("00000001"), bstr("00000010"), EMPTY),
        VecSpec::new("V_SUB", sstr("00000001"), sstr("00000010"), EMPTY),
        VecSpec::new("V_SUB", bstr("1010"), bstr("10x1"), EMPTY),
        // mul
        VecSpec::new("V_MUL", bstr("0110"), bstr("0011"), EMPTY),
        VecSpec::new("V_MUL", bstr("00000010"), bstr("00000011"), EMPTY),
        VecSpec::new("V_MUL", bstr("x0000000"), bstr("00000011"), EMPTY),
        VecSpec::new("V_MUL", bstr("z0000000"), bstr("00000011"), EMPTY),
        VecSpec::new("V_MUL", sstr("11111111"), sstr("00000010"), EMPTY),
        // Arithmetic operands are coerced to max width with signedness only
        // when both operands are signed.
        VecSpec::new("V_ADD", sstr("1111"), sstr("00000001"), EMPTY),
        VecSpec::new("V_ADD", sstr("1111"), bstr("00000001"), EMPTY),
        VecSpec::new("V_SUB", sstr("1111"), sstr("00000001"), EMPTY),
        VecSpec::new("V_SUB", sstr("1111"), bstr("00000001"), EMPTY),
        VecSpec::new("V_MUL", sstr("1111"), sstr("00000001"), EMPTY),
        VecSpec::new("V_MUL", sstr("1111"), bstr("00000001"), EMPTY),
        // Div/mod vectors avoid zero divisors and include the INT64_MIN edge.
        VecSpec::new("V_DIV", sstr("1111"), sstr("00000001"), EMPTY),
        VecSpec::new("V_DIV", sstr("1111"), bstr("00000001"), EMPTY),
        VecSpec::new("V_MOD", sstr("1111"), sstr("00000001"), EMPTY),
        VecSpec::new("V_MOD", sstr("1111"), bstr("00000001"), EMPTY),
        // Power keeps the base width/signedness; the exponent is self-determined.
        VecSpec::new("V_POW", um(3, 4), um(2, 8), EMPTY),
        VecSpec::new("V_POW", sm((-2i64) as u64, 4), um(3, 8), EMPTY),
        VecSpec::new("V_POW", sm(2, 4), sm((-1i64) as u64, 8), EMPTY),
        VecSpec::new(
            "V_DIV",
            sm(i64::MIN as u64, 64),
            sm((-1i64) as u64, 64),
            EMPTY,
        ),
        VecSpec::new(
            "V_MOD",
            sm(i64::MIN as u64, 64),
            sm((-1i64) as u64, 64),
            EMPTY,
        ),
        // shl
        VecSpec::new("V_SHL", bstr("0001"), bstr("0010"), EMPTY),
        VecSpec::new("V_SHL", bstr("0001"), bstr("0100"), EMPTY),
        VecSpec::new("V_SHL", bstr("00000001"), bstr("00001000"), EMPTY),
        VecSpec::new("V_SHL", bstr("x001"), bstr("0001"), EMPTY),
        VecSpec::new("V_SHL", bstr("0001"), bstr("x001"), EMPTY),
        // shr
        VecSpec::new("V_SHR", bstr("10000000"), bstr("00000001"), EMPTY),
        VecSpec::new("V_SHR", bstr("10000000"), bstr("00001000"), EMPTY),
        VecSpec::new("V_SHR", bstr("10000000"), bstr("x0000001"), EMPTY),
        // eq
        VecSpec::new("V_EQ", bstr("00000101"), bstr("00000101"), EMPTY),
        VecSpec::new("V_EQ", bstr("00000101"), bstr("00000110"), EMPTY),
        VecSpec::new("V_EQ", bstr("00000101"), bstr("00000x01"), EMPTY),
        VecSpec::new("V_EQ", bstr("0101"), bstr("00000101"), EMPTY),
        // lt
        VecSpec::new("V_LT", bstr("00000011"), bstr("00000101"), EMPTY),
        VecSpec::new("V_LT", bstr("00000101"), bstr("00000011"), EMPTY),
        VecSpec::new("V_LT", sstr("11111111"), sstr("00000000"), EMPTY),
        VecSpec::new("V_LT", sstr("00000001"), sstr("11111111"), EMPTY),
        VecSpec::new("V_LT", sstr("11111111"), bstr("00000000"), EMPTY),
        VecSpec::new("V_LT", sstr("11111111"), bstr("x0000000"), EMPTY),
        // 64-bit unsigned regression: u64::MAX < 1 is false
        VecSpec::new("V_LT", um(u64::MAX, 64), um(1, 64), EMPTY),
        // mux (c = selector)
        VecSpec::new("V_MUX", bstr("1010"), bstr("0101"), bstr("1")),
        VecSpec::new("V_MUX", bstr("1010"), bstr("0101"), bstr("0")),
        VecSpec::new("V_MUX", bstr("1010"), bstr("1010"), bstr("x")),
        VecSpec::new("V_MUX", bstr("1010"), bstr("0101"), bstr("x")),
        VecSpec::new("V_MUX", bstr("1010"), bstr("0101"), bstr("z")),
        VecSpec::new("V_MUX", bstr("10z1"), bstr("0000"), bstr("1")),
        // resize (c = target width + signedness)
        VecSpec::new("V_RESIZE", sstr("1000"), EMPTY, resize_target(8, true)),
        VecSpec::new("V_RESIZE", sstr("1000"), EMPTY, resize_target(8, false)),
        VecSpec::new("V_RESIZE", bstr("1001"), EMPTY, resize_target(2, false)),
        VecSpec::new("V_RESIZE", bstr("10z1"), EMPTY, resize_target(8, false)),
        VecSpec::new("V_RESIZE", bstr("10x1"), EMPTY, resize_target(8, false)),
        VecSpec::new("V_RESIZE", sstr("1000"), EMPTY, resize_target(4, true)),
        // cast (value-preserving conversion; c = target width + signedness)
        // unsigned MSB-set source widens by ZERO-extension even into a
        // signed target (int'(8'hFF) = 255):
        VecSpec::new("V_CAST", bstr("11111111"), EMPTY, resize_target(16, true)),
        VecSpec::new("V_CAST", bstr("1001"), EMPTY, resize_target(8, true)),
        // signed negative source widens by SIGN-extension even into an
        // unsigned target:
        VecSpec::new("V_CAST", sstr("11111111"), EMPTY, resize_target(16, false)),
        VecSpec::new("V_CAST", sstr("11111110"), EMPTY, resize_target(16, false)),
        // signed positive source keeps zero-extension:
        VecSpec::new("V_CAST", sstr("0101"), EMPTY, resize_target(8, true)),
        VecSpec::new("V_CAST", sstr("0011"), EMPTY, resize_target(8, false)),
        // same-width retags (sign-only casts):
        VecSpec::new("V_CAST", bstr("1000"), EMPTY, resize_target(4, true)),
        VecSpec::new("V_CAST", sstr("1010"), EMPTY, resize_target(4, false)),
        // narrowing truncates the MSBs either way:
        VecSpec::new("V_CAST", bstr("1010"), EMPTY, resize_target(3, true)),
        VecSpec::new("V_CAST", sstr("0111"), EMPTY, resize_target(2, false)),
        // X/Z handling: a signed x/z sign bit fills with that same state;
        // unsigned sources always zero-fill.
        VecSpec::new("V_CAST", sstr("10z1"), EMPTY, resize_target(8, true)),
        VecSpec::new("V_CAST", sstr("10x1"), EMPTY, resize_target(8, true)),
        VecSpec::new("V_CAST", bstr("10z1"), EMPTY, resize_target(8, true)),
        VecSpec::new("V_CAST", sstr("x000"), EMPTY, resize_target(8, true)),
        // concat (a = hi, b = lo)
        VecSpec::new("V_CONCAT", bstr("1010"), bstr("0001"), EMPTY),
        VecSpec::new("V_CONCAT", bstr("11111111"), bstr("00000000"), EMPTY),
        VecSpec::new("V_CONCAT", bstr("10z1"), bstr("0x01"), EMPTY),
        VecSpec::new("V_CONCAT", bstr("0000"), bstr("1111"), EMPTY),
        // casez
        VecSpec::new("V_CASEZ", bstr("1000"), bstr("1z0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1110"), bstr("1z0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1x00"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1010"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1x0z"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("00001000"), bstr("1z0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1z00"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEZ", bstr("1001"), bstr("1000"), EMPTY),
        // casex
        VecSpec::new("V_CASEX", bstr("1001"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEX", bstr("1000"), bstr("1x0z"), EMPTY),
        VecSpec::new("V_CASEX", bstr("1x0z"), bstr("1000"), EMPTY),
        VecSpec::new("V_CASEX", bstr("1100"), bstr("1000"), EMPTY),
        VecSpec::new("V_CASEX", bstr("1001"), bstr("1000"), EMPTY),
        VecSpec::new("V_CASEX", bstr("00001000"), bstr("1x0z"), EMPTY),
    ]
}

#[test]
#[ignore]
fn gen_c_vectors() {
    println!("// Deterministic cross-check vectors: elab::Value computes the expected");
    println!("// values, the C selftest verifies sv4_* matches them.");
    println!("// Regenerate: cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc");
    println!("static const sv4_vec_t VECTORS[] = {{");
    println!("    // Generated by tests/property_elab.rs gen_c_vectors — do not edit by hand.");
    for v in vector_specs() {
        println!("{}", v.to_c());
    }
    println!("}};");
    println!("static const int N_VECTORS = (int)(sizeof(VECTORS) / sizeof(VECTORS[0]));");
}
