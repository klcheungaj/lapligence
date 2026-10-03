#include "internal.h"
#include <stdio.h>
#ifndef LLG_SV4_GMP_KERNELS
#define LLG_SV4_GMP_KERNELS 0
#endif
#if LLG_SV4_GMP_KERNELS != 0 && LLG_SV4_GMP_KERNELS != 1
#error LLG_SV4_GMP_KERNELS must be 0 or 1
#endif
#if LLG_SV4_GMP_KERNELS
#include <gmp.h>
_Static_assert(GMP_LIMB_BITS == 64 && GMP_NAIL_BITS == 0, "64-bit nail-free GMP required");
_Static_assert(sizeof(mp_limb_t) == 8, "64-bit GMP limb storage required");
/* Pass native typed arrays, never cast uint64_t buffers to a GMP limb view. */
_Static_assert(_Generic((mp_limb_t*)0, uint64_t*: 1, default: 0),
               "GMP and uint64_t must have compatible limb types");
#else
static void multiply_word(uint64_t a, uint64_t b, uint64_t* lo, uint64_t* hi) {
    uint64_t a0 = (uint32_t)a, a1 = a >> 32, b0 = (uint32_t)b, b1 = b >> 32;
    uint64_t p = a0 * b0, t = a1 * b0 + (p >> 32), u = a0 * b1 + (uint32_t)t;
    *lo = (u << 32) | (uint32_t)p;
    *hi = a1 * b1 + (t >> 32) + (u >> 32);
}
#endif

void llg_gmp_sv4_kernel_mul(uint64_t* out, const uint64_t* a, const uint64_t* b, size_t n) {
#if LLG_SV4_GMP_KERNELS
    if (n >= LLG_SV4_MUL_FULL_THRESHOLD) {
        mpn_mul_n(out, a, b, (mp_size_t)n);
        return;
    }
    (void)mpn_mul_1(out, a, (mp_size_t)n, b[0]);
    for (size_t i = 1; i < n; ++i)
        (void)mpn_addmul_1(out + i, a, (mp_size_t)(n - i), b[i]);
#else
    memset(out, 0, n * 8u);
    for (size_t i = 0; i < n; ++i) {
        uint64_t carry = 0;
        for (size_t j = 0; j < n - i; ++j) {
            uint64_t lo, hi;
            multiply_word(a[j], b[i], &lo, &hi);
            uint64_t sum = out[i + j] + lo;
            hi += sum < lo;
            uint64_t r = sum + carry;
            hi += r < sum;
            out[i + j] = r;
            carry = hi;
        }
    }
#endif
}

#if LLG_SV4_GMP_KERNELS
void llg_gmp_sv4_kernel_div(uint64_t* out, const uint64_t* a, const uint64_t* b, size_t n,
                            int remainder) {
    size_t an = n, bn = n;
    while (an && !a[an - 1])
        --an;
    while (bn && !b[bn - 1])
        --bn;
    if (an < bn) {
        if (remainder && an)
            memcpy(out, a, an * 8u);
        return;
    }
    uint64_t *scratch = llg_gmp_sv4_alloc(an + 1u + bn), *q = scratch, *r = q + an + 1u;
    mpn_tdiv_qr(q, r, 0, a, (mp_size_t)an, b, (mp_size_t)bn);
    memcpy(out, remainder ? r : q, (remainder ? bn : an - bn + 1u) * 8u);
    free(scratch);
}
#else
static uint32_t word32(const uint64_t* v, int index) {
    return (uint32_t)(v[index / 2] >> ((index % 2) * 32));
}
static void set_word32(uint64_t* v, int index, uint32_t word) {
    unsigned shift = (unsigned)(index % 2) * 32u;
    uint64_t mask = (uint64_t)UINT32_MAX << shift;
    v[index / 2] = (v[index / 2] & ~mask) | ((uint64_t)word << shift);
}
void llg_gmp_sv4_kernel_div(uint64_t* result, const uint64_t* dividend, const uint64_t* divisor,
                            size_t words, int want_remainder) {
    int cmp = 0;
    for (size_t i = words; i-- > 0;)
        if (dividend[i] != divisor[i]) {
            cmp = dividend[i] < divisor[i] ? -1 : 1;
            break;
        }
    if (cmp < 0) {
        if (want_remainder)
            memcpy(result, dividend, words * 8u);
        return;
    }
    int n = (int)(2u * words);
    int m = n;
    while (n && !word32(dividend, n - 1))
        --n;
    while (m && !word32(divisor, m - 1))
        --m;
    if (m == 1) {
        uint64_t remainder = 0;
        uint32_t denominator = word32(divisor, 0);
        for (int i = n - 1; i >= 0; --i) {
            uint64_t partial = (remainder << 32) | word32(dividend, i);
            if (!want_remainder)
                set_word32(result, i, (uint32_t)(partial / denominator));
            remainder = partial % denominator;
        }
        if (want_remainder)
            result[0] = remainder;
        return;
    }

    size_t count = (size_t)n + 1u + (size_t)m;
    if (count > SIZE_MAX / sizeof(uint32_t)) {
        fputs("llg compact value fatal: division scratch size overflow\n", stderr);
        abort();
    }
    uint32_t* scratch = (uint32_t*)calloc(count, sizeof(uint32_t));
    if (!scratch) {
        fputs("llg compact value fatal: division scratch allocation failed\n", stderr);
        abort();
    }
    uint32_t* u = scratch;
    uint32_t* v = u + n + 1;
    unsigned shift = 0;
    uint32_t top = word32(divisor, m - 1);
    while (!(top & UINT32_C(0x80000000))) {
        top <<= 1;
        ++shift;
    }
    for (int i = 0; i < m; ++i) {
        v[i] = word32(divisor, i) << shift;
        if (shift && i)
            v[i] |= word32(divisor, i - 1) >> (32u - shift);
    }
    for (int i = 0; i < n; ++i) {
        u[i] = word32(dividend, i) << shift;
        if (shift && i)
            u[i] |= word32(dividend, i - 1) >> (32u - shift);
    }
    if (shift)
        u[n] = word32(dividend, n - 1) >> (32u - shift);
    const uint64_t radix = UINT64_C(1) << 32;
    for (int j = n - m; j >= 0; --j) {
        uint64_t q, rem;
        if (u[j + m] == v[m - 1]) {
            q = UINT32_MAX;
            rem = (uint64_t)u[j + m - 1] + v[m - 1];
        } else {
            uint64_t numerator = ((uint64_t)u[j + m] << 32) | u[j + m - 1];
            q = numerator / v[m - 1];
            rem = numerator % v[m - 1];
        }
        while (rem < radix && q * v[m - 2] > (rem << 32) + u[j + m - 2]) {
            --q;
            rem += v[m - 1];
        }
        uint64_t borrow = 0;
        for (int i = 0; i < m; ++i) {
            uint64_t product = q * v[i] + borrow;
            uint32_t low = (uint32_t)product;
            uint32_t before = u[j + i];
            u[j + i] = before - low;
            borrow = (product >> 32) + (before < low);
        }
        uint32_t before = u[j + m];
        u[j + m] = before - (uint32_t)borrow;
        if ((uint64_t)before < borrow) {
            --q;
            uint64_t carry = 0;
            for (int i = 0; i < m; ++i) {
                uint64_t sum = (uint64_t)u[j + i] + v[i] + carry;
                u[j + i] = (uint32_t)sum;
                carry = sum >> 32;
            }
            u[j + m] += (uint32_t)carry;
        }
        if (!want_remainder)
            set_word32(result, j, (uint32_t)q);
    }
    if (want_remainder) {
        for (int i = 0; i < m; ++i) {
            uint32_t word = u[i] >> shift;
            if (shift)
                word |= u[i + 1] << (32u - shift);
            set_word32(result, i, word);
        }
    }
    free(scratch);
}

#endif
