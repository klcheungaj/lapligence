#if LLG_ADAPTER_COMPACT
#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#else
#include "llg_value.h"
#endif
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void parse_hex(const char* text, uint64_t* out, size_t words) {
    size_t len = strlen(text), word = 0;
    while (len && word < words) {
        size_t take = len < 16 ? len : 16;
        char chunk[17]; memcpy(chunk, text + len - take, take); chunk[take] = 0;
        out[word++] = strtoull(chunk, NULL, 16); len -= take;
    }
}
static void print_value(sv4_t value) {
    int unknown = sv4_is_unknown(value), all_x = unknown;
    if (unknown)
        for (size_t i = 0; i < llg_sv4_words(value); ++i) {
            uint32_t left = llg_sv4_width(value) - (uint32_t)i * 64u;
            uint64_t mask = left >= 64 ? UINT64_MAX : UINT64_MAX >> (64u - left);
            all_x &= llg_sv4_word(value, i, LLG_SV4_X) == mask;
        }
    printf("%u %d %d ", llg_sv4_width(value), llg_sv4_signed(value), unknown ? all_x ? 1 : 2 : 0);
    size_t n = unknown ? 0 : llg_sv4_words(value);
    while (n && !llg_sv4_word(value, n - 1u, LLG_SV4_BITS)) --n;
    if (!n) putchar('0');
    while (n) printf("%016" PRIx64, llg_sv4_word(value, --n, LLG_SV4_BITS));
    putchar('\n');
}
int main(void) {
    size_t text_capacity = LLG_SUPPORTED_WIDTH_LIMIT / 4u + 32u;
    char* texts = (char*)malloc(3u * text_capacity);
    if (!texts) return 2;
    char op[16];
    while (scanf("%15s", op) == 1) {
        uint32_t width; int sign;
        if (!strcmp(op, "from")) {
            double real;
            if (scanf("%u %d %la", &width, &sign, &real) != 3) return 2;
            sv4_t value = sv4_from_real(real, width, (int8_t)sign);
            print_value(value); sv4_destroy(&value);
        } else if (!strcmp(op, "rtoi")) {
            double real; if (scanf("%la", &real) != 1) return 2;
            sv4_t value = sv4_rtoi(real); print_value(value); sv4_destroy(&value);
        } else if (!strcmp(op, "inspect")) {
            size_t cap;
            char *bits = texts, *x = bits + text_capacity, *z = x + text_capacity;
            if (scanf("%u %d %zu %262160s %262160s %262160s", &width, &sign, &cap, bits, x, z) != 6)
                return 2;
            if (width >= LLG_SUPPORTED_WIDTH_LIMIT || cap > LLG_SUPPORTED_WIDTH_LIMIT + 32u) return 2;
            size_t n = ((size_t)width + 63u) / 64u;
            uint64_t* words = (uint64_t*)calloc(3u * (n ? n : 1u), 8u);
            if (!words) return 2;
            parse_hex(bits, words, n); parse_hex(x, words + n, n); parse_hex(z, words + 2u * n, n);
            for (size_t i = 0; i < n; ++i) words[i] &= ~(words[n + i] | words[2u * n + i]);
            sv4_t value = sv4_from_limbs(words, words + n, words + 2u * n, width, (int8_t)sign);
            int64_t result = INT64_C(1234567);
            int valid = sv4_to_index_i64(value, &result);
            printf("%" PRId64 " %d %" PRIu64 " %d %" PRId64 " %a %a %a",
                   sv4_to_i64(value), sv4_fits_i64(value), sv4_to_index(value), valid, result,
                   sv4_to_real(value), sv4_bitstoreal(value), sv4_bitstoshortreal(value));
            char* buf = (char*)malloc(cap ? cap : 1u); if (!buf) return 2;
            const char formats[] = {'d', 'h', 'b', 'o'};
            for (unsigned i = 0; i < 4; ++i) {
                buf[0] = 0; sv4_format(formats[i], value, buf, cap);
                printf(" %s", buf[0] ? buf : "#");
            }
            putchar('\n'); free(buf); free(words); sv4_destroy(&value);
        } else return 2;
    }
    free(texts); return 0;
}
