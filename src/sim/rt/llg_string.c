#include "llg_string.h"

#include <float.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void string_fail(const char *message) {
    fprintf(stderr, "llg: string runtime: %s\n", message);
    abort();
}

static llg_string_t string_alloc(size_t length) {
    if (length == SIZE_MAX) string_fail("allocation size overflow");
    llg_string_t value = {NULL, length};
    if (length) {
        value.data = malloc(length + 1);
        if (!value.data) string_fail("out of memory");
        value.data[length] = 0;
    }
    return value;
}

llg_string_t llg_string_bytes(const char *bytes, size_t length) {
    llg_string_t value = string_alloc(length);
    if (length) memcpy(value.data, bytes, length);
    return value;
}

llg_string_t llg_string_clone(const llg_string_t *value) {
    return llg_string_bytes(value->data, value->len);
}

void llg_string_destroy(llg_string_t *value) {
    free(value->data);
    value->data = NULL;
    value->len = 0;
}

void llg_string_move(llg_string_t *target, llg_string_t value) {
    llg_string_destroy(target);
    *target = value;
}

llg_string_t llg_string_concat(llg_string_t a, llg_string_t b) {
    if (b.len > SIZE_MAX - a.len) string_fail("concatenation size overflow");
    llg_string_t value = string_alloc(a.len + b.len);
    if (a.len) memcpy(value.data, a.data, a.len);
    if (b.len) memcpy(value.data + a.len, b.data, b.len);
    llg_string_destroy(&a);
    llg_string_destroy(&b);
    return value;
}

llg_string_t llg_string_repeat(llg_string_t value, sv4_t count) {
    int64_t n;
    if (!sv4_to_index_i64(count, &n) || n < 0)
        string_fail("replication count is negative, unknown, or unrepresentable");
    if (value.len && (uint64_t)n > (SIZE_MAX - 1) / value.len)
        string_fail("replication size overflow");
    llg_string_t result = string_alloc(value.len * (size_t)n);
    if (value.len) for (size_t i = 0; i < (size_t)n; ++i)
        memcpy(result.data + i * value.len, value.data, value.len);
    llg_string_destroy(&value);
    return result;
}

static unsigned char ascii_case(unsigned char c, int upper) {
    if (upper && c >= 'a' && c <= 'z') return (unsigned char)(c - 'a' + 'A');
    if (!upper && c >= 'A' && c <= 'Z') return (unsigned char)(c - 'A' + 'a');
    return c;
}

llg_string_t llg_string_case(llg_string_t value, int upper) {
    for (size_t i = 0; i < value.len; ++i)
        value.data[i] = (char)ascii_case((unsigned char)value.data[i], upper);
    return value;
}

llg_string_t llg_string_substr(llg_string_t value, sv4_t first, sv4_t last) {
    int64_t i, j;
    llg_string_t result = {0};
    if (sv4_to_index_i64(first, &i) && sv4_to_index_i64(last, &j) &&
        i >= 0 && j >= i && (uint64_t)j < value.len)
        result = llg_string_bytes(value.data + (size_t)i, (size_t)(j - i) + 1);
    llg_string_destroy(&value);
    return result;
}

llg_string_t llg_string_from_packed(sv4_t value) {
    value = sv4_to_two_state(value);
    size_t raw_length = ((size_t)value.width + 7) / 8;
    size_t length = 0;
    for (size_t i = raw_length; i; --i) {
        size_t bit = (i - 1) * 8;
        unsigned char byte = (unsigned char)(value.bits[bit / 64] >> (bit % 64));
        if (byte) ++length;
    }
    llg_string_t result = string_alloc(length);
    size_t out = 0;
    for (size_t i = raw_length; i; --i) {
        size_t bit = (i - 1) * 8;
        unsigned char byte = (unsigned char)(value.bits[bit / 64] >> (bit % 64));
        if (byte) result.data[out++] = (char)byte;
    }
    return result;
}

sv4_t llg_string_to_packed(llg_string_t value, uint32_t width, int is_signed) {
    sv4_t result = sv4_from_u64(0, width, is_signed);
    size_t bytes = ((size_t)width + 7) / 8;
    if (bytes > value.len) bytes = value.len;
    for (size_t i = 0; i < bytes; ++i)
        result.bits[i / 8] |= (uint64_t)(unsigned char)value.data[value.len - 1 - i] << ((i % 8) * 8);
    llg_string_destroy(&value);
    return sv4_resize(result, width, is_signed);
}

sv4_t llg_string_len(llg_string_t value) {
    sv4_t result = sv4_from_u64((uint32_t)value.len, 32, 1);
    llg_string_destroy(&value);
    return result;
}

sv4_t llg_string_getc(llg_string_t value, sv4_t index) {
    int64_t i;
    unsigned char c = 0;
    if (sv4_to_index_i64(index, &i) && i >= 0 && (uint64_t)i < value.len)
        c = (unsigned char)value.data[(size_t)i];
    llg_string_destroy(&value);
    return sv4_from_u64(c, 8, 1);
}

void llg_string_putc(llg_string_t *value, sv4_t index, sv4_t character) {
    int64_t i;
    unsigned char c = (unsigned char)sv4_to_two_state(character).bits[0];
    if (sv4_to_index_i64(index, &i) && i >= 0 && (uint64_t)i < value->len)
        value->data[(size_t)i] = (char)c;
}

sv4_t llg_string_compare(llg_string_t a, llg_string_t b, int ignore_case) {
    int cmp = 0;
    size_t n = a.len < b.len ? a.len : b.len;
    for (size_t i = 0; i < n; ++i) {
        unsigned char ac = (unsigned char)a.data[i], bc = (unsigned char)b.data[i];
        if (ignore_case) { ac = ascii_case(ac, 0); bc = ascii_case(bc, 0); }
        if (ac != bc) { cmp = ac < bc ? -1 : 1; break; }
    }
    if (!cmp) cmp = a.len < b.len ? -1 : a.len > b.len ? 1 : 0;
    llg_string_destroy(&a);
    llg_string_destroy(&b);
    return sv4_from_u64((uint32_t)cmp, 32, 1);
}

sv4_t llg_string_atoi(llg_string_t value, unsigned base) {
    uint32_t result = 0;
    if (base != 2 && base != 8 && base != 10 && base != 16) string_fail("invalid numeric base");
    for (size_t i = 0; i < value.len; ++i) {
        unsigned char c = ascii_case((unsigned char)value.data[i], 0);
        if (c == '_') continue;
        unsigned digit = c >= '0' && c <= '9' ? (unsigned)(c - '0') : c >= 'a' && c <= 'f' ? (unsigned)(c - 'a' + 10) : base;
        if (digit >= base) break;
        result = result * base + digit;
    }
    llg_string_destroy(&value);
    return sv4_from_u64(result, 32, 1);
}

void llg_string_itoa(llg_string_t *target, sv4_t value, unsigned base) {
    char buffer[34];
    size_t n = 0;
    if (base != 2 && base != 8 && base != 10 && base != 16) string_fail("invalid numeric base");
    value = sv4_cast(value, 32, 1);
    if (sv4_is_unknown(value)) {
        sv4_format(base == 2 ? 'b' : base == 8 ? 'o' : base == 16 ? 'h' : 'd', value, buffer, sizeof(buffer));
        llg_string_move(target, llg_string_bytes(buffer, strlen(buffer)));
        return;
    }
    uint32_t number = (uint32_t)sv4_to_two_state(value).bits[0];
    int negative = base == 10 && (number >> 31);
    if (negative) number = 0u - number;
    do { buffer[n++] = "0123456789abcdef"[number % base]; number /= base; } while (number);
    if (negative) buffer[n++] = '-';
    for (size_t i = 0; i < n / 2; ++i) { char c = buffer[i]; buffer[i] = buffer[n - 1 - i]; buffer[n - 1 - i] = c; }
    llg_string_move(target, llg_string_bytes(buffer, n));
}

void llg_string_print(llg_string_t value) {
    if (value.len) fwrite(value.data, 1, value.len, stdout);
    llg_string_destroy(&value);
}

static int decimal_digit(char c) { return c >= '0' && c <= '9'; }

/* Compact underscores only within an unsigned decimal number. */
static size_t real_digits(char *text, size_t length, size_t read, size_t *write) {
    while (read < length && (decimal_digit(text[read]) || text[read] == '_')) {
        if (text[read] != '_') text[(*write)++] = text[read];
        ++read;
    }
    return read;
}

double llg_string_atoreal(llg_string_t value) {
    size_t read = 0, write = 0;
    while (read < value.len && strchr(" \t\n\r\f\v", value.data[read])) ++read;
    if (read < value.len && (value.data[read] == '+' || value.data[read] == '-'))
        value.data[write++] = value.data[read++];
    if (read == value.len || !decimal_digit(value.data[read])) {
        llg_string_destroy(&value);
        return 0.0;
    }
    read = real_digits(value.data, value.len, read, &write);
    if (read + 1 < value.len && value.data[read] == '.' && decimal_digit(value.data[read + 1])) {
        value.data[write++] = value.data[read++];
        read = real_digits(value.data, value.len, read, &write);
    }
    if (read < value.len && (value.data[read] == 'e' || value.data[read] == 'E')) {
        size_t exponent = read + 1;
        if (exponent < value.len && (value.data[exponent] == '+' || value.data[exponent] == '-')) ++exponent;
        if (exponent < value.len && decimal_digit(value.data[exponent])) {
            while (read < exponent) value.data[write++] = value.data[read++];
            (void)real_digits(value.data, value.len, read, &write);
        }
    }
    value.data[write] = 0;
    double result = strtod(value.data, NULL);
    llg_string_destroy(&value);
    return result;
}

void llg_string_realtoa(llg_string_t *target, double value) {
    int length = snprintf(NULL, 0, "%.*g", DBL_DECIMAL_DIG, value);
    if (length <= 0) string_fail("real conversion failed");
    llg_string_t result = string_alloc((size_t)length);
    if (snprintf(result.data, (size_t)length + 1, "%.*g", DBL_DECIMAL_DIG, value) != length)
        string_fail("real conversion length changed");
    llg_string_move(target, result);
}
