#include "internal.h"

int llg_gmp_sv4_fits_i64_wide(g4_t value) {
    if (llg_gmp_sv4_is_unknown(value))
        return 0;
    const uint64_t* a = g4_a(&value);
    uint64_t fill = value.is_signed && (a[0] >> 63) ? UINT64_MAX : 0;
    if (!value.is_signed && (a[0] >> 63))
        return 0;
    size_t n = llg_gmp_sv4_words(value);
    for (size_t i = 1; i < n; ++i) {
        uint64_t mask = i + 1u == n ? g4_topmask(value.width) : UINT64_MAX;
        if (a[i] != (fill & mask))
            return 0;
    }
    return 1;
}
uint64_t llg_gmp_sv4_to_index_wide(g4_t value) {
    if (llg_gmp_sv4_is_unknown(value))
        return UINT64_MAX;
    const uint64_t* a = g4_a(&value);
    size_t n = llg_gmp_sv4_words(value);
    if (value.is_signed && ((a[n - 1u] >> ((value.width - 1u) % 64u)) & 1u))
        return UINT64_MAX;
    for (size_t i = 1; i < n; ++i)
        if (a[i])
            return UINT64_MAX;
    return a[0];
}
void llg_gmp_sv4_to_dec_string_wide(g4_t value, char* buf, size_t cap) {
    if (!cap)
        return;
    if (llg_gmp_sv4_is_unknown(value)) {
        buf[0] = cap > 1 ? 'x' : 0;
        if (cap > 1)
            buf[1] = 0;
        return;
    }
    size_t n = llg_gmp_sv4_words(value);
    const uint64_t* a = g4_a(&value);
    int negative = value.is_signed && ((a[n - 1u] >> ((value.width - 1u) % 64u)) & 1u);
    uint64_t* magnitude = llg_gmp_sv4_alloc(n);
    uint64_t carry = 1;
    for (size_t i = 0; i < n; ++i) {
        magnitude[i] = negative ? ~a[i] + carry : a[i];
        carry &= magnitude[i] == 0;
    }
    magnitude[n - 1u] &= g4_topmask(value.width);
    while (n && !magnitude[n - 1u])
        --n;
    size_t capacity = ((size_t)value.width * 30103u) / 100000u + 2u;
    unsigned char* digits = (unsigned char*)malloc(capacity);
    if (!digits)
        llg_gmp_sv4_fail("value-format allocation failed");
    size_t count = n ? llg_gmp_sv4_kernel_decimal(digits, magnitude, n) : 0;
    size_t len = 0, first = 0;
    if (negative && len + 1u < cap)
        buf[len++] = '-';
    while (first + 1u < count && !digits[first])
        ++first;
    if (!count && len + 1u < cap)
        buf[len++] = '0';
    for (; first < count && len + 1u < cap; ++first)
        buf[len++] = (char)('0' + digits[first]);
    buf[len] = 0;
    free(digits);
    free(magnitude);
}
static uint64_t group(const uint64_t* p, uint32_t width, uint32_t bit) {
    if (!p || bit >= width)
        return 0;
    size_t word = bit / 64u;
    unsigned shift = bit % 64u;
    uint64_t out = p[word] >> shift;
    if (shift && word + 1u < ((size_t)width + 63u) / 64u)
        out |= p[word + 1u] << (64u - shift);
    return out;
}
void llg_gmp_sv4_format(char fmt, g4_t value, char* buf, size_t cap) {
    if (!cap)
        return;
    if (fmt == 'd') {
        llg_gmp_sv4_to_dec_string(value, buf, cap);
        return;
    }
    unsigned size = fmt == 'b' ? 1u : fmt == 'h' ? 4u : fmt == 'o' ? 3u : 0;
    if (!size) {
        buf[0] = cap > 1 ? '?' : 0;
        if (cap > 1)
            buf[1] = 0;
        return;
    }
    const uint64_t *a = g4_a(&value), *b = g4_b(&value);
    uint32_t digits = (value.width + size - 1u) / size;
    size_t len = 0;
    while (digits && len + 1u < cap) {
        uint32_t bit = --digits * size;
        uint64_t mask = g4_mask(size), aa = group(a, value.width, bit) & mask;
        uint64_t bb = group(b, value.width, bit) & mask;
        buf[len++] = aa & bb ? 'x' : bb ? 'z' : "0123456789abcdef"[aa];
    }
    buf[len] = 0;
}
