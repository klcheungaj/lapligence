//! Parameter / constant-expression resolution over an elaborated UHDM model.
//!
//! Surelog's `-elabuhdm` output keeps per-instance parameter values in two
//! places:
//!   - the `parameter` object itself carries the *definition* (default) value;
//!     for overridden parameters this is NOT the real value;
//!   - a `param_assign` on the instance scope holds the real value in its RHS,
//!     which Surelog usually folds to a `constant` but may leave as an
//!     `operation` tree (e.g. `localparam W2 = W + 1`) whose refs are unbound.
//!
//! `Resolver::scope_params` reconciles the two and evaluates every parameter of
//! a scope to a concrete 4-state value, memoising results and detecting
//! definition cycles.  `Resolver::eval_expr` evaluates an arbitrary constant
//! expression (operations, selects, casts, `$clog2` / `$bits`, …) in the
//! context of a scope.
//!
//! Handle-lifetime rule (a real bug class in this codebase): handles from
//! `vpi::iterate` and `vpi::handle(...)` are `OwnedHandle` values that free
//! their wrappers on drop. Whenever an owned handle is needed for recursive
//! evaluation, keep it alive in a local and use `.raw()` for nested calls —
//! never store a raw pointer extracted from an `OwnedHandle`.

#![allow(non_upper_case_globals)]

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::os::raw::c_int;

use crate::ffi::vpi::{self, OwnedHandle, ValueData, VpiHandle};

/// Maximum nesting depth of constant function calls (parameter expressions).
/// Guarantees termination for runaway recursion (`f(n) = f(n + 1)`).
const MAX_FUNC_DEPTH: usize = 256;
/// Upper bound for values synthesized while resolving types and replication.
/// This prevents untrusted HDL from requesting effectively unbounded vectors.
const MAX_RESOLVED_BITS: usize = 1 << 24;

fn checked_inclusive_width(left: i128, right: i128, context: &str) -> Result<usize, ElabError> {
    let width = left
        .abs_diff(right)
        .checked_add(1)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| ElabError::Unsupported(format!("{context} width overflow")))?;
    if width > MAX_RESOLVED_BITS {
        return Err(ElabError::Unsupported(format!(
            "{context} is too wide ({width} bits)"
        )));
    }
    Ok(width)
}

// ── 4-state values ────────────────────────────────────────────────────────────

/// A single 4-state bit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bit {
    Zero,
    One,
    X,
    Z,
}

/// Resolved 4-state vector, MSB-first (bit index 0 = MSB).
///
/// `signed` records how the vector was interpreted when it was produced;
/// `resize` uses it to sign- vs. zero-extend.  `fill` is `Some(b)` when the
/// value came from an unsized fill literal (`'1`, `'0`, `'x`, `'z`): a later
/// `resize` then fills with `b` instead of extending.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Value {
    pub bits: Vec<Bit>,
    pub signed: bool,
    /// `Some(b)` for unsized fill literals; `resize` fills with `b`.
    pub fill: Option<Bit>,
}

impl Value {
    /// Number of bits (MSB-first vector length).
    pub fn width(&self) -> usize {
        self.bits.len()
    }

    /// Bit at index `i` counted from the LSB (index 0 = LSB).
    /// Out-of-range indices return `X`.
    pub fn bit_lsb(&self, i: usize) -> Bit {
        if i < self.bits.len() {
            self.bits[self.bits.len() - 1 - i]
        } else {
            Bit::X
        }
    }

    /// True when any bit is `X` or `Z`.
    pub fn is_unknown(&self) -> bool {
        self.bits.iter().any(|b| matches!(b, Bit::X | Bit::Z))
    }

    /// Interpret the bits as an unsigned integer; `None` if any bit is X/Z or
    /// the clean value does not fit in `u64`.
    pub fn to_u64(&self) -> Option<u64> {
        self.to_u128().and_then(|value| value.try_into().ok())
    }

    /// Interpret the bits as a two's-complement signed integer; `None` if any
    /// bit is X/Z or the clean value does not fit in `i64`.
    pub fn to_i64(&self) -> Option<i64> {
        self.to_i128().and_then(|value| value.try_into().ok())
    }

    /// Interpret the bits as an unsigned integer; `None` if any bit is X/Z or
    /// the clean value does not fit in `u128`.
    pub fn to_u128(&self) -> Option<u128> {
        let excess = self.bits.len().saturating_sub(u128::BITS as usize);
        if self.bits[..excess].iter().any(|bit| *bit != Bit::Zero) {
            return None;
        }
        self.bits[excess..]
            .iter()
            .try_fold(0u128, |value, bit| match bit {
                Bit::Zero => Some(value << 1),
                Bit::One => Some((value << 1) | 1),
                Bit::X | Bit::Z => None,
            })
    }

    /// Interpret the bits as a two's-complement signed integer; `None` if any
    /// bit is X/Z or the clean value does not fit in `i128`.
    pub fn to_i128(&self) -> Option<i128> {
        if self.bits.is_empty() {
            return Some(0);
        }
        if self.is_unknown() {
            return None;
        }
        let negative = self.bits[0] == Bit::One;
        let excess = self.bits.len().saturating_sub(u128::BITS as usize);
        let expected = if negative { Bit::One } else { Bit::Zero };
        if self.bits[..excess].iter().any(|bit| *bit != expected) {
            return None;
        }
        if self.bits.len() >= u128::BITS as usize && self.bits[excess] != expected {
            return None;
        }
        let raw = self.bits[excess..].iter().fold(0u128, |value, bit| {
            (value << 1) | u128::from(*bit == Bit::One)
        });
        let retained_width = self.bits.len().min(u128::BITS as usize);
        if retained_width < u128::BITS as usize && negative {
            let sign_extension = !0u128 << retained_width;
            Some((raw | sign_extension) as i128)
        } else {
            Some(raw as i128)
        }
    }

    /// Convert to a real value. X/Z bits contribute zero; signed values use
    /// two's-complement interpretation. All limbs participate.
    pub fn to_real(&self) -> f64 {
        let negative = self.signed && self.bits.first() == Some(&Bit::One);
        if !negative {
            return self.bits.iter().fold(0.0, |value, bit| {
                value * 2.0 + if *bit == Bit::One { 1.0 } else { 0.0 }
            });
        }

        let mut magnitude = 0.0;
        let mut weight = 1.0;
        let mut carry = true;
        for bit in self.bits.iter().rev() {
            let inverted = *bit != Bit::One;
            let sum = inverted ^ carry;
            if sum {
                magnitude += weight;
            }
            carry = inverted && carry;
            weight *= 2.0;
        }
        -magnitude
    }

    /// Build a value from an unsigned integer, truncated (wrapping) to
    /// `width` bits.
    pub fn from_u64(v: u64, width: usize, signed: bool) -> Value {
        let bits = (0..width)
            .map(|i| {
                if i < u64::BITS as usize && v & (1u64 << i) != 0 {
                    Bit::One
                } else {
                    Bit::Zero
                }
            })
            .rev()
            .collect();
        Value {
            bits,
            signed,
            fill: None,
        }
    }

    /// Build a value directly from an MSB-first bit vector.
    pub fn from_bits(bits: Vec<Bit>, signed: bool) -> Value {
        Value {
            bits,
            signed,
            fill: None,
        }
    }

    /// Resize to `width` bits and set `signed`.  Values carrying an unsized
    /// fill literal (`self.fill`) are filled with that bit; otherwise a signed
    /// value sign-extends, including an X/Z sign bit, and an unsigned value
    /// zero-extends.  The fill marker is preserved so later resizes keep
    /// filling.
    pub fn resize(&self, width: usize, signed: bool) -> Value {
        if let Some(f) = self.fill {
            return Value {
                bits: vec![f; width],
                signed,
                fill: self.fill,
            };
        }
        let mut bits = self.bits.clone();
        if width < bits.len() {
            // Narrowing keeps the least-significant bits.
            bits = bits[bits.len() - width..].to_vec();
        } else if width > bits.len() {
            let ext = if signed {
                match self.bits.first().copied() {
                    Some(Bit::One) => Bit::One,
                    Some(Bit::X) => Bit::X,
                    Some(Bit::Z) => Bit::Z,
                    Some(Bit::Zero) | None => Bit::Zero,
                }
            } else {
                Bit::Zero
            };
            let pad = vec![ext; width - bits.len()];
            bits.splice(0..0, pad);
        }
        Value {
            bits,
            signed,
            fill: self.fill,
        }
    }

    /// Value-preserving conversion to `(width, signed)` — the elaboration-side
    /// twin of the runtime's `sv4_cast` (LRM 1800-2009 §6.24.1: a cast yields
    /// "the value that a variable of the casting type would hold after being
    /// assigned the expression", and §10.7/§11.8.3 pad assignments by the
    /// RIGHT-HAND side's signedness).  Widening extends by the SOURCE's
    /// signedness (an unsigned 8'hFF widens to 255 even into a signed target;
    /// a signed value sign-extends even into an unsigned target), narrowing
    /// truncates; the result carries the target `signed` tag.  Fill literals
    /// keep their fill semantics.
    pub fn cast(&self, width: usize, signed: bool) -> Value {
        if self.fill.is_some() {
            return self.resize(width, signed);
        }
        let mut v = self.resize(width, self.signed);
        v.signed = signed;
        v
    }

    /// Verilog literal representation: `8'd255` for a clean unsigned value,
    /// `8'sd-1` for a clean signed value, and `4'h10xz` when any bit is X/Z.
    pub fn format_verilog(&self) -> String {
        if self.is_unknown() {
            format!("{}'h{}", self.width(), self.hex_str())
        } else if self.signed {
            match self.to_i128() {
                Some(value) => format!("{}'sd{value}", self.width()),
                None => format!("{}'sh{}", self.width(), self.hex_str()),
            }
        } else {
            match self.to_u128() {
                Some(value) => format!("{}'d{value}", self.width()),
                None => format!("{}'h{}", self.width(), self.hex_str()),
            }
        }
    }

    /// Always `None`: string payloads live in `Val::Str`, keeping bit math
    /// free of string cases.
    pub fn to_string_val(&self) -> Option<String> {
        None
    }

    fn hex_str(&self) -> String {
        let mut out = String::new();
        let n = self.bits.len();
        let mut i = n;
        while i > 0 {
            let start = i.saturating_sub(4);
            let group = &self.bits[start..i];
            if group.iter().any(|b| matches!(b, Bit::X | Bit::Z)) {
                let mut d = String::with_capacity(group.len());
                for b in group {
                    d.push(match b {
                        Bit::Zero => '0',
                        Bit::One => '1',
                        Bit::X => 'x',
                        Bit::Z => 'z',
                    });
                }
                out.insert_str(0, &d);
            } else {
                let mut val = 0u8;
                for b in group {
                    val = (val << 1) | if *b == Bit::One { 1 } else { 0 };
                }
                out.insert(0, std::char::from_digit(val as u32, 16).unwrap());
            }
            i = start;
        }
        out
    }
}

/// A resolved parameter value: a 4-state bit vector, string, or real number.
///
/// Strings are kept out of `Value` so `Bit` arithmetic never has to handle a
/// non-bit payload.
#[derive(Clone, PartialEq, Debug)]
pub enum Val {
    Bits(Value),
    Str(String),
    Real(f64),
}

impl Val {
    /// Verilog literal representation: `format_verilog` for bit values, the
    /// double-quoted string for string values.
    pub fn format_verilog(&self) -> String {
        match self {
            Val::Bits(v) => v.format_verilog(),
            Val::Str(s) => format!("\"{s}\""),
            Val::Real(v) => format!("{v}"),
        }
    }

    /// The string payload, if this is a string value.
    pub fn to_string_val(&self) -> Option<String> {
        match self {
            Val::Str(s) => Some(s.clone()),
            Val::Bits(_) | Val::Real(_) => None,
        }
    }
}

// ── Pure value math (IEEE 1364 / 1800 semantics) ─────────────────────────────
//
// All functions operate on self-determined operand widths and return the value
// in the operation's result width; the caller (`eval_operation`) resizes to
// the declared parameter width. Arithmetic and relational comparison produce
// X when an operand is ambiguous; logical equality still returns a known false
// when another bit proves a mismatch. Case equality compares X/Z literally.

fn all_x(width: usize, signed: bool) -> Value {
    Value {
        bits: vec![Bit::X; width],
        signed,
        fill: None,
    }
}

fn zero(width: usize, signed: bool) -> Value {
    Value::from_bits(vec![Bit::Zero; width], signed)
}

fn is_zero(value: &Value) -> bool {
    value.bits.iter().all(|bit| *bit == Bit::Zero)
}

/// Add two known, equally-sized vectors and discard carry beyond their width.
fn add_known(a: &Value, b: &Value, signed: bool) -> Value {
    debug_assert_eq!(a.width(), b.width());
    debug_assert!(!a.is_unknown() && !b.is_unknown());
    let mut bits = vec![Bit::Zero; a.width()];
    let mut carry = false;
    for index in (0..a.width()).rev() {
        let lhs = u8::from(a.bits[index] == Bit::One);
        let rhs = u8::from(b.bits[index] == Bit::One);
        let sum = lhs + rhs + u8::from(carry);
        bits[index] = if sum & 1 == 1 { Bit::One } else { Bit::Zero };
        carry = sum >= 2;
    }
    Value::from_bits(bits, signed)
}

/// Two's-complement negation of a known vector in its existing width.
fn negate_known(value: &Value) -> Value {
    debug_assert!(!value.is_unknown());
    let inverted = Value::from_bits(
        value
            .bits
            .iter()
            .map(|bit| {
                if *bit == Bit::One {
                    Bit::Zero
                } else {
                    Bit::One
                }
            })
            .collect(),
        value.signed,
    );
    let mut one = zero(value.width(), value.signed);
    if let Some(last) = one.bits.last_mut() {
        *last = Bit::One;
    }
    add_known(&inverted, &one, value.signed)
}

fn sub_known(a: &Value, b: &Value, signed: bool) -> Value {
    let mut negated = negate_known(b);
    negated.signed = signed;
    add_known(a, &negated, signed)
}

/// Multiply known vectors modulo `2^width`.
fn mul_known(a: &Value, b: &Value, signed: bool) -> Value {
    debug_assert_eq!(a.width(), b.width());
    debug_assert!(!a.is_unknown() && !b.is_unknown());
    let width = a.width();
    let mut result_lsb = vec![false; width];
    for rhs_index in 0..width {
        if b.bit_lsb(rhs_index) != Bit::One {
            continue;
        }
        let mut carry = false;
        for lhs_index in 0..width.saturating_sub(rhs_index) {
            let out_index = rhs_index + lhs_index;
            let sum = u8::from(result_lsb[out_index])
                + u8::from(a.bit_lsb(lhs_index) == Bit::One)
                + u8::from(carry);
            result_lsb[out_index] = sum & 1 == 1;
            carry = sum >= 2;
        }
    }
    Value::from_bits(
        result_lsb
            .into_iter()
            .rev()
            .map(|bit| if bit { Bit::One } else { Bit::Zero })
            .collect(),
        signed,
    )
}

fn unsigned_cmp(a: &Value, b: &Value) -> Ordering {
    debug_assert_eq!(a.width(), b.width());
    debug_assert!(!a.is_unknown() && !b.is_unknown());
    a.bits
        .iter()
        .zip(&b.bits)
        .find_map(
            |(lhs, rhs)| match (*lhs == Bit::One).cmp(&(*rhs == Bit::One)) {
                Ordering::Equal => None,
                ordering => Some(ordering),
            },
        )
        .unwrap_or(Ordering::Equal)
}

/// Unsigned restoring division over known, equally-sized vectors.
fn unsigned_div_rem(dividend: &Value, divisor: &Value) -> (Value, Value) {
    debug_assert_eq!(dividend.width(), divisor.width());
    debug_assert!(!dividend.is_unknown() && !divisor.is_unknown());
    debug_assert!(!is_zero(divisor));
    let width = dividend.width();
    let mut quotient = zero(width, false);
    let mut remainder = zero(width, false);
    for (index, bit) in dividend.bits.iter().copied().enumerate() {
        if width > 0 {
            remainder.bits.rotate_left(1);
            remainder.bits[width - 1] = bit;
        }
        if unsigned_cmp(&remainder, divisor) != Ordering::Less {
            remainder = sub_known(&remainder, divisor, false);
            quotient.bits[index] = Bit::One;
        }
    }
    (quotient, remainder)
}

pub(crate) fn real_to_bits(value: f64, width: usize, signed: bool) -> Value {
    if !value.is_finite() {
        return all_x(width, signed);
    }
    let value = value.round();
    let modulus = 18_446_744_073_709_551_616.0_f64;
    let mut bits = (value.abs() % modulus) as u64;
    if value.is_sign_negative() {
        bits = bits.wrapping_neg();
    }
    Value::from_u64(bits, width, signed)
}

/// `$rtoi`: truncate a real toward zero into a signed 32-bit integer.
/// Non-finite values have no integral representation and produce X.
pub(crate) fn rtoi_value(value: f64) -> Value {
    if !value.is_finite() {
        return Value::from_bits(vec![Bit::X; 32], true);
    }
    let magnitude = value.trunc().abs() % 4_294_967_296.0;
    let mut bits = magnitude as u64;
    if value.is_sign_negative() {
        bits = bits.wrapping_neg();
    }
    Value::from_u64(bits, 32, true)
}

/// `$realtobits`: preserve the IEEE-754 double representation exactly.
pub(crate) fn real_to_ieee_bits(value: f64) -> Value {
    Value::from_u64(value.to_bits(), 64, false)
}

/// `$bitstoreal`: reinterpret exactly 64 packed bits as an IEEE-754 double.
/// X/Z positions contribute zero, matching [`Value::to_real`].
pub(crate) fn ieee_bits_to_real(value: &Value) -> Option<f64> {
    (value.width() == 64).then(|| f64::from_bits(known_low_bits(value)))
}

/// `$shortrealtobits`: round through IEEE-754 single precision and preserve
/// the resulting 32-bit representation.
pub(crate) fn shortreal_to_ieee_bits(value: f64) -> Value {
    Value::from_u64((value as f32).to_bits().into(), 32, false)
}

/// `$bitstoshortreal`: reinterpret exactly 32 packed bits as an IEEE-754
/// single and widen it losslessly to the core's `f64` real container.
pub(crate) fn ieee_bits_to_shortreal(value: &Value) -> Option<f64> {
    (value.width() == 32).then(|| f32::from_bits(known_low_bits(value) as u32) as f64)
}

fn known_low_bits(value: &Value) -> u64 {
    let mut bits = 0u64;
    for index in 0..value.width().min(64) {
        if value.bit_lsb(index) == Bit::One {
            bits |= 1u64 << index;
        }
    }
    bits
}

fn bit_x() -> Value {
    Value::from_bits(vec![Bit::X], false)
}

fn invert_known_one_bit(value: Value) -> Value {
    debug_assert_eq!(value.bits.len(), 1);
    debug_assert!(!value.is_unknown());
    Value::from_bits(
        vec![if value.bits.first() == Some(&Bit::One) {
            Bit::Zero
        } else {
            Bit::One
        }],
        false,
    )
}

fn max_width(a: &Value, b: &Value) -> usize {
    a.width().max(b.width())
}

/// Coerce add/sub/mul/div/rem operands to their common self-determined width
/// and signedness before evaluating the operation.
fn binary_arith_operands(a: &Value, b: &Value) -> (Value, Value, usize, bool) {
    let w = max_width(a, b);
    let signed = a.signed && b.signed;
    (a.resize(w, signed), b.resize(w, signed), w, signed)
}

/// Truncating integer addition.
pub fn add(a: &Value, b: &Value) -> Value {
    let (a, b, w, s) = binary_arith_operands(a, b);
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    add_known(&a, &b, s)
}

/// Truncating integer subtraction.
pub fn sub(a: &Value, b: &Value) -> Value {
    let (a, b, w, s) = binary_arith_operands(a, b);
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    sub_known(&a, &b, s)
}

/// Truncating integer multiplication.
pub fn mul(a: &Value, b: &Value) -> Value {
    let (a, b, w, s) = binary_arith_operands(a, b);
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    mul_known(&a, &b, s)
}

/// Truncating integer division; division by zero yields X.
pub fn div(a: &Value, b: &Value) -> Value {
    let (a, b, w, s) = binary_arith_operands(a, b);
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    if is_zero(&b) {
        return all_x(w, s);
    }
    let a_negative = s && a.bits.first() == Some(&Bit::One);
    let b_negative = s && b.bits.first() == Some(&Bit::One);
    let mut dividend = if a_negative { negate_known(&a) } else { a };
    let mut divisor = if b_negative { negate_known(&b) } else { b };
    dividend.signed = false;
    divisor.signed = false;
    let (mut quotient, _) = unsigned_div_rem(&dividend, &divisor);
    if a_negative != b_negative {
        quotient = negate_known(&quotient);
    }
    quotient.signed = s;
    quotient
}

/// Truncating integer remainder (sign of the dividend); modulo zero yields X.
pub fn rem(a: &Value, b: &Value) -> Value {
    let (a, b, w, s) = binary_arith_operands(a, b);
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    if is_zero(&b) {
        return all_x(w, s);
    }
    let a_negative = s && a.bits.first() == Some(&Bit::One);
    let b_negative = s && b.bits.first() == Some(&Bit::One);
    let mut dividend = if a_negative { negate_known(&a) } else { a };
    let mut divisor = if b_negative { negate_known(&b) } else { b };
    dividend.signed = false;
    divisor.signed = false;
    let (_, mut remainder) = unsigned_div_rem(&dividend, &divisor);
    if a_negative {
        remainder = negate_known(&remainder);
    }
    remainder.signed = s;
    remainder
}

/// Integer exponentiation (`**`); the result has the base operand's width and
/// signedness, while the exponent remains self-determined.  A negative
/// exponent yields 0.
pub fn power(a: &Value, b: &Value) -> Value {
    let w = a.width();
    let s = a.signed;
    if a.is_unknown() || b.is_unknown() {
        return all_x(w, s);
    }
    if b.signed && b.bits.first() == Some(&Bit::One) {
        if is_zero(a) {
            return all_x(w, s);
        }
        let base_is_negative_one =
            s && !a.bits.is_empty() && a.bits.iter().all(|bit| *bit == Bit::One);
        if base_is_negative_one {
            return if b.bit_lsb(0) == Bit::One {
                a.clone()
            } else {
                Value::from_u64(1, w, s)
            };
        }
        let base_is_one = a.bits.last() == Some(&Bit::One)
            && a.bits[..a.bits.len().saturating_sub(1)]
                .iter()
                .all(|bit| *bit == Bit::Zero);
        if base_is_one {
            return Value::from_u64(1, w, s);
        }
        return zero(w, s);
    }
    let mut result = zero(w, s);
    if let Some(last) = result.bits.last_mut() {
        *last = Bit::One;
    }
    let mut base = a.clone();
    for bit in b.bits.iter().rev() {
        if *bit == Bit::One {
            result = mul_known(&result, &base, s);
        }
        base = mul_known(&base, &base, s);
    }
    result
}

/// Bitwise AND (`&`); `0` dominates, X/Z propagates otherwise.
pub fn bit_and(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let s = a.signed && b.signed;
    let ra = a.resize(w, s);
    let rb = b.resize(w, s);
    let bits = ra
        .bits
        .iter()
        .zip(rb.bits.iter())
        .map(|(x, y)| match (x, y) {
            (Bit::Zero, _) | (_, Bit::Zero) => Bit::Zero,
            (Bit::One, Bit::One) => Bit::One,
            _ => Bit::X,
        })
        .collect();
    Value {
        bits,
        signed: s,
        fill: None,
    }
}

/// Bitwise OR (`|`); `1` dominates, X/Z propagates otherwise.
pub fn bit_or(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let s = a.signed && b.signed;
    let ra = a.resize(w, s);
    let rb = b.resize(w, s);
    let bits = ra
        .bits
        .iter()
        .zip(rb.bits.iter())
        .map(|(x, y)| match (x, y) {
            (Bit::One, _) | (_, Bit::One) => Bit::One,
            (Bit::Zero, Bit::Zero) => Bit::Zero,
            _ => Bit::X,
        })
        .collect();
    Value {
        bits,
        signed: s,
        fill: None,
    }
}

/// Bitwise XOR (`^`); X/Z propagates.
pub fn bit_xor(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let s = a.signed && b.signed;
    let ra = a.resize(w, s);
    let rb = b.resize(w, s);
    let bits = ra
        .bits
        .iter()
        .zip(rb.bits.iter())
        .map(|(x, y)| match (x, y) {
            (Bit::X, _) | (Bit::Z, _) | (_, Bit::X) | (_, Bit::Z) => Bit::X,
            (Bit::One, Bit::One) | (Bit::Zero, Bit::Zero) => Bit::Zero,
            _ => Bit::One,
        })
        .collect();
    Value {
        bits,
        signed: s,
        fill: None,
    }
}

/// Bitwise XNOR (`~^`); X/Z propagates.
pub fn bit_xnor(a: &Value, b: &Value) -> Value {
    let x = bit_xor(a, b);
    Value {
        bits: x.bits.into_iter().map(bit_neg_one).collect(),
        signed: x.signed,
        fill: None,
    }
}

/// Bitwise negation (`~`); `~z` yields X.
pub fn bit_neg(a: &Value) -> Value {
    let bits = a.bits.iter().map(|b| bit_neg_one(*b)).collect();
    Value {
        bits,
        signed: a.signed,
        fill: None,
    }
}

fn bit_neg_one(b: Bit) -> Bit {
    match b {
        Bit::Zero => Bit::One,
        Bit::One => Bit::Zero,
        Bit::X | Bit::Z => Bit::X,
    }
}

/// Unary minus (`-`): two's-complement negation in the operand width.
pub fn minus(a: &Value) -> Value {
    let w = a.width();
    if a.is_unknown() {
        return all_x(w, a.signed);
    }
    negate_known(a)
}

fn logical_bit(a: &Value) -> Bit {
    if a.bits.contains(&Bit::One) {
        Bit::One
    } else if a.is_unknown() {
        Bit::X
    } else {
        Bit::Zero
    }
}

/// Logical NOT (`!`): a known one bit makes the operand true even when other
/// bits are X/Z; an otherwise ambiguous operand yields X.
pub fn log_not(a: &Value) -> Value {
    match logical_bit(a) {
        Bit::Zero => Value::from_u64(1, 1, false),
        Bit::One => Value::from_u64(0, 1, false),
        Bit::X | Bit::Z => bit_x(),
    }
}

/// Logical AND (`&&`): false dominates an ambiguous operand.
pub fn log_and(a: &Value, b: &Value) -> Value {
    match (logical_bit(a), logical_bit(b)) {
        (Bit::Zero, _) | (_, Bit::Zero) => Value::from_u64(0, 1, false),
        (Bit::One, Bit::One) => Value::from_u64(1, 1, false),
        _ => bit_x(),
    }
}

/// Logical OR (`||`): true dominates an ambiguous operand.
pub fn log_or(a: &Value, b: &Value) -> Value {
    match (logical_bit(a), logical_bit(b)) {
        (Bit::One, _) | (_, Bit::One) => Value::from_u64(1, 1, false),
        (Bit::Zero, Bit::Zero) => Value::from_u64(0, 1, false),
        _ => bit_x(),
    }
}

/// Unary reduction AND (`&`); a known zero dominates X/Z.
pub fn unary_and(a: &Value) -> Value {
    if a.bits.contains(&Bit::Zero) {
        Value::from_u64(0, 1, false)
    } else if a.is_unknown() {
        bit_x()
    } else {
        Value::from_u64(1, 1, false)
    }
}

/// Unary reduction NAND (`~&`); X when any input bit is X/Z.
pub fn unary_nand(a: &Value) -> Value {
    let r = unary_and(a);
    if r.is_unknown() {
        r
    } else {
        invert_known_one_bit(r)
    }
}

/// Unary reduction OR (`|`); a known one dominates X/Z.
pub fn unary_or(a: &Value) -> Value {
    if a.bits.contains(&Bit::One) {
        Value::from_u64(1, 1, false)
    } else if a.is_unknown() {
        bit_x()
    } else {
        Value::from_u64(0, 1, false)
    }
}

/// Unary reduction NOR (`~|`); X when any input bit is X/Z.
pub fn unary_nor(a: &Value) -> Value {
    let r = unary_or(a);
    if r.is_unknown() {
        r
    } else {
        invert_known_one_bit(r)
    }
}

/// Unary reduction XOR (`^`); X when any input bit is X/Z.
pub fn unary_xor(a: &Value) -> Value {
    if a.is_unknown() {
        return bit_x();
    }
    let ones = a.bits.iter().filter(|b| **b == Bit::One).count();
    Value::from_u64((ones % 2 == 1) as u64, 1, false)
}

/// Unary reduction XNOR (`~^`); X when any input bit is X/Z.
pub fn unary_xnor(a: &Value) -> Value {
    let r = unary_xor(a);
    if r.is_unknown() {
        r
    } else {
        invert_known_one_bit(r)
    }
}

/// Logical left shift (`<<`); result width is the LHS width.
pub fn shl(a: &Value, b: &Value) -> Value {
    shift(a, b, false, false)
}

/// Logical right shift (`>>`); result width is the LHS width.
pub fn shr(a: &Value, b: &Value) -> Value {
    shift(a, b, true, false)
}

/// Arithmetic left shift (`<<<`); result width is the LHS width.
pub fn arith_shl(a: &Value, b: &Value) -> Value {
    shift(a, b, false, true)
}

/// Arithmetic right shift (`>>>`); result width is the LHS width, vacated
/// bits take the sign bit.
pub fn arith_shr(a: &Value, b: &Value) -> Value {
    shift(a, b, true, true)
}

fn shift(a: &Value, b: &Value, right: bool, arith: bool) -> Value {
    let w = a.width();
    let s = a.signed;
    if b.is_unknown() {
        return all_x(w, s);
    }
    let sh = b.bits.iter().fold(0usize, |value, bit| {
        value
            .saturating_mul(2)
            .saturating_add(usize::from(*bit == Bit::One))
    });
    if sh >= w {
        let fill = if right && arith && s && w > 0 {
            a.bits[0]
        } else {
            Bit::Zero
        };
        return Value::from_bits(vec![fill; w], s);
    }
    let bits = &a.bits;
    let out = if right {
        let fill = if arith && s { bits[0] } else { Bit::Zero };
        let mut v = vec![fill; sh];
        v.extend_from_slice(&bits[..w - sh]);
        v
    } else {
        let mut v = bits[sh..].to_vec();
        v.extend(std::iter::repeat_n(Bit::Zero, sh));
        v
    };
    Value {
        bits: out,
        signed: s,
        fill: None,
    }
}

/// Equality (`==`, IEEE 1800-2009 §11.4.5): a known mismatch determines
/// zero even when another bit is X/Z; otherwise an X/Z bit makes the result X.
pub fn eq(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    let mut unknown = false;
    for (left, right) in ra.bits.iter().zip(&rb.bits) {
        if matches!(left, Bit::X | Bit::Z) || matches!(right, Bit::X | Bit::Z) {
            unknown = true;
        } else if left != right {
            return Value::from_u64(0, 1, false);
        }
    }
    if unknown {
        bit_x()
    } else {
        Value::from_u64(1, 1, false)
    }
}

/// Inequality (`!=`): logical complement of [`eq`] while preserving X.
pub fn neq(a: &Value, b: &Value) -> Value {
    let result = eq(a, b);
    if result.is_unknown() {
        result
    } else {
        invert_known_one_bit(result)
    }
}

/// Case equality (`===`): bitwise comparison including X/Z bits as literal
/// values; the result is never X.
pub fn case_eq(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    Value::from_u64((ra.bits == rb.bits) as u64, 1, false)
}

/// Case inequality (`!==`): bitwise comparison including X/Z bits; never X.
pub fn case_neq(a: &Value, b: &Value) -> Value {
    invert_known_one_bit(case_eq(a, b))
}

/// Wildcard equality (`==?`, LRM 1800-2009 §11.4.6): X/Z bits in the
/// right operand are don't-cares. A known mismatch on any cared bit makes the
/// result 0; otherwise an X/Z bit in the left operand makes the result X.
pub fn wildcard_eq(a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    let mut unknown = false;
    for i in 0..w {
        let right = rb.bit_lsb(i);
        if matches!(right, Bit::X | Bit::Z) {
            continue;
        }
        let left = ra.bit_lsb(i);
        if matches!(left, Bit::X | Bit::Z) {
            unknown = true;
        } else if left != right {
            return Value::from_u64(0, 1, false);
        }
    }
    if unknown {
        bit_x()
    } else {
        Value::from_u64(1, 1, false)
    }
}

/// Wildcard inequality (`!=?`); logical complement of [`wildcard_eq`] while
/// preserving an unknown result.
pub fn wildcard_neq(a: &Value, b: &Value) -> Value {
    let result = wildcard_eq(a, b);
    if result.is_unknown() {
        result
    } else {
        invert_known_one_bit(result)
    }
}

/// Count known one bits, ignoring X/Z, as a signed SystemVerilog int.
pub fn countones(value: &Value) -> Value {
    let count = value.bits.iter().filter(|bit| **bit == Bit::One).count();
    Value::from_u64(count as u64, 32, true)
}

/// Test whether exactly one bit is one; X/Z do not contribute to the count.
pub fn onehot(value: &Value) -> Value {
    let count = value.bits.iter().filter(|bit| **bit == Bit::One).count();
    Value::from_u64(u64::from(count == 1), 1, false)
}

/// Test whether at most one bit is one; X/Z do not contribute to the count.
pub fn onehot0(value: &Value) -> Value {
    let count = value.bits.iter().filter(|bit| **bit == Bit::One).count();
    Value::from_u64(u64::from(count <= 1), 1, false)
}

/// Return a known one-bit predicate for the presence of X or Z.
pub fn isunknown(value: &Value) -> Value {
    Value::from_u64(u64::from(value.is_unknown()), 1, false)
}

/// Casez wildcard equality (`casez` item match, LRM 12.5.1): 1-bit result,
/// never X.  Operands are zero-extended to max width (like `case`).  Per-bit
/// rules, mirroring `sv4_casez_eq` in the C runtime:
/// - item `z` (or `?`) is a don't-care;
/// - item `x` matches a selector `x` only;
/// - a known item bit must equal the selector bit (a selector x/z never
///   matches a known item bit).
pub fn casez_eq(sel: &Value, item: &Value) -> Value {
    let w = max_width(sel, item);
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
                // item x matches a selector x only
                return Value::from_u64(0, 1, false);
            }
        } else if sb != ib {
            // known item: selector must equal it
            return Value::from_u64(0, 1, false);
        }
    }
    Value::from_u64(1, 1, false)
}

/// Casex wildcard equality (`casex` item match, LRM 12.5.1): 1-bit result,
/// never X.  Operands are zero-extended to max width.  Per-bit rules,
/// mirroring `sv4_casex_eq` in the C runtime:
/// - item `x`/`z` (or `?`) is a don't-care;
/// - a known item bit matches unless the selector holds the opposite known
///   bit (a selector x/z is a don't-care in casex).
pub fn casex_eq(sel: &Value, item: &Value) -> Value {
    let w = max_width(sel, item);
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
        return Value::from_u64(0, 1, false); // opposite known bit -> no match
    }
    Value::from_u64(1, 1, false)
}

/// Less than (`<`): 1-bit result, X when any operand bit is X/Z.  Operands
/// compare as unsigned unless both are signed.
pub fn lt(a: &Value, b: &Value) -> Value {
    cmp_result(a, b, |ordering| ordering == Ordering::Less)
}

/// Less or equal (`<=`): see `lt`.
pub fn le(a: &Value, b: &Value) -> Value {
    cmp_result(a, b, |ordering| ordering != Ordering::Greater)
}

/// Greater than (`>`): see `lt`.
pub fn gt(a: &Value, b: &Value) -> Value {
    cmp_result(a, b, |ordering| ordering == Ordering::Greater)
}

/// Greater or equal (`>=`): see `lt`.
pub fn ge(a: &Value, b: &Value) -> Value {
    cmp_result(a, b, |ordering| ordering != Ordering::Less)
}

fn cmp_result(a: &Value, b: &Value, predicate: fn(Ordering) -> bool) -> Value {
    if a.is_unknown() || b.is_unknown() {
        return bit_x();
    }
    Value::from_u64(predicate(known_cmp(a, b)) as u64, 1, false)
}

fn known_cmp(a: &Value, b: &Value) -> Ordering {
    let both_signed = a.signed && b.signed;
    let w = max_width(a, b);
    let ra = a.resize(w, both_signed);
    let rb = b.resize(w, both_signed);
    if both_signed && w > 0 && ra.bits[0] != rb.bits[0] {
        return if ra.bits[0] == Bit::One {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    unsigned_cmp(&ra, &rb)
}

/// Concatenation; `parts[0]` is the most-significant part.
pub fn concat(parts: &[Value]) -> Value {
    let mut bits = Vec::new();
    for p in parts {
        bits.extend(p.bits.iter().cloned());
    }
    Value::from_bits(bits, false)
}

/// Conditional (`sel ? a : b`).  An X/Z selector merges the branches bit by
/// bit: identical four-state bits survive and differing bits become X.
pub fn cond(sel: &Value, a: &Value, b: &Value) -> Value {
    let w = max_width(a, b);
    let signed = a.signed && b.signed;
    let ra = a.resize(w, signed);
    let rb = b.resize(w, signed);
    match logical_bit(sel) {
        Bit::One => return ra,
        Bit::Zero => return rb,
        Bit::X | Bit::Z => {}
    }
    let bits = ra
        .bits
        .into_iter()
        .zip(rb.bits)
        .map(|(left, right)| if left == right { left } else { Bit::X })
        .collect();
    Value::from_bits(bits, signed)
}

/// `$clog2`: 32-bit unsigned result; `$clog2(0) = $clog2(1) = 0`.
pub fn clog2(a: &Value) -> Value {
    if a.is_unknown() {
        return all_x(32, false);
    }
    let Some(first_one) = a.bits.iter().position(|bit| *bit == Bit::One) else {
        return Value::from_u64(0, 32, false);
    };
    let bit_length = a.width() - first_one;
    let lower_bits_set = a.bits[first_one + 1..].contains(&Bit::One);
    let r = if bit_length <= 1 {
        0
    } else if lower_bits_set {
        bit_length
    } else {
        bit_length - 1
    };
    Value::from_u64(r as u64, 32, false)
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Resolution failure.
#[derive(Debug, Clone, PartialEq)]
pub enum ElabError {
    /// A UHDM construct this resolver does not support.
    Unsupported(String),
    /// A symbol name not found in the current scope.
    Unresolved(String),
    /// A parameter that (transitively) references itself.
    CycleDetected(String),
    /// A parameter with neither a `param_assign` RHS nor an own value.
    NoValue(String),
}

impl std::fmt::Display for ElabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElabError::Unsupported(m) => write!(f, "unsupported: {m}"),
            ElabError::Unresolved(n) => write!(f, "unresolved symbol: {n}"),
            ElabError::CycleDetected(n) => write!(f, "parameter cycle: {n}"),
            ElabError::NoValue(n) => write!(f, "no value for: {n}"),
        }
    }
}

impl std::error::Error for ElabError {}

// ── Scope model ───────────────────────────────────────────────────────────────

/// Where a parameter's real value comes from.
enum ParamSrc<'session> {
    /// Value read from the `parameter` object itself (gen-scope params,
    /// params without a `param_assign`).
    OwnValue,
    /// Value obtained by evaluating the `param_assign` RHS expression.  The
    /// `OwnedHandle` is stored (not the raw pointer) so the wrapper stays
    /// alive for the whole evaluation; a raw pointer extracted from a dropped
    /// `OwnedHandle` would dangle.
    AssignExpr(OwnedHandle<'session>),
}

/// One parameter declaration in a scope.
struct ParamEntry<'session> {
    /// Owned handle to the `parameter` object (for value and typespec reads).
    handle: OwnedHandle<'session>,
    src: ParamSrc<'session>,
}

/// The per-scope view the resolver evaluates against: parameters in
/// `vpi_iterate(vpiParameter)` order plus the `param_assign` map.
struct Scope<'session> {
    params: Vec<(String, ParamEntry<'session>)>,
    by_name: HashMap<String, usize>,
}

impl<'session> Scope<'session> {
    fn build(scope: VpiHandle<'session>) -> Result<Scope<'session>, ElabError> {
        let mut assigns: HashMap<String, OwnedHandle<'session>> = HashMap::new();
        for pa in iter(vpi::vpiParamAssign, scope) {
            if let Some(lhs) = pa.child(vpi::vpiLhs) {
                let name = vpi::obj_name(lhs.raw());
                if let Some(rhs) = pa.child(vpi::vpiRhs) {
                    assigns.insert(name, rhs);
                }
            }
        }

        let mut params: Vec<(String, ParamEntry<'session>)> = Vec::new();
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for p in iter(vpi::vpiParameter, scope) {
            let name = vpi::obj_name(p.raw());
            let src = match assigns.remove(&name) {
                Some(rhs) => ParamSrc::AssignExpr(rhs),
                None => ParamSrc::OwnValue,
            };
            let idx = params.len();
            by_name.insert(name.clone(), idx);
            params.push((name, ParamEntry { handle: p, src }));
        }
        Ok(Scope { params, by_name })
    }
}

// ── VPI traversal helpers ─────────────────────────────────────────────────────

fn iter<'session>(type_: c_int, obj: VpiHandle<'session>) -> Vec<OwnedHandle<'session>> {
    vpi::iterate(type_, obj)
        .map(|it| it.collect())
        .unwrap_or_default()
}

/// `OwnedHandle` for a 1-to-1 relationship; the caller must keep it alive
/// (or use `.raw()`) for as long as the child handle is in use.
fn child<'session>(type_: c_int, obj: VpiHandle<'session>) -> Option<OwnedHandle<'session>> {
    vpi::handle(type_, obj)
}

/// A call argument Surelog synthesizes for a *missing named* argument: a
/// `constant 0` with no source location.  Genuine `0` literals written in the
/// source always carry a file/line, so a location-less zero constant is
/// treated as a placeholder and the formal's default is used instead.
fn is_synthetic_arg(a: VpiHandle) -> bool {
    if vpi::obj_line(a) != 0 || !vpi::obj_file(a).is_empty() {
        return false;
    }
    matches!(
        vpi::read_value(a),
        ValueData::Int(0) | ValueData::UInt(0) | ValueData::Scalar(vpi::vpi0)
    )
}

// ── Resolver ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum DeclaredType {
    Packed(usize, bool),
    Real,
    ShortReal,
    Other,
}

/// Resolves parameter values and constant expressions in an elaborated UHDM
/// scope (`module_inst` or `gen_scope`).
///
/// Stateless by design: every public call builds a fresh per-scope view, so
/// handles from one Surelog session can never leak into another.
pub struct Resolver;

impl Resolver {
    pub fn new() -> Resolver {
        Resolver
    }

    /// Resolve all parameters visible in `scope`, returning `(name, value)`
    /// pairs in `vpi_iterate(vpiParameter)` declaration order.
    ///
    /// For each parameter the value is the evaluated `param_assign` RHS when
    /// one exists, otherwise the parameter object's own value.  Every value is
    /// resized to the declared typespec width/signedness (kept as computed when
    /// the typespec carries no width, e.g. `string`).
    pub fn scope_params(&mut self, scope: VpiHandle) -> Result<Vec<(String, Val)>, ElabError> {
        let sc = Scope::build(scope)?;
        let mut resolved: HashMap<String, Val> = HashMap::new();
        let mut in_progress: HashSet<String> = HashSet::new();
        let mut out = Vec::with_capacity(sc.params.len());
        for (name, _) in &sc.params {
            let v = self.resolve_param(&sc, &mut resolved, &mut in_progress, name)?;
            out.push((name.clone(), v));
        }
        Ok(out)
    }

    /// Evaluate the constant expression `expr` in the context of `scope`.
    pub fn eval_expr(&mut self, scope: VpiHandle, expr: VpiHandle) -> Result<Val, ElabError> {
        let sc = Scope::build(scope)?;
        let mut resolved: HashMap<String, Val> = HashMap::new();
        let mut in_progress: HashSet<String> = HashSet::new();
        let frame = HashMap::new();
        self.eval_expr_ctx(&sc, &mut resolved, &mut in_progress, &frame, expr)
    }

    // ── Parameter resolution ────────────────────────────────────────────────

    fn resolve_param(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        name: &str,
    ) -> Result<Val, ElabError> {
        if let Some(v) = resolved.get(name) {
            return Ok(v.clone());
        }
        if in_progress.contains(name) {
            return Err(ElabError::CycleDetected(name.to_string()));
        }
        let idx = *sc
            .by_name
            .get(name)
            .ok_or_else(|| ElabError::Unresolved(name.to_string()))?;
        in_progress.insert(name.to_string());
        let frame = HashMap::new();
        let val = match &sc.params[idx].1.src {
            ParamSrc::AssignExpr(h) => {
                self.eval_expr_ctx(sc, resolved, in_progress, &frame, h.raw())?
            }
            ParamSrc::OwnValue => read_value(sc.params[idx].1.handle.raw())?,
        };
        in_progress.remove(name);
        let val = self.resize_to_declared(sc, resolved, in_progress, idx, val)?;
        resolved.insert(name.to_string(), val.clone());
        Ok(val)
    }

    /// Coerce a resolved value to the parameter's declared packed, real, or
    /// shortreal type.
    fn resize_to_declared(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        idx: usize,
        val: Val,
    ) -> Result<Val, ElabError> {
        let handle = sc.params[idx].1.handle.raw();
        match self.declared_of(sc, resolved, in_progress, handle)? {
            DeclaredType::Packed(width, signed) => match val {
                // Assignment into the declared parameter type pads by the
                // VALUE's signedness (LRM §10.7), then carries the declared
                // tag.
                Val::Bits(v) => Ok(Val::Bits(v.cast(width, signed))),
                Val::Real(v) if width <= 64 => Ok(Val::Bits(real_to_bits(v, width, signed))),
                Val::Real(_) => Err(ElabError::Unsupported(
                    "real parameter conversion target wider than 64 bits".to_string(),
                )),
                Val::Str(s) => Ok(Val::Str(s)),
            },
            DeclaredType::Real => match val {
                Val::Bits(v) => Ok(Val::Real(v.to_real())),
                Val::Real(v) => Ok(Val::Real(v)),
                Val::Str(s) => Ok(Val::Str(s)),
            },
            DeclaredType::ShortReal => {
                let value = match val {
                    Val::Bits(v) => v.to_real(),
                    Val::Real(v) => v,
                    Val::Str(s) => return Ok(Val::Str(s)),
                };
                Ok(Val::Real((value as f32) as f64))
            }
            DeclaredType::Other => Ok(val),
        }
    }

    /// Declared category of a parameter typespec. Packed widths may depend on
    /// other parameters and use the normal memoized expression path.
    fn declared_of(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        param: VpiHandle,
    ) -> Result<DeclaredType, ElabError> {
        if vpi::obj_type(param) == vpi::vpiTypeParameter {
            return Err(ElabError::Unsupported(format!(
                "type parameter {}",
                vpi::obj_name(param)
            )));
        }
        // A `-PNAME=VALUE` top-level override makes Surelog strip the
        // parameter object's typespec (the overridden value then lives only
        // in the `param_assign` RHS).  Without a declared type there is
        // nothing to coerce to, so keep the evaluated value as-is.
        let ts = match child(vpi::vpiTypespec, param) {
            Some(ts) => ts,
            None => return Ok(DeclaredType::Other),
        };
        let mut current = ts;
        let mut hops = 0;
        while vpi::obj_type(current.raw()) == vpi::vpiRefTypespec {
            if hops > 16 {
                return Err(ElabError::Unsupported(
                    "cyclic ref_typespec chain".to_string(),
                ));
            }
            current = current
                .child(vpi::vpiActual)
                .ok_or_else(|| ElabError::Unsupported("unbound ref_typespec".to_string()))?;
            hops += 1;
        }
        match vpi::obj_type(current.raw()) {
            vpi::vpiRealTypespec => Ok(DeclaredType::Real),
            vpi::vpiShortRealTypespec => Ok(DeclaredType::ShortReal),
            vpi::vpiStringTypespec => Ok(DeclaredType::Other),
            _ => match self.typespec_size(sc, resolved, in_progress, current.raw())? {
                Some((width, signed)) => Ok(DeclaredType::Packed(width, signed)),
                None => Ok(DeclaredType::Other),
            },
        }
    }

    /// `(width, signed)` of a typespec; `Ok(None)` for string typespecs.
    fn typespec_size(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        ts: VpiHandle,
    ) -> Result<Option<(usize, bool)>, ElabError> {
        // Follow `ref_typespec → vpiActual` chains, keeping every intermediate
        // OwnedHandle alive until the loop ends.  The hop limit guards against
        // malformed cyclic chains (elaborated typespecs never cycle).
        let mut current: Option<OwnedHandle> = None;
        let mut hops = 0;
        loop {
            if hops > 16 {
                return Err(ElabError::Unsupported(
                    "cyclic ref_typespec chain".to_string(),
                ));
            }
            let cur = current.as_ref().map_or(ts, OwnedHandle::raw);
            if vpi::obj_type(cur) == vpi::vpiRefTypespec {
                let next = match current.as_ref() {
                    Some(owner) => owner.child(vpi::vpiActual),
                    None => child(vpi::vpiActual, ts),
                };
                match next {
                    Some(actual) => current = Some(actual),
                    None => return Err(ElabError::Unsupported("unbound ref_typespec".to_string())),
                }
                hops += 1;
            } else {
                let t = vpi::obj_type(cur);
                let signed_prop = vpi::get(vpi::vpiSigned, cur) != 0;
                return match t {
                    vpi::vpiIntTypespec | vpi::vpiIntegerTypespec | vpi::vpiTimeTypespec => {
                        Ok(Some((32, signed_prop)))
                    }
                    vpi::vpiLongIntTypespec => Ok(Some((64, signed_prop))),
                    vpi::vpiByteTypespec => Ok(Some((8, true))),
                    vpi::vpiShortIntTypespec => Ok(Some((16, true))),
                    vpi::vpiLogicTypespec | vpi::vpiBitTypespec => {
                        self.typespec_ranges(sc, resolved, in_progress, cur)
                    }
                    vpi::vpiEnumTypespec => match child(vpi::vpiBaseTypespec, cur) {
                        Some(base) => self.typespec_size(sc, resolved, in_progress, base.raw()),
                        None => Err(ElabError::Unsupported(
                            "enum_typespec without base".to_string(),
                        )),
                    },
                    vpi::vpiPackedArrayTypespec => {
                        self.typespec_ranges(sc, resolved, in_progress, cur)
                    }
                    vpi::vpiStringTypespec => Ok(None),
                    vpi::vpiRealTypespec | vpi::vpiShortRealTypespec => Ok(None),
                    other => Err(ElabError::Unsupported(format!("typespec type {other}"))),
                };
            }
        }
    }

    /// Width from a typespec's `vpiRange` list (1 bit when no range, product
    /// across dimensions); signedness from `vpiSigned`.
    fn typespec_ranges(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        ts: VpiHandle,
    ) -> Result<Option<(usize, bool)>, ElabError> {
        let mut total: u64 = 1;
        let mut any = false;
        for r in iter(vpi::vpiRange, ts) {
            any = true;
            let l = self.range_bound(sc, resolved, in_progress, vpi::vpiLeftRange, r.raw())?;
            let rr = self.range_bound(sc, resolved, in_progress, vpi::vpiRightRange, r.raw())?;
            let dim = checked_inclusive_width(l, rr, "typespec range")?;
            total = total.saturating_mul(dim as u64);
        }
        let width = if any { total } else { 1 };
        if width > MAX_RESOLVED_BITS as u64 {
            return Err(ElabError::Unsupported("typespec too wide".to_string()));
        }
        Ok(Some((width as usize, vpi::get(vpi::vpiSigned, ts) != 0)))
    }

    /// Evaluate one bound of a range object as a clean integer.
    fn range_bound(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        rel: c_int,
        r: VpiHandle,
    ) -> Result<i128, ElabError> {
        let b = child(rel, r)
            .ok_or_else(|| ElabError::Unsupported("range without bound".to_string()))?;
        let frame = HashMap::new();
        let v = match self.eval_expr_ctx(sc, resolved, in_progress, &frame, b.raw())? {
            Val::Bits(v) => v,
            Val::Str(_) | Val::Real(_) => {
                return Err(ElabError::Unsupported("string range bound".to_string()))
            }
        };
        if v.is_unknown() {
            return Err(ElabError::Unsupported("unknown range bound".to_string()));
        }
        if v.signed {
            v.to_i128()
        } else {
            v.to_u128().and_then(|value| value.try_into().ok())
        }
        .ok_or_else(|| ElabError::Unsupported("range bound does not fit in i128".to_string()))
    }

    // ── Expression evaluation ───────────────────────────────────────────────

    /// Evaluate `expr` in the context of `scope`, with `frame` overlaying the
    /// symbol scope.  `frame` maps function-local names (io_decls, locals, the
    /// function-name return var) to values and takes precedence over module
    /// parameters; pass an empty map outside function bodies.
    fn eval_expr_ctx(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        expr: VpiHandle,
    ) -> Result<Val, ElabError> {
        match vpi::obj_type(expr) {
            vpi::vpiConstant | vpi::vpiEnumConst => read_value(expr),
            vpi::vpiOperation => self.eval_operation(sc, resolved, in_progress, frame, expr),
            vpi::vpiRefObj | vpi::vpiVarSelect => {
                self.eval_ref(sc, resolved, in_progress, frame, expr)
            }
            vpi::vpiBitSelect => self.eval_bit_select(sc, resolved, in_progress, frame, expr),
            vpi::vpiPartSelect => self.eval_part_select(sc, resolved, in_progress, frame, expr),
            vpi::vpiIndexedPartSelect => {
                self.eval_indexed_part_select(sc, resolved, in_progress, frame, expr)
            }
            vpi::vpiSysFuncCall => self.eval_sys_func(sc, resolved, in_progress, frame, expr),
            vpi::vpiFuncCall => self.eval_func_call(sc, resolved, in_progress, frame, expr),
            vpi::vpiHierPath => Err(ElabError::Unsupported("hierarchical path".to_string())),
            vpi::vpiParameter => {
                let name = vpi::obj_name(expr);
                self.resolve_param(sc, resolved, in_progress, &name)
            }
            other => Err(ElabError::Unsupported(format!(
                "expression node type {other}"
            ))),
        }
    }

    /// Evaluate operand `i` of an operation as a bit vector.
    fn op_bits(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        ops: &[VpiHandle],
        i: usize,
    ) -> Result<Value, ElabError> {
        let h = *ops
            .get(i)
            .ok_or_else(|| ElabError::Unsupported("missing operand".to_string()))?;
        match self.eval_expr_ctx(sc, resolved, in_progress, frame, h)? {
            Val::Bits(v) => Ok(v),
            Val::Str(s) => Err(ElabError::Unsupported(format!(
                "string operand {:?} in arithmetic",
                s
            ))),
            Val::Real(_) => Err(ElabError::Unsupported(
                "real operand in constant integer expression".to_string(),
            )),
        }
    }

    fn eval_real_operation(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        op: VpiHandle,
        vals: &[Val],
    ) -> Result<Val, ElabError> {
        use vpi::*;
        let real = |i: usize| -> Result<f64, ElabError> {
            match vals.get(i) {
                Some(Val::Real(v)) => Ok(*v),
                Some(Val::Bits(v)) => Ok(v.to_real()),
                Some(Val::Str(_)) => Err(ElabError::Unsupported(
                    "string operand in real constant expression".to_string(),
                )),
                None => Err(ElabError::Unsupported("missing operand".to_string())),
            }
        };
        let boolean = |i: usize| -> Result<bool, ElabError> {
            let v = real(i)?;
            Ok(v != 0.0)
        };
        let bit = |v: bool| Val::Bits(Value::from_u64(v as u64, 1, false));

        match vpi::get(vpi::vpiOpType, op) {
            vpiMinusOp => Ok(Val::Real(-real(0)?)),
            vpiPlusOp => Ok(Val::Real(real(0)?)),
            vpiNotOp => Ok(bit(!boolean(0)?)),
            vpiSubOp => Ok(Val::Real(real(0)? - real(1)?)),
            vpiDivOp => Ok(Val::Real(real(0)? / real(1)?)),
            vpiModOp => Ok(Val::Real(real(0)? % real(1)?)),
            vpiPowerOp => Ok(Val::Real(real(0)?.powf(real(1)?))),
            vpiAddOp => Ok(Val::Real(real(0)? + real(1)?)),
            vpiMultOp => Ok(Val::Real(real(0)? * real(1)?)),
            vpiEqOp => Ok(bit(real(0)? == real(1)?)),
            vpiNeqOp => Ok(bit(real(0)? != real(1)?)),
            vpiGtOp => Ok(bit(real(0)? > real(1)?)),
            vpiGeOp => Ok(bit(real(0)? >= real(1)?)),
            vpiLtOp => Ok(bit(real(0)? < real(1)?)),
            vpiLeOp => Ok(bit(real(0)? <= real(1)?)),
            vpiLogAndOp => Ok(bit(boolean(0)? && boolean(1)?)),
            vpiLogOrOp => Ok(bit(boolean(0)? || boolean(1)?)),
            vpiConditionOp => {
                let selected = if boolean(0)? { &vals[1] } else { &vals[2] };
                match (&vals[1], &vals[2], selected) {
                    (Val::Bits(_), Val::Bits(_), Val::Bits(v)) => Ok(Val::Bits(v.clone())),
                    _ => Ok(Val::Real(real(if boolean(0)? { 1 } else { 2 })?)),
                }
            }
            vpiMinTypMaxOp => vals
                .first()
                .cloned()
                .ok_or_else(|| ElabError::Unsupported("missing operand".to_string())),
            vpiCastOp => {
                let ts = child(vpi::vpiTypespec, op)
                    .ok_or_else(|| ElabError::Unsupported("cast without typespec".to_string()))?;
                let ty = vpi::get(vpi::vpiType, ts.raw());
                if ty == vpi::vpiRealTypespec || ty == vpi::vpiShortRealTypespec {
                    let value = real(0)?;
                    Ok(Val::Real(if ty == vpi::vpiShortRealTypespec {
                        (value as f32) as f64
                    } else {
                        value
                    }))
                } else {
                    let (width, signed) = self
                        .typespec_size(sc, resolved, in_progress, ts.raw())?
                        .ok_or_else(|| {
                            ElabError::Unsupported("cast to unsizable type".to_string())
                        })?;
                    if width > 64 {
                        return Err(ElabError::Unsupported(
                            "real constant cast target wider than 64 bits".to_string(),
                        ));
                    }
                    Ok(Val::Bits(real_to_bits(real(0)?, width, signed)))
                }
            }
            vpiUnaryAndOp | vpiUnaryNandOp | vpiUnaryOrOp | vpiUnaryNorOp | vpiUnaryXorOp
            | vpiUnaryXNorOp | vpiBitNegOp | vpiBitAndOp | vpiBitOrOp | vpiBitXorOp
            | vpiBitXNorOp | vpiLShiftOp | vpiRShiftOp | vpiArithLShiftOp | vpiArithRShiftOp
            | vpiCaseEqOp | vpiCaseNeqOp | vpiWildEqOp | vpiWildNeqOp | vpiConcatOp
            | vpiMultiConcatOp => Err(ElabError::Unsupported(
                "unsupported real constant expression operation".to_string(),
            )),
            other => Err(ElabError::Unsupported(format!(
                "operation op type {other} with real operand"
            ))),
        }
    }

    fn eval_operation(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        op: VpiHandle,
    ) -> Result<Val, ElabError> {
        let otype = vpi::get(vpi::vpiOpType, op);
        let op_handles = iter(vpi::vpiOperand, op);
        let ops: Vec<VpiHandle> = op_handles.iter().map(OwnedHandle::raw).collect();
        let vals: Vec<Val> = ops
            .iter()
            .map(|h| self.eval_expr_ctx(sc, resolved, in_progress, frame, *h))
            .collect::<Result<_, _>>()?;
        if vals.iter().any(|v| matches!(v, Val::Real(_))) {
            return self.eval_real_operation(sc, resolved, in_progress, op, &vals);
        }
        let u = |i: usize| -> Result<Value, ElabError> {
            match vals.get(i) {
                Some(Val::Bits(v)) => Ok(v.clone()),
                Some(Val::Str(s)) => Err(ElabError::Unsupported(format!(
                    "string operand {:?} in arithmetic",
                    s
                ))),
                Some(Val::Real(_)) => unreachable!("real operation handled above"),
                None => Err(ElabError::Unsupported("missing operand".to_string())),
            }
        };

        use vpi::*;
        match otype {
            vpiMinusOp => Ok(Val::Bits(minus(&u(0)?))),
            vpiPlusOp => Ok(Val::Bits(u(0)?)),
            vpiNotOp => Ok(Val::Bits(log_not(&u(0)?))),
            vpiBitNegOp => Ok(Val::Bits(bit_neg(&u(0)?))),
            vpiUnaryAndOp => Ok(Val::Bits(unary_and(&u(0)?))),
            vpiUnaryNandOp => Ok(Val::Bits(unary_nand(&u(0)?))),
            vpiUnaryOrOp => Ok(Val::Bits(unary_or(&u(0)?))),
            vpiUnaryNorOp => Ok(Val::Bits(unary_nor(&u(0)?))),
            vpiUnaryXorOp => Ok(Val::Bits(unary_xor(&u(0)?))),
            vpiUnaryXNorOp => Ok(Val::Bits(unary_xnor(&u(0)?))),
            vpiSubOp => Ok(Val::Bits(sub(&u(0)?, &u(1)?))),
            vpiDivOp => Ok(Val::Bits(div(&u(0)?, &u(1)?))),
            vpiModOp => Ok(Val::Bits(rem(&u(0)?, &u(1)?))),
            vpiEqOp => Ok(Val::Bits(eq(&u(0)?, &u(1)?))),
            vpiNeqOp => Ok(Val::Bits(neq(&u(0)?, &u(1)?))),
            vpiCaseEqOp => Ok(Val::Bits(case_eq(&u(0)?, &u(1)?))),
            vpiCaseNeqOp => Ok(Val::Bits(case_neq(&u(0)?, &u(1)?))),
            vpiWildEqOp => Ok(Val::Bits(wildcard_eq(&u(0)?, &u(1)?))),
            vpiWildNeqOp => Ok(Val::Bits(wildcard_neq(&u(0)?, &u(1)?))),
            vpiGtOp => Ok(Val::Bits(gt(&u(0)?, &u(1)?))),
            vpiGeOp => Ok(Val::Bits(ge(&u(0)?, &u(1)?))),
            vpiLtOp => Ok(Val::Bits(lt(&u(0)?, &u(1)?))),
            vpiLeOp => Ok(Val::Bits(le(&u(0)?, &u(1)?))),
            vpiLShiftOp => Ok(Val::Bits(shl(&u(0)?, &u(1)?))),
            vpiRShiftOp => Ok(Val::Bits(shr(&u(0)?, &u(1)?))),
            vpiArithLShiftOp => Ok(Val::Bits(arith_shl(&u(0)?, &u(1)?))),
            vpiArithRShiftOp => Ok(Val::Bits(arith_shr(&u(0)?, &u(1)?))),
            vpiAddOp => Ok(Val::Bits(add(&u(0)?, &u(1)?))),
            vpiMultOp => Ok(Val::Bits(mul(&u(0)?, &u(1)?))),
            vpiPowerOp => Ok(Val::Bits(power(&u(0)?, &u(1)?))),
            vpiLogAndOp => Ok(Val::Bits(log_and(&u(0)?, &u(1)?))),
            vpiLogOrOp => Ok(Val::Bits(log_or(&u(0)?, &u(1)?))),
            vpiBitAndOp => Ok(Val::Bits(bit_and(&u(0)?, &u(1)?))),
            vpiBitOrOp => Ok(Val::Bits(bit_or(&u(0)?, &u(1)?))),
            vpiBitXorOp => Ok(Val::Bits(bit_xor(&u(0)?, &u(1)?))),
            vpiBitXNorOp => Ok(Val::Bits(bit_xnor(&u(0)?, &u(1)?))),
            vpiConditionOp => {
                let sel = u(0)?;
                let a = u(1)?;
                let b = u(2)?;
                Ok(Val::Bits(cond(&sel, &a, &b)))
            }
            vpiMinTypMaxOp => Ok(Val::Bits(u(0)?)),
            vpiConcatOp => {
                // Surelog's reorderAssignmentPattern reverses concat operands
                // (and sets vpiReordered) when the target range is descending;
                // nested concats are left in source order.  Reverse only when
                // the flag is set so MSB-first concatenation is preserved.
                let reordered = vpi::get(vpi::vpiReordered, op) != 0;
                let mut parts = Vec::with_capacity(ops.len());
                for i in 0..ops.len() {
                    parts.push(u(i)?);
                }
                if reordered {
                    parts.reverse();
                }
                Ok(Val::Bits(concat(&parts)))
            }
            vpiMultiConcatOp => {
                let count = u(0)?;
                if count.is_unknown() {
                    return Err(ElabError::Unsupported(
                        "unknown replication count".to_string(),
                    ));
                }
                let n: usize = count
                    .to_u128()
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| {
                        ElabError::Unsupported(
                            "replication count does not fit in usize".to_string(),
                        )
                    })?;
                let mut parts = Vec::with_capacity(ops.len().saturating_sub(1));
                for i in 1..ops.len() {
                    parts.push(u(i)?);
                }
                let pat = concat(&parts);
                let total_width = pat.width().checked_mul(n).ok_or_else(|| {
                    ElabError::Unsupported("replication width overflow".to_string())
                })?;
                if total_width > MAX_RESOLVED_BITS {
                    return Err(ElabError::Unsupported(format!(
                        "replication result is too wide ({total_width} bits)"
                    )));
                }
                let mut bits = Vec::with_capacity(total_width);
                for _ in 0..n {
                    bits.extend(pat.bits.iter().cloned());
                }
                Ok(Val::Bits(Value::from_bits(bits, false)))
            }
            vpiCastOp => {
                let v = u(0)?;
                let ts = child(vpi::vpiTypespec, op)
                    .ok_or_else(|| ElabError::Unsupported("cast without typespec".to_string()))?;
                let (w, signed) = self
                    .typespec_size(sc, resolved, in_progress, ts.raw())?
                    .ok_or_else(|| ElabError::Unsupported("cast to unsizable type".to_string()))?;
                // Value-preserving cast (§6.24.1): extension by the SOURCE's
                // signedness, result tagged with the cast type. The cast's
                // target width materializes an unbased unsized fill before
                // any enclosing assignment supplies a second context.
                let mut cast = v.cast(w, signed);
                cast.fill = None;
                Ok(Val::Bits(cast))
            }
            other => Err(ElabError::Unsupported(format!("operation op type {other}"))),
        }
    }

    /// Resolve a `ref_obj` / `var_select` to a value.  Names bound in the
    /// current function frame (io_decls, locals, the return var) resolve from
    /// it first; refs bound to a concrete `parameter` via `vpiActual` (and
    /// unbound refs) resolve by name in the current scope; refs to nets/vars
    /// are not resolvable.
    fn eval_ref(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        r: VpiHandle,
    ) -> Result<Val, ElabError> {
        if let Some(v) = frame.get(&vpi::obj_name(r)) {
            return Ok(v.clone());
        }
        if let Some(actual) = child(vpi::vpiActual, r) {
            let t = vpi::obj_type(actual.raw());
            if matches!(
                t,
                vpi::vpiParameter | vpi::vpiSpecParam | vpi::vpiTypeParameter
            ) {
                let name = vpi::obj_name(actual.raw());
                return self.resolve_param(sc, resolved, in_progress, &name);
            }
            if matches!(
                t,
                vpi::vpiNet
                    | vpi::vpiNetBit
                    | vpi::vpiReg
                    | vpi::vpiIntegerVar
                    | vpi::vpiLogicVar
                    | vpi::vpiArrayVar
                    | vpi::vpiRegArray
                    | vpi::vpiIntVar
                    | vpi::vpiLongIntVar
                    | vpi::vpiShortIntVar
                    | vpi::vpiByteVar
                    | vpi::vpiBitVar
                    | vpi::vpiStringVar
            ) {
                return Err(ElabError::Unsupported(format!(
                    "ref to runtime variable {}",
                    vpi::obj_name(actual.raw())
                )));
            }
        }
        let name = vpi::obj_name(r);
        if name.is_empty() {
            return Err(ElabError::Unsupported(
                "unbound ref without name".to_string(),
            ));
        }
        self.resolve_param(sc, resolved, in_progress, &name)
    }

    /// Resolve the base object of a select (`bit_select`, `part_select`,
    /// `indexed_part_select`) to a bit vector.  The base is reached through
    /// `vpiActual` (selects are `ref_obj` subclasses) or by name.
    fn select_base(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        sel: VpiHandle,
    ) -> Result<Value, ElabError> {
        if let Some(v) = frame.get(&vpi::obj_name(sel)) {
            if let Val::Bits(b) = v {
                return Ok(b.clone());
            }
            return Err(ElabError::Unsupported("select on string value".to_string()));
        }
        if let Some(actual) = child(vpi::vpiActual, sel) {
            if matches!(
                vpi::obj_type(actual.raw()),
                vpi::vpiParameter | vpi::vpiSpecParam
            ) {
                let name = vpi::obj_name(actual.raw());
                let v = self.resolve_param(sc, resolved, in_progress, &name)?;
                match v {
                    Val::Bits(b) => return Ok(b),
                    Val::Str(_) | Val::Real(_) => {
                        return Err(ElabError::Unsupported("select on string value".to_string()))
                    }
                }
            }
        }
        let name = vpi::obj_name(sel);
        if !name.is_empty() {
            if let Ok(Val::Bits(b)) = self.resolve_param(sc, resolved, in_progress, &name) {
                return Ok(b);
            }
        }
        Err(ElabError::Unsupported(format!(
            "select base not resolvable: {}",
            vpi::obj_name(sel)
        )))
    }

    fn eval_bit_select(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        sel: VpiHandle,
    ) -> Result<Val, ElabError> {
        let base = self.select_base(sc, resolved, in_progress, frame, sel)?;
        let idx = child(vpi::vpiIndex, sel)
            .ok_or_else(|| ElabError::Unsupported("bit_select without index".to_string()))?;
        let iv = self.op_bits(sc, resolved, in_progress, frame, &[idx.raw()], 0)?;
        if iv.is_unknown() {
            return Ok(Val::Bits(all_x(1, false)));
        }
        let b = iv
            .to_u128()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|index| *index < base.width())
            .map_or(Bit::X, |index| base.bit_lsb(index));
        Ok(Val::Bits(Value::from_bits(vec![b], false)))
    }

    fn eval_part_select(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        sel: VpiHandle,
    ) -> Result<Val, ElabError> {
        let base = self.select_base(sc, resolved, in_progress, frame, sel)?;
        let l = self.range_bound(sc, resolved, in_progress, vpi::vpiLeftRange, sel)?;
        let r = self.range_bound(sc, resolved, in_progress, vpi::vpiRightRange, sel)?;
        let w = base.width() as i128;
        if l < 0 || r < 0 || l >= w || r >= w {
            // Out-of-range part select → X of the part width.
            let width = checked_inclusive_width(l, r, "part select")?;
            return Ok(Val::Bits(all_x(width, false)));
        }
        let mut bits = Vec::new();
        if l > r {
            for i in (r..=l).rev() {
                bits.push(base.bit_lsb(i as usize));
            }
        } else {
            for i in l..=r {
                bits.push(base.bit_lsb(i as usize));
            }
        }
        Ok(Val::Bits(Value::from_bits(bits, false)))
    }

    fn eval_indexed_part_select(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        sel: VpiHandle,
    ) -> Result<Val, ElabError> {
        let base = self.select_base(sc, resolved, in_progress, frame, sel)?;
        let idx_h = child(vpi::vpiBaseExpr, sel).ok_or_else(|| {
            ElabError::Unsupported("indexed_part_select without index".to_string())
        })?;
        let width = match child(vpi::vpiWidthExpr, sel) {
            Some(wh) => {
                let wv = self.op_bits(sc, resolved, in_progress, frame, &[wh.raw()], 0)?;
                if wv.is_unknown() {
                    return Err(ElabError::Unsupported(
                        "unknown indexed_part_select width".to_string(),
                    ));
                }
                wv.to_u128()
                    .and_then(|value| i128::try_from(value).ok())
                    .ok_or_else(|| {
                        ElabError::Unsupported(
                            "indexed_part_select width does not fit in i128".to_string(),
                        )
                    })?
            }
            None => vpi::get(vpi::vpiSize, sel) as i128,
        };
        let width = usize::try_from(width)
            .ok()
            .filter(|width| *width > 0 && *width <= MAX_RESOLVED_BITS)
            .ok_or_else(|| {
                ElabError::Unsupported(format!(
                    "indexed_part_select width must be in 1..={MAX_RESOLVED_BITS}"
                ))
            })?;
        let iv = self.op_bits(sc, resolved, in_progress, frame, &[idx_h.raw()], 0)?;
        if iv.is_unknown() {
            return Ok(Val::Bits(all_x(width, false)));
        }
        let i = if iv.signed {
            iv.to_i128()
        } else {
            iv.to_u128().and_then(|value| value.try_into().ok())
        }
        .ok_or_else(|| {
            ElabError::Unsupported("indexed_part_select base does not fit in i128".to_string())
        })?;
        let pos = vpi::get(vpi::vpiIndexedPartSelectType, sel);
        let w = base.width() as i128;
        let mut bits = Vec::with_capacity(width);
        let top_offset = i128::try_from(width - 1).map_err(|_| {
            ElabError::Unsupported("indexed_part_select offset overflow".to_string())
        })?;
        for offset in 0..width {
            let offset = i128::try_from(offset).map_err(|_| {
                ElabError::Unsupported("indexed_part_select offset overflow".to_string())
            })?;
            let idx = if pos == vpi::vpiNegIndexed {
                i.checked_sub(offset)
            } else {
                i.checked_add(top_offset - offset)
            }
            .ok_or_else(|| {
                ElabError::Unsupported("indexed_part_select endpoint overflow".to_string())
            })?;
            bits.push(if idx >= 0 && idx < w {
                base.bit_lsb(idx as usize)
            } else {
                Bit::X
            });
        }
        Ok(Val::Bits(Value::from_bits(bits, false)))
    }

    fn eval_sys_func(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        call: VpiHandle,
    ) -> Result<Val, ElabError> {
        let name = vpi::obj_name(call);
        let arg_handles = iter(vpi::vpiArgument, call);
        let args: Vec<VpiHandle> = arg_handles.iter().map(OwnedHandle::raw).collect();
        match name.as_str() {
            "$rtoi" | "$itor" | "$realtobits" | "$bitstoreal" | "$shortrealtobits"
            | "$bitstoshortreal" => {
                let [arg] = args.as_slice() else {
                    return Err(ElabError::Unsupported(format!(
                        "{name} requires exactly one argument"
                    )));
                };
                let arg = self.eval_expr_ctx(sc, resolved, in_progress, frame, *arg)?;
                match (name.as_str(), arg) {
                    ("$rtoi", Val::Real(value)) => Ok(Val::Bits(rtoi_value(value))),
                    ("$rtoi", Val::Bits(value)) => Ok(Val::Bits(rtoi_value(value.to_real()))),
                    ("$itor", Val::Bits(value)) => Ok(Val::Real(value.to_real())),
                    ("$itor", Val::Real(value)) => {
                        Ok(Val::Real(real_to_bits(value, 32, true).to_real()))
                    }
                    ("$realtobits", Val::Real(value)) => Ok(Val::Bits(real_to_ieee_bits(value))),
                    ("$realtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(real_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoreal", Val::Bits(value)) => {
                        ieee_bits_to_real(&value).map(Val::Real).ok_or_else(|| {
                            ElabError::Unsupported(
                                "$bitstoreal requires an exactly 64-bit packed argument"
                                    .to_string(),
                            )
                        })
                    }
                    ("$shortrealtobits", Val::Real(value)) => {
                        Ok(Val::Bits(shortreal_to_ieee_bits(value)))
                    }
                    ("$shortrealtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(shortreal_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoshortreal", Val::Bits(value)) => ieee_bits_to_shortreal(&value)
                        .map(Val::Real)
                        .ok_or_else(|| {
                            ElabError::Unsupported(
                                "$bitstoshortreal requires an exactly 32-bit packed argument"
                                    .to_string(),
                            )
                        }),
                    ("$rtoi" | "$itor" | "$realtobits", _) => Err(ElabError::Unsupported(format!(
                        "{name} requires a numeric argument"
                    ))),
                    ("$bitstoreal", _) => Err(ElabError::Unsupported(
                        "$bitstoreal requires an exactly 64-bit packed argument".to_string(),
                    )),
                    ("$shortrealtobits", _) => Err(ElabError::Unsupported(
                        "$shortrealtobits requires a numeric argument".to_string(),
                    )),
                    _ => Err(ElabError::Unsupported(
                        "$bitstoshortreal requires an exactly 32-bit packed argument".to_string(),
                    )),
                }
            }
            "$countones" | "$onehot" | "$onehot0" | "$isunknown" => {
                if args.len() != 1 {
                    return Err(ElabError::Unsupported(format!(
                        "{name} requires exactly one argument"
                    )));
                }
                let arg = self.op_bits(sc, resolved, in_progress, frame, &args, 0)?;
                let value = match name.as_str() {
                    "$countones" => countones(&arg),
                    "$onehot" => onehot(&arg),
                    "$onehot0" => onehot0(&arg),
                    _ => isunknown(&arg),
                };
                Ok(Val::Bits(value))
            }
            "$clog2" => {
                let a = self.op_bits(sc, resolved, in_progress, frame, &args, 0)?;
                Ok(Val::Bits(clog2(&a)))
            }
            "$bits" => {
                let a = self.op_bits(sc, resolved, in_progress, frame, &args, 0)?;
                Ok(Val::Bits(Value::from_u64(a.width() as u64, 32, true)))
            }
            "$signed" => {
                let a = self.op_bits(sc, resolved, in_progress, frame, &args, 0)?;
                Ok(Val::Bits(Value {
                    bits: a.bits,
                    signed: true,
                    fill: a.fill,
                }))
            }
            "$unsigned" => {
                let a = self.op_bits(sc, resolved, in_progress, frame, &args, 0)?;
                Ok(Val::Bits(Value {
                    bits: a.bits,
                    signed: false,
                    fill: a.fill,
                }))
            }
            _ => Err(ElabError::Unsupported(format!("system function {name}"))),
        }
    }

    // ── Constant function evaluation ─────────────────────────────────────────

    /// Evaluate a constant `func_call` (e.g. `localparam int X = f(3);`).
    ///
    /// Resolves the callee through `vpiFunction`, binds the positional
    /// arguments in formal order (missing or synthetic placeholders fall back
    /// to the formal's default expression), and interprets the body with a
    /// local name→value frame (io_decls, locals, the function-name return
    /// var).  Recursion is detected through the shared `in_progress` set
    /// (function full names), which also bounds the nesting depth.
    fn eval_func_call(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &HashMap<String, Val>,
        call: VpiHandle,
    ) -> Result<Val, ElabError> {
        let func = child(vpi::vpiFunction, call)
            .ok_or_else(|| ElabError::Unsupported("function call without callee".to_string()))?;
        let func = func.raw();
        let fname = vpi::obj_full_name(func);
        if in_progress.contains(&fname) {
            return Err(ElabError::CycleDetected(format!(
                "recursive constant function {fname}"
            )));
        }
        if in_progress.len() >= MAX_FUNC_DEPTH {
            return Err(ElabError::Unsupported(format!(
                "constant function recursion limit ({MAX_FUNC_DEPTH}) exceeded in {fname}"
            )));
        }
        in_progress.insert(fname.clone());

        let io_handles = iter(vpi::vpiIODecl, func);
        let io: Vec<VpiHandle> = io_handles.iter().map(OwnedHandle::raw).collect();
        let arg_handles = iter(vpi::vpiArgument, call);
        let args: Vec<VpiHandle> = arg_handles.iter().map(OwnedHandle::raw).collect();

        let mut callee_frame: HashMap<String, Val> = HashMap::new();
        let mut decl: HashMap<String, (usize, bool)> = HashMap::new();
        for (i, io_d) in io.iter().enumerate() {
            let name = vpi::obj_name(*io_d);
            let ts = child(vpi::vpiTypedef, *io_d)
                .ok_or_else(|| ElabError::Unsupported("formal without typespec".to_string()))?;
            if let Some((w, s)) = self.typespec_size(sc, resolved, in_progress, ts.raw())? {
                decl.insert(name.clone(), (w, s));
            }
            let val = match args.get(i) {
                Some(a) if !is_synthetic_arg(*a) => {
                    self.eval_expr_ctx(sc, resolved, in_progress, frame, *a)?
                }
                _ => match child(vpi::vpiExpr, *io_d) {
                    // Defaults are written in the callee's scope and may
                    // reference earlier formals (`input b = a + 1`); evaluate
                    // them against the accumulating callee frame so those
                    // formals are in scope.
                    Some(d) => {
                        self.eval_expr_ctx(sc, resolved, in_progress, &callee_frame, d.raw())?
                    }
                    None => {
                        return Err(ElabError::Unsupported(format!(
                            "missing argument for formal `{name}` of {fname}"
                        )))
                    }
                },
            };
            // Assignment into the formal pads by the argument's signedness
            // (LRM §10.7).
            let val = match (&val, decl.get(&name)) {
                (Val::Bits(v), Some((w, s))) => Val::Bits(v.cast(*w, *s)),
                _ => val,
            };
            callee_frame.insert(name, val);
        }

        // The function-name return variable (initialized to all-X) and the
        // body's locals, both from their declared widths.
        let ret_ts = child(vpi::vpiReturn, func).and_then(|rv| rv.child(vpi::vpiTypespec));
        let ret_name = vpi::obj_name(func);
        let ret_decl = match &ret_ts {
            Some(ts) => self.typespec_size(sc, resolved, in_progress, ts.raw())?,
            None => None,
        };
        if let Some((w, s)) = ret_decl {
            decl.insert(ret_name.clone(), (w, s));
            callee_frame.insert(ret_name.clone(), Val::Bits(all_x(w, s)));
        }
        if let Some(body) = child(vpi::vpiStmt, func) {
            self.collect_func_locals(
                sc,
                resolved,
                in_progress,
                &mut callee_frame,
                &mut decl,
                body.raw(),
            )?;
        }

        // Interpret the body; an explicit `return` short-circuits.
        let body = child(vpi::vpiStmt, func)
            .ok_or_else(|| ElabError::Unsupported("function without body".to_string()))?;
        let ret_name_opt = ret_decl.map(|_| ret_name.as_str());
        let explicit = self.eval_func_stmt(
            sc,
            resolved,
            in_progress,
            &mut callee_frame,
            &decl,
            ret_name_opt,
            body.raw(),
        )?;
        in_progress.remove(&fname);

        let result = match (explicit, ret_decl) {
            // Falling off the end returns the function-name variable.
            (None, Some(_)) => callee_frame
                .get(&ret_name)
                .cloned()
                .ok_or_else(|| ElabError::Unsupported("no value for return var".to_string()))?,
            (Some(v), _) => v,
            (None, None) => {
                return Err(ElabError::Unsupported(format!(
                    "void function {fname} used as a value"
                )))
            }
        };
        match (&result, ret_decl) {
            // The return value is assigned to the declared return type.
            (Val::Bits(v), Some((w, s))) => Ok(Val::Bits(v.cast(w, s))),
            _ => Ok(result),
        }
    }

    /// Collect the locals declared by a function body's `begin` blocks into
    /// `frame` (all-X of the declared width) and record their widths in
    /// `decl`.  Nested begins are walked recursively.
    fn collect_func_locals(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &mut HashMap<String, Val>,
        decl: &mut HashMap<String, (usize, bool)>,
        stmt: VpiHandle,
    ) -> Result<(), ElabError> {
        if matches!(vpi::obj_type(stmt), vpi::vpiBegin | vpi::vpiNamedBegin) {
            for v in iter(vpi::vpiVariables, stmt) {
                let v = v.raw();
                let name = vpi::obj_name(v);
                if decl.contains_key(&name) {
                    continue;
                }
                let ts = child(vpi::vpiTypespec, v)
                    .ok_or_else(|| ElabError::Unsupported("local without typespec".to_string()))?;
                let (w, s) = self
                    .typespec_size(sc, resolved, in_progress, ts.raw())?
                    .unwrap_or((1, false));
                decl.insert(name.clone(), (w, s));
                frame.insert(name.clone(), Val::Bits(all_x(w, s)));
            }
            for s in iter(vpi::vpiStmt, stmt) {
                self.collect_func_locals(sc, resolved, in_progress, frame, decl, s.raw())?;
            }
        }
        Ok(())
    }

    /// Interpret one function-body statement against `frame`.  Returns
    /// `Ok(Some(v))` when a `return` statement produced the function's value
    /// (a bare `return;` yields the function-name variable's value), and
    /// `Ok(None)` when execution falls off the end of a block.  `ret_name` is
    /// the function name for non-void functions (`None` for void).
    // The interpreter threads separate immutable scope/declaration context and
    // mutable parameter/frame state; bundling them would obscure ownership.
    #[allow(clippy::too_many_arguments)]
    fn eval_func_stmt(
        &self,
        sc: &Scope,
        resolved: &mut HashMap<String, Val>,
        in_progress: &mut HashSet<String>,
        frame: &mut HashMap<String, Val>,
        decl: &HashMap<String, (usize, bool)>,
        ret_name: Option<&str>,
        stmt: VpiHandle,
    ) -> Result<Option<Val>, ElabError> {
        match vpi::obj_type(stmt) {
            vpi::vpiBegin | vpi::vpiNamedBegin => {
                for s in iter(vpi::vpiStmt, stmt) {
                    if let Some(v) = self.eval_func_stmt(
                        sc,
                        resolved,
                        in_progress,
                        frame,
                        decl,
                        ret_name,
                        s.raw(),
                    )? {
                        return Ok(Some(v));
                    }
                }
                Ok(None)
            }
            vpi::vpiIf | vpi::vpiIfElse => {
                let cond = child(vpi::vpiCondition, stmt)
                    .ok_or_else(|| ElabError::Unsupported("if without condition".to_string()))?;
                let take_then =
                    match self.eval_expr_ctx(sc, resolved, in_progress, frame, cond.raw())? {
                        Val::Bits(b) => {
                            if b.is_unknown() {
                                return Err(ElabError::Unsupported(
                                    "unknown if condition in constant function".to_string(),
                                ));
                            }
                            b.bits.contains(&Bit::One)
                        }
                        Val::Str(_) | Val::Real(_) => {
                            return Err(ElabError::Unsupported(
                                "string if condition in constant function".to_string(),
                            ))
                        }
                    };
                if take_then {
                    let then = child(vpi::vpiStmt, stmt).ok_or_else(|| {
                        ElabError::Unsupported("if without then branch".to_string())
                    })?;
                    self.eval_func_stmt(
                        sc,
                        resolved,
                        in_progress,
                        frame,
                        decl,
                        ret_name,
                        then.raw(),
                    )
                } else if let Some(els) = child(vpi::vpiElseStmt, stmt) {
                    self.eval_func_stmt(sc, resolved, in_progress, frame, decl, ret_name, els.raw())
                } else {
                    Ok(None)
                }
            }
            vpi::vpiCase => {
                let sel = child(vpi::vpiCondition, stmt)
                    .ok_or_else(|| ElabError::Unsupported("case without selector".to_string()))?;
                let sel = match self.eval_expr_ctx(sc, resolved, in_progress, frame, sel.raw())? {
                    Val::Bits(b) => b,
                    Val::Str(_) | Val::Real(_) => {
                        return Err(ElabError::Unsupported(
                            "string case selector in constant function".to_string(),
                        ))
                    }
                };
                let mut default: Option<OwnedHandle> = None;
                for item in iter(vpi::vpiCaseItem, stmt) {
                    let exprs: Vec<OwnedHandle> = item
                        .iterate(vpi::vpiExpr)
                        .map(|expressions| expressions.collect())
                        .unwrap_or_default();
                    if exprs.is_empty() {
                        default = Some(item);
                        continue;
                    }
                    for e in exprs {
                        let ev =
                            match self.eval_expr_ctx(sc, resolved, in_progress, frame, e.raw())? {
                                Val::Bits(b) => b,
                                Val::Str(_) | Val::Real(_) => continue,
                            };
                        if case_eq(&sel, &ev).to_u64() == Some(1) {
                            return match child(vpi::vpiStmt, item.raw()) {
                                Some(body) => self.eval_func_stmt(
                                    sc,
                                    resolved,
                                    in_progress,
                                    frame,
                                    decl,
                                    ret_name,
                                    body.raw(),
                                ),
                                None => Ok(None),
                            };
                        }
                    }
                }
                if let Some(d) = default {
                    return match child(vpi::vpiStmt, d.raw()) {
                        Some(body) => self.eval_func_stmt(
                            sc,
                            resolved,
                            in_progress,
                            frame,
                            decl,
                            ret_name,
                            body.raw(),
                        ),
                        None => Ok(None),
                    };
                }
                Ok(None)
            }
            vpi::vpiAssignment => {
                let lhs = child(vpi::vpiLhs, stmt)
                    .ok_or_else(|| ElabError::Unsupported("assignment without LHS".to_string()))?;
                let name = vpi::obj_name(lhs.raw());
                if name.is_empty() {
                    return Err(ElabError::Unsupported(
                        "select assignment LHS in constant function".to_string(),
                    ));
                }
                let rhs = child(vpi::vpiRhs, stmt)
                    .ok_or_else(|| ElabError::Unsupported("assignment without RHS".to_string()))?;
                let v = self.eval_expr_ctx(sc, resolved, in_progress, frame, rhs.raw())?;
                // Assignment pads by the RHS signedness (LRM §10.7).
                let v = match (&v, decl.get(&name)) {
                    (Val::Bits(b), Some((w, s))) => Val::Bits(b.cast(*w, *s)),
                    _ => v,
                };
                frame.insert(name, v);
                Ok(None)
            }
            vpi::vpiReturnStmt => match child(vpi::vpiCondition, stmt) {
                Some(v) => Ok(Some(self.eval_expr_ctx(
                    sc,
                    resolved,
                    in_progress,
                    frame,
                    v.raw(),
                )?)),
                None => {
                    // Bare `return;` in a non-void function: the value of the
                    // function-name variable.
                    let name = ret_name.ok_or_else(|| {
                        ElabError::Unsupported("bare return in void constant function".to_string())
                    })?;
                    Ok(Some(frame.get(name).cloned().ok_or_else(|| {
                        ElabError::Unsupported("no value for return var".to_string())
                    })?))
                }
            },
            other => Err(ElabError::Unsupported(format!(
                "statement type {other} in constant function"
            ))),
        }
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Resolver::new()
    }
}

// ── Value reading from the VPI value API ──────────────────────────────────────

/// Read an object's value via [`vpi::read_value`] and convert it to a `Val`.
///
/// The bit width of string-format values (BIN/OCT/HEX/DEC) comes from the
/// string itself: Surelog's `vpiSize` on constants carries the *context* width
/// (e.g. a `4'h5` override is stored with size 8 or 16), which is not the
/// literal width.  `vpiSize == -1` marks an unsized fill literal (`'1`, `'x`).
pub fn read_value(h: VpiHandle) -> Result<Val, ElabError> {
    let size = vpi::get(vpi::vpiSize, h);
    decode_value_data(&vpi::read_value(h), size)
}

/// Decode an owned VPI value captured by [`crate::core::db`] without needing
/// the live VPI handle.  This is the single validation path for elaboration
/// and simulator-bound constants.
pub fn decode_value_data(value: &ValueData, size: c_int) -> Result<Val, ElabError> {
    match value {
        ValueData::Bin(s) => Ok(Val::Bits(parse_radix(s, 1, size)?)),
        ValueData::Oct(s) => Ok(Val::Bits(parse_radix(s, 3, size)?)),
        ValueData::Hex(s) => Ok(Val::Bits(parse_radix(s, 4, size)?)),
        ValueData::Dec(s) => Ok(Val::Bits(parse_decimal(s, size)?)),
        ValueData::Scalar(sc) => {
            let b = match *sc {
                vpi::vpi0 => Bit::Zero,
                vpi::vpi1 | vpi::vpiH => Bit::One,
                vpi::vpiZ => Bit::Z,
                vpi::vpiL => Bit::Zero,
                _ => Bit::X,
            };
            let fill = if size == -1 { Some(b) } else { None };
            Ok(Val::Bits(Value {
                bits: vec![b],
                signed: false,
                fill,
            }))
        }
        ValueData::Int(val) => {
            let width = stored_width(size, 32)?;
            let stored = Value::from_u64(*val as u64, width.min(u64::BITS as usize), true);
            Ok(Val::Bits(stored.resize(width, true)))
        }
        ValueData::UInt(val) => {
            let width = stored_width(size, 64)?;
            Ok(Val::Bits(Value::from_u64(*val, width, false)))
        }
        ValueData::Str(s) => Ok(Val::Str(s.clone())),
        ValueData::Real(v) => Ok(Val::Real(*v)),
        ValueData::None => Err(ElabError::NoValue("object has no stored value".to_string())),
        ValueData::Vector(_) => Err(ElabError::Unsupported("vector value format".to_string())),
        ValueData::TooWide { bits } => Err(ElabError::Unsupported(format!(
            "stored vector value is too wide ({bits} bits)"
        ))),
    }
}

/// Parse a BIN/OCT/HEX digit string (which may contain x/z) into bits.
/// `base_bits` is the bits per digit (1/3/4).  The stored string omits leading
/// zeros, so when `vpiSize` is positive the value is LSB-aligned into that
/// width (zero-extending when the string is shorter).  A one-digit string with
/// `size == -1` marks an unsized fill literal (`'1`, `'0`, `'x`, `'z`).
fn parse_radix(s: &str, base_bits: usize, size: c_int) -> Result<Value, ElabError> {
    let radix = match base_bits {
        1 => 2,
        3 => 8,
        4 => 16,
        _ => {
            return Err(ElabError::Unsupported(format!(
                "invalid stored radix width {base_bits}"
            )))
        }
    };
    let text = s.trim();
    if text.is_empty() {
        return Err(ElabError::NoValue("empty stored radix value".to_string()));
    }
    let encoded_width = text
        .len()
        .checked_mul(base_bits)
        .ok_or_else(|| ElabError::Unsupported("stored radix value width overflow".to_string()))?;
    let target_width = if size > 0 {
        stored_width(size, encoded_width)?
    } else {
        encoded_width
    };
    if encoded_width > MAX_RESOLVED_BITS || target_width > MAX_RESOLVED_BITS {
        return Err(ElabError::Unsupported(format!(
            "stored radix value is too wide ({} bits)",
            encoded_width.max(target_width)
        )));
    }
    let mut bits = Vec::with_capacity(encoded_width);
    for ch in text.chars() {
        let c = ch.to_ascii_lowercase();
        match c {
            'x' => bits.extend(std::iter::repeat_n(Bit::X, base_bits)),
            'z' | '?' => bits.extend(std::iter::repeat_n(Bit::Z, base_bits)),
            _ => {
                let d = c.to_digit(radix).ok_or_else(|| {
                    ElabError::Unsupported(format!(
                        "invalid base-{radix} digit `{ch}` in stored value"
                    ))
                })?;
                for i in (0..base_bits).rev() {
                    bits.push(if d & (1 << i) != 0 {
                        Bit::One
                    } else {
                        Bit::Zero
                    });
                }
            }
        }
    }
    let fill = if size == -1 && bits.len() == base_bits && base_bits == 1 {
        bits.first().copied()
    } else {
        None
    };
    if size > 0 {
        if bits.len() < target_width {
            bits.splice(
                0..0,
                std::iter::repeat_n(Bit::Zero, target_width - bits.len()),
            );
        } else if bits.len() > target_width {
            bits = bits[bits.len() - target_width..].to_vec();
        }
    }
    Ok(Value {
        bits,
        signed: false,
        fill,
    })
}

fn stored_width(size: c_int, default: usize) -> Result<usize, ElabError> {
    let width = if size > 0 { size as usize } else { default };
    if width > MAX_RESOLVED_BITS {
        Err(ElabError::Unsupported(format!(
            "stored value is too wide ({width} bits)"
        )))
    } else {
        Ok(width)
    }
}

/// Parse a decimal VPI payload directly into a fixed-width bit vector.  This
/// deliberately avoids an intermediate machine integer so wide parameters do
/// not truncate and malformed external values cannot silently become zero.
fn parse_decimal(s: &str, size: c_int) -> Result<Value, ElabError> {
    let width = stored_width(size, 64)?;
    let text = s.trim();
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ElabError::Unsupported(format!(
            "invalid stored decimal value `{text}`"
        )));
    }
    let ten = Value::from_u64(10, width, false);
    let mut value = zero(width, false);
    for digit in digits.bytes() {
        value = mul_known(&value, &ten, false);
        value = add_known(
            &value,
            &Value::from_u64(u64::from(digit - b'0'), width, false),
            false,
        );
    }
    if negative {
        value = negate_known(&value);
    }
    Ok(value)
}

// ── Unit tests: pure value math (no UHDM) ────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: &str) -> Value {
        Value::from_bits(
            s.chars()
                .map(|c| match c {
                    '0' => Bit::Zero,
                    '1' => Bit::One,
                    'x' | 'X' => Bit::X,
                    'z' | 'Z' => Bit::Z,
                    _ => panic!("bad test bit"),
                })
                .collect(),
            false,
        )
    }

    fn bits_signed(s: &str) -> Value {
        Value::from_bits(
            s.chars()
                .map(|c| match c {
                    '0' => Bit::Zero,
                    '1' => Bit::One,
                    'x' | 'X' => Bit::X,
                    'z' | 'Z' => Bit::Z,
                    _ => panic!("bad test bit"),
                })
                .collect(),
            true,
        )
    }

    fn v(v: u64, w: usize, s: bool) -> Value {
        Value::from_u64(v, w, s)
    }

    fn one_at(width: usize, lsb_index: usize) -> Value {
        let mut value = zero(width, false);
        value.bits[width - 1 - lsb_index] = Bit::One;
        value
    }

    #[test]
    fn packed_to_real_uses_all_bits_and_zeros_unknowns() {
        let wide = bits(&format!("1{}", "0".repeat(64)));
        assert_eq!(wide.to_real(), 18_446_744_073_709_551_616.0);
        assert_eq!(bits("1x01").to_real(), 9.0);
        assert_eq!(bits_signed("1x01").to_real(), -7.0);
        assert_eq!(bits_signed(&"1".repeat(128)).to_real(), -1.0);
    }

    #[test]
    fn add_carry_drop_and_unsigned() {
        let a = bits("1001");
        let b = bits("0001");
        assert_eq!(add(&a, &b), bits("1010"));
    }

    #[test]
    fn add_x_propagates() {
        let a = bits("10x1");
        let b = bits("0001");
        assert_eq!(add(&a, &b), bits("xxxx"));
    }

    #[test]
    fn add_signed() {
        let a = bits_signed("1111");
        let b = bits_signed("0001");
        assert_eq!(add(&a, &b), bits_signed("0000"));
    }

    #[test]
    fn sub_and_mul() {
        assert_eq!(sub(&bits("1010"), &bits("0011")), bits("0111"));
        // Multiplication is truncated to the max operand width (self-determined).
        assert_eq!(mul(&bits("0110"), &bits("0011")), bits("0010"));
    }

    #[test]
    fn arithmetic_coerces_to_common_width_and_signedness() {
        let signed_narrow = bits_signed("1111"); // -1 in four bits
        let signed_wide = bits_signed("00000001");
        assert_eq!(add(&signed_narrow, &signed_wide), bits_signed("00000000"));
        assert_eq!(sub(&signed_narrow, &signed_wide), bits_signed("11111110"));
        assert_eq!(mul(&signed_narrow, &signed_wide), bits_signed("11111111"));
        assert_eq!(div(&signed_narrow, &signed_wide), bits_signed("11111111"));
        assert_eq!(rem(&signed_narrow, &signed_wide), bits_signed("00000000"));

        let unsigned_wide = bits("00000001");
        // An unsigned operand makes the common arithmetic expression
        // unsigned, so the signed four-bit operand zero-extends to 15.
        assert_eq!(add(&signed_narrow, &unsigned_wide), bits("00010000"));
        assert_eq!(sub(&signed_narrow, &unsigned_wide), bits("00001110"));
        assert_eq!(mul(&signed_narrow, &unsigned_wide), bits("00001111"));
        assert_eq!(div(&signed_narrow, &unsigned_wide), bits("00001111"));
        assert_eq!(rem(&signed_narrow, &unsigned_wide), bits("00000000"));
    }

    #[test]
    fn div_and_rem() {
        assert_eq!(div(&v(10, 32, false), &v(3, 32, false)), v(3, 32, false));
        assert_eq!(rem(&v(10, 32, false), &v(3, 32, false)), v(1, 32, false));
        // Truncating signed division: -7 / 2 = -3, -7 % 2 = -1.
        assert_eq!(
            div(&v((-7i64) as u64, 32, true), &v(2, 32, true)),
            v((-3i64) as u64, 32, true)
        );
        assert_eq!(
            rem(&v((-7i64) as u64, 32, true), &v(2, 32, true)),
            v((-1i64) as u64, 32, true)
        );
        // A mixed signed/unsigned expression is unsigned: 32'hffff_fff9 / 2.
        assert_eq!(
            div(&v((-7i64) as u64, 32, true), &v(2, 32, false)),
            v(2_147_483_644, 32, false)
        );
        assert_eq!(
            rem(&v((-7i64) as u64, 32, true), &v(2, 32, false)),
            v(1, 32, false)
        );
        // The fixed-width two's-complement result of INT64_MIN / -1 keeps
        // the INT64_MIN bit pattern; the remainder is zero.
        let min = v(i64::MIN as u64, 64, true);
        let neg_one = v((-1i64) as u64, 64, true);
        assert_eq!(div(&min, &neg_one), min);
        assert_eq!(rem(&min, &neg_one), v(0, 64, true));
        // Division by zero → X.
        assert!(div(&v(10, 32, false), &v(0, 32, false)).is_unknown());
    }

    #[test]
    fn shifts() {
        assert_eq!(shl(&bits("0001"), &v(2, 8, false)), bits("0100"));
        assert_eq!(shr(&bits("1000"), &v(1, 8, false)), bits("0100"));
        // Known X/Z bits on the LHS move positionally; only an unknown shift
        // amount makes the entire result X.
        assert_eq!(shl(&bits("10z1"), &v(1, 4, false)), bits("0z10"));
        assert_eq!(shr(&bits("10z1"), &v(1, 4, false)), bits("010z"));
        assert_eq!(shr(&bits("x001"), &v(1, 4, false)), bits("0x00"));
        assert_eq!(shl(&bits("0001"), &bits("0x01")), bits("xxxx"));
        // Arithmetic right shift sign-extends.
        assert_eq!(
            arith_shr(&bits_signed("10000000"), &v(4, 8, false)),
            bits_signed("11111000")
        );
        assert_eq!(
            arith_shr(&bits_signed("z001"), &v(1, 4, false)),
            bits_signed("zz00")
        );
        assert_eq!(arith_shr(&bits("z001"), &v(1, 4, false)), bits("0z00"));
        assert_eq!(
            arith_shr(&bits_signed("z001"), &v(4, 4, false)),
            bits_signed("zzzz")
        );
        assert_eq!(arith_shl(&bits("0001"), &v(2, 8, false)), bits("0100"));
        // Shift >= width → 0.
        assert_eq!(shl(&bits("0001"), &v(4, 8, false)), bits("0000"));
        // High set bits in a wide shift amount must not be truncated to the
        // low host word. The known amount is far beyond the LHS width.
        let mut wide_count = vec![Bit::Zero; 130];
        wide_count[0] = Bit::One;
        assert_eq!(
            shl(&bits("0001"), &Value::from_bits(wide_count, false)),
            bits("0000")
        );
    }

    #[test]
    fn concat_and_replicate() {
        assert_eq!(concat(&[bits("1010"), bits("0001")]), bits("10100001"));
        let pat = bits("1010");
        let mut parts = Vec::new();
        for _ in 0..2 {
            parts.push(pat.clone());
        }
        assert_eq!(concat(&parts), bits("10101010"));
    }

    #[test]
    fn comparisons() {
        assert_eq!(eq(&bits("10x0"), &bits("10x0")), bits("x"));
        // A known mismatch determines logical equality even when a different
        // bit is unknown (IEEE 1800-2009 §11.4.5).
        assert_eq!(eq(&bits("10x0"), &bits("00x0")), bits("0"));
        assert_eq!(neq(&bits("10z0"), &bits("00z0")), bits("1"));
        assert_eq!(eq(&bits("10x0"), &bits("10z0")), bits("x"));
        assert_eq!(case_eq(&bits("10x0"), &bits("10x0")), bits("1"));
        assert_eq!(case_neq(&bits("10x0"), &bits("10x0")), bits("0"));
        let signed_narrow = bits_signed("1111");
        let signed_wide = bits_signed("00000000");
        assert_eq!(lt(&signed_narrow, &signed_wide), bits("1"));
        assert_eq!(eq(&signed_narrow, &signed_wide), bits("0"));
        let unsigned_wide = bits("00000000");
        assert_eq!(lt(&signed_narrow, &unsigned_wide), bits("0"));
        assert_eq!(gt(&signed_narrow, &unsigned_wide), bits("1"));
        assert_eq!(
            case_eq(&bits_signed("z001"), &bits_signed("zzzzz001")),
            bits("1")
        );
        assert_eq!(case_eq(&bits_signed("z001"), &bits("00000001")), bits("0"));
        assert_eq!(wildcard_eq(&bits("10xz"), &bits("10xz")), bits("1"));
        assert_eq!(wildcard_eq(&bits("10xz"), &bits("10x0")), bits("x"));
        assert_eq!(wildcard_eq(&bits("11xz"), &bits("10x0")), bits("0"));
        assert_eq!(wildcard_neq(&bits("11xz"), &bits("10x0")), bits("1"));
        assert_eq!(wildcard_neq(&bits("10xz"), &bits("10x0")), bits("x"));
        assert_eq!(wildcard_eq(&bits("1010"), &bits("10xz")), bits("1"));
        assert_eq!(
            wildcard_eq(&bits_signed("x001"), &bits_signed("xzzzz001")),
            bits("1")
        );
        assert_eq!(
            wildcard_eq(&bits_signed("1001"), &bits("00001001")),
            bits("1")
        );
        assert_eq!(lt(&v(3, 8, false), &v(5, 8, false)), bits("1"));
        assert_eq!(le(&v(5, 8, false), &v(3, 8, false)), bits("0"));
        assert_eq!(gt(&v(9, 8, false), &v(5, 8, false)), bits("1"));
        assert_eq!(ge(&v(5, 8, false), &v(5, 8, false)), bits("1"));
        assert_eq!(eq(&v(1, 8, false), &v(1, 8, false)), bits("1"));
        assert_eq!(neq(&v(1, 8, false), &v(2, 8, false)), bits("1"));
        // Comparison with unknown → X.
        assert_eq!(lt(&bits("1x"), &bits("01")), bits("x"));
    }

    #[test]
    fn unsigned_64bit_compare_ordering() {
        // Regression: `cmp_vals` used to cast u64 → i64 for unsigned
        // comparisons, so `u64::MAX < 1` was wrongly true (the C runtime's
        // limb compare returns false).
        let max = v(u64::MAX, 64, false);
        let one = v(1, 64, false);
        assert_eq!(lt(&max, &one), bits("0"));
        assert_eq!(le(&max, &one), bits("0"));
        assert_eq!(gt(&max, &one), bits("1"));
        assert_eq!(ge(&max, &one), bits("1"));
        assert_eq!(lt(&one, &max), bits("1"));
        assert_eq!(eq(&max, &max), bits("1"));
        assert_eq!(neq(&max, &one), bits("1"));
    }

    #[test]
    fn casez_eq_wildcards() {
        // casez: ?/z in the ITEM is a don't-care
        assert_eq!(casez_eq(&bits("1000"), &bits("1z0z")), bits("1"));
        assert_eq!(casez_eq(&bits("1110"), &bits("1z0z")), bits("0"));
        // casez: x in the item matches a selector x only
        assert_eq!(casez_eq(&bits("1x00"), &bits("1x0z")), bits("1"));
        assert_eq!(casez_eq(&bits("1010"), &bits("1x0z")), bits("0"));
        assert_eq!(casez_eq(&bits("1000"), &bits("100z")), bits("1"));
        // plain known match / mismatch
        assert_eq!(casez_eq(&bits("1000"), &bits("1000")), bits("1"));
        assert_eq!(casez_eq(&bits("1001"), &bits("1000")), bits("0"));
        // matches are never X
        assert_eq!(casez_eq(&bits("1x0z"), &bits("1x0z")), bits("1"));
        assert!(!casez_eq(&bits("1x0z"), &bits("1x0z")).is_unknown());
        // width mismatch zero-extends both operands
        assert_eq!(casez_eq(&bits("00001000"), &bits("1z0z")), bits("1"));
    }

    #[test]
    fn casex_eq_wildcards() {
        // casex: x/z/? in the ITEM are don't-cares
        assert_eq!(casex_eq(&bits("1001"), &bits("1x0z")), bits("1"));
        assert_eq!(casex_eq(&bits("1000"), &bits("1x0z")), bits("1"));
        // casex: a selector x/z is a don't-care against a known item bit
        assert_eq!(casex_eq(&bits("1x0z"), &bits("1000")), bits("1"));
        // opposite known bits never match
        assert_eq!(casex_eq(&bits("1100"), &bits("1000")), bits("0"));
        assert_eq!(casex_eq(&bits("1001"), &bits("1000")), bits("0"));
        // matches are never X
        assert_eq!(casex_eq(&bits("1x0z"), &bits("1x0z")), bits("1"));
        assert!(!casex_eq(&bits("1x0z"), &bits("1x0z")).is_unknown());
    }

    #[test]
    fn reductions() {
        assert_eq!(unary_and(&bits("1111")), bits("1"));
        assert_eq!(unary_and(&bits("1011")), bits("0"));
        assert_eq!(unary_or(&bits("0000")), bits("0"));
        assert_eq!(unary_or(&bits("0100")), bits("1"));
        assert_eq!(unary_xor(&bits("1010")), bits("0"));
        assert_eq!(unary_xor(&bits("1011")), bits("1"));
        assert_eq!(unary_and(&bits("1x11")), bits("x"));
        assert_eq!(unary_and(&bits("0x11")), bits("0"));
        assert_eq!(unary_or(&bits("1x00")), bits("1"));
        assert_eq!(unary_or(&bits("0x00")), bits("x"));
        assert_eq!(unary_nand(&bits("1111")), bits("0"));
        assert_eq!(unary_nor(&bits("0000")), bits("1"));
        assert_eq!(unary_xnor(&bits("1010")), bits("1"));
    }

    #[test]
    fn logical_ops() {
        assert_eq!(log_not(&v(0, 8, false)), bits("1"));
        assert_eq!(log_not(&v(3, 8, false)), bits("0"));
        assert_eq!(log_and(&v(3, 8, false), &v(0, 8, false)), bits("0"));
        assert_eq!(log_and(&v(3, 8, false), &v(4, 8, false)), bits("1"));
        assert_eq!(log_or(&v(0, 8, false), &v(0, 8, false)), bits("0"));
        assert_eq!(log_or(&v(0, 8, false), &v(5, 8, false)), bits("1"));
        assert_eq!(log_not(&bits("1x")), bits("0"));
        assert_eq!(log_not(&bits("0x")), bits("x"));
        assert_eq!(log_and(&bits("0"), &bits("x")), bits("0"));
        assert_eq!(log_and(&bits("1x"), &bits("1")), bits("1"));
        assert_eq!(log_or(&bits("1"), &bits("x")), bits("1"));
        assert_eq!(log_or(&bits("00"), &bits("0x")), bits("x"));
    }

    #[test]
    fn bitwise_ops() {
        assert_eq!(bit_and(&bits("1100"), &bits("1010")), bits("1000"));
        assert_eq!(bit_or(&bits("1100"), &bits("1010")), bits("1110"));
        assert_eq!(bit_xor(&bits("1100"), &bits("1010")), bits("0110"));
        assert_eq!(bit_xnor(&bits("1100"), &bits("1010")), bits("1001"));
        assert_eq!(bit_neg(&bits("1010")), bits("0101"));
        assert_eq!(bit_neg(&bits("10xz")), bits("01xx"));
        // 0 dominates AND; 1 dominates OR.
        assert_eq!(bit_and(&bits("0x"), &bits("1x")), bits("0x"));
        assert_eq!(bit_or(&bits("1x"), &bits("0x")), bits("1x"));
        let signed_narrow = bits_signed("1111");
        assert_eq!(bit_or(&signed_narrow, &bits("00000000")), bits("00001111"));
        assert_eq!(
            bit_or(&signed_narrow, &bits_signed("00000000")),
            bits_signed("11111111")
        );
        assert_eq!(
            bit_or(&bits_signed("x001"), &bits_signed("00000000")),
            bits_signed("xxxxx001")
        );
    }

    #[test]
    fn unary_minus() {
        assert_eq!(minus(&bits_signed("0010")), bits_signed("1110"));
        assert_eq!(minus(&bits("0011")), bits("1101"));
    }

    #[test]
    fn power_op() {
        assert_eq!(power(&v(2, 32, false), &v(8, 32, false)), v(256, 32, false));
        assert_eq!(power(&v(0, 32, false), &v(0, 32, false)), v(1, 32, false));
        // `**` has the base width/type; the exponent is self-determined.
        assert_eq!(power(&v(3, 4, false), &v(2, 8, false)), v(9, 4, false));
        let signed_base = v((-2i64) as u64, 4, true);
        assert_eq!(power(&signed_base, &v(3, 8, false)), v(8, 4, true));
        assert_eq!(
            power(&v(2, 4, true), &v((-1i64) as u64, 8, true)),
            v(0, 4, true)
        );
        assert_eq!(
            power(&v(1, 4, true), &v((-7i64) as u64, 8, true)),
            v(1, 4, true)
        );
        let negative_one = v((-1i64) as u64, 4, true);
        assert_eq!(
            power(&negative_one, &v((-3i64) as u64, 8, true)),
            negative_one
        );
        assert_eq!(
            power(&negative_one, &v((-2i64) as u64, 8, true)),
            v(1, 4, true)
        );
        assert!(power(&v(0, 4, false), &v((-1i64) as u64, 8, true)).is_unknown());
    }

    #[test]
    fn clog2_values() {
        assert_eq!(clog2(&v(256, 32, false)), v(8, 32, false));
        assert_eq!(clog2(&v(0, 32, false)), v(0, 32, false));
        assert_eq!(clog2(&v(1, 32, false)), v(0, 32, false));
        assert_eq!(clog2(&v(255, 32, false)), v(8, 32, false));
        assert_eq!(clog2(&v(3, 32, false)), v(2, 32, false));
        assert_eq!(clog2(&v(2, 32, false)), v(1, 32, false));
    }

    #[test]
    fn resize_sign_zero_fill() {
        let s = bits_signed("1000");
        assert_eq!(s.resize(8, true), bits_signed("11111000"));
        assert_eq!(s.resize(8, false), bits("00001000"));
        assert_eq!(bits("1001").resize(2, false), bits("01"));
        assert_eq!(v(7, 4, false).resize(8, false), v(7, 8, false));
        // Unsized fill literal: '1 resizes by filling.
        let f = Value {
            bits: vec![Bit::One],
            signed: false,
            fill: Some(Bit::One),
        };
        assert_eq!(
            f.resize(8, false),
            Value {
                bits: vec![Bit::One; 8],
                signed: false,
                fill: Some(Bit::One)
            }
        );
        let xf = Value {
            bits: vec![Bit::X],
            signed: false,
            fill: Some(Bit::X),
        };
        assert!(xf.resize(8, false).is_unknown());
        assert_eq!(xf.resize(8, false).bits, vec![Bit::X; 8]);
    }

    #[test]
    fn cast_is_value_preserving() {
        // LRM 1800-2009 §6.24.1 / §10.7: widening extends by the SOURCE's
        // signedness, then the result carries the target tag.
        // Unsigned 8'hFF into a wider signed target zero-extends to 255
        // (int'(8'hFF) must be 255, not -1).
        assert_eq!(bits("11111111").cast(16, true), v(255, 16, true));
        // Signed -2 into a wider unsigned target sign-extends to 65534.
        assert_eq!(bits_signed("11111110").cast(16, false), v(65534, 16, false));
        // Signed positive keeps zero-extension.
        assert_eq!(bits_signed("0101").cast(8, true), v(5, 8, true));
        // Same-width retag only (sign-only casts).
        assert_eq!(bits("1000").cast(4, true), bits_signed("1000"));
        assert_eq!(bits_signed("1010").cast(4, false), v(10, 4, false));
        // Narrowing truncates the MSBs either way.
        assert_eq!(bits("1010").cast(3, true), v(2, 3, true));
        assert_eq!(bits_signed("0111").cast(2, false), v(3, 2, false));
        // A signed X/Z MSB fills with the same state under §11.8.4.
        assert_eq!(
            Value::from_bits(vec![Bit::X, Bit::Zero, Bit::Zero], true).cast(6, true),
            Value::from_bits(
                vec![Bit::X, Bit::X, Bit::X, Bit::X, Bit::Zero, Bit::Zero],
                true
            )
        );
    }

    #[test]
    fn conditional_op() {
        // sel = 1 → a
        assert_eq!(cond(&bits("1"), &bits("1010"), &bits("0101")), bits("1010"));
        // sel = 0 → b
        assert_eq!(cond(&bits("0"), &bits("1010"), &bits("0101")), bits("0101"));
        // A known one makes a multi-bit condition true even when another bit
        // is unknown (IEEE 1800-2009 §11.4.11).
        assert_eq!(
            cond(&bits("1x"), &bits("1010"), &bits("0101")),
            bits("1010")
        );
        // sel = X with equal branches → value
        let same = bits("1010");
        assert_eq!(cond(&bits("x"), &same, &same), bits("1010"));
        // sel = X merges equal bits and marks only differing bits unknown.
        assert_eq!(cond(&bits("x"), &bits("1010"), &bits("1001")), bits("10xx"));
        // Identical X and Z bits remain distinct through the merge.
        assert_eq!(cond(&bits("z"), &bits("10xz"), &bits("10xz")), bits("10xz"));
        // Arms are coerced before either selecting or merging: common width
        // is max and common signedness requires both arms to be signed.
        assert_eq!(
            cond(&bits("1"), &bits_signed("1111"), &bits("00000000")),
            bits("00001111")
        );
        assert_eq!(
            cond(&bits("0"), &bits_signed("1111"), &bits("00000000")),
            bits("00000000")
        );
        assert_eq!(
            cond(&bits("x"), &bits_signed("1111"), &bits("00000000")),
            bits("0000xxxx")
        );
        // Logical truth is determined before branch selection, so the known
        // one dominates the unknown low bit.
        assert_eq!(
            cond(&bits("1x"), &bits("1010"), &bits("1000")),
            bits("1010")
        );
    }

    #[test]
    fn format_verilog_strings() {
        assert_eq!(v(255, 8, false).format_verilog(), "8'd255");
        assert_eq!(v((-1i64) as u64, 8, true).format_verilog(), "8'sd-1");
        assert_eq!(bits("10xz").format_verilog(), "4'h10xz");
        assert_eq!(v(18, 9, false).format_verilog(), "9'd18");
        assert_eq!(v((-2i64) as u64, 32, true).format_verilog(), "32'sd-2");
    }

    #[test]
    fn to_u64_i64() {
        assert_eq!(v(0x5A, 8, false).to_u64(), Some(0x5A));
        assert_eq!(bits_signed("1000").to_i64(), Some(-8));
        // to_i64 always interprets the bits as two's complement.
        assert_eq!(bits("1000").to_i64(), Some(-8));
        assert_eq!(bits("10xz").to_u64(), None);
        assert_eq!(v(5, 4, false).bit_lsb(0), Bit::One);
        assert_eq!(bits("1010").bit_lsb(3), Bit::One);
    }

    #[test]
    fn wide_integer_conversions_are_checked_instead_of_panicking() {
        let high = one_at(129, 100);
        assert_eq!(high.to_u64(), None);
        assert_eq!(high.to_u128(), Some(1u128 << 100));
        assert_eq!(bits_signed(&"1".repeat(128)).to_i128(), Some(-1));
        assert_eq!(
            bits_signed(&format!("10{}", "0".repeat(127))).to_i128(),
            None
        );

        let widened = Value::from_u64(u64::MAX, 128, false);
        assert!(widened.bits[..64].iter().all(|bit| *bit == Bit::Zero));
        assert!(widened.bits[64..].iter().all(|bit| *bit == Bit::One));
    }

    #[test]
    fn wide_arithmetic_and_comparison_use_high_bits() {
        let high = one_at(128, 100);
        let one = Value::from_u64(1, 128, false);
        let two = Value::from_u64(2, 128, false);
        let three = Value::from_u64(3, 128, false);

        let sum = add(&high, &one);
        assert_eq!(sum.bit_lsb(100), Bit::One);
        assert_eq!(sum.bit_lsb(0), Bit::One);
        assert_eq!(sub(&sum, &one), high);

        let product = mul(&high, &three);
        assert_eq!(product.bit_lsb(101), Bit::One);
        assert_eq!(product.bit_lsb(100), Bit::One);
        assert_eq!(div(&product, &three), high);
        assert_eq!(rem(&product, &three), zero(128, false));

        assert_eq!(gt(&high, &one), Value::from_u64(1, 1, false));
        assert_eq!(clog2(&high), Value::from_u64(100, 32, false));
        assert_eq!(clog2(&add(&high, &one)), Value::from_u64(101, 32, false));

        let squared = power(&two, &Value::from_u64(100, 32, false));
        assert_eq!(squared, high);
    }

    #[test]
    fn wide_signed_division_preserves_verilog_sign_rules() {
        let minus_seven = bits_signed(&format!("{}1001", "1".repeat(124)));
        let three = Value::from_u64(3, 128, true);
        let minus_two = bits_signed(&format!("{}10", "1".repeat(126)));
        let minus_one = bits_signed(&"1".repeat(128));

        assert_eq!(div(&minus_seven, &three), minus_two);
        assert_eq!(rem(&minus_seven, &three), minus_one);
    }

    #[test]
    fn stored_numeric_values_validate_input_and_support_wide_decimal() {
        let max_u128 = parse_decimal("340282366920938463463374607431768211455", 128)
            .expect("valid wide decimal");
        assert!(max_u128.bits.iter().all(|bit| *bit == Bit::One));
        assert!(parse_decimal("12not-a-number", 128).is_err());
        assert!(parse_radix("8", 3, 4).is_err());
        assert!(parse_radix("2", 1, 1).is_err());

        let Val::Bits(question) = decode_value_data(&ValueData::Hex("?".into()), 128)
            .expect("question mark is a legal Z digit")
        else {
            panic!("radix value must be bits");
        };
        assert_eq!(question.width(), 128);
        assert!(question.bits[..124].iter().all(|bit| *bit == Bit::Zero));
        assert!(question.bits[124..].iter().all(|bit| *bit == Bit::Z));

        let Val::Bits(unknown) =
            decode_value_data(&ValueData::Hex("x".into()), 128).expect("wide X value")
        else {
            panic!("radix value must be bits");
        };
        assert!(unknown.bits[..124].iter().all(|bit| *bit == Bit::Zero));
        assert!(unknown.bits[124..].iter().all(|bit| *bit == Bit::X));

        let Val::Bits(negative) =
            decode_value_data(&ValueData::Int(-1), 128).expect("wide signed integer")
        else {
            panic!("integer value must be bits");
        };
        assert_eq!(negative.width(), 128);
        assert!(negative.bits.iter().all(|bit| *bit == Bit::One));
    }

    #[test]
    fn range_width_checks_extreme_endpoints_without_overflow() {
        assert_eq!(checked_inclusive_width(7, 0, "test").unwrap(), 8);
        assert!(checked_inclusive_width(i128::MIN, i128::MAX, "test").is_err());
        assert!(checked_inclusive_width(0, MAX_RESOLVED_BITS as i128, "test").is_err());
    }
}
