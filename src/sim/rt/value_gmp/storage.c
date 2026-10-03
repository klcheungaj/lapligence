#if LLG_SV4_USE_GMP
#include "../llg_value.h"
void llg_value_require_abi(void) {}
#endif
#include "internal.h"
#include <stdio.h>

void llg_gmp_sv4_fail(const char* message) {
    fprintf(stderr, "llg compact value fatal: %s\n", message);
    abort();
}
uint64_t* llg_gmp_sv4_alloc(size_t words) {
    if (!words || words > SIZE_MAX / sizeof(uint64_t))
        llg_gmp_sv4_fail("allocation size overflow");
    uint64_t* p = (uint64_t*)calloc(words, sizeof(uint64_t));
    if (!p)
        llg_gmp_sv4_fail("allocation failed");
    return p;
}
g4_t llg_gmp_sv4_new(uint32_t width, int8_t sign, int with_b) {
    g4_width_check(width);
    g4_t v = LLG_GMP_SV4_EMPTY;
    v.width = width;
    v.is_signed = sign != 0;
    if (width > 64) {
        size_t n = llg_gmp_sv4_words(v);
        v.data.wide.a = llg_gmp_sv4_alloc(n * (with_b ? 2u : 1u));
        v.data.wide.b = with_b ? v.data.wide.a + n : NULL;
    }
    return v;
}
g4_t llg_gmp_sv4_zero_wide(uint32_t width, int8_t sign) { return llg_gmp_sv4_new(width, sign, 0); }
void llg_gmp_sv4_destroy_wide(g4_t* v) { free(v->data.wide.a); }
g4_t llg_gmp_sv4_clone_wide(const g4_t* v) {
    g4_t out = llg_gmp_sv4_new(v->width, v->is_signed, v->data.wide.b != NULL);
    memcpy(out.data.wide.a, v->data.wide.a, llg_gmp_sv4_bytes(v));
    return out;
}
void llg_gmp_sv4_copy_wide(g4_t* out, const g4_t* v) {
    if (out->width > 64 && v->width == out->width &&
        (out->data.wide.b != NULL) == (v->data.wide.b != NULL)) {
        if (out->data.wide.a != v->data.wide.a)
            memcpy(out->data.wide.a, v->data.wide.a, llg_gmp_sv4_bytes(v));
        out->is_signed = v->is_signed;
        return;
    }
    llg_gmp_sv4_replace(out, llg_gmp_sv4_clone(v));
}
void llg_gmp_sv4_promote(g4_t* v) {
    if (v->width <= 64 || v->data.wide.b)
        return;
    size_t n = llg_gmp_sv4_words(*v);
    uint64_t* p = (uint64_t*)realloc(v->data.wide.a, n * 16u);
    if (!p)
        llg_gmp_sv4_fail("allocation failed");
    memset(p + n, 0, n * 8u);
    v->data.wide.a = p;
    v->data.wide.b = p + n;
}
void llg_gmp_sv4_finish(g4_t* v) {
    size_t n = llg_gmp_sv4_words(*v);
    if (!n)
        return;
    uint64_t mask = g4_topmask(v->width);
    uint64_t *a = g4_mut_a(v), *b = g4_mut_b(v);
    a[n - 1] &= mask;
    if (!b)
        return;
    b[n - 1] &= mask;
    if (v->width <= 64)
        return;
    for (size_t i = 0; i < n; ++i)
        if (b[i])
            return;
    uint64_t* p = (uint64_t*)realloc(a, n * 8u);
    if (!p)
        llg_gmp_sv4_fail("allocation failed");
    v->data.wide.a = p;
    v->data.wide.b = NULL;
}
g4_t llg_gmp_sv4_fill_wide(uint8_t state, uint32_t width, int8_t sign) {
    g4_t v = llg_gmp_sv4_new(width, sign, state >= 2);
    size_t n = llg_gmp_sv4_words(v);
    uint64_t *a = g4_mut_a(&v), *b = g4_mut_b(&v);
    if (state == 1 || state == 2)
        memset(a, 0xff, n * 8u);
    if (state >= 2 && b)
        memset(b, 0xff, n * 8u);
    llg_gmp_sv4_finish(&v);
    return v;
}
g4_t llg_gmp_sv4_from_limbs_wide(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                                 uint32_t width, int8_t sign) {
    g4_width_check(width);
    size_t n = ((size_t)width + 63u) / 64u;
    int unknown = 0;
    for (size_t i = 0; i < n; ++i) {
        uint64_t mask = i + 1 == n ? g4_topmask(width) : UINT64_MAX;
        if (((x ? x[i] : 0) | (z ? z[i] : 0)) & mask) {
            unknown = 1;
            break;
        }
    }
    g4_t out = llg_gmp_sv4_new(width, sign, unknown);
    uint64_t *a = g4_mut_a(&out), *b = g4_mut_b(&out);
    for (size_t i = 0; i < n; ++i) {
        uint64_t xx = x ? x[i] : 0, zz = z ? z[i] : 0, bb = xx | zz;
        a[i] = ((bits ? bits[i] : 0) & ~bb) | xx;
        if (b)
            b[i] = bb;
    }
    llg_gmp_sv4_finish(&out);
    return out;
}
void llg_gmp_sv4_extend_plane(uint64_t* out, uint32_t width, const uint64_t* input, uint32_t source,
                              int sign) {
    size_t n = ((size_t)width + 63u) / 64u, m = ((size_t)source + 63u) / 64u;
    uint64_t fill =
        sign && source && input && ((input[m - 1] >> ((source - 1) % 64)) & 1) ? UINT64_MAX : 0;
    size_t common = n < m ? n : m;
    if (input && common)
        memcpy(out, input, common * 8u);
    else if (common)
        memset(out, 0, common * 8u);
    if (source < width && m)
        out[m - 1] |= fill & ~g4_topmask(source);
    for (size_t i = m; i < n; ++i)
        out[i] = fill;
    if (n)
        out[n - 1] &= g4_topmask(width);
}
void llg_gmp_sv4_extend_value(g4_t* out, g4_t input, int sign) {
    llg_gmp_sv4_extend_plane(g4_mut_a(out), out->width, g4_a(&input), input.width, sign);
    uint64_t* b = g4_mut_b(out);
    if (b)
        llg_gmp_sv4_extend_plane(b, out->width, g4_b(&input), input.width, sign);
}
g4_t llg_gmp_sv4_convert_wide(g4_t v, uint32_t width, int8_t sign, int extend) {
    g4_t out = llg_gmp_sv4_new(width, sign, llg_gmp_sv4_is_unknown(v));
    llg_gmp_sv4_extend_value(&out, v, extend);
    llg_gmp_sv4_finish(&out);
    return out;
}
g4_t llg_gmp_sv4_two_state_wide(g4_t v) {
    g4_t out = llg_gmp_sv4_new(v.width, v.is_signed, 0);
    const uint64_t *a = v.data.wide.a, *b = v.data.wide.b;
    size_t n = llg_gmp_sv4_words(v);
    if (!b)
        memcpy(out.data.wide.a, a, n * 8u);
    else
        for (size_t i = 0; i < n; ++i)
            out.data.wide.a[i] = a[i] & ~b[i];
    return out;
}
void llg_gmp_sv4_set_ab_wide(g4_t* v, size_t word, uint64_t a, uint64_t b) {
    if (word + 1 == llg_gmp_sv4_words(*v)) {
        a &= g4_topmask(v->width);
        b &= g4_topmask(v->width);
    }
    if (b)
        llg_gmp_sv4_promote(v);
    v->data.wide.a[word] = a;
    if (v->data.wide.b)
        v->data.wide.b[word] = b;
    llg_gmp_sv4_finish(v);
}
void llg_gmp_sv4_export_vpi_words_wide(g4_t v, size_t first, llg_gmp_sv4_vpi_word_t* out,
                                       size_t count) {
    size_t m = llg_gmp_sv4_word_range(v, first, count);
    const uint64_t *a = g4_a(&v), *b = g4_b(&v);
    for (size_t i = 0; i < m; ++i) {
        out[i].aval = a[first + i];
        out[i].bval = b ? b[first + i] : 0;
    }
    for (size_t i = m; i < count; ++i)
        out[i] = (llg_gmp_sv4_vpi_word_t){0, 0};
}
void llg_gmp_sv4_export_words_wide(g4_t v, size_t first, llg_gmp_sv4_word_t* out, size_t count) {
    size_t m = llg_gmp_sv4_word_range(v, first, count);
    const uint64_t *a = g4_a(&v), *b = g4_b(&v);
    for (size_t i = 0; i < m; ++i) {
        uint64_t aa = a[first + i], bb = b ? b[first + i] : 0;
        out[i] = (llg_gmp_sv4_word_t){aa & ~bb, aa & bb, ~aa & bb};
    }
    for (size_t i = m; i < count; ++i)
        out[i] = (llg_gmp_sv4_word_t){0, 0, 0};
}
void llg_gmp_sv4_import_vpi_words_wide(g4_t* v, size_t first, const llg_gmp_sv4_vpi_word_t* in,
                                       size_t count) {
    size_t m = llg_gmp_sv4_word_range(*v, first, count);
    if (!m)
        return;
    for (size_t i = 0; i < m; ++i)
        if (in[i].bval) {
            llg_gmp_sv4_promote(v);
            break;
        }
    uint64_t *a = g4_mut_a(v), *b = g4_mut_b(v);
    for (size_t i = 0; i < m; ++i) {
        a[first + i] = in[i].aval;
        if (b)
            b[first + i] = in[i].bval;
    }
    llg_gmp_sv4_finish(v);
}
void llg_gmp_sv4_import_words_wide(g4_t* v, size_t first, const llg_gmp_sv4_word_t* in,
                                   size_t count) {
    size_t m = llg_gmp_sv4_word_range(*v, first, count);
    if (!m)
        return;
    for (size_t i = 0; i < m; ++i)
        if (in[i].x | in[i].z) {
            llg_gmp_sv4_promote(v);
            break;
        }
    uint64_t *a = g4_mut_a(v), *b = g4_mut_b(v);
    for (size_t i = 0; i < m; ++i) {
        uint64_t bb = in[i].x | in[i].z;
        a[first + i] = (in[i].bits & ~bb) | in[i].x;
        if (b)
            b[first + i] = bb;
    }
    llg_gmp_sv4_finish(v);
}
