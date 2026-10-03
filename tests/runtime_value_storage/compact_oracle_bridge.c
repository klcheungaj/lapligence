#if LLG_ORACLE_COMPACT
#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#else
#include "llg_value.h"
#endif

/* Stable test ABI: copied words only, never backend descriptors. */
int llg_compact_test_eval(unsigned op, uint32_t aw, int8_t as, const uint64_t* a,
                          const uint64_t* ax, const uint64_t* az, uint32_t bw, int8_t bs,
                          const uint64_t* b, const uint64_t* bx, const uint64_t* bz, uint64_t* r,
                          uint64_t* rx, uint64_t* rz, uint32_t* rw, int8_t* rs, size_t* bytes) {
    sv4_t x = sv4_from_limbs(a, ax, az, aw, as), y = sv4_from_limbs(b, bx, bz, bw, bs),
          out = SV4_EMPTY;
    switch (op) {
    case 0:
        out = sv4_add(x, y);
        break;
    case 1:
        out = sv4_sub(x, y);
        break;
    case 2:
        out = sv4_mul(x, y);
        break;
    case 3:
        out = sv4_div(x, y);
        break;
    case 4:
        out = sv4_mod(x, y);
        break;
    case 5:
        out = sv4_pow(x, y);
        break;
    case 6:
        out = sv4_neg(x);
        break;
    case 7:
        out = sv4_clog2(x);
        break;
    case 8:
        out = sv4_and(x, y);
        break;
    case 9:
        out = sv4_or(x, y);
        break;
    case 10:
        out = sv4_xor(x, y);
        break;
    case 11:
        out = sv4_xnor(x, y);
        break;
    case 12:
        out = sv4_bitneg(x);
        break;
    case 13:
        out = sv4_eq(x, y);
        break;
    case 14:
        out = sv4_neq(x, y);
        break;
    case 15:
        out = sv4_case_eq(x, y);
        break;
    case 16:
        out = sv4_case_neq(x, y);
        break;
    case 17:
        out = sv4_lt(x, y);
        break;
    case 18:
        out = sv4_le(x, y);
        break;
    case 19:
        out = sv4_gt(x, y);
        break;
    case 20:
        out = sv4_ge(x, y);
        break;
    case 21:
        out = sv4_logand(x, y);
        break;
    case 22:
        out = sv4_logor(x, y);
        break;
    case 23:
        out = sv4_lognot(x);
        break;
    case 24: {
        sv4_t cond = sv4_x(1, 0);
        out = sv4_mux(cond, x, y);
        sv4_destroy(&cond);
        break;
    }
    case 25:
        out = sv4_cast(x, bw, bs);
        break;
    case 26:
        out = sv4_resize(x, bw, bs);
        break;
    case 27:
        out = sv4_to_two_state(x);
        break;
    case 28:
        out = sv4_clone(&x);
        break;
    default:
        sv4_destroy(&x);
        sv4_destroy(&y);
        return 0;
    }
    *rw = llg_sv4_width(out);
    *rs = llg_sv4_signed(out);
    *bytes = sv4_bytes(&out);
    for (size_t i = 0; i < llg_sv4_words(out); ++i) {
        r[i] = llg_sv4_word(out, i, LLG_SV4_BITS);
        rx[i] = llg_sv4_word(out, i, LLG_SV4_X);
        rz[i] = llg_sv4_word(out, i, LLG_SV4_Z);
    }
    sv4_destroy(&out);
    sv4_destroy(&x);
    sv4_destroy(&y);
    return 1;
}

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    unsigned op, aw, bw;
    int sa, sb;
    while (scanf("%u %u %d %u %d", &op, &aw, &sa, &bw, &sb) == 5) {
        size_t an = ((size_t)aw + 63) / 64, bn = ((size_t)bw + 63) / 64, n = (aw > bw ? aw : bw);
        if (n < 32)
            n = 32;
        n = (n + 63) / 64;
        uint64_t* p = (uint64_t*)calloc(an + bn + 3 * n, sizeof(uint64_t));
        if (!p)
            return 1;
        uint64_t *a = p, *b = a + an, *r = b + bn, *x = r + n, *z = x + n;
        for (size_t i = 0; i < an; ++i)
            if (scanf("%" SCNx64, &a[i]) != 1)
                return 2;
        for (size_t i = 0; i < bn; ++i)
            if (scanf("%" SCNx64, &b[i]) != 1)
                return 2;
        uint32_t rw;
        int8_t rs;
        size_t bytes;
        if (!llg_compact_test_eval(op, aw, (int8_t)sa, a, NULL, NULL, bw, (int8_t)sb, b, NULL, NULL,
                                   r, x, z, &rw, &rs, &bytes))
            return 3;
        printf("%u %d %zu", rw, rs, bytes);
        for (size_t i = 0; i < ((size_t)rw + 63) / 64; ++i)
            printf(" %" PRIx64 " %" PRIx64 " %" PRIx64, r[i], x[i], z[i]);
        putchar('\n');
        fflush(stdout);
        free(p);
    }
    return 0;
}
