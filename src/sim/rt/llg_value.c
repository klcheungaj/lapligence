// llg_value.c — four-state values and numeric conversions for generated C11 models.

#include "llg_value.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(double) == sizeof(uint64_t),
               "$realtobits requires a 64-bit C double");
_Static_assert(sizeof(float) == sizeof(uint32_t),
               "$shortrealtobits requires a 32-bit C float");

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
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
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

sv4_t sv4_rtoi(double v) {
    if (!isfinite(v)) return sv4_x(32, 1);
    const double modulus = 4294967296.0;
    double magnitude = fmod(fabs(trunc(v)), modulus);
    uint64_t bits = (uint64_t)magnitude;
    if (signbit(v)) bits = 0ULL - bits;
    return sv4_from_u64(bits, 32, 1);
}

sv4_t sv4_realtobits(double v) {
    uint64_t bits;
    memcpy(&bits, &v, sizeof(bits));
    return sv4_from_u64(bits, 64, 0);
}

double sv4_bitstoreal(sv4_t v) {
    uint64_t bits = v.bits[0] & ~(v.x[0] | v.z[0]);
    double result;
    memcpy(&result, &bits, sizeof(result));
    return result;
}

sv4_t sv4_shortrealtobits(double v) {
    float value = (float)v;
    uint32_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return sv4_from_u64(bits, 32, 0);
}

double sv4_bitstoshortreal(sv4_t v) {
    uint32_t bits = (uint32_t)(v.bits[0] & ~(v.x[0] | v.z[0]));
    float value;
    memcpy(&value, &bits, sizeof(value));
    return (double)value;
}

int llg_real_to_bool(double v) { return v != 0.0; }

sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                     uint16_t width, int8_t is_signed) {
    if (width > LLG_MAX_WIDTH) width = LLG_MAX_WIDTH;
    sv4_t r;
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
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
    for (int i = 0; i < (int)LLG_LIMBS; i++)
        if (v.x[i] | v.z[i]) return 1;
    return 0;
}

sv4_t sv4_countones(sv4_t v) {
    uint64_t count = 0;
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
        uint64_t ones = v.bits[i] & ~(v.x[i] | v.z[i]) & sv4_limb_mask(v.width, i);
        while (ones) {
            count++;
            ones &= ones - 1;
        }
    }
    return sv4_from_u64(count, 32, 1);
}

sv4_t sv4_onehot(sv4_t v, int allow_zero) {
    uint64_t count = sv4_countones(v).bits[0];
    return sv4_from_u64(allow_zero ? count <= 1 : count == 1, 1, 0);
}

int sv4_to_bool(sv4_t v) {
    if (sv4_is_unknown(v)) return 0;
    for (int i = 0; i < (int)LLG_LIMBS; i++)
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
    for (int i = 0; i < (int)LLG_LIMBS; i++)
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
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
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
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
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
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
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

sv4_t sv4_wild_eq(sv4_t lhs, sv4_t rhs) {
    uint16_t w = sv4_maxw(lhs, rhs);
    int8_t s = lhs.is_signed && rhs.is_signed;
    sv4_t left = sv4_resize(lhs, w, s);
    sv4_t right = sv4_resize(rhs, w, s);
    int unknown = 0;
    for (int i = 0; i < sv4_nlimbs(w); i++) {
        uint64_t mask = sv4_limb_mask(w, i);
        uint64_t wildcard = (right.x[i] | right.z[i]) & mask;
        uint64_t care = mask & ~wildcard;
        uint64_t left_unknown = (left.x[i] | left.z[i]) & care;
        uint64_t known = care & ~left_unknown;
        if (((left.bits[i] ^ right.bits[i]) & known) != 0) return SV4_C(0, 1);
        unknown |= left_unknown != 0;
    }
    return unknown ? SV4_X(1) : SV4_C(1, 1);
}

sv4_t sv4_wild_neq(sv4_t lhs, sv4_t rhs) {
    sv4_t result = sv4_wild_eq(lhs, rhs);
    if (sv4_is_unknown(result)) return result;
    return SV4_C(1 - sv4_to_u64(result), 1);
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

static uint16_t llg_part_select_width(int64_t left, int64_t right) {
    uint64_t delta = left >= right
        ? (uint64_t)left - (uint64_t)right
        : (uint64_t)right - (uint64_t)left;
    if (delta >= LLG_MAX_WIDTH) {
        fprintf(stderr, "llg runtime fatal: invalid part-select width\n");
        abort();
    }
    return (uint16_t)(delta + 1);
}

sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right) {
    uint16_t w = llg_part_select_width(left, right);
    if (left < 0 || right < 0 || left >= (int64_t)v.width || right >= (int64_t)v.width) {
        return sv4_x(w, 0);
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = w;
    int64_t step = left > right ? -1 : 1;
    int out = 0;
    for (int64_t i = left; ; i += step) {
        int b = sv4_lsb_bit(v, (int)i);
        int pos = w - 1 - out; // first index (left) is the MSB
        sv4_lsb_bit_set(&r, pos, b);
        out++;
        if (i == right) break;
    }
    return r;
}

void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value) {
    (void)llg_part_select_width(left, right);
    int64_t step = left > right ? -1 : 1;
    int in = (int)value.width - 1; // value MSB maps to the first target index
    for (int64_t i = left; ; i += step) {
        if (i < 0 || i >= (int64_t)tgt->width) {
            in--;
            if (i == right) break;
            continue;
        }
        sv4_lsb_bit_set(tgt, (int)i, sv4_lsb_bit(value, in));
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
        for (int i = 0; i < (int)LLG_LIMBS; i++) tmp[i] = mag.bits[i];
    } else {
        for (int i = 0; i < (int)LLG_LIMBS; i++) tmp[i] = v.bits[i];
    }
    int nl = sv4_nlimbs(v.width);
    if (nl > 0) tmp[nl - 1] &= sv4_limb_mask(v.width, nl - 1);
    char digits[320]; // 1024 bits -> at most 309 decimal digits
    int n = 0;
    for (;;) {
        int nonzero = 0;
        for (int i = 0; i < (int)LLG_LIMBS; i++)
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
