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
static int sv4_nlimbs(uint32_t w) { return w == 0 ? 0 : (int)((w + 63u) / 64u); }

static void sv4_require_width(uint64_t width, const char* operation) {
    if (width <= LLG_MAX_WIDTH) return;
    fprintf(stderr,
            "llg runtime fatal: %s width %llu exceeds model capacity %u\n",
            operation, (unsigned long long)width, (unsigned)LLG_MAX_WIDTH);
    abort();
}

// Mask for limb `i` of a `w`-bit vector: full for interior limbs, partial for
// the top limb, zero beyond the width.
static uint64_t sv4_limb_mask(uint32_t w, int i) {
    int nl = sv4_nlimbs(w);
    if (i < 0 || i >= nl) return 0;
    if (i == nl - 1 && (w % 64) != 0) return LLG_MASK(w % 64);
    return ~0ULL;
}

sv4_t sv4_x(uint32_t width, int8_t is_signed) {
    sv4_require_width(width, "value");
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

sv4_t sv4_from_u64(uint64_t v, uint32_t width, int8_t is_signed) {
    sv4_require_width(width, "value");
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.bits[0] = v & LLG_MASK(width);
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

sv4_t sv4_from_i64(int64_t v, uint32_t width) {
    uint32_t source_width = width < 64 ? width : 64;
    sv4_t result = sv4_from_u64((uint64_t)v, source_width, 1);
    return sv4_resize(result, width, 1);
}

double sv4_to_real(sv4_t v) {
    int limbs = sv4_nlimbs(v.width);
    for (int i = 0; i < limbs; i++) {
        v.bits[i] &= ~(v.x[i] | v.z[i]) & sv4_limb_mask(v.width, i);
        v.x[i] = 0;
        v.z[i] = 0;
    }
    int negative = v.is_signed && v.width > 0 &&
        ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL);
    if (negative) v = sv4_neg(v);
    double out = 0.0;
    for (int i = limbs - 1; i >= 0; i--)
        out = ldexp(out, 64) + (double)v.bits[i];
    return negative ? -out : out;
}

sv4_t sv4_from_real(double v, uint32_t width, int8_t is_signed) {
    if (!isfinite(v)) return sv4_x(width, is_signed);
    sv4_require_width(width, "real conversion");
    double rounded = round(v);
    const double modulus = 18446744073709551616.0;
    double magnitude = fabs(rounded);
    sv4_t result;
    memset(&result, 0, sizeof(result));
    result.width = width;
    result.is_signed = is_signed;
    for (int i = 0; i < sv4_nlimbs(width) && magnitude != 0.0; i++) {
        result.bits[i] = (uint64_t)fmod(magnitude, modulus);
        magnitude = floor(ldexp(magnitude, -64));
    }
    if (sv4_nlimbs(width) > 0)
        result.bits[sv4_nlimbs(width) - 1] &=
            sv4_limb_mask(width, sv4_nlimbs(width) - 1);
    if (signbit(rounded)) result = sv4_neg(result);
    return result;
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
                     uint32_t width, int8_t is_signed) {
    sv4_require_width(width, "value");
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
    for (int i = 0; i < sv4_nlimbs(v.width); i++)
        if (v.x[i] | v.z[i]) return 1;
    return 0;
}

sv4_t sv4_countones(sv4_t v) {
    uint64_t count = 0;
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
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
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        uint64_t known_ones = v.bits[i] & ~(v.x[i] | v.z[i]) &
                              sv4_limb_mask(v.width, i);
        if (known_ones) return 1;
    }
    return 0;
}

uint64_t sv4_to_u64(sv4_t v) { return v.bits[0] & LLG_MASK(v.width); }

uint64_t sv4_to_index(sv4_t v) {
    if (sv4_is_unknown(v)) return UINT64_MAX;
    if (v.is_signed && v.width > 0 &&
        ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL)) {
        return UINT64_MAX;
    }
    for (int i = 1; i < sv4_nlimbs(v.width); i++) {
        if (v.bits[i]) return UINT64_MAX;
    }
    return sv4_to_u64(v);
}

int64_t sv4_to_i64(sv4_t v) {
    uint64_t b = sv4_to_u64(v);
    if (v.width == 0) return 0;
    if (v.width >= 64) return (int64_t)b;
    uint64_t sign = 1ULL << (v.width - 1);
    if (b & sign) return (int64_t)(b | ~(sign - 1));
    return (int64_t)b;
}

int sv4_to_index_i64(sv4_t v, int64_t* result) {
    if (!result || !sv4_fits_i64(v)) return 0;
    *result = v.is_signed ? sv4_to_i64(v) : (int64_t)sv4_to_u64(v);
    return 1;
}

uint32_t sv4_checked_width(sv4_t v) {
    if (sv4_is_unknown(v) ||
        (v.is_signed && v.width > 0 &&
         ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL))) {
        fprintf(stderr, "llg runtime fatal: invalid dynamic packed width\n");
        abort();
    }
    for (int i = 1; i < sv4_nlimbs(v.width); i++) {
        if (v.bits[i]) sv4_require_width(UINT64_MAX, "dynamic packed");
    }
    uint64_t width = sv4_to_u64(v);
    sv4_require_width(width, "dynamic packed");
    return (uint32_t)width;
}

int sv4_same(sv4_t a, sv4_t b) {
    for (int i = 0; i < (int)LLG_LIMBS; i++)
        if (a.bits[i] != b.bits[i] || a.x[i] != b.x[i] || a.z[i] != b.z[i])
            return 0;
    return 1;
}

sv4_t sv4_resolve(const sv4_t* const* drivers, int n_drivers,
                  uint32_t width, int8_t is_signed, int mode) {
    sv4_require_width(width, "net");
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = width;
    r.is_signed = is_signed;
    int nl = sv4_nlimbs(width);
    for (int i = 0; i < nl; i++) {
        uint64_t m = sv4_limb_mask(width, i);
        if (mode == LLG_RESOLVE_SUPPLY0 || mode == LLG_RESOLVE_SUPPLY1) {
            r.bits[i] = mode == LLG_RESOLVE_SUPPLY1 ? m : 0;
            continue;
        }
        uint64_t any0 = 0, any1 = 0, anyx = 0;
        for (int d = 0; d < n_drivers; d++) {
            const sv4_t* v = drivers[d];
            if (!v) continue;
            uint64_t bits = v->bits[i] & m;
            uint64_t x = v->x[i] & m;
            uint64_t z = v->z[i] & m;
            any0 |= (~bits) & ~(x | z) & m;
            any1 |= bits & ~(x | z) & m;
            anyx |= x;
        }
        uint64_t driven = any0 | any1 | anyx;
        uint64_t known1;
        uint64_t unknown;
        if (mode == LLG_RESOLVE_WAND) {
            known1 = any1 & ~anyx & ~any0;
            unknown = anyx & ~any0;
        } else if (mode == LLG_RESOLVE_WOR) {
            known1 = any1;
            unknown = anyx & ~any1;
        } else {
            unknown = anyx | (any0 & any1);
            known1 = any1 & ~unknown;
        }
        r.bits[i] = known1 & m;
        r.x[i] = unknown & m;
        r.z[i] = ~driven & m;
        if (mode == LLG_RESOLVE_TRI0 || mode == LLG_RESOLVE_TRI1) {
            if (mode == LLG_RESOLVE_TRI1) r.bits[i] |= r.z[i];
            r.z[i] = 0;
        }
    }
    return r;
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
static sv4_t sv4_resize_ext(sv4_t v, uint32_t width, int8_t is_signed, int8_t ext_signed) {
    sv4_require_width(width, "resize");
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

sv4_t sv4_resize(sv4_t v, uint32_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, is_signed);
}

// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`) — an unsigned source
// zero-extends even into a signed target and vice versa — narrowing
// truncates; the result carries `is_signed`.
sv4_t sv4_cast(sv4_t v, uint32_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, v.is_signed);
}

sv4_t sv4_to_two_state(sv4_t v) {
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        v.bits[i] &= ~(v.x[i] | v.z[i]);
        v.x[i] = 0;
        v.z[i] = 0;
    }
    return v;
}

sv4_t sv4_fill(uint8_t bit, uint32_t width, int8_t is_signed) {
    sv4_require_width(width, "fill");
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
    for (int i = sv4_nlimbs(v.width) - 1; i >= 0; i--)
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

static uint32_t sv4_maxw(sv4_t a, sv4_t b) {
    return a.width > b.width ? a.width : b.width;
}

static void* sv4_scratch_alloc(size_t count);

sv4_t sv4_add(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
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
    uint32_t w = sv4_maxw(a, b);
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
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    a = sv4_resize(a, w, s);
    b = sv4_resize(b, w, s);
    int nl = sv4_nlimbs(w);
    // Schoolbook product modulo 2^w.  Terms at limb nl and above cannot affect
    // the truncated result, so avoid allocating or computing them.
    uint64_t* acc = sv4_scratch_alloc((size_t)nl);
    for (int i = 0; i < nl; i++) {
        for (int j = 0; j < nl - i; j++) {
            __uint128_t prod = (__uint128_t)a.bits[i] * b.bits[j];
            uint64_t plo = (uint64_t)prod;
            uint64_t phi = (uint64_t)(prod >> 64);
            // add plo to acc[i+j], propagating the carry upward
            uint64_t t = acc[i + j] + plo;
            uint64_t carry = t < acc[i + j] ? 1 : 0;
            acc[i + j] = t;
            for (int k = i + j + 1; carry && k < nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
            // add phi to acc[i+j+1], propagating the carry upward
            if (i + j + 1 >= nl) continue;
            t = acc[i + j + 1] + phi;
            carry = t < acc[i + j + 1] ? 1 : 0;
            acc[i + j + 1] = t;
            for (int k = i + j + 2; carry && k < nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
        }
    }
    sv4_t r;
    memset(&r, 0, sizeof(r));
    for (int i = 0; i < nl; i++) r.bits[i] = acc[i];
    free(acc);
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

static int sv4_raw_nlimbs(const sv4_t* v) {
    int limbs = sv4_nlimbs(v->width);
    while (limbs > 0 && v->bits[limbs - 1] == 0) limbs--;
    return limbs;
}

static int sv4_raw_ucmp(const sv4_t* a, const sv4_t* b) {
    int a_limbs = sv4_raw_nlimbs(a);
    int b_limbs = sv4_raw_nlimbs(b);
    if (a_limbs != b_limbs) return a_limbs < b_limbs ? -1 : 1;
    for (int i = a_limbs - 1; i >= 0; i--) {
        if (a->bits[i] != b->bits[i])
            return a->bits[i] < b->bits[i] ? -1 : 1;
    }
    return 0;
}

static void* sv4_scratch_alloc(size_t count) {
    if (count == 0) count = 1;
    void* allocation = calloc(count, sizeof(uint64_t));
    if (allocation) return allocation;
    fprintf(stderr, "llg runtime fatal: value-operation allocation failed\n");
    abort();
}

// Divide two known, unsigned, equal-width vectors using normalized base-2^64
// long division.  Knuth's quotient estimate keeps the work proportional to
// the populated limbs rather than to every individual bit.
static void sv4_unsigned_divmod(const sv4_t* dividend, const sv4_t* divisor,
                                int want_remainder, sv4_t* result) {
    int dividend_limbs = sv4_raw_nlimbs(dividend);
    int divisor_limbs = sv4_raw_nlimbs(divisor);
    if (sv4_raw_ucmp(dividend, divisor) < 0) {
        if (want_remainder) *result = *dividend;
        return;
    }

    if (divisor_limbs == 1) {
        uint64_t remainder = 0;
        uint64_t divisor_word = divisor->bits[0];
        for (int i = dividend_limbs - 1; i >= 0; i--) {
            __uint128_t partial = ((__uint128_t)remainder << 64) |
                                  dividend->bits[i];
            uint64_t quotient_word = (uint64_t)(partial / divisor_word);
            remainder = (uint64_t)(partial % divisor_word);
            if (!want_remainder) result->bits[i] = quotient_word;
        }
        if (want_remainder) result->bits[0] = remainder;
        return;
    }

    size_t normalized_divisor_count = (size_t)divisor_limbs;
    size_t normalized_dividend_count = (size_t)dividend_limbs + 1;
    size_t quotient_count = (size_t)(dividend_limbs - divisor_limbs + 1);
    size_t total = normalized_divisor_count + normalized_dividend_count;
    if (!want_remainder) total += quotient_count;
    uint64_t* scratch = sv4_scratch_alloc(total);
    uint64_t* normalized_divisor = scratch;
    uint64_t* normalized_dividend = normalized_divisor + normalized_divisor_count;
    uint64_t* quotient = want_remainder
        ? NULL
        : normalized_dividend + normalized_dividend_count;

    unsigned shift = (unsigned)__builtin_clzll(divisor->bits[divisor_limbs - 1]);
    if (shift == 0) {
        memcpy(normalized_divisor, divisor->bits,
               normalized_divisor_count * sizeof(uint64_t));
        memcpy(normalized_dividend, dividend->bits,
               (size_t)dividend_limbs * sizeof(uint64_t));
    } else {
        for (int i = 0; i < divisor_limbs; i++) {
            normalized_divisor[i] = divisor->bits[i] << shift;
            if (i > 0) normalized_divisor[i] |= divisor->bits[i - 1] >> (64 - shift);
        }
        for (int i = 0; i < dividend_limbs; i++) {
            normalized_dividend[i] = dividend->bits[i] << shift;
            if (i > 0)
                normalized_dividend[i] |= dividend->bits[i - 1] >> (64 - shift);
        }
        normalized_dividend[dividend_limbs] =
            dividend->bits[dividend_limbs - 1] >> (64 - shift);
    }

    for (int j = dividend_limbs - divisor_limbs; j >= 0; j--) {
        uint64_t quotient_digit;
        uint64_t estimate_remainder;
        uint64_t high = normalized_dividend[j + divisor_limbs];
        uint64_t next = normalized_dividend[j + divisor_limbs - 1];
        uint64_t divisor_high = normalized_divisor[divisor_limbs - 1];
        int estimate_remainder_overflow = 0;
        if (high == divisor_high) {
            quotient_digit = UINT64_MAX;
            estimate_remainder = next + divisor_high;
            estimate_remainder_overflow = estimate_remainder < next;
        } else {
            __uint128_t numerator = ((__uint128_t)high << 64) | next;
            quotient_digit = (uint64_t)(numerator / divisor_high);
            estimate_remainder = (uint64_t)(numerator % divisor_high);
        }
        while (!estimate_remainder_overflow &&
               (__uint128_t)quotient_digit *
                   normalized_divisor[divisor_limbs - 2] >
               (((__uint128_t)estimate_remainder << 64) |
                normalized_dividend[j + divisor_limbs - 2])) {
            quotient_digit--;
            uint64_t previous = estimate_remainder;
            estimate_remainder += divisor_high;
            estimate_remainder_overflow = estimate_remainder < previous;
        }

        uint64_t borrow = 0;
        for (int i = 0; i < divisor_limbs; i++) {
            __uint128_t product = (__uint128_t)quotient_digit *
                                      normalized_divisor[i] +
                                  borrow;
            uint64_t subtrahend = (uint64_t)product;
            uint64_t previous = normalized_dividend[j + i];
            normalized_dividend[j + i] = previous - subtrahend;
            borrow = (uint64_t)(product >> 64) + (previous < subtrahend);
        }
        uint64_t previous_high = normalized_dividend[j + divisor_limbs];
        normalized_dividend[j + divisor_limbs] = previous_high - borrow;
        if (previous_high < borrow) {
            quotient_digit--;
            uint64_t carry = 0;
            for (int i = 0; i < divisor_limbs; i++) {
                __uint128_t sum = (__uint128_t)normalized_dividend[j + i] +
                                  normalized_divisor[i] + carry;
                normalized_dividend[j + i] = (uint64_t)sum;
                carry = (uint64_t)(sum >> 64);
            }
            normalized_dividend[j + divisor_limbs] += carry;
        }
        if (quotient) quotient[j] = quotient_digit;
    }

    if (want_remainder) {
        if (shift == 0) {
            memcpy(result->bits, normalized_dividend,
                   normalized_divisor_count * sizeof(uint64_t));
        } else {
            for (int i = 0; i < divisor_limbs; i++) {
                result->bits[i] = normalized_dividend[i] >> shift;
                result->bits[i] |= normalized_dividend[i + 1] << (64 - shift);
            }
        }
    } else {
        memcpy(result->bits, quotient, quotient_count * sizeof(uint64_t));
    }
    free(scratch);
}

static int sv4_is_negative(sv4_t v) {
    return v.is_signed && v.width > 0 &&
           sv4_lsb_bit(v, (int)v.width - 1) == 1;
}

static sv4_t sv4_divmod(sv4_t a, sv4_t b, int want_remainder) {
    uint32_t width = sv4_maxw(a, b);
    int8_t is_signed = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b))
        return sv4_x(width, is_signed);
    a = sv4_resize(a, width, is_signed);
    b = sv4_resize(b, width, is_signed);
    if (sv4_raw_nlimbs(&b) == 0) return sv4_x(width, is_signed);

    int dividend_negative = is_signed && sv4_is_negative(a);
    int divisor_negative = is_signed && sv4_is_negative(b);
    if (dividend_negative) a = sv4_neg(a);
    if (divisor_negative) b = sv4_neg(b);

    sv4_t result;
    memset(&result, 0, sizeof(result));
    result.width = width;
    result.is_signed = is_signed;
    sv4_unsigned_divmod(&a, &b, want_remainder, &result);
    if ((want_remainder && dividend_negative) ||
        (!want_remainder && dividend_negative != divisor_negative)) {
        result = sv4_neg(result);
    }
    return result;
}

sv4_t sv4_div(sv4_t a, sv4_t b) { return sv4_divmod(a, b, 0); }

sv4_t sv4_mod(sv4_t a, sv4_t b) { return sv4_divmod(a, b, 1); }

static int sv4_is_all_ones(sv4_t v) {
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        if ((v.bits[i] & sv4_limb_mask(v.width, i)) !=
            sv4_limb_mask(v.width, i)) {
            return 0;
        }
    }
    return v.width != 0;
}

sv4_t sv4_pow(sv4_t a, sv4_t b) {
    uint32_t width = a.width;
    int8_t is_signed = a.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b))
        return sv4_x(width, is_signed);
    if (sv4_is_negative(b)) {
        if (sv4_raw_nlimbs(&a) == 0) return sv4_x(width, is_signed);
        if (sv4_is_negative(a) && sv4_is_all_ones(a)) {
            return (b.bits[0] & 1ULL)
                ? a
                : sv4_from_u64(1, width, is_signed);
        }
        if (sv4_raw_nlimbs(&a) == 1 && a.bits[0] == 1)
            return sv4_from_u64(1, width, is_signed);
        return sv4_from_u64(0, width, is_signed);
    }

    sv4_t result = sv4_from_u64(1, width, is_signed);
    sv4_t base = a;
    int exponent_msb = sv4_msb(b);
    for (int bit = 0; bit <= exponent_msb; bit++) {
        if (sv4_lsb_bit(b, bit) == 1) result = sv4_mul(result, base);
        if (bit != exponent_msb) base = sv4_mul(base, base);
    }
    return result;
}

sv4_t sv4_neg(sv4_t a) {
    uint32_t w = a.width;
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
    uint32_t w = sv4_maxw(a, b);
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
    uint32_t w = a.width;
    if (sv4_is_unknown(b)) return sv4_x(w, a.is_signed);
    int oversized = 0;
    if (b.width > 64) {
        for (int i = 1; i < sv4_nlimbs(b.width); i++)
            if (b.bits[i]) {
                oversized = 1;
                break;
            }
    }
    uint64_t sh = sv4_to_u64(b);
    if (oversized || sh >= w) {
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
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    int unknown = 0;
    for (int i = 0; i < sv4_nlimbs(w); i++) {
        uint64_t mask = sv4_limb_mask(w, i);
        uint64_t either_unknown = (ra.x[i] | ra.z[i] | rb.x[i] | rb.z[i]) & mask;
        uint64_t both_known = mask & ~either_unknown;
        if (((ra.bits[i] ^ rb.bits[i]) & both_known) != 0) return SV4_C(0, 1);
        unknown |= either_unknown != 0;
    }
    return unknown ? SV4_X(1) : SV4_C(1, 1);
}

sv4_t sv4_neq(sv4_t a, sv4_t b) {
    sv4_t result = sv4_eq(a, b);
    if (sv4_is_unknown(result)) return result;
    return SV4_C(1 - sv4_to_u64(result), 1);
}

sv4_t sv4_case_eq(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
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
    uint32_t w = sv4_maxw(lhs, rhs);
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
    uint32_t w = sv4_maxw(sel, item);
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
    uint32_t w = sv4_maxw(sel, item);
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
    uint32_t w = sv4_maxw(a, b);
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
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t ra = sv4_resize(a, w, s);
    sv4_t rb = sv4_resize(b, w, s);
    int truth = sv4_logical_truth(sel);
    if (truth == 1) return ra;
    if (truth == 0) return rb;
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
    uint64_t total = (uint64_t)hi.width + (uint64_t)lo.width;
    sv4_require_width(total, "concatenation");
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = (uint32_t)total;
    r.is_signed = 0;
    for (int i = 0; i < (int)lo.width; i++)
        sv4_lsb_bit_set(&r, i, sv4_lsb_bit(lo, i));
    for (int i = 0; i < (int)hi.width && (int)lo.width + i < (int)total; i++)
        sv4_lsb_bit_set(&r, (int)lo.width + i, sv4_lsb_bit(hi, i));
    return r;
}

sv4_t sv4_repeat(sv4_t pat, uint64_t n) {
    if (pat.width != 0 && n > UINT64_MAX / pat.width) {
        sv4_require_width(UINT64_MAX, "replication");
    }
    uint64_t w_total64 = (uint64_t)pat.width * n;
    sv4_require_width(w_total64, "replication");
    uint32_t w_total = (uint32_t)w_total64;
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
    if (i >= v.width) return SV4_X(1);
    int b = sv4_lsb_bit(v, (int)i);
    if (b == 2) return SV4_X(1);
    if (b == 3) return SV4_Z(1);
    return sv4_from_u64(b, 1, 0);
}

void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value) {
    if (i >= tgt->width) return;
    sv4_lsb_bit_set(tgt, (int)i, sv4_lsb_bit(value, 0));
}

static uint32_t llg_part_select_width(int64_t left, int64_t right) {
    uint64_t delta = left >= right
        ? (uint64_t)left - (uint64_t)right
        : (uint64_t)right - (uint64_t)left;
    if (delta == UINT64_MAX) sv4_require_width(delta, "part-select");
    sv4_require_width(delta + 1, "part-select");
    return (uint32_t)(delta + 1);
}

sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right) {
    uint32_t w = llg_part_select_width(left, right);
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

sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint32_t width, int neg) {
    sv4_require_width(width, "indexed part-select");
    sv4_t r;
    memset(&r, 0, sizeof(r));
    r.width = width;
    for (uint32_t output_bit = 0; output_bit < width; output_bit++) {
        uint64_t source_bit;
        int in_range;
        if (!neg) {
            source_bit = base + output_bit;
            in_range = source_bit >= base && source_bit < v.width;
        } else {
            uint64_t distance = (uint64_t)width - 1 - output_bit;
            in_range = base >= distance;
            source_bit = in_range ? base - distance : 0;
            in_range = in_range && source_bit < v.width;
        }
        sv4_lsb_bit_set(&r, (int)output_bit,
                        in_range ? sv4_lsb_bit(v, (int)source_bit) : 2);
    }
    return r;
}

void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint32_t width, int neg,
                             sv4_t value) {
    for (uint32_t value_bit = 0; value_bit < width; value_bit++) {
        uint64_t target_bit;
        int in_range;
        if (!neg) {
            target_bit = base + value_bit;
            in_range = target_bit >= base && target_bit < tgt->width;
        } else {
            uint64_t distance = (uint64_t)width - 1 - value_bit;
            in_range = base >= distance;
            target_bit = in_range ? base - distance : 0;
            in_range = in_range && target_bit < tgt->width;
        }
        if (in_range)
            sv4_lsb_bit_set(tgt, (int)target_bit,
                            sv4_lsb_bit(value, (int)value_bit));
    }
}

static int sv4_indexed_source(int64_t base, uint32_t width,
                              uint32_t output_bit, int neg,
                              uint64_t* source_bit) {
    if (!neg) {
        if (base >= 0) {
            *source_bit = (uint64_t)base + output_bit;
            return 1;
        }
        uint64_t magnitude = 0 - (uint64_t)base;
        if ((uint64_t)output_bit < magnitude) return 0;
        *source_bit = (uint64_t)output_bit - magnitude;
        return 1;
    }
    uint64_t distance = (uint64_t)width - 1 - output_bit;
    if (base < 0 || (uint64_t)base < distance) return 0;
    *source_bit = (uint64_t)base - distance;
    return 1;
}

sv4_t sv4_idx_part_select_value(sv4_t v, sv4_t base, uint32_t width, int neg) {
    sv4_require_width(width, "indexed part-select");
    int64_t signed_base;
    if (!sv4_to_index_i64(base, &signed_base)) return sv4_x(width, 0);
    sv4_t result = sv4_x(width, 0);
    for (uint32_t output_bit = 0; output_bit < width; output_bit++) {
        uint64_t source_bit;
        if (sv4_indexed_source(signed_base, width, output_bit, neg, &source_bit) &&
            source_bit < v.width) {
            sv4_lsb_bit_set(&result, (int)output_bit,
                            sv4_lsb_bit(v, (int)source_bit));
        }
    }
    return result;
}

void sv4_idx_part_select_set_value(sv4_t* tgt, sv4_t base, uint32_t width,
                                   int neg, sv4_t value) {
    sv4_require_width(width, "indexed part-select");
    int64_t signed_base;
    if (!sv4_to_index_i64(base, &signed_base)) return;
    for (uint32_t value_bit = 0; value_bit < width; value_bit++) {
        uint64_t target_bit;
        if (sv4_indexed_source(signed_base, width, value_bit, neg, &target_bit) &&
            target_bit < tgt->width) {
            sv4_lsb_bit_set(tgt, (int)target_bit,
                            sv4_lsb_bit(value, (int)value_bit));
        }
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
    if (cap == 0) return;
    if (sv4_is_unknown(v)) {
        if (cap > 1) {
            buf[0] = 'x';
            buf[1] = 0;
        } else {
            buf[0] = 0;
        }
        return;
    }
    int negative = 0;
    int nl = sv4_nlimbs(v.width);
    uint64_t* tmp = sv4_scratch_alloc((size_t)(nl > 0 ? nl : 1));
    if (v.is_signed && v.width > 0 && sv4_lsb_bit(v, (int)v.width - 1) == 1) {
        negative = 1;
        sv4_t mag = sv4_neg(v);
        for (int i = 0; i < nl; i++) tmp[i] = mag.bits[i];
    } else {
        for (int i = 0; i < nl; i++) tmp[i] = v.bits[i];
    }
    if (nl > 0) tmp[nl - 1] &= sv4_limb_mask(v.width, nl - 1);
    size_t digit_capacity = ((size_t)v.width * 30103u) / 100000u + 2u;
    char* digits = malloc(digit_capacity);
    if (!digits) {
        free(tmp);
        fprintf(stderr, "llg runtime fatal: value-format allocation failed\n");
        abort();
    }
    size_t n = 0;
    for (;;) {
        int nonzero = 0;
        for (int i = 0; i < nl; i++)
            if (tmp[i]) { nonzero = 1; break; }
        if (!nonzero) break;
        uint64_t rem = 0;
        for (int i = nl - 1; i >= 0; i--) {
            __uint128_t cur = ((__uint128_t)rem << 64) | tmp[i];
            tmp[i] = (uint64_t)(cur / 10);
            rem = (uint64_t)(cur % 10);
        }
        if (n < digit_capacity) digits[n++] = (char)('0' + (int)rem);
    }
    size_t len = 0;
    if (negative) llg_append(buf, cap, &len, '-');
    if (n == 0) {
        llg_append(buf, cap, &len, '0');
    } else {
        while (n > 0) llg_append(buf, cap, &len, digits[--n]);
    }
    buf[len] = 0;
    free(digits);
    free(tmp);
}

void sv4_format(char fmt, sv4_t v, char* buf, size_t cap) {
    if (cap == 0) return;
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
