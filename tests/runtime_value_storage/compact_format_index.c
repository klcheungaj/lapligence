#include "compact_adapters_test.h"
#include <inttypes.h>

static void check_adapters(sv4_t old, g4_t v) {
    CHECK(sv4_to_i64(old) == llg_gmp_sv4_to_i64(v));
    CHECK(sv4_fits_i64(old) == llg_gmp_sv4_fits_i64(v));
    CHECK(sv4_to_index(old) == llg_gmp_sv4_to_index(v));
    int64_t a = INT64_C(0x78abcde), b = a;
    CHECK(sv4_to_index_i64(old, &a) == llg_gmp_sv4_to_index_i64(v, &b));
    CHECK(a == b);
    CHECK(!llg_gmp_sv4_to_index_i64(v, NULL));
}
static void format_buffers(sv4_t old, g4_t v, int full) {
    size_t max = full ? (size_t)v.width + 3u : 4098u;
    if (max < 22u) max = 22u;
    char* a = (char*)malloc(max + 4u); char* b = (char*)malloc(max + 4u); CHECK(a && b);
    const char formats[] = {'d', 'b', 'h', 'o', '?'};
    size_t caps[] = {0, 1, 2, 7, 22, max};
    for (unsigned f = 0; f < sizeof(formats); ++f) for (unsigned c = 0; c < 6; ++c) {
        memset(a, '#', max + 4u); memset(b, '#', max + 4u);
        size_t cap = caps[c];
        sv4_format(formats[f], old, a + 1, cap); llg_gmp_sv4_format(formats[f], v, b + 1, cap);
        CHECK(!memcmp(a, b, max + 4u));
        CHECK(b[0] == '#' && b[cap + 1u] == '#');
        if (cap) CHECK(memchr(b + 1, 0, cap));
        if (formats[f] == 'd') {
            memset(b, '#', max + 4u); llg_gmp_sv4_to_dec_string(v, b + 1, cap);
            CHECK(!memcmp(a, b, max + 4u));
        }
    }
    llg_gmp_sv4_format('d', v, NULL, 0); llg_gmp_sv4_to_dec_string(v, NULL, 0);
    free(a); free(b);
}
static void exhaustive(void) {
    for (uint32_t width = 0; width <= 4; ++width) for (unsigned sign = 0; sign < 2; ++sign)
        for (unsigned code = 0; code < (1u << (2u * width)); ++code) {
            g4_t v = llg_gmp_sv4_zero(width, (int8_t)sign); sv4_t old = sv4_zero(width, (int8_t)sign);
            char expected[8] = {0}; uint64_t known = 0; int unknown = 0, has_x = 0;
            for (uint32_t bit = 0; bit < width; ++bit) {
                unsigned state = (code >> (2u * bit)) & 3;
                llg_sv4_set_state(&old, bit, state); llg_gmp_sv4_set_state(&v, bit, state);
                expected[width - 1u - bit] = "01xz"[state];
                if (state == 1) known |= UINT64_C(1) << bit;
                unknown |= state >= 2; has_x |= state == 2;
            }
            char buf[24]; llg_gmp_sv4_format('b', v, buf, sizeof(buf)); CHECK(!strcmp(buf, expected));
            llg_gmp_sv4_format('h', v, buf, sizeof(buf));
            if (width) CHECK(buf[0] == (has_x ? 'x' : unknown ? 'z' : "0123456789abcdef"[known]));
            int64_t number = (int64_t)known;
            if (sign && width && (known >> (width - 1u))) number -= INT64_C(1) << width;
            snprintf(expected, sizeof(expected), "%" PRId64, number);
            llg_gmp_sv4_to_dec_string(v, buf, sizeof(buf)); CHECK(!strcmp(buf, unknown ? "x" : expected));
            CHECK(llg_gmp_sv4_fits_i64(v) == !unknown);
            CHECK(llg_gmp_sv4_to_index(v) == (unknown || number < 0 ? UINT64_MAX : known));
            check_adapters(old, v); format_buffers(old, v, 1); sv4_destroy(&old);
        }
}
static void wide(void) {
    uint64_t seed = UINT64_C(0x12eeddd344785);
    for (size_t w = 0; w < sizeof(adapter_widths) / sizeof(adapter_widths[0]); ++w) {
        uint32_t width = adapter_widths[w]; size_t n = ((size_t)width + 63u) / 64u;
        uint64_t* p = (uint64_t*)calloc(3u * (n ? n : 1u), 8); CHECK(p);
        for (unsigned pattern = 0; pattern < 8; ++pattern) {
            for (size_t i = 0; i < n; ++i) {
                p[i] = pattern == 0 ? 0 : pattern == 1 ? UINT64_MAX : next_word(&seed);
                p[n + i] = pattern >= 6 ? next_word(&seed) : 0;
                p[2u * n + i] = pattern >= 6 ? next_word(&seed) & ~p[n + i] : 0;
                p[i] &= ~(p[n + i] | p[2u * n + i]);
                if (width > 16384 && pattern < 6 && i) p[i] = 0;
            }
            if (pattern == 3 && n) {
                p[0] = UINT64_C(1) << 63;
                for (size_t i = 1; i < n; ++i) p[i] = UINT64_MAX;
            }
            if (pattern == 4 && n) {
                p[0] = INT64_MAX;
                for (size_t i = 1; i < n; ++i) p[i] = 0;
            }
            if (pattern == 5 && n) {
                p[0] = UINT64_MAX;
                for (size_t i = 1; i < n; ++i) p[i] = 0;
            }
            for (int8_t sign = 0; sign < 2; ++sign) {
                sv4_t old = sv4_from_limbs(p, p + n, p + 2u * n, width, sign);
                g4_t v = llg_gmp_sv4_from_limbs(p, p + n, p + 2u * n, width, sign);
                check_adapters(old, v);
                if (width <= 16384 || pattern >= 6 || pattern == 4 || pattern == 5)
                    format_buffers(old, v, width <= 4096);
                sv4_destroy(&old); llg_gmp_sv4_destroy(&v);
            }
        }
        free(p);
    }
}
int main(void) {
    exhaustive(); wide();
    printf("format/index: %zu checks passed\n", checks);
    return 0;
}
