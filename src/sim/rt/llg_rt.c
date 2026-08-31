// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with vendor/libaco (aco.c + acosw.S) and the generated
// model.c by the host C compiler; never linked into the Rust binaries.

#define _GNU_SOURCE

#include "llg_rt.h"
#include "aco.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>

// ── 4-state value ops ─────────────────────────────────────────────────────────

// Number of 64-bit limbs covering `w` bits.
static int sv4_nlimbs(uint16_t w) { return w == 0 ? 0 : (int)((w + 63u) / 64u); }

// Mask for limb `i` of a `w`-bit vector: full for interior limbs, partial for
// the top limb, zero beyond the width.
static uint64_t sv4_limb_mask(uint16_t w, int i) {
    int nl = sv4_nlimbs(w);
    if (i < 0 || i >= nl) return 0;
    if (i == nl - 1 && (w % 64) != 0) return LLG_MASK((uint16_t)(w % 64));
    return ~0ULL;
}

sv4_t sv4_x(uint16_t width, int8_t is_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    sv4_t r;
    for (int i = 0; i < LLG_LIMBS; i++) {
        r.bits[i] = 0;
        r.x[i] = sv4_limb_mask(width, i);
        r.z[i] = 0;
    }
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

sv4_t sv4_from_u64(uint64_t v, uint16_t width, int8_t is_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.bits[0] = v & LLG_MASK(width);
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

sv4_t sv4_from_i64(int64_t v, uint16_t width) {
    return sv4_from_u64((uint64_t)v, width, 1);
}

double sv4_to_real(sv4_t v) {
    int limbs = sv4_nlimbs(v.width);
    uint64_t clean[LLG_LIMBS] = {0};
    for (int i = 0; i < limbs; i++)
        clean[i] = v.bits[i] & ~(v.x[i] | v.z[i]) & sv4_limb_mask(v.width, i);
    int negative = v.is_signed && v.width > 0 &&
        ((clean[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL);
    uint64_t magnitude[LLG_LIMBS] = {0};
    if (negative) {
        uint64_t carry = 1;
        for (int i = 0; i < limbs; i++) {
            uint64_t word = (~clean[i]) & sv4_limb_mask(v.width, i);
            uint64_t sum = word + carry;
            magnitude[i] = sum & sv4_limb_mask(v.width, i);
            if (carry && sum != 0) carry = 0;
        }
    } else {
        for (int i = 0; i < limbs; i++) magnitude[i] = clean[i];
    }
    double out = 0.0;
    for (int i = limbs - 1; i >= 0; i--)
        out = ldexp(out, 64) + (double)magnitude[i];
    return negative ? -out : out;
}

sv4_t sv4_from_real(double v, uint16_t width, int8_t is_signed) {
    if (!isfinite(v)) return sv4_x(width, is_signed);
    v = round(v);
    const double modulus = 18446744073709551616.0;
    double magnitude = fmod(fabs(v), modulus);
    uint64_t bits = (uint64_t)magnitude;
    if (signbit(v)) bits = 0ULL - bits;
    return sv4_from_u64(bits, width, is_signed);
}

int llg_real_to_bool(double v) { return v != 0.0; }

sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                     uint16_t width, int8_t is_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    sv4_t r;
    for (int i = 0; i < LLG_LIMBS; i++) {
        uint64_t m = sv4_limb_mask(width, i);
        r.bits[i] = (bits && m) ? bits[i] & m : 0;
        r.x[i] = (x && m) ? x[i] & m : 0;
        r.z[i] = (z && m) ? z[i] & m : 0;
    }
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

int sv4_is_unknown(sv4_t v) {
    for (int i = 0; i < LLG_LIMBS; i++)
        if (v.x[i] | v.z[i]) return 1;
    return 0;
}

int sv4_to_bool(sv4_t v) {
    if (sv4_is_unknown(v)) return 0;
    for (int i = 0; i < LLG_LIMBS; i++)
        if (v.bits[i]) return 1;
    return 0;
}

uint64_t sv4_to_u64(sv4_t v) { return v.bits[0] & LLG_MASK(v.width); }

int64_t sv4_to_i64(sv4_t v) {
    uint64_t b = sv4_to_u64(v);
    if (v.width == 0) return 0;
    if (v.width >= 64) return (int64_t)b;
    uint64_t sign = 1ULL << (v.width - 1);
    if (b & sign) return (int64_t)(b | ~(sign - 1));
    return (int64_t)b;
}

int sv4_same(sv4_t a, sv4_t b) {
    for (int i = 0; i < LLG_LIMBS; i++)
        if (a.bits[i] != b.bits[i] || a.x[i] != b.x[i] || a.z[i] != b.z[i])
            return 0;
    return 1;
}

// Bit `i` counted from the LSB; out-of-range -> 2 (X), X -> 2, Z -> 3,
// else 0/1.
static int sv4_lsb_bit(sv4_t v, int i) {
    if (i < 0 || i >= (int)v.width) return 2;
    int l = i >> 6, b = i & 63;
    if ((v.x[l] >> b) & 1) return 2;
    if ((v.z[l] >> b) & 1) return 3;
    return (v.bits[l] >> b) & 1;
}

int sv4_fits_i64(sv4_t v) {
    if (sv4_is_unknown(v)) return 0;
    if (v.width == 0) return 1;
    if (!v.is_signed) {
        if (v.width >= 64 && sv4_lsb_bit(v, 63) != 0) return 0;
        for (int i = 64; i < (int)v.width; i++)
            if (sv4_lsb_bit(v, i) != 0) return 0;
        return 1;
    }
    if (v.width <= 64) return 1;
    int sign = sv4_lsb_bit(v, 63);
    for (int i = 64; i < (int)v.width; i++)
        if (sv4_lsb_bit(v, i) != sign) return 0;
    return 1;
}

// Set bit `i` (LSB-indexed) of `v` to `val` (0, 1, 2 = X or 3 = Z).
static void sv4_lsb_bit_set(sv4_t* v, int i, int val) {
    if (i < 0 || i >= (int)v->width) return;
    int l = i >> 6;
    uint64_t m = 1ULL << (i & 63);
    if (val == 2) {
        v->x[l] |= m;
        v->z[l] &= ~m;
        v->bits[l] &= ~m;
    } else if (val == 3) {
        v->z[l] |= m;
        v->x[l] &= ~m;
        v->bits[l] &= ~m;
    } else if (val) {
        v->bits[l] |= m;
        v->x[l] &= ~m;
        v->z[l] &= ~m;
    } else {
        v->bits[l] &= ~m;
        v->x[l] &= ~m;
        v->z[l] &= ~m;
    }
}

// Shared resize core: pad/truncate `v` to `width` bits, tagging the result
// `is_signed`.  Widening fills the new MSBs per `ext_signed`; a signed X/Z
// sign bit fills with that same literal state (IEEE 1800-2009 §11.8.4).
static sv4_t sv4_resize_ext(sv4_t v, uint16_t width, int8_t is_signed, int8_t ext_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    if (v.width == width) {
        v.is_signed = is_signed;
        return v;
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = width;
    r.is_signed = is_signed;
    int oln = sv4_nlimbs(v.width), nln = sv4_nlimbs(width);
    int copy = oln < nln ? oln : nln;
    for (int i = 0; i < copy; i++) {
        r.bits[i] = v.bits[i];
        r.x[i] = v.x[i];
        r.z[i] = v.z[i];
    }
    if (width > v.width && ext_signed && v.width > 0) {
        int sign = sv4_lsb_bit(v, (int)v.width - 1);
        for (int i = (int)v.width; i < (int)width; i++)
            sv4_lsb_bit_set(&r, i, sign);
    }
    if (nln > 0) {
        uint64_t m = sv4_limb_mask(width, nln - 1);
        r.bits[nln - 1] &= m;
        r.x[nln - 1] &= m;
        r.z[nln - 1] &= m;
    }
    return r;
}

sv4_t sv4_resize(sv4_t v, uint16_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, is_signed);
}

// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`) — an unsigned source
// zero-extends even into a signed target and vice versa — narrowing
// truncates; the result carries `is_signed`.
sv4_t sv4_cast(sv4_t v, uint16_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, v.is_signed);
}

sv4_t sv4_fill(uint8_t bit, uint16_t width, int8_t is_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    sv4_t r;
    for (int i = 0; i < LLG_LIMBS; i++) {
        uint64_t m = sv4_limb_mask(width, i);
        r.bits[i] = (bit == 1) ? m : 0;
        r.x[i] = (bit == 2) ? m : 0;
        r.z[i] = (bit == 3) ? m : 0;
    }
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

// Highest set known bit, or -1 when the value is zero.
static int sv4_msb(sv4_t v) {
    for (int i = LLG_LIMBS - 1; i >= 0; i--)
        if (v.bits[i]) return i * 64 + (63 - __builtin_clzll(v.bits[i]));
    return -1;
}

sv4_t sv4_clog2(sv4_t v) {
    if (sv4_is_unknown(v)) return sv4_x(32, 0);
    if (sv4_msb(v) <= 0) return sv4_from_u64(0, 32, 0); // 0 or 1
    // ceil(log2(x)) = msb(x-1) + 1
    sv4_t xm1 = v;
    for (int i = 0; i < LLG_LIMBS; i++) {
        uint64_t before = xm1.bits[i];
        xm1.bits[i] -= 1;
        if (before != 0) break;
    }
    return sv4_from_u64((uint64_t)(sv4_msb(xm1) + 1), 32, 0);
}

// ── sv4 arithmetic ────────────────────────────────────────────────────────────

static uint16_t sv4_maxw(sv4_t a, sv4_t b) {
    return a.width > b.width ? a.width : b.width;
}

sv4_t sv4_add(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    sv4_t r;
    memset(&r, 0, sizeof(r));
    int nl = sv4_nlimbs(w);
    uint64_t carry = 0;
    for (int i = 0; i < nl; i++) {
        uint64_t t = a.bits[i] + b.bits[i];
        uint64_t c1 = t < a.bits[i] ? 1 : 0;
        uint64_t u = t + carry;
        carry = c1 | (u < t ? 1 : 0);
        r.bits[i] = u;
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_sub(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    sv4_t r;
    memset(&r, 0, sizeof(r));
    int nl = sv4_nlimbs(w);
    uint64_t borrow = 0;
    for (int i = 0; i < nl; i++) {
        uint64_t t = b.bits[i] + borrow; // mod 2^64
        uint64_t f = t < b.bits[i];      // b + borrow overflowed
        r.bits[i] = a.bits[i] - t;       // mod 2^64
        borrow = f || (a.bits[i] < t);
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_mul(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    int nl = sv4_nlimbs(w);
    // Schoolbook product into a full 2*nl-limb accumulator, then truncate.
    uint64_t acc[2 * LLG_LIMBS];
    memset(acc, 0, sizeof(acc));
    for (int i = 0; i < nl; i++) {
        for (int j = 0; j < nl; j++) {
            __uint128_t prod = (__uint128_t)a.bits[i] * b.bits[j];
            uint64_t plo = (uint64_t)prod;
            uint64_t phi = (uint64_t)(prod >> 64);
            // add plo to acc[i+j], propagating the carry upward
            uint64_t t = acc[i + j] + plo;
            uint64_t carry = t < acc[i + j] ? 1 : 0;
            acc[i + j] = t;
            for (int k = i + j + 1; carry && k < 2 * nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
            // add phi to acc[i+j+1], propagating the carry upward
            t = acc[i + j + 1] + phi;
            carry = t < acc[i + j + 1] ? 1 : 0;
            acc[i + j + 1] = t;
            for (int k = i + j + 2; carry && k < 2 * nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
        }
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    for (int i = 0; i < nl; i++) r.bits[i] = acc[i];
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

// div/mod/pow keep the uint64 fast path; operands wider than 64 bits yield
// all-X (the code generator rejects wide div/mod/pow before emission).
sv4_t sv4_div(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    if (a.width > 64 || b.width > 64) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    uint64_t y = sv4_to_u64(b);
    if (y == 0) return sv4_x(w, s);
    uint64_t v;
    if (s) {
        int64_t x = sv4_to_i64(a);
        int64_t divisor = sv4_to_i64(b);
        if (x == INT64_MIN && divisor == -1) {
            v = (uint64_t)INT64_MIN;
        } else {
            v = (uint64_t)(x / divisor);
        }
    } else {
        v = sv4_to_u64(a) / y;
    }
    return sv4_from_u64(v & LLG_MASK(w), w, s);
}

sv4_t sv4_mod(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    if (a.width > 64 || b.width > 64) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    uint64_t y = sv4_to_u64(b);
    if (y == 0) return sv4_x(w, s);
    uint64_t v;
    if (s) {
        int64_t x = sv4_to_i64(a);
        int64_t divisor = sv4_to_i64(b);
        if (x == INT64_MIN && divisor == -1) {
            v = 0;
        } else {
            v = (uint64_t)(x % divisor);
        }
    } else {
        v = sv4_to_u64(a) % y;
    }
    return sv4_from_u64(v & LLG_MASK(w), w, s);
}

sv4_t sv4_pow(sv4_t a, sv4_t b) {
    uint16_t w = a.width;
    int8_t s = a.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    if (a.width > 64 || b.width > 64) return sv4_x(w, s);
    if (b.is_signed && sv4_to_i64(b) < 0) {
        return sv4_from_u64(0, w, s);
    }
    uint64_t base = sv4_to_u64(a);
    uint64_t exp = sv4_to_u64(b);
    uint64_t r = 1;
    while (exp) {
        if (exp & 1) r *= base;
        base *= base;
        exp >>= 1;
    }
    return sv4_from_u64(r & LLG_MASK(w), w, s);
}

sv4_t sv4_neg(sv4_t a) {
    uint16_t w = a.width;
    if (sv4_is_unknown(a)) return sv4_x(w, a.is_signed);
    sv4_t r;
    memset(&r, 0, sizeof(r));
    int nl = sv4_nlimbs(w);
    uint64_t carry = 1; // two's complement: ~a + 1
    for (int i = 0; i < nl; i++) {
        uint64_t t = ~a.bits[i] + carry;
        carry = t < (~a.bits[i]) ? 1 : 0;
        r.bits[i] = t;
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = a.is_signed;
    return r;
}

sv4_t sv4_bitneg(sv4_t a) {
    sv4_t r;
    for (int i = 0; i < LLG_LIMBS; i++) {
        uint64_t m = sv4_limb_mask(a.width, i);
        // X stays X; Z degrades to X (IEEE 4-state NOT table: NOT z = x).
        uint64_t unk = (a.x[i] | a.z[i]) & m;
        r.bits[i] = (~a.bits[i]) & ~unk & m;
        r.x[i] = unk;
        r.z[i] = 0;
    }
    r.width = a.width;
    r.is_signed = a.is_signed;
    return r;
}

// Logical truth is determined by a known one even when other bits are X/Z;
// without a known one, an unknown bit leaves the truth value indeterminate.
static int sv4_logical_truth(sv4_t v) {
    int unknown = 0;
    for (int i = 0; i < (int)v.width; i++) {
        int bit = sv4_lsb_bit(v, i);
        if (bit == 1) return 1;
        if (bit >= 2) unknown = 1;
    }
    return unknown ? 2 : 0;
}

sv4_t sv4_lognot(sv4_t a) {
    int truth = sv4_logical_truth(a);
    if (truth == 2) return SV4_X(1);
    return sv4_from_u64(truth ? 0 : 1, 1, 0);
}

sv4_t sv4_logand(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 0 || tb == 0) return SV4_C(0, 1);
    if (ta == 1 && tb == 1) return SV4_C(1, 1);
    return SV4_X(1);
}

sv4_t sv4_logor(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 1 || tb == 1) return SV4_C(1, 1);
    if (ta == 0 && tb == 0) return SV4_C(0, 1);
    return SV4_X(1);
}

// Per-bit AND/OR/XOR/XNOR with operands resized to the result width.
static sv4_t sv4_bitwise(sv4_t a, sv4_t b, int op) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = w;
    for (int i = 0; i < (int)w; i++) {
        int ab = sv4_lsb_bit(ra, i), ax = ab >= 2; // Z behaves as X here (LRM 11.4.5)
        int bb = sv4_lsb_bit(rb, i), bx = bb >= 2;
        int o_b = 0, o_x = 0;
        if (op == 0) { // AND: 0 dominates
            if ((!ax && !ab) || (!bx && !bb)) o_b = 0;
            else if (!ax && ab && !bx && bb) o_b = 1;
            else o_x = 1;
        } else if (op == 1) { // OR: 1 dominates
            if ((!ax && ab) || (!bx && bb)) o_b = 1;
            else if (!ax && !ab && !bx && !bb) o_b = 0;
            else o_x = 1;
        } else if (op == 2) { // XOR
            if (ax || bx) o_x = 1;
            else o_b = ab ^ bb;
        } else { // XNOR
            if (ax || bx) o_x = 1;
            else o_b = !(ab ^ bb);
        }
        sv4_lsb_bit_set(&r, i, o_x ? 2 : o_b);
    }
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_and(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 0); }
sv4_t sv4_or(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 1); }
sv4_t sv4_xor(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 2); }
sv4_t sv4_xnor(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 3); }

static sv4_t sv4_reduce(sv4_t a, int op) {
    int ones = 0, zeros = 0, unknown = 0;
    for (int i = 0; i < (int)a.width; i++) {
        int bit = sv4_lsb_bit(a, i);
        if (bit == 1) ones++;
        else if (bit == 0) zeros++;
        else unknown = 1;
    }
    int r;
    switch (op) {
        case 0: // AND: a known zero dominates; otherwise X beats all-one.
            if (zeros != 0) return SV4_C(0, 1);
            if (unknown) return SV4_X(1);
            r = 1;
            break;
        case 1: // OR: a known one dominates; otherwise X beats all-zero.
            if (ones != 0) return SV4_C(1, 1);
            if (unknown) return SV4_X(1);
            r = 0;
            break;
        case 2: // XOR: any X/Z makes the parity unknowable.
            if (unknown) return SV4_X(1);
            r = ones & 1;
            break;
        case 3:
            if (zeros != 0) r = 1;
            else if (unknown) return SV4_X(1);
            else r = 0;
            break;
        case 4:
            if (ones != 0) r = 0;
            else if (unknown) return SV4_X(1);
            else r = 1;
            break;
        default:
            if (unknown) return SV4_X(1);
            r = !(ones & 1);
            break;
    }
    return sv4_from_u64(r, 1, 0);
}

sv4_t sv4_reduce_and(sv4_t a) { return sv4_reduce(a, 0); }
sv4_t sv4_reduce_or(sv4_t a) { return sv4_reduce(a, 1); }
sv4_t sv4_reduce_xor(sv4_t a) { return sv4_reduce(a, 2); }
sv4_t sv4_reduce_nand(sv4_t a) { return sv4_reduce(a, 3); }
sv4_t sv4_reduce_nor(sv4_t a) { return sv4_reduce(a, 4); }
sv4_t sv4_reduce_xnor(sv4_t a) { return sv4_reduce(a, 5); }

static sv4_t sv4_shift(sv4_t a, sv4_t b, int right, int arith) {
    uint16_t w = a.width;
    if (sv4_is_unknown(b)) return sv4_x(w, a.is_signed);
    if (b.width > 64) { // shift amount beyond 64 bits: any high bit set -> 0
        for (int i = 1; i < sv4_nlimbs(b.width); i++)
            if (b.bits[i]) return sv4_from_u64(0, w, a.is_signed);
    }
    uint64_t sh = sv4_to_u64(b);
    if (sh >= w) {
        if (right && arith && a.is_signed && w > 0)
            return sv4_fill((uint8_t)sv4_lsb_bit(a, (int)w - 1), w, a.is_signed);
        return sv4_from_u64(0, w, a.is_signed);
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = w;
    r.is_signed = a.is_signed;
    int nsh = (int)sh;
    if (!right) {
        for (int i = nsh; i < (int)w; i++)
            sv4_lsb_bit_set(&r, i, sv4_lsb_bit(a, i - nsh));
    } else {
        for (int i = 0; i + nsh < (int)w; i++)
            sv4_lsb_bit_set(&r, i, sv4_lsb_bit(a, i + nsh));
        if (arith && a.is_signed && nsh > 0) {
            int msb = sv4_lsb_bit(a, (int)w - 1); // 2 = X fills X
            for (int i = (int)w - nsh; i < (int)w; i++)
                sv4_lsb_bit_set(&r, i, msb);
        }
    }
    return r;
}

sv4_t sv4_shl(sv4_t a, sv4_t b) { return sv4_shift(a, b, 0, 0); }
sv4_t sv4_shr(sv4_t a, sv4_t b) { return sv4_shift(a, b, 1, 0); }
sv4_t sv4_ashl(sv4_t a, sv4_t b) { return sv4_shift(a, b, 0, 1); }
sv4_t sv4_ashr(sv4_t a, sv4_t b) { return sv4_shift(a, b, 1, 1); }

static sv4_t sv4_cmp_bit(int known, int val) {
    if (!known) return SV4_X(1);
    return sv4_from_u64(val, 1, 0);
}

// Unsigned MSB-first limb compare of equal-width values: -1, 0 or +1.
static int sv4_ucmp(sv4_t a, sv4_t b) {
    int nl = sv4_nlimbs(a.width);
    for (int i = nl - 1; i >= 0; i--) {
        if (a.bits[i] != b.bits[i]) return a.bits[i] < b.bits[i] ? -1 : 1;
    }
    return 0;
}

sv4_t sv4_eq(sv4_t a, sv4_t b) {
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return SV4_X(1);
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    return sv4_cmp_bit(1, sv4_ucmp(ra, rb) == 0);
}

sv4_t sv4_neq(sv4_t a, sv4_t b) {
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return SV4_X(1);
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    return sv4_cmp_bit(1, sv4_ucmp(ra, rb) != 0);
}

sv4_t sv4_case_eq(sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    return sv4_cmp_bit(1, sv4_same(ra, rb));
}

sv4_t sv4_case_neq(sv4_t a, sv4_t b) {
    sv4_t r = sv4_case_eq(a, b);
    return sv4_from_u64(1 - sv4_to_u64(r), 1, 0);
}

// casez per LRM 12.5.1: a z (or ?) bit in the case ITEM is a don't-care; an x
// in the item matches an x selector bit only; a known item bit must equal the
// selector bit (a selector x/z never matches a known item bit).  Operands are
// zero-extended to max width before comparing, like `case`.
sv4_t sv4_casez_eq(sv4_t sel, sv4_t item) {
    uint16_t w = sv4_maxw(sel, item);
    sv4_t rs = sv4_resize(sel, w, 0);
    sv4_t ri = sv4_resize(item, w, 0);
    for (int i = 0; i < (int)w; i++) {
        int ib = sv4_lsb_bit(ri, i); // 0/1/2(x)/3(z)
        if (ib == 3) continue;       // item z/? -> don't-care
        int sb = sv4_lsb_bit(rs, i);
        if (ib == 2) {               // item x matches selector x only
            if (sb != 2) return SV4_C(0, 1);
        } else if (sb != ib) {       // known item: selector must equal it
            return SV4_C(0, 1);
        }
    }
    return SV4_C(1, 1);
}

// casex per LRM 12.5.1: x/z (or ?) bits in the ITEM are don't-cares, and a
// selector x/z is a don't-care against a known item bit too — only an
// opposite known bit fails the match.
sv4_t sv4_casex_eq(sv4_t sel, sv4_t item) {
    uint16_t w = sv4_maxw(sel, item);
    sv4_t rs = sv4_resize(sel, w, 0);
    sv4_t ri = sv4_resize(item, w, 0);
    for (int i = 0; i < (int)w; i++) {
        int ib = sv4_lsb_bit(ri, i);
        if (ib >= 2) continue;       // item x/z -> don't-care
        int sb = sv4_lsb_bit(rs, i);
        if (sb == ib) continue;      // equal known bits match
        if (sb >= 2) continue;       // selector x/z is a don't-care in casex
        return SV4_C(0, 1);          // opposite known bit -> no match
    }
    return SV4_C(1, 1);
}

static sv4_t sv4_cmp(sv4_t a, sv4_t b, int op) {
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return SV4_X(1);
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    int c;
    if (s) {
        // Flip the sign bit of both: the unsigned order then matches the
        // two's-complement signed order.
        if (w > 0) {
            int l = (int)(w - 1) >> 6;
            uint64_t m = 1ULL << ((w - 1) & 63);
            ra.bits[l] ^= m;
            rb.bits[l] ^= m;
        }
        c = sv4_ucmp(ra, rb);
    } else {
        c = sv4_ucmp(ra, rb);
    }
    int r;
    if (op == 0) r = c < 0;
    else if (op == 1) r = c <= 0;
    else if (op == 2) r = c > 0;
    else r = c >= 0;
    return sv4_cmp_bit(1, r);
}

sv4_t sv4_lt(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 0); }
sv4_t sv4_le(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 1); }
sv4_t sv4_gt(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 2); }
sv4_t sv4_ge(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 3); }

sv4_t sv4_mux(sv4_t sel, sv4_t a, sv4_t b) {
    uint16_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    int s_known = !sv4_is_unknown(sel);
    int s_one = s_known && sv4_to_bool(sel);
    if (s_one) return ra;
    if (s_known) return rb;
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = w;
    r.is_signed = s;
    for (int i = 0; i < (int)w; i++) {
        int ab = sv4_lsb_bit(ra, i), bb = sv4_lsb_bit(rb, i);
        sv4_lsb_bit_set(&r, i, ab == bb ? ab : 2);
    }
    return r;
}

sv4_t sv4_concat(sv4_t hi, sv4_t lo) {
    uint32_t total = (uint32_t)hi.width + (uint32_t)lo.width;
    if (total > LLG_MAX_WIDTH) total = LLG_MAX_WIDTH;
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = (uint16_t)total;
    r.is_signed = 0;
    for (int i = 0; i < (int)lo.width; i++)
        sv4_lsb_bit_set(&r, i, sv4_lsb_bit(lo, i));
    for (int i = 0; i < (int)hi.width && (int)lo.width + i < (int)total; i++)
        sv4_lsb_bit_set(&r, (int)lo.width + i, sv4_lsb_bit(hi, i));
    return r;
}

sv4_t sv4_repeat(sv4_t pat, uint64_t n) {
    uint64_t w_total64;
    if (pat.width == 0 || n <= LLG_MAX_WIDTH / pat.width) {
        w_total64 = (uint64_t)pat.width * n;
    } else {
        w_total64 = LLG_MAX_WIDTH; // clamp
    }
    uint16_t w_total = (uint16_t)w_total64;
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = w_total;
    r.is_signed = 0;
    for (uint64_t rep = 0; rep < n; rep++) {
        uint64_t dst64 = rep * (uint64_t)pat.width;
        if (dst64 >= w_total64) break;
        int dst = (int)dst64;
        for (int i = 0; i < (int)pat.width && dst + i < (int)w_total; i++)
            sv4_lsb_bit_set(&r, dst + i, sv4_lsb_bit(pat, i));
    }
    return r;
}

sv4_t sv4_bit_select(sv4_t v, uint64_t i) {
    int b = sv4_lsb_bit(v, (int)i);
    if (b == 2) return SV4_X(1);
    if (b == 3) return SV4_Z(1);
    return sv4_from_u64(b, 1, 0);
}

void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value) {
    if (i >= tgt->width) return;
    sv4_lsb_bit_set(tgt, (int)i, sv4_lsb_bit(value, 0));
}

sv4_t sv4_part_select(sv4_t v, int left, int right) {
    int w = (left > right ? left - right : right - left) + 1;
    if (w < 0) w = 0;
    if (left < 0 || right < 0 || left >= (int)v.width || right >= (int)v.width) {
        return sv4_x((uint16_t)w, 0);
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = (uint16_t)w;
    int step = left > right ? -1 : 1;
    int out = 0;
    for (int i = left; ; i += step) {
        int b = sv4_lsb_bit(v, i);
        int pos = w - 1 - out; // first index (left) is the MSB
        sv4_lsb_bit_set(&r, pos, b);
        out++;
        if (i == right) break;
    }
    return r;
}

void sv4_part_select_set(sv4_t* tgt, int left, int right, sv4_t value) {
    int step = left > right ? -1 : 1;
    int in = (int)value.width - 1; // value MSB maps to the first target index
    for (int i = left; ; i += step) {
        if (i < 0 || i >= (int)tgt->width) {
            in--;
            if (i == right) break;
            continue;
        }
        sv4_lsb_bit_set(tgt, i, sv4_lsb_bit(value, in));
        in--;
        if (i == right) break;
    }
}

sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint16_t width, int neg) {
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = width;
    // pos: {v[base+width-1] .. v[base]}; neg: {v[base] .. v[base-width+1]}
    int i = neg ? (int)base : (int)base + (int)width - 1;
    for (int out = 0; out < (int)width; out++) {
        int b = sv4_lsb_bit(v, i);
        int pos = (int)width - 1 - out; // first index is the MSB
        sv4_lsb_bit_set(&r, pos, b);
        i--;
    }
    return r;
}

void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint16_t width, int neg,
                             sv4_t value) {
    // pos: first target index base+width-1; neg: first target index base.
    int i = neg ? (int)base : (int)base + (int)width - 1;
    int in = (int)width - 1;
    for (int k = 0; k < (int)width; k++) {
        if (i >= 0 && i < (int)tgt->width) {
            sv4_lsb_bit_set(tgt, i, sv4_lsb_bit(value, in));
        }
        i--;
        in--;
    }
}

// ── Formatting ────────────────────────────────────────────────────────────────

static void llg_append(char* buf, size_t cap, size_t* len, char c) {
    if (*len + 1 < cap) buf[(*len)++] = c;
}

// Unsigned decimal via repeated long division by 10 across the limbs.  A
// signed value (`is_signed`) with the sign bit set prints '-' followed by its
// two's-complement magnitude (~v + 1 within the value's width).
void sv4_to_dec_string(sv4_t v, char* buf, size_t cap) {
    if (sv4_is_unknown(v)) {
        buf[0] = 'x';
        buf[1] = 0;
        return;
    }
    int negative = 0;
    uint64_t tmp[LLG_LIMBS];
    if (v.is_signed && v.width > 0 && sv4_lsb_bit(v, (int)v.width - 1) == 1) {
        negative = 1;
        sv4_t mag = sv4_neg(v);
        for (int i = 0; i < LLG_LIMBS; i++) tmp[i] = mag.bits[i];
    } else {
        for (int i = 0; i < LLG_LIMBS; i++) tmp[i] = v.bits[i];
    }
    int nl = sv4_nlimbs(v.width);
    if (nl > 0) tmp[nl - 1] &= sv4_limb_mask(v.width, nl - 1);
    char digits[320]; // 1024 bits -> at most 309 decimal digits
    int n = 0;
    for (;;) {
        int nonzero = 0;
        for (int i = 0; i < LLG_LIMBS; i++)
            if (tmp[i]) { nonzero = 1; break; }
        if (!nonzero) break;
        uint64_t rem = 0;
        for (int i = nl - 1; i >= 0; i--) {
            __uint128_t cur = ((__uint128_t)rem << 64) | tmp[i];
            tmp[i] = (uint64_t)(cur / 10);
            rem = (uint64_t)(cur % 10);
        }
        digits[n++] = (char)('0' + (int)rem);
    }
    size_t len = 0;
    if (negative) llg_append(buf, cap, &len, '-');
    if (n == 0) {
        llg_append(buf, cap, &len, '0');
    } else {
        while (n > 0) llg_append(buf, cap, &len, digits[--n]);
    }
    buf[len] = 0;
}

void sv4_format(char fmt, sv4_t v, char* buf, size_t cap) {
    size_t len = 0;
    buf[0] = 0;
    switch (fmt) {
        case 'd':
            sv4_to_dec_string(v, buf, cap);
            return;
        case 'b':
            for (int i = (int)v.width - 1; i >= 0; i--) {
                int b = sv4_lsb_bit(v, i);
                llg_append(buf, cap, &len, b == 2 ? 'x' : b == 3 ? 'z' : (b ? '1' : '0'));
            }
            break;
        case 'h': {
            int digits = ((int)v.width + 3) / 4;
            for (int d = digits - 1; d >= 0; d--) {
                int has_x = 0, has_z = 0, val = 0;
                for (int k = 0; k < 4; k++) {
                    int idx = d * 4 + k;
                    int b = idx < (int)v.width ? sv4_lsb_bit(v, idx) : 0;
                    if (b == 2) { has_x = 1; break; } // X wins over Z
                    if (b == 3) { has_z = 1; }
                    else val |= b << k;
                }
                if (has_x) {
                    llg_append(buf, cap, &len, 'x');
                } else if (has_z) {
                    llg_append(buf, cap, &len, 'z');
                } else {
                    llg_append(buf, cap, &len,
                                val < 10 ? (char)('0' + val) : (char)('a' + val - 10));
                }
            }
            break;
        }
        case 'o': {
            int digits = ((int)v.width + 2) / 3;
            for (int d = digits - 1; d >= 0; d--) {
                int has_x = 0, has_z = 0, val = 0;
                for (int k = 0; k < 3; k++) {
                    int idx = d * 3 + k;
                    int b = idx < (int)v.width ? sv4_lsb_bit(v, idx) : 0;
                    if (b == 2) { has_x = 1; break; } // X wins over Z
                    if (b == 3) { has_z = 1; }
                    else val |= b << k;
                }
                if (has_x) {
                    llg_append(buf, cap, &len, 'x');
                } else if (has_z) {
                    llg_append(buf, cap, &len, 'z');
                } else {
                    llg_append(buf, cap, &len, (char)('0' + val));
                }
            }
            break;
        }
        default:
            llg_append(buf, cap, &len, '?');
            break;
    }
    buf[len] = 0;
}

// ── Scheduler state ───────────────────────────────────────────────────────────

typedef enum {
    W_NONE,
    W_TIME,
    W_EVENTS,
    W_EVENT, // waiting on one or more named events
    W_MIXED, // atomic named-event + signal or-list (@(posedge a or ev))
    W_LEVEL,
    W_FORK,    // llg_join: waiting for a fork group
    W_FORK_ALL // llg_wait_fork: waiting for all of the current proc's groups
} llg_wait_kind_t;

typedef struct llg_nba {
    struct llg_nba* next;
    sv4_t* target;
    sv4_t value;
    int is_real;
    double* real_target;
    double real_value;
} llg_nba_t;

typedef struct llg_wait {
    struct llg_wait* next;         // all active waits (signal + timed + inactive)
    struct llg_wait* time_next;    // sorted timed list
    struct llg_wait* inactive_next; // inactive list (#0 waiters)
    llg_proc_t* proc;
    llg_wait_kind_t kind;
    uint64_t time;                // W_TIME
    llg_event_spec_t* specs;     // W_EVENTS: copied array; W_MIXED: signal half
    sv4_t* last;                  // W_EVENTS/W_MIXED: last-seen values
    int n;                        // W_EVENTS/W_MIXED (signal entry count)
    const llg_event_t** evs;     // W_EVENT/W_MIXED: copied event list
    int n_evs;                    // W_EVENT/W_MIXED
    sv4_t* sig;                   // W_LEVEL
    sv4_t level_val;              // W_LEVEL
    llg_fork_group_t* grp;       // W_FORK: group being joined
    llg_proc_t* parent;          // W_FORK_ALL: the waiting proc itself
} llg_wait_t;

typedef struct llg_fork_child {
    struct llg_fork_child* next;
    llg_proc_t* proc;            // NULL once freed by disable_fork
} llg_fork_child_t;

struct llg_fork_group {
    int join_kind;                // LLG_JOIN / LLG_JOIN_NONE / LLG_JOIN_ANY
    int remaining;                // live children; decremented on done AND on kill
    int resumed;                  // join_any: parent already woken
    llg_proc_t* parent;          // spawning proc
    llg_fork_child_t* children;  // for disable_fork
    struct llg_fork_group* next_g; // per-proc live-group list (or zombie list)
};

struct llg_proc {
    aco_t* co;
    const char* name;
    void (*fn)(llg_proc_t*);
    llg_nba_t* nba_head;
    llg_nba_t* nba_tail;
    llg_wait_t wait;
    llg_proc_t* next_ready;
    llg_fork_group_t* fork_groups; // live groups spawned by this proc
    llg_fork_group_t* grp;         // group this proc belongs to (NULL top-level)
};

#define LLG_ZERO_LOOP_LIMIT 10000000ULL

// ── $monitor / $strobe state ──────────────────────────────────────────────────

typedef struct {
    int active;          // a monitor is registered
    int enabled;         // $monitoron / $monitoroff
    char* fmt;           // strdup'd format string
    int n;               // number of displayed arguments
    llg_mon_eval_fn eval;
    sv4_t* last;         // last-printed argument values (n)
    sv4_t* work;         // scratch buffer the eval fn fills (n)
} llg_monitor_state_t;

typedef struct llg_strobe {
    struct llg_strobe* next;
    char* fmt;           // strdup'd format string
    int n;
    llg_mon_eval_fn eval;
    sv4_t* work;         // scratch buffer the eval fn fills (n)
} llg_strobe_t;

typedef struct {
    aco_t* main_co;
    aco_share_stack_t* share_stack;
    llg_proc_t* ready_head;
    llg_proc_t* ready_tail;
    llg_wait_t* timed_head;   // sorted ascending by time
    llg_wait_t* inactive_head; // #0 waiters at the current time (FIFO)
    llg_wait_t* inactive_tail;
    llg_wait_t* waiters;      // all active waits
    int wait_count;
    uint64_t now;
    int finish;
    uint64_t region_passes;    // zero-delay guard units in the current time step (region passes + coroutine resumes)
    llg_proc_t* all_procs[LLG_MAX_PROCS];
    int n_procs;
    llg_fork_group_t* zombie_groups; // completed/killed groups awaiting teardown
    llg_monitor_state_t mon;   // the active $monitor (at most one)
    llg_strobe_t* strobes;     // pending $strobe lines for this time step
    // Active procedural forces: signal pointer -> pre-force saved value.  A
    // second `force` on an already forced signal updates the forced value in
    // place but keeps `saved`; `release` removes the entry and writes `saved`
    // back through `sig_write`.
    struct { sv4_t* sig; sv4_t saved; } force_table[LLG_MAX_FORCE];
    int force_count;
} llg_rt_ctx_t;

static llg_rt_ctx_t g;

// Registered final-block processes (see llg_rt.h).  Kept OUTSIDE the runtime
// context: `llg_rt_cleanup` memsets the context, and registration happens
// around the `llg_rt_run()` call in generated `main()`.
static struct {
    void (*fn)(llg_proc_t*);
    const char* name;
} llg_finals[LLG_MAX_FINALS];
static int llg_n_finals;
static int llg_in_finals;
// Scheduler time when `llg_rt_run` exited; `$time` inside finals reports it.
static uint64_t llg_final_time;

static void register_proc(llg_proc_t* p) {
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == NULL) {
            g.all_procs[i] = p;
            return;
        }
    }
    if (g.n_procs >= LLG_MAX_PROCS) {
        fprintf(stderr, "llg: too many processes (limit %d)\n", LLG_MAX_PROCS);
        abort();
    }
    g.all_procs[g.n_procs++] = p;
}

static void unregister_proc(llg_proc_t* p) {
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == p) {
            g.all_procs[i] = NULL;
            while (g.n_procs > 0 && g.all_procs[g.n_procs - 1] == NULL)
                g.n_procs--;
            return;
        }
    }
}

static llg_proc_t* llg_current(void) {
    return (llg_proc_t*)aco_get_arg();
}

static void enqueue_ready(llg_proc_t* p) {
    p->next_ready = NULL;
    if (g.ready_tail) {
        g.ready_tail->next_ready = p;
        g.ready_tail = p;
    } else {
        g.ready_head = g.ready_tail = p;
    }
}

static void remove_waiters_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.waiters;
    while (*pp) {
        if (*pp == w) {
            *pp = w->next;
            return;
        }
        pp = &(*pp)->next;
    }
}

static void remove_timed_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->time_next;
            return;
        }
        pp = &(*pp)->time_next;
    }
}

static void insert_inactive(llg_wait_t* w) {
    w->inactive_next = NULL;
    if (g.inactive_tail) {
        g.inactive_tail->inactive_next = w;
    } else {
        g.inactive_head = w;
    }
    g.inactive_tail = w;
}

static void remove_inactive_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.inactive_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->inactive_next;
            if (g.inactive_tail == w) {
                g.inactive_tail = NULL;
                for (llg_wait_t* q = g.inactive_head; q; q = q->inactive_next)
                    g.inactive_tail = q;
            }
            w->inactive_next = NULL;
            return;
        }
        pp = &(*pp)->inactive_next;
    }
}

static void remove_ready_entry(llg_proc_t* p) {
    llg_proc_t** pp = &g.ready_head;
    while (*pp) {
        if (*pp == p) {
            *pp = p->next_ready;
            if (g.ready_tail == p) {
                g.ready_tail = NULL;
                for (llg_proc_t* q = g.ready_head; q; q = q->next_ready)
                    g.ready_tail = q;
            }
            p->next_ready = NULL;
            return;
        }
        pp = &(*pp)->next_ready;
    }
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on; defined below with the other named-event helpers.
static void event_unlink(llg_wait_t* w);

static void insert_timed(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp && (*pp)->time <= w->time) pp = &(*pp)->time_next;
    w->time_next = *pp;
    *pp = w;
}

// Wake a suspended process: clear its wait node and schedule it.
static void wake_proc(llg_proc_t* p) {
    llg_wait_t* w = &p->wait;
    if (w->kind == W_NONE) return;
    remove_waiters_entry(w);
    if (w->kind == W_TIME) {
        remove_timed_entry(w);
        remove_inactive_entry(w);
    }
    if (w->kind == W_EVENT || w->kind == W_MIXED) {
        event_unlink(w);
    }
    free(w->specs);
    free(w->last);
    free(w->evs);
    w->specs = NULL;
    w->last = NULL;
    w->evs = NULL;
    w->n = 0;
    w->n_evs = 0;
    w->kind = W_NONE;
    g.wait_count--;
    enqueue_ready(p);
}

static void register_wait(void) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->proc = p;
    w->next = g.waiters;
    g.waiters = w;
    g.wait_count++;
}

// ── Named events ──────────────────────────────────────────────────────────────

// Register `p` on `ev`'s waiter table (fixed capacity, like the other
// runtime resource limits).
static void event_list_add(llg_event_t* ev, llg_proc_t* p) {
    if (ev->n_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->waiters[ev->n_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    for (int i = 0; i < w->n_evs; i++) {
        llg_event_t* ev = (llg_event_t*)w->evs[i];
        for (int k = 0; k < ev->n_waiters; k++) {
            if (ev->waiters[k] == w->proc) {
                ev->waiters[k] = ev->waiters[ev->n_waiters - 1];
                ev->n_waiters--;
                break;
            }
        }
    }
}

// ── fork/join (coroutine children) ────────────────────────────────────────────

static void llg_kill_proc_tree(llg_proc_t* p); // mutual recursion below
static void llg_proc_entry(void);               // defined in the public API section

// Unlink a suspended or ready proc from every scheduler queue, free its
// pending NBA list, destroy its coroutine and free the proc struct.  The proc
// must not be running (children are suspended or ready while the parent
// executes disable_fork).
static void llg_kill_proc(llg_proc_t* p) {
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* nx = n->next;
        free(n);
        n = nx;
    }
    p->nba_head = p->nba_tail = NULL;

    llg_wait_t* w = &p->wait;
    if (w->kind != W_NONE) {
        remove_waiters_entry(w);
        if (w->kind == W_TIME) {
            remove_timed_entry(w);
            remove_inactive_entry(w);
        }
        if (w->kind == W_EVENT || w->kind == W_MIXED) {
            event_unlink(w);
        }
        free(w->specs);
        free(w->last);
        free(w->evs);
        w->specs = NULL;
        w->last = NULL;
        w->evs = NULL;
        w->n = 0;
        w->n_evs = 0;
        w->kind = W_NONE;
        g.wait_count--;
    }
    remove_ready_entry(p);

    aco_destroy(p->co);
    unregister_proc(p);
    free(p);
}

// Kill every group spawned by `p`: each child (and its descendants) is freed
// recursively, the group structs go onto the zombie list for teardown.  `p`
// itself is untouched — used by disable_fork, which kills only descendants.
static void llg_kill_proc_groups(llg_proc_t* p) {
    llg_fork_group_t* grp = p->fork_groups;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            if (c->proc) {
                llg_kill_proc_tree(c->proc);
                c->proc = NULL; // freed inline; teardown skips it
            }
            c = next_c;
        }
        grp->remaining = 0;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        grp = next_g;
    }
    p->fork_groups = NULL;
}

// Kill `p` and all of its descendants.
static void llg_kill_proc_tree(llg_proc_t* p) {
    llg_kill_proc_groups(p);
    llg_kill_proc(p);
}

// One child of `grp` finished (llg_proc_done).  Decrement the live count,
// wake a join/wait_fork waiter whose condition is now met, and move the group
// to the zombie list once the last child is done.
static void llg_fork_group_child_done(llg_fork_group_t* grp) {
    llg_proc_t* parent = grp->parent;
    grp->remaining--;
    int wake = 0;
    if (grp->join_kind == LLG_JOIN) {
        if (grp->remaining == 0) wake = 1;
    } else if (grp->join_kind == LLG_JOIN_ANY) {
        if (!grp->resumed) {
            grp->resumed = 1;
            wake = 1;
        }
    }
    if (wake && parent->wait.kind == W_FORK && parent->wait.grp == grp) {
        wake_proc(parent);
    }
    if (grp->remaining == 0) {
        // Unlink from the parent's live-group list; the group and its child
        // list are freed by process_zombie_groups at the next safe point.
        // join_any / join_none groups stay live until the last child finishes
        // so wait_fork still works.
        llg_fork_group_t** pp = &parent->fork_groups;
        while (*pp && *pp != grp) pp = &(*pp)->next_g;
        if (*pp) *pp = grp->next_g;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        // Wake any wait_fork waiter whose own groups are now all done.
        llg_wait_t* w = g.waiters;
        while (w) {
            llg_wait_t* next = w->next;
            if (w->kind == W_FORK_ALL && w->parent->fork_groups == NULL) {
                wake_proc(w->proc);
            }
            w = next;
        }
    }
}

llg_fork_group_t* llg_fork_group_new(int join_kind) {
    llg_proc_t* parent = llg_current();
    llg_fork_group_t* grp = (llg_fork_group_t*)calloc(1, sizeof(llg_fork_group_t));
    grp->join_kind = join_kind;
    grp->parent = parent;
    grp->next_g = parent->fork_groups;
    parent->fork_groups = grp;
    return grp;
}

llg_proc_t* llg_fork(void (*fn)(llg_proc_t*), const char* name, llg_fork_group_t* grp) {
    llg_proc_t* p = (llg_proc_t*)calloc(1, sizeof(llg_proc_t));
    p->name = name;
    p->fn = fn;
    p->grp = grp;
    // aco_create from inside a coroutine is safe (mallocs/zeroes an aco_t and
    // sets registers only; no global state).  Children yield to g.main_co, the
    // scheduler, so aco_resume in llg_rt_run regains control.
    p->co = aco_create(g.main_co, g.share_stack, 256u << 10, llg_proc_entry, p);
    grp->remaining++;
    llg_fork_child_t** pp = &grp->children;
    while (*pp) pp = &(*pp)->next;
    llg_fork_child_t* c = (llg_fork_child_t*)malloc(sizeof(llg_fork_child_t));
    c->proc = p;
    c->next = NULL;
    *pp = c;
    register_proc(p);
    enqueue_ready(p);
    return p;
}

void llg_join(llg_fork_group_t* grp) {
    if (grp->remaining == 0) {
        // Empty fork groups never receive a child-done callback, so finalize
        // them here before join or wait_fork can observe a permanently live
        // group. The parent is the currently running process.
        llg_fork_group_t** pp = &grp->parent->fork_groups;
        while (*pp && *pp != grp) pp = &(*pp)->next_g;
        if (*pp) *pp = grp->next_g;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        return;
    }
    if (grp->join_kind == LLG_JOIN_NONE) return;
    if (grp->join_kind == LLG_JOIN_ANY && grp->resumed) return; // already met
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK;
    w->grp = grp;
    register_wait();
    aco_yield();
}

void llg_wait_fork(void) {
    llg_proc_t* p = llg_current();
    if (p->fork_groups == NULL) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK_ALL;
    w->parent = p;
    register_wait();
    aco_yield();
}

void llg_disable_fork(void) {
    llg_kill_proc_groups(llg_current());
}

// Free completed/killed fork groups: their child procs that were not already
// freed by disable_fork, the child list nodes and the group struct.  Called
// from llg_rt_run after commit_nbas with no coroutine running, so done
// children's NBAs have been committed and their all_procs slots can be NULLed
// safely (the next commit_nbas then skips them).
static void process_zombie_groups(void) {
    llg_fork_group_t* grp = g.zombie_groups;
    g.zombie_groups = NULL;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        int deferred = 0;
        for (llg_fork_child_t* c = grp->children; c; c = c->next) {
            if (!c->proc) continue;
            // A completed child can still own live join_none descendants.
            // Keep its process object until those groups unlink themselves;
            // their completion path dereferences the parent pointer.
            if (c->proc->fork_groups) {
                deferred = 1;
                continue;
            }
            aco_destroy(c->proc->co);
            unregister_proc(c->proc);
            free(c->proc);
            c->proc = NULL;
        }
        if (deferred) {
            grp->next_g = g.zombie_groups;
            g.zombie_groups = grp;
        } else {
            llg_fork_child_t* c = grp->children;
            while (c) {
                llg_fork_child_t* next_c = c->next;
                free(c);
                c = next_c;
            }
            free(grp);
        }
        grp = next_g;
    }
}

// ── Signal writes and waiter scanning ─────────────────────────────────────────

static int sv4_is_zero(sv4_t v) { return !sv4_is_unknown(v) && !sv4_to_bool(v); }
static int sv4_is_one(sv4_t v) { return !sv4_is_unknown(v) && sv4_to_bool(v); }

static int ev_matches(sv4_t old, sv4_t new, int kind) {
    if (kind == LLG_EV_ANY) return !sv4_same(old, new);
    if (kind == LLG_EV_POSEDGE) {
        return (sv4_is_zero(old) && (!sv4_is_zero(new))) ||
               (sv4_is_unknown(old) && sv4_is_one(new));
    }
    // negedge
    return (sv4_is_one(old) && (!sv4_is_one(new))) ||
           (sv4_is_unknown(old) && sv4_is_zero(new));
}

static void sig_write(sv4_t* target, sv4_t value) {
    // Mask the written limbs to the vector's width before comparing/storing.
    for (int i = 0; i < LLG_LIMBS; i++) {
        uint64_t m = sv4_limb_mask(value.width, i);
        value.bits[i] &= m;
        value.x[i] &= m;
        value.z[i] &= m;
    }
    if (target->width == value.width && sv4_same(*target, value)) return;
    *target = value;
    llg_wait_t* w = g.waiters;
    while (w) {
        llg_wait_t* next = w->next;
        int wake = 0;
        if (w->kind == W_EVENTS || w->kind == W_MIXED) {
            for (int i = 0; i < w->n; i++) {
                if (w->specs[i].sig == target) {
                    sv4_t old = w->last[i];
                    w->last[i] = *target;
                    if (ev_matches(old, *target, w->specs[i].kind)) wake = 1;
                }
            }
        } else if (w->kind == W_LEVEL) {
            if (w->sig == target && sv4_same(*target, w->level_val)) wake = 1;
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
}

// ── Procedural force / release ───────────────────────────────────────────────

// Is `sig` currently forced?  Procedural writes (llg_ba and NBA commits) are
// dropped while a signal is forced; net resolution and monitor reads are not
// affected.
static int llg_is_forced(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++)
        if (g.force_table[i].sig == sig) return 1;
    return 0;
}

void llg_force(sv4_t* sig, sv4_t value) {
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].sig == sig) {
            // Re-force: update the forced value, keep the ORIGINAL saved value.
            sig_write(sig, value);
            return;
        }
    }
    if (g.force_count >= LLG_MAX_FORCE) {
        fprintf(stderr, "llg: too many forced signals (limit %d)\n", LLG_MAX_FORCE);
        abort();
    }
    g.force_table[g.force_count].sig = sig;
    g.force_table[g.force_count].saved = *sig;
    g.force_count++;
    sig_write(sig, value);
}

void llg_release(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].sig == sig) {
            sv4_t saved = g.force_table[i].saved;
            // Swap-with-last keeps the table compact; the restored value is
            // written through sig_write so waiters wake on the change.
            g.force_count--;
            g.force_table[i] = g.force_table[g.force_count];
            sig_write(sig, saved);
            return;
        }
    }
    // Releasing an unforced signal is a no-op (LRM 10.6.2).
}

// ── Public scheduler API ──────────────────────────────────────────────────────

static void llg_last_word(void) {
    fprintf(stderr, "llg: fatal: coroutine returned without aco_exit "
                    "(codegen bug)\n");
    abort();
}

static void llg_proc_entry(void) {
    llg_proc_t* self = (llg_proc_t*)aco_get_arg();
    self->fn(self);
    llg_last_word(); // never reached when the body called llg_proc_done
}

static void free_group_storage(llg_fork_group_t* grp) {
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            free(c);
            c = next_c;
        }
        free(grp);
        grp = next_g;
    }
}

static void free_proc_storage(llg_proc_t* p) {
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* next = n->next;
        free(n);
        n = next;
    }
    free(p->wait.specs);
    free(p->wait.last);
    free(p->wait.evs);
    if (p->co) aco_destroy(p->co);
    free(p);
}

void llg_rt_cleanup(void) {
    // Groups own only child-list nodes; process objects are owned once by
    // all_procs and are released separately below.
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (p && p->fork_groups) {
            free_group_storage(p->fork_groups);
            p->fork_groups = NULL;
        }
    }
    free_group_storage(g.zombie_groups);
    g.zombie_groups = NULL;

    while (g.strobes) {
        llg_strobe_t* next = g.strobes->next;
        free(g.strobes->fmt);
        free(g.strobes->work);
        free(g.strobes);
        g.strobes = next;
    }
    free(g.mon.fmt);
    free(g.mon.last);
    free(g.mon.work);

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    if (g.share_stack) aco_share_stack_destroy(g.share_stack);
    if (g.main_co) aco_destroy(g.main_co);
    memset(&g, 0, sizeof(g));
}

void llg_rt_init(void) {
    llg_rt_cleanup();
    llg_n_finals = 0; // a fresh run never inherits final registrations
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(4u << 20);
}

void llg_rt_finish(void) {
    g.finish = 1;
    if (llg_in_finals) llg_proc_done(llg_current());
}

uint64_t llg_time(void) { return g.now; }

int llg_rt_process_count(void) {
    int count = 0;
    for (int i = 0; i < g.n_procs; i++)
        if (g.all_procs[i]) count++;
    return count;
}

llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name) {
    llg_proc_t* p = (llg_proc_t*)calloc(1, sizeof(llg_proc_t));
    p->name = name;
    p->fn = fn;
    p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
    register_proc(p);
    enqueue_ready(p);
    return p;
}

void llg_proc_done(llg_proc_t* self) {
    if (self->grp) llg_fork_group_child_done(self->grp);
    aco_exit(); // never returns
}

void llg_wait_time(uint64_t ticks) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_TIME;
    w->time = g.now + ticks;
    if (ticks == 0) {
        // `#0` yields into the INACTIVE region of the current time step
        // (LRM §4.4.2): it runs after the active region drains and before
        // the NBA region commits.
        insert_inactive(w);
    } else {
        insert_timed(w);
    }
    register_wait();
    aco_yield();
}

void llg_wait_any(sv4_t** sigs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->n = n;
    w->specs = (llg_event_spec_t*)malloc((size_t)n * sizeof(llg_event_spec_t));
    w->last = (sv4_t*)malloc((size_t)n * sizeof(sv4_t));
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = sigs[i];
        w->specs[i].kind = LLG_EV_ANY;
        w->last[i] = *sigs[i];
    }
    register_wait();
    aco_yield();
}

void llg_wait_any_events(llg_event_spec_t* specs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->n = n;
    w->specs = (llg_event_spec_t*)malloc((size_t)n * sizeof(llg_event_spec_t));
    w->last = (sv4_t*)malloc((size_t)n * sizeof(sv4_t));
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = specs[i].sig;
        w->specs[i].kind = specs[i].kind;
        w->last[i] = *specs[i].sig;
    }
    register_wait();
    aco_yield();
}

void llg_wait_edge(sv4_t* sig, int posedge) {
    llg_event_spec_t spec;
    spec.sig = sig;
    spec.kind = posedge ? LLG_EV_POSEDGE : LLG_EV_NEGEDGE;
    llg_wait_any_events(&spec, 1);
}

void llg_wait_level(sv4_t* sig, sv4_t value) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_LEVEL;
    w->sig = sig;
    w->level_val = value;
    register_wait();
    aco_yield();
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────

void llg_event_trigger(llg_event_t* ev) {
    int n = ev->n_waiters;
    if (n == 0) return;
    // Snapshot and detach everyone first: wake_proc unlinks the waiter from
    // every event list it registered on, which must not fight the iteration
    // over this event's own table.  Wake order is the snapshot order, i.e.
    // the current table order: deterministic, and equal to registration
    // order unless earlier partial unlinks (swap-with-last) reordered it.
    llg_proc_t* wake[LLG_MAX_EVENT_WAITERS];
    memcpy(wake, ev->waiters, (size_t)n * sizeof(llg_proc_t*));
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        wake_proc(wake[i]);
    }
}

void llg_wait_event(llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    llg_wait_events(list, 1);
}

void llg_wait_events(const llg_event_t* const* evs, int n) {
    if (n <= 0) return;
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT;
    w->n_evs = n;
    w->evs = (const llg_event_t**)malloc((size_t)n * sizeof(llg_event_t*));
    memcpy(w->evs, evs, (size_t)n * sizeof(llg_event_t*));
    for (int i = 0; i < n; i++) {
        // The lists are owned by the generated model's non-const globals.
        event_list_add((llg_event_t*)w->evs[i], p);
    }
    register_wait();
    aco_yield();
}

void llg_wait_mixed(llg_wait_src_t* srcs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->n = nsig;
    w->specs = nsig ? (llg_event_spec_t*)malloc((size_t)nsig * sizeof(llg_event_spec_t)) : NULL;
    w->last = nsig ? (sv4_t*)malloc((size_t)nsig * sizeof(sv4_t)) : NULL;
    w->n_evs = nev;
    w->evs = nev ? (const llg_event_t**)malloc((size_t)nev * sizeof(llg_event_t*)) : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            w->specs[si].sig = srcs[i].sig;
            w->specs[si].kind = srcs[i].kind;
            w->last[si] = *srcs[i].sig;
            si++;
        } else {
            w->evs[ei++] = srcs[i].ev;
            event_list_add((llg_event_t*)srcs[i].ev, p);
        }
    }
    register_wait();
    aco_yield();
}

void llg_nba(sv4_t* target, sv4_t value) {
    llg_proc_t* p = llg_current();
    llg_nba_t* n = (llg_nba_t*)malloc(sizeof(llg_nba_t));
    n->target = target;
    n->value = value;
    n->is_real = 0;
    n->real_target = NULL;
    n->real_value = 0.0;
    n->next = NULL;
    if (p->nba_tail) p->nba_tail->next = n;
    else p->nba_head = n;
    p->nba_tail = n;
}

void llg_ba(sv4_t* target, sv4_t value) {
    // Procedural blocking writes to a forced signal are ignored (LRM 10.6.2).
    if (llg_is_forced(target)) return;
    sig_write(target, value);
}

void llg_nba_d(double* target, double value) {
    llg_proc_t* p = llg_current();
    llg_nba_t* n = (llg_nba_t*)malloc(sizeof(llg_nba_t));
    n->target = NULL;
    n->value = sv4_x(0, 0);
    n->is_real = 1;
    n->real_target = target;
    n->real_value = value;
    n->next = NULL;
    if (p->nba_tail) p->nba_tail->next = n;
    else p->nba_head = n;
    p->nba_tail = n;
}

void llg_ba_d(double* target, double value) {
    *target = value;
}

// ── Collapsed inout nets ──────────────────────────────────────────────────────

// The resolved value is the per-bit wire/tri combination (equal strengths,
// LRM Table 6-2): all drivers Z -> Z; exactly one non-Z value -> that value;
// equal non-Z values -> that value; any X or mixed 0/1 -> X.  Z driver bits
// contribute nothing to any0/any1/anyx, so an all-Z bit (or a bit with no
// driver at all) resolves to Z.
static sv4_t llg_net_compute(const llg_net_t* net) {
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = net->width;
    r.is_signed = net->is_signed;
    int nl = sv4_nlimbs(net->width);
    for (int i = 0; i < nl; i++) {
        uint64_t m = sv4_limb_mask(net->width, i);
        uint64_t any0 = 0, any1 = 0, anyx = 0;
        for (int d = 0; d < net->n_drivers; d++) {
            const sv4_t* v = net->drivers[d];
            if (!v) continue;
            uint64_t bits = v->bits[i] & m;
            uint64_t x = v->x[i] & m;
            uint64_t z = v->z[i] & m;
            uint64_t known = bits & ~(x | z);
            any0 |= (~bits) & ~(x | z) & m;
            any1 |= known;
            anyx |= x;
        }
        uint64_t r_x = anyx | (any0 & any1);
        r.x[i] = r_x & m;
        r.z[i] = (~(any0 | any1 | anyx)) & m;
        r.bits[i] = (any1 & ~r_x) & m;
    }
    return r;
}

void llg_net_resolve(llg_net_t* net) {
    net->resolved = llg_net_compute(net);
}

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    if (idx < 0 || idx >= net->n_drivers) return;
    value = sv4_resize(value, net->width, net->is_signed);
    sv4_t* slot = net->drivers[idx];
    if (!slot) return;
    if (slot->width == value.width && sv4_same(*slot, value)) return;
    *slot = value;
    // `sig_write` stores and wakes waiters only when the resolved value
    // actually changed, so equal drivers never re-fire the net's readers.
    sig_write(&net->resolved, llg_net_compute(net));
}

static void commit_nbas(void) {
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (!p) continue; // slot freed by fork/join teardown or disable_fork
        while (p->nba_head) {
            llg_nba_t* n = p->nba_head;
            p->nba_head = n->next;
            if (!p->nba_head) p->nba_tail = NULL;
            if (n->is_real) {
                *n->real_target = n->real_value;
            } else if (!llg_is_forced(n->target)) {
                sig_write(n->target, n->value);
            }
            free(n);
        }
    }
}

// ── $monitor / $strobe ────────────────────────────────────────────────────────

// Format `fmt` with `n` sv4_t arguments from `args`: %d/%h/%b/%o/%t consume
// arguments in order, %% prints '%', and an unknown or missing specifier
// prints verbatim without consuming an argument (mirrors `llg_display`).
static void llg_format_array(char* out, size_t cap, const char* fmt,
                              const sv4_t* args, int n) {
    size_t len = 0;
    const char* p = fmt;
    int argi = 0;
    while (*p && len + 1 < cap) {
        char c = *p++;
        if (c == '%') {
            // Skip flags and width/precision digits.
            while (*p == '-' || *p == '+' || *p == ' ' || *p == '#' ||
                   *p == '0' || *p == '.' || (*p >= '0' && *p <= '9')) {
                p++;
            }
            c = *p++;
            if (c == '%') {
                llg_append(out, cap, &len, '%');
            } else if ((c == 'd' || c == 'h' || c == 'b' || c == 'o' || c == 't') &&
                       argi < n) {
                // %t prints its argument's value in ticks, like $display
                // (sv4_format has no 't' case, so format as decimal).
                char tmp[1100]; // wide enough for a full 1024-bit %b
                sv4_format(c == 't' ? 'd' : c, args[argi++], tmp, sizeof(tmp));
                for (char* q = tmp; *q && len + 1 < cap; q++) out[len++] = *q;
            } else {
                out[len++] = '%';
                if (c && len + 1 < cap) out[len++] = c;
            }
        } else {
            out[len++] = c;
        }
    }
    out[len] = 0;
}

// Print one formatted line to stdout (shared by display/monitor/strobe).
static void llg_print_line(const char* line) {
    fputs(line, stdout);
    fputc('\n', stdout);
    fflush(stdout);
}

void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval) {
    if (g.mon.active) {
        free(g.mon.fmt);
        free(g.mon.last);
        free(g.mon.work);
    }
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.n = n;
    g.mon.eval = eval;
    g.mon.fmt = (char*)malloc(strlen(fmt) + 1);
    strcpy(g.mon.fmt, fmt);
    int alloc = n > 0 ? n : 1;
    g.mon.last = (sv4_t*)calloc((size_t)alloc, sizeof(sv4_t));
    g.mon.work = (sv4_t*)calloc((size_t)alloc, sizeof(sv4_t));
    // Initial print: the current values at registration time.
    eval(g.mon.work);
    for (int i = 0; i < n; i++) g.mon.last[i] = g.mon.work[i];
    char out[4096];
    llg_format_array(out, sizeof(out), fmt, g.mon.work, n);
    llg_print_line(out);
}

void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval) {
    llg_strobe_t* e = (llg_strobe_t*)malloc(sizeof(llg_strobe_t));
    e->fmt = (char*)malloc(strlen(fmt) + 1);
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = eval;
    int alloc = n > 0 ? n : 1;
    e->work = (sv4_t*)calloc((size_t)alloc, sizeof(sv4_t));
    e->next = g.strobes;
    g.strobes = e;
}

// Re-print the monitor line when any argument differs from the last printed
// snapshot.  Called after every NBA commit (and on $monitoron resume).
static void check_monitor(void) {
    if (!g.mon.active || !g.mon.enabled) return;
    g.mon.eval(g.mon.work);
    int changed = 0;
    for (int i = 0; i < g.mon.n; i++)
        if (!sv4_same(g.mon.work[i], g.mon.last[i])) {
            changed = 1;
            break;
        }
    if (!changed) return;
    for (int i = 0; i < g.mon.n; i++) g.mon.last[i] = g.mon.work[i];
    char out[4096];
    llg_format_array(out, sizeof(out), g.mon.fmt, g.mon.work, g.mon.n);
    llg_print_line(out);
}

// Print every queued $strobe line with the values committed by the NBA region
// of the current time step, then clear the queue.
static void flush_strobes(void) {
    while (g.strobes) {
        llg_strobe_t* e = g.strobes;
        g.strobes = e->next;
        e->eval(e->work);
        char out[4096];
        llg_format_array(out, sizeof(out), e->fmt, e->work, e->n);
        llg_print_line(out);
        free(e->fmt);
        free(e->work);
        free(e);
    }
}

void llg_monitor_set(int on) {
    if (!g.mon.active) return;
    if (on) {
        if (!g.mon.enabled) {
            g.mon.enabled = 1;
            // Resume: print now if the values changed while suspended.
            check_monitor();
        }
    } else {
        g.mon.enabled = 0;
    }
}

static void report_zero_delay_loop(void) {
    fprintf(stderr, "llg: zero-delay loop detected at time %llu\n",
            (unsigned long long)g.now);
}

// ── final blocks (see llg_rt.h) ─────────────────────────────────────────────

void llg_spawn_final(void (*fn)(llg_proc_t*), const char* name) {
    if (llg_n_finals >= LLG_MAX_FINALS) {
        fprintf(stderr, "llg: too many final blocks (limit %d)\n", LLG_MAX_FINALS);
        abort();
    }
    llg_finals[llg_n_finals].fn = fn;
    llg_finals[llg_n_finals].name = name;
    llg_n_finals++;
}

void llg_rt_run_finals(void) {
    if (llg_n_finals == 0) return;
    // Explicit reset, decoupled from the cleanup-memset invariant: a stale
    // $finish flag left by the scheduler exit must never read as
    // "$finish inside a final" after the first final completes.
    g.finish = 0;
    // Rebuild a minimal coroutine context: the scheduler-exit teardown in
    // llg_rt_run released the previous one.
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(4u << 20);
    g.now = llg_final_time;
    uint64_t guard = 0;
    llg_in_finals = 1;
    for (int i = 0; i < llg_n_finals; i++) {
        llg_proc_t* p = (llg_proc_t*)calloc(1, sizeof(llg_proc_t));
        p->name = llg_finals[i].name;
        p->fn = llg_finals[i].fn;
        p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
        register_proc(p);
        if (++guard > LLG_ZERO_LOOP_LIMIT) {
            report_zero_delay_loop();
            break;
        }
        aco_resume(p->co);
        // A final never suspends on timing controls and fork/join is
        // rejected by codegen, so nothing else can be left on the ready
        // queue; this drain is dead-defensive only (if a future lowering
        // ever lets a final spawn children, joined descendants land here
        // and would run to completion before the next final starts).
        while (g.ready_head) {
            if (++guard > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                break;
            }
            llg_proc_t* q = g.ready_head;
            g.ready_head = q->next_ready;
            if (!g.ready_head) g.ready_tail = NULL;
            q->next_ready = NULL;
            aco_resume(q->co);
        }
        if (p->wait.kind != W_NONE) {
            fprintf(stderr,
                    "llg: fatal: final block `%s` suspended on a wait "
                    "(timing controls are rejected by codegen)\n",
                    p->name ? p->name : "final");
            abort();
        }
        // Finals permit function statements only. Codegen rejects NBAs,
        // deferred output tasks, waits, and forks, so no scheduler region is
        // run between these sequential zero-time calls.
        if (g.finish) break;
    }
    llg_in_finals = 0;
    llg_rt_cleanup();
    llg_n_finals = 0;
}

void llg_rt_run(void) {
    int zero_loop = 0;
    for (;;) {
        // Zero-delay guard: one counter per time step, tripped when the
        // design never reaches quiescence at `now` (e.g. `always #0;` or an
        // NBA-oscillation loop).  Reset whenever time advances below.
        if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
            report_zero_delay_loop();
            break;
        }
        // Active region: run every ready coroutine once.  Each resume counts
        // toward the zero-delay guard so a trigger cascade that never
        // quiesces (e.g. two processes ping-ponging named-event triggers
        // with no suspension point) trips the guard instead of spinning
        // forever inside one region pass.
        while (g.ready_head) {
            if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                zero_loop = 1;
                break;
            }
            llg_proc_t* p = g.ready_head;
            g.ready_head = p->next_ready;
            if (!g.ready_head) g.ready_tail = NULL;
            p->next_ready = NULL;
            aco_resume(p->co);
            // p either ended (aco_exit) or suspended in a fresh wait.
        }
        if (zero_loop) break;
        // Inactive region (#0): runs between the active region and the NBA
        // region.  Drain it in a loop so a `#0` executed from an inactive
        // continuation schedules a re-inactive pass; the woken continuations
        // (and anything they schedule) run in a fresh active pass.
        while (g.inactive_head) {
            if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                zero_loop = 1;
                break;
            }
            // Wake every #0 waiter at the current time (FIFO).
            llg_wait_t* w = g.inactive_head;
            g.inactive_head = NULL;
            g.inactive_tail = NULL;
            while (w) {
                llg_wait_t* next = w->inactive_next;
                wake_proc(w->proc);
                w = next;
            }
            while (g.ready_head) {
                if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                    report_zero_delay_loop();
                    zero_loop = 1;
                    break;
                }
                llg_proc_t* p = g.ready_head;
                g.ready_head = p->next_ready;
                if (!g.ready_head) g.ready_tail = NULL;
                p->next_ready = NULL;
                aco_resume(p->co);
            }
            if (zero_loop) break;
        }
        if (zero_loop) break;
        // NBA region: commit recorded non-blocking assignments.
        commit_nbas();
        // Free fork groups whose children all finished (or were killed);
        // done children's NBAs were just committed, killed ones were
        // discarded by disable_fork, so freeing is safe here.
        process_zombie_groups();
        // $strobe lines print with the values committed above; the monitor
        // re-prints when any of its arguments changed.
        flush_strobes();
        check_monitor();
        if (g.ready_head) continue; // new events this time step
        if (g.finish) break;
        if (g.timed_head) {
            uint64_t t = g.timed_head->time;
            if (t == g.now) {
                // #0 waiters now live on the inactive list, so a timed
                // wakeup at `now` cannot come from them; keep the guard for
                // safety against a corrupted time list.
                if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                    report_zero_delay_loop();
                    break;
                }
            } else {
                g.now = t;
                g.region_passes = 0;
            }
            llg_wait_t* w = g.timed_head;
            while (w && w->time == t) {
                llg_wait_t* next = w->time_next;
                wake_proc(w->proc);
                w = next;
            }
            continue;
        }
        if (g.wait_count == 0) {
            fprintf(stderr, "llg: simulation ended without $finish "
                            "(no processes remain) at time %llu\n",
                    (unsigned long long)g.now);
            break;
        }
        fprintf(stderr, "llg: simulation deadlock at time %llu "
                        "(waiters never woken, no future events)\n",
                (unsigned long long)g.now);
        break;
    }
    // Finals ($time inside them) report when the scheduler loop ended.
    llg_final_time = g.now;
    llg_rt_cleanup();
}

// ── $display / $write ─────────────────────────────────────────────────────────

static void llg_vprint(const char* fmt, va_list ap, int newline) {
    char out[4096];
    size_t len = 0;
    const char* p = fmt;
    while (*p && len + 1 < sizeof(out)) {
        char c = *p++;
        if (c == '%') {
            const char* spec_start = p - 1;
            // Skip flags and width/precision digits.
            while (*p == '-' || *p == '+' || *p == ' ' || *p == '#' ||
                   *p == '0' || *p == '.' || (*p >= '0' && *p <= '9')) {
                p++;
            }
            c = *p++;
            if (c == '%') {
                out[len++] = '%';
            } else if (c == 't') {
                // %t prints the value of its argument (typically $time) in
                // ticks, matching the generated code which passes the arg.
                sv4_t v = va_arg(ap, sv4_t);
                char tmp[32];
                sv4_format('d', v, tmp, sizeof(tmp));
                for (char* q = tmp; *q && len + 1 < sizeof(out); q++) out[len++] = *q;
            } else if (c == 's') {
                const char* s = va_arg(ap, const char*);
                if (s) {
                    for (; *s && len + 1 < sizeof(out); s++) out[len++] = *s;
                }
            } else if (c == 'd' || c == 'h' || c == 'b' || c == 'o') {
                sv4_t v = va_arg(ap, sv4_t);
                // Wide enough for a full 1024-bit %b plus NUL.
                char tmp[1100];
                sv4_format(c, v, tmp, sizeof(tmp));
                for (char* q = tmp; *q && len + 1 < sizeof(out); q++) out[len++] = *q;
            } else if (c == 'f' || c == 'e' || c == 'g') {
                double v = va_arg(ap, double);
                char real_fmt[128];
                size_t spec_len = (size_t)(p - spec_start);
                if (spec_len >= sizeof(real_fmt)) spec_len = sizeof(real_fmt) - 1;
                memcpy(real_fmt, spec_start, spec_len);
                real_fmt[spec_len] = 0;
                char tmp[128];
                snprintf(tmp, sizeof(tmp), real_fmt, v);
                for (char* q = tmp; *q && len + 1 < sizeof(out); q++) out[len++] = *q;
            } else {
                // Unknown specifier: print it verbatim.
                out[len++] = '%';
                if (c && len + 1 < sizeof(out)) out[len++] = c;
            }
        } else {
            out[len++] = c;
        }
    }
    out[len] = 0;
    fputs(out, stdout);
    if (newline) fputc('\n', stdout);
    fflush(stdout);
}

void llg_display(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 1);
    va_end(ap);
}

void llg_write(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 0);
    va_end(ap);
}
