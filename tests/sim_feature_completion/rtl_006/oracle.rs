//! Independent four-state arithmetic oracle for the RTL-006 fixtures.
//!
//! Values are plain little-endian `u64` limbs with separate X and Z planes.
//! Every operation is written from IEEE 1800-2009 §§11.4.3, 11.4.10, 11.6-11.8
//! and Table 11-4 (and the matching 1364-2001 clauses) without reusing the
//! simulator's value code, so the expected text never comes from `llg`.

#[derive(Clone, Debug)]
pub(super) struct V {
    pub(super) w: usize,
    v: Vec<u64>,
    x: Vec<u64>,
    z: Vec<u64>,
}

const PAT: [u64; 4] = [
    0xf86c_6a11_d0c1_8e95,
    0x1082_276b_f3a2_7251,
    0xf39c_c060_5ced_c834,
    0x9e37_79b9_7f4a_7c15,
];

fn words(w: usize) -> usize {
    w.div_ceil(64)
}

fn top_mask(w: usize) -> u64 {
    if w.is_multiple_of(64) {
        u64::MAX
    } else {
        (1u64 << (w % 64)) - 1
    }
}

fn masked(mut limbs: Vec<u64>, w: usize) -> Vec<u64> {
    limbs.resize(words(w), 0);
    if let Some(last) = limbs.last_mut() {
        *last &= top_mask(w);
    }
    limbs
}

impl V {
    pub(super) fn known(limbs: Vec<u64>, w: usize) -> V {
        V {
            w,
            v: masked(limbs, w),
            x: vec![0; words(w)],
            z: vec![0; words(w)],
        }
    }

    fn all_x(w: usize) -> V {
        V {
            w,
            v: vec![0; words(w)],
            x: masked(vec![u64::MAX; words(w)], w),
            z: vec![0; words(w)],
        }
    }

    fn is_known(&self) -> bool {
        self.x.iter().chain(&self.z).all(|limb| *limb == 0)
    }

    fn is_zero(&self) -> bool {
        self.v.iter().all(|limb| *limb == 0)
    }

    fn is_one(&self) -> bool {
        self.v.first() == Some(&1) && self.v[1..].iter().all(|limb| *limb == 0)
    }

    fn is_all_ones(&self) -> bool {
        self.w != 0 && self.v == masked(vec![u64::MAX; words(self.w)], self.w)
    }

    /// One bit as `(value, x, z)`.
    fn bit(&self, index: usize) -> (bool, bool, bool) {
        let get = |plane: &[u64]| plane[index / 64] >> (index % 64) & 1 == 1;
        (get(&self.v), get(&self.x), get(&self.z))
    }

    fn set_bit(&mut self, index: usize, bit: (bool, bool, bool)) {
        let mask = 1u64 << (index % 64);
        for (plane, on) in [
            (&mut self.v, bit.0),
            (&mut self.x, bit.1),
            (&mut self.z, bit.2),
        ] {
            if on {
                plane[index / 64] |= mask;
            } else {
                plane[index / 64] &= !mask;
            }
        }
    }

    fn top_bit(&self) -> (bool, bool, bool) {
        self.bit(self.w - 1)
    }

    fn negative(&self) -> bool {
        self.top_bit().0
    }

    /// Resize to `w` bits; widening replicates the (four-state) sign bit only
    /// when `signed`, otherwise it fills with zero.
    pub(super) fn resize(&self, w: usize, signed: bool) -> V {
        let mut out = V {
            w,
            v: vec![0; words(w)],
            x: vec![0; words(w)],
            z: vec![0; words(w)],
        };
        let fill = if signed && self.w > 0 {
            self.top_bit()
        } else {
            (false, false, false)
        };
        for index in 0..w {
            out.set_bit(
                index,
                if index < self.w {
                    self.bit(index)
                } else {
                    fill
                },
            );
        }
        out
    }
}

/// Operand `index` of the shared fixture formulas at width `w`.
pub(super) fn operand(w: usize, index: usize) -> V {
    let ones = masked(vec![u64::MAX; words(w)], w);
    let small = |value: u64| V::known(vec![value], w);
    let mut smin = vec![0u64; words(w)];
    smin[(w - 1) / 64] = 1u64 << ((w - 1) % 64);
    let pattern = (0..words(w)).map(|i| PAT[i % 4]).collect::<Vec<_>>();
    match index {
        0..=3 => small(index as u64),
        4 => V::known(ones, w),
        5 => V::known(smin, w),
        6 => V::known(sub(&smin, &[1], w), w),
        7 => V::known(add(&smin, &[1], w), w),
        8 => V::known(pattern, w),
        9 => V::known(pattern.iter().map(|limb| !limb).collect(), w),
        10 => V::known(sub(&ones, &[2], w), w),
        11 => V::all_x(w),
        12 => {
            let mut value = V::known(pattern, w);
            value.set_bit(0, (false, false, true));
            value
        }
        _ => unreachable!("operand index {index}"),
    }
}

fn add(a: &[u64], b: &[u64], w: usize) -> Vec<u64> {
    let mut out = vec![0u64; words(w)];
    let mut carry = 0u128;
    for (i, slot) in out.iter_mut().enumerate() {
        let sum = u128::from(*a.get(i).unwrap_or(&0)) + u128::from(*b.get(i).unwrap_or(&0)) + carry;
        *slot = sum as u64;
        carry = sum >> 64;
    }
    masked(out, w)
}

fn not(a: &[u64], w: usize) -> Vec<u64> {
    let full = masked(a.to_vec(), w);
    masked(full.iter().map(|limb| !limb).collect(), w)
}

fn neg(a: &[u64], w: usize) -> Vec<u64> {
    add(&not(a, w), &[1], w)
}

fn sub(a: &[u64], b: &[u64], w: usize) -> Vec<u64> {
    add(a, &neg(&masked(b.to_vec(), w), w), w)
}

/// Product modulo 2^w (schoolbook over the low limbs only).
fn mul(a: &[u64], b: &[u64], w: usize) -> Vec<u64> {
    let n = words(w);
    let mut out = vec![0u64; n];
    for i in 0..n {
        let mut carry = 0u128;
        for j in 0..n - i {
            let cell = u128::from(out[i + j])
                + u128::from(*a.get(j).unwrap_or(&0)) * u128::from(*b.get(i).unwrap_or(&0))
                + carry;
            out[i + j] = cell as u64;
            carry = cell >> 64;
        }
    }
    masked(out, w)
}

fn to_digits(a: &[u64]) -> Vec<u32> {
    a.iter()
        .flat_map(|limb| [*limb as u32, (*limb >> 32) as u32])
        .collect()
}

fn from_digits(d: &[u32], n: usize) -> Vec<u64> {
    (0..n)
        .map(|i| {
            u64::from(*d.get(2 * i).unwrap_or(&0))
                | u64::from(*d.get(2 * i + 1).unwrap_or(&0)) << 32
        })
        .collect()
}

/// Unsigned quotient and remainder (Knuth, TAOCP vol. 2, algorithm 4.3.1 D).
fn divmod(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let limbs = a.len().max(b.len());
    let mut u = to_digits(a);
    let mut v = to_digits(b);
    while u.last() == Some(&0) {
        u.pop();
    }
    while v.last() == Some(&0) {
        v.pop();
    }
    let (n, m) = (u.len(), v.len());
    assert!(m > 0, "oracle division by zero");
    if n < m {
        return (vec![0; limbs], from_digits(&u, limbs));
    }
    if m == 1 {
        let divisor = u64::from(v[0]);
        let mut quotient = vec![0u32; n];
        let mut rem = 0u64;
        for i in (0..n).rev() {
            let cur = rem << 32 | u64::from(u[i]);
            quotient[i] = (cur / divisor) as u32;
            rem = cur % divisor;
        }
        return (from_digits(&quotient, limbs), vec![rem]);
    }
    let shift = v[m - 1].leading_zeros();
    let shl = |digits: &[u32], extra: bool| {
        let mut out = Vec::with_capacity(digits.len() + 1);
        let mut carry = 0u32;
        for digit in digits {
            out.push(if shift == 0 {
                *digit
            } else {
                digit << shift | carry
            });
            carry = if shift == 0 { 0 } else { digit >> (32 - shift) };
        }
        if extra {
            out.push(carry);
        }
        out
    };
    let vn = shl(&v, false);
    let mut un = shl(&u, true);
    let mut quotient = vec![0u32; n - m + 1];
    let base = 1u64 << 32;
    for j in (0..=n - m).rev() {
        let numerator = u64::from(un[j + m]) << 32 | u64::from(un[j + m - 1]);
        let mut qhat = numerator / u64::from(vn[m - 1]);
        let mut rhat = numerator % u64::from(vn[m - 1]);
        while qhat >= base || qhat * u64::from(vn[m - 2]) > (rhat << 32 | u64::from(un[j + m - 2]))
        {
            qhat -= 1;
            rhat += u64::from(vn[m - 1]);
            if rhat >= base {
                break;
            }
        }
        let mut k = 0i64;
        for i in 0..m {
            let p = qhat * u64::from(vn[i]);
            let t = i64::from(un[i + j]) - k - (p & 0xffff_ffff) as i64;
            un[i + j] = t as u32;
            k = (p >> 32) as i64 - (t >> 32);
        }
        let t = i64::from(un[j + m]) - k;
        un[j + m] = t as u32;
        if t < 0 {
            qhat -= 1;
            let mut carry = 0u64;
            for i in 0..m {
                let sum = u64::from(un[i + j]) + u64::from(vn[i]) + carry;
                un[i + j] = sum as u32;
                carry = sum >> 32;
            }
            un[j + m] = un[j + m].wrapping_add(carry as u32);
        }
        quotient[j] = qhat as u32;
    }
    let remainder = (0..m)
        .map(|i| {
            if shift == 0 {
                un[i]
            } else {
                un[i] >> shift | un[i + 1] << (32 - shift)
            }
        })
        .collect::<Vec<_>>();
    (
        from_digits(&quotient, limbs),
        from_digits(&remainder, limbs),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// Binary `+ - * / %` on equal-width operands. Any X/Z bit or a zero divisor
/// makes every result bit X; division truncates toward zero and the
/// remainder takes the dividend's sign.
pub(super) fn arith(op: Op, a: &V, b: &V, signed: bool) -> V {
    let w = a.w;
    assert_eq!(w, b.w);
    if !a.is_known() || !b.is_known() {
        return V::all_x(w);
    }
    let bits = match op {
        Op::Add => add(&a.v, &b.v, w),
        Op::Sub => sub(&a.v, &b.v, w),
        Op::Mul => mul(&a.v, &b.v, w),
        Op::Div | Op::Mod => {
            if b.is_zero() {
                return V::all_x(w);
            }
            let (na, nb) = (signed && a.negative(), signed && b.negative());
            let ma = if na { neg(&a.v, w) } else { a.v.clone() };
            let mb = if nb { neg(&b.v, w) } else { b.v.clone() };
            let (q, r) = divmod(&ma, &mb);
            if op == Op::Div {
                if na != nb {
                    neg(&q, w)
                } else {
                    masked(q, w)
                }
            } else if na {
                neg(&r, w)
            } else {
                masked(r, w)
            }
        }
    };
    V::known(bits, w)
}

pub(super) fn negate(a: &V) -> V {
    if !a.is_known() {
        return V::all_x(a.w);
    }
    V::known(neg(&a.v, a.w), a.w)
}

/// `base ** exponent` at the base's width (Table 11-4). The exponent is
/// self-determined and negative only when `exponent_signed`.
pub(super) fn power(base: &V, base_signed: bool, exponent: &V, exponent_signed: bool) -> V {
    let w = base.w;
    if !base.is_known() || !exponent.is_known() {
        return V::all_x(w);
    }
    let minus_one = base_signed && base.is_all_ones();
    let odd = exponent.v[0] & 1 == 1;
    if exponent_signed && exponent.negative() {
        return if base.is_zero() {
            V::all_x(w)
        } else if minus_one {
            if odd {
                base.clone()
            } else {
                V::known(vec![1], w)
            }
        } else if base.is_one() {
            V::known(vec![1], w)
        } else {
            V::known(vec![0], w)
        };
    }
    // Square and multiply modulo 2^w. Once the running square is 0 or 1 the
    // remaining exponent bits cannot change the product except through 0.
    let mut result = V::known(vec![1], w).v;
    let mut square = base.v.clone();
    let Some(top) = (0..exponent.w).rev().find(|index| exponent.bit(*index).0) else {
        return V::known(result, w);
    };
    for index in 0..=top {
        if exponent.bit(index).0 {
            result = mul(&result, &square, w);
        }
        if index == top {
            break;
        }
        let square_is_one = square.first() == Some(&1) && square[1..].iter().all(|l| *l == 0);
        if square_is_one {
            break;
        }
        if square.iter().all(|limb| *limb == 0) {
            result = vec![0; words(w)];
            break;
        }
        square = mul(&square, &square, w);
    }
    V::known(result, w)
}

/// Shift amount, saturated at `limit`; `None` for an unknown amount.
fn amount(count: &V, limit: usize) -> Option<usize> {
    if !count.is_known() {
        return None;
    }
    if count.v[1..].iter().any(|limb| *limb != 0) {
        return Some(limit);
    }
    Some(usize::try_from(count.v[0]).map_or(limit, |n| n.min(limit)))
}

/// `<<`/`<<<` (zero fill) and `>>`/`>>>` (sign fill only for `arithmetic`).
/// The count is always unsigned; an X/Z count makes the result all X.
pub(super) fn shift(a: &V, count: &V, left: bool, arithmetic: bool) -> V {
    let w = a.w;
    let Some(n) = amount(count, w) else {
        return V::all_x(w);
    };
    let fill = if !left && arithmetic {
        a.top_bit()
    } else {
        (false, false, false)
    };
    let mut out = V::known(vec![0], w);
    for index in 0..w {
        let bit = if left {
            if index >= n {
                a.bit(index - n)
            } else {
                (false, false, false)
            }
        } else if index + n < w {
            a.bit(index + n)
        } else {
            fill
        };
        out.set_bit(index, bit);
    }
    out
}

pub(super) fn count_ones(a: &V) -> u32 {
    a.v.iter()
        .zip(&a.x)
        .zip(&a.z)
        .map(|((v, x), z)| (v & !x & !z).count_ones())
        .sum()
}

/// `%h` per IEEE 1800-2009 §21.2.1.4: full-width, lowercase x/z for a fully
/// unknown digit and uppercase X/Z for a partially unknown one.
pub(super) fn hex(a: &V) -> String {
    let digits = a.w.div_ceil(4);
    let mut out = String::with_capacity(digits);
    for digit in (0..digits).rev() {
        let bits = (digit * 4..(digit * 4 + 4).min(a.w))
            .map(|i| a.bit(i))
            .collect::<Vec<_>>();
        let xs = bits.iter().filter(|bit| bit.1).count();
        let zs = bits.iter().filter(|bit| bit.2).count();
        let c = if xs == bits.len() {
            'x'
        } else if zs == bits.len() {
            'z'
        } else if xs > 0 {
            'X'
        } else if zs > 0 {
            'Z'
        } else {
            let value = bits
                .iter()
                .enumerate()
                .fold(0u32, |acc, (i, bit)| acc | u32::from(bit.0) << i);
            char::from_digit(value, 16).expect("hex digit")
        };
        out.push(c);
    }
    out
}

pub(super) fn bin(a: &V) -> String {
    (0..a.w)
        .rev()
        .map(|i| match a.bit(i) {
            (_, true, _) => 'x',
            (_, _, true) => 'z',
            (true, _, _) => '1',
            _ => '0',
        })
        .collect()
}

/// Bits `[lsb +: width]` as their own value.
pub(super) fn slice(a: &V, lsb: usize, width: usize) -> V {
    let mut out = V::known(vec![0], width);
    for index in 0..width {
        out.set_bit(index, a.bit(lsb + index));
    }
    out
}
