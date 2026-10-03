#include "llg_value.h"
#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #c);                                \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
static unsigned long checks;
static unsigned state(g4_t v, uint32_t bit) {
    return bit < v.width ? llg_gmp_sv4_state(v, bit) : 0;
}
static g4_t states(uint32_t width, unsigned code) {
    g4_t v = llg_gmp_sv4_zero(width, code & 1);
    for (uint32_t i = 0; i < width; ++i) {
        llg_gmp_sv4_set_state(&v, i, code % 4);
        code /= 4;
    }
    return v;
}
static sv4_t legacy(g4_t v) {
    sv4_t out = sv4_zero(v.width, v.is_signed);
    for (size_t i = 0; i < llg_sv4_words(out); ++i) {
        llg_gmp_sv4_vpi_word_t w = llg_gmp_sv4_vpi_word(v, i);
        llg_sv4_set_vpi_word(&out, i, (llg_sv4_vpi_word_t){w.aval, w.bval});
    }
    return out;
}
static void compare(sv4_t old, g4_t v) {
    CHECK(old.width == v.width && old.is_signed == v.is_signed);
    for (size_t i = 0; i < llg_sv4_words(old); ++i)
        for (unsigned p = 0; p < 3; ++p)
            CHECK(llg_sv4_word(old, i, p) == llg_gmp_sv4_word(v, i, p));
    CHECK(llg_gmp_sv4_bytes(&v) ==
          (v.width <= 64 ? 0 : 8u * llg_gmp_sv4_words(v) * (sv4_is_unknown(old) ? 2u : 1u)));
    if (v.width % 64) {
        llg_gmp_sv4_vpi_word_t w = llg_gmp_sv4_vpi_word(v, llg_gmp_sv4_words(v) - 1);
        CHECK(!((w.aval | w.bval) & ~LLG_GMP_MASK(v.width % 64)));
    }
    ++checks;
}
static void dispose(sv4_t* old, g4_t* v) {
    compare(*old, *v);
    sv4_destroy(old);
    llg_gmp_sv4_destroy(v);
}
static void masks(g4_t a, g4_t b, g4_t mask) {
    sv4_t oa = legacy(a), ob = legacy(b), om = legacy(mask);
    uint32_t width = a.width < b.width ? a.width : b.width;
    int same = 1, all = 1;
    for (uint32_t i = 0; i < width; ++i) {
        if (state(a, i) != state(b, i)) {
            all = 0;
            if (state(mask, i) == 1)
                same = 0;
        }
    }
    CHECK(llg_gmp_sv4_masked_same(a, b, &mask) == same);
    CHECK(llg_sv4_masked_same(oa, ob, &om) == same);
    CHECK(llg_gmp_sv4_masked_same(a, b, NULL) == all);
    CHECK(llg_sv4_masked_same(oa, ob, NULL) == all);
    for (unsigned clipped = 0; clipped < 2; ++clipped) {
        g4_t out = llg_gmp_sv4_clone(&a);
        sv4_t old = sv4_clone(&oa);
        if (clipped) {
            llg_gmp_sv4_masked_copy(&out, b, mask);
            llg_sv4_masked_copy(&old, ob, om);
        } else {
            g4_t valid_mask = llg_gmp_sv4_clone(&mask);
            llg_gmp_sv4_range_fill(&valid_mask, a.width, UINT32_MAX, 0);
            sv4_t ovm = legacy(valid_mask);
            llg_gmp_sv4_masked_merge(&out, b, valid_mask);
            llg_sv4_masked_merge(&old, ob, ovm);
            sv4_destroy(&ovm);
            llg_gmp_sv4_destroy(&valid_mask);
        }
        for (uint32_t i = 0; i < a.width; ++i) {
            int selected =
                state(mask, i) == 1 && (clipped ? i < width : i / 64u < llg_gmp_sv4_words(b));
            CHECK(state(out, i) == (selected ? state(b, i) : state(a, i)));
        }
        dispose(&old, &out);
    }
    sv4_destroy(&oa);
    sv4_destroy(&ob);
    sv4_destroy(&om);
}
static void ranges(g4_t target, g4_t source, uint32_t offset) {
    sv4_t old = legacy(target), os = legacy(source);
    int same = 1;
    for (uint32_t i = 0; i < source.width; ++i)
        if (state(target, offset + i) != state(source, i))
            same = 0;
    CHECK(llg_gmp_sv4_range_same(target, offset, source) == same);
    CHECK(llg_sv4_range_same(old, offset, os) == same);
    g4_t out = llg_gmp_sv4_clone(&target);
    llg_gmp_sv4_range_copy(&out, offset, source);
    llg_sv4_range_copy(&old, offset, os);
    for (uint32_t i = 0; i < out.width; ++i)
        CHECK(state(out, i) == (i >= offset && i - offset < source.width ? state(source, i - offset)
                                                                         : state(target, i)));
    CHECK(llg_gmp_sv4_range_same(out, offset, source));
    dispose(&old, &out);
    sv4_destroy(&os);
}
static void fill(g4_t target, uint32_t low, uint32_t count, unsigned s) {
    g4_t out = llg_gmp_sv4_clone(&target);
    sv4_t old = legacy(target);
    llg_gmp_sv4_range_fill(&out, low, count, s);
    llg_sv4_range_fill(&old, low, count, s);
    for (uint32_t i = 0; i < target.width; ++i)
        CHECK(state(out, i) == (i >= low && i - low < count ? s : state(target, i)));
    dispose(&old, &out);
}
static void append(g4_t v, unsigned count, unsigned s, unsigned digit) {
    sv4_t old = legacy(v);
    g4_t out = llg_gmp_sv4_clone(&v);
    llg_gmp_sv4_append_digit(&out, count, s, digit);
    llg_sv4_append_digit(&old, count, s, digit);
    for (uint32_t i = 0; i < v.width; ++i) {
        unsigned expected = i >= count         ? state(v, i - count)
                            : s == 0           ? (digit >> i) & 1u
                            : s == 2 || s == 3 ? s
                                               : 0;
        CHECK(state(out, i) == expected);
    }
    dispose(&old, &out);
}
static void numeric(g4_t v, uint32_t factor, uint32_t addend) {
    sv4_t old = legacy(v);
    g4_t out = llg_gmp_sv4_clone(&v);
    llg_gmp_sv4_mul_add_known(&out, factor, addend);
    llg_sv4_mul_add_known(&old, factor, addend);
    /* Binary long multiplication by a scalar, independent of limb products. */
    uint64_t carry = addend;
    for (uint32_t bit = 0; bit < v.width; ++bit) {
        carry += state(v, bit) ? factor : 0;
        CHECK(state(out, bit) == (carry & 1u));
        carry >>= 1;
    }
    dispose(&old, &out);
    old = legacy(v);
    out = llg_gmp_sv4_clone(&v);
    llg_gmp_sv4_negate_known(&out);
    llg_sv4_negate_known(&old);
    carry = 1;
    for (uint32_t bit = 0; bit < v.width; ++bit) {
        carry += !state(v, bit);
        CHECK(state(out, bit) == (carry & 1u));
        carry >>= 1;
    }
    dispose(&old, &out);
}
static void snapshots(g4_t v) {
    size_t n = llg_gmp_sv4_words(v), count = n + 2;
    llg_gmp_sv4_vpi_word_t* words = calloc(count, sizeof(*words));
    CHECK(words);
    llg_gmp_sv4_export_vpi_words(v, 0, words, count);
    llg_sv4_vpi_word_t* ow = calloc(count, sizeof(*ow));
    CHECK(ow);
    for (size_t i = 0; i < count; ++i) {
        ow[i].aval = words[i].aval;
        ow[i].bval = words[i].bval;
    }
    sv4_t old = legacy(v);
    CHECK(llg_gmp_sv4_same_vpi_words(v, words, count));
    CHECK(llg_sv4_same_vpi_words(old, ow, count));
    CHECK(llg_gmp_sv4_same_vpi_words(v, words, 0) == (!sv4_to_bool(old) && !sv4_is_unknown(old)));
    for (size_t i = 0; i < count; ++i) {
        if (count > 8 && i != 0 && i != n / 2 && i + 1 != n && i < n)
            continue;
        words[i].aval ^= 1;
        ow[i].aval ^= 1;
        CHECK(!llg_gmp_sv4_same_vpi_words(v, words, count));
        CHECK(!llg_sv4_same_vpi_words(old, ow, count));
        words[i].aval ^= 1;
        ow[i].aval ^= 1;
        words[i].bval ^= UINT64_C(1) << 63;
        ow[i].bval ^= UINT64_C(1) << 63;
        CHECK(!llg_gmp_sv4_same_vpi_words(v, words, count));
        CHECK(!llg_sv4_same_vpi_words(old, ow, count));
        words[i].bval ^= UINT64_C(1) << 63;
        ow[i].bval ^= UINT64_C(1) << 63;
    }
    free(words);
    free(ow);
    uint32_t widths[] = {0, v.width / 2, v.width, v.width + 67};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k) {
        uint32_t w = widths[k];
        char *text = malloc((size_t)w + 2), *otext = malloc((size_t)w + 2);
        CHECK(text && otext);
        memset(text, '!', (size_t)w + 2);
        memset(otext, '!', (size_t)w + 2);
        llg_gmp_sv4_export_text(v, w, text);
        llg_sv4_export_text(old, w, otext);
        CHECK(!memcmp(text, otext, (size_t)w + 2));
        for (uint32_t bit = 0; bit < w; ++bit)
            CHECK(text[w - 1 - bit] == "01xz"[state(v, bit)]);
        CHECK(text[w] == 0 && text[w + 1] == '!');
        free(text);
        free(otext);
    }
    for (unsigned plane = 0; plane < 3; ++plane) {
        uint32_t lows[] = {0, 1, 31, 32, 63, 64, v.width, UINT32_MAX};
        for (size_t k = 0; k < 64 + sizeof(lows) / sizeof(lows[0]); ++k)
            for (unsigned take = 1; take <= 64; ++take) {
                uint64_t expected = 0;
                for (unsigned bit = 0; bit < take; ++bit) {
                    uint32_t low = k < 64 ? (uint32_t)k : lows[k - 64];
                    uint64_t index = (uint64_t)low + bit;
                    unsigned s = index < v.width ? state(v, (uint32_t)index) : 0;
                    if (s == plane + 1)
                        expected |= UINT64_C(1) << bit;
                }
                CHECK(llg_gmp_sv4_plane_slice(v, k < 64 ? (uint32_t)k : lows[k - 64], take,
                                              plane) == expected);
                CHECK(llg_sv4_plane_slice(old, k < 64 ? (uint32_t)k : lows[k - 64], take, plane) ==
                      expected);
            }
    }
    sv4_destroy(&old);
}
static void foreign(g4_t v, size_t count, size_t stride) {
    size_t bytes = (count + 2) * stride + 1;
    unsigned char *raw = malloc(bytes), *oldraw = malloc(bytes);
    CHECK(raw && oldraw);
    memset(raw, 0xa5, bytes);
    memset(oldraw, 0xa5, bytes);
    sv4_t old = legacy(v);
    llg_gmp_sv4_export_vpi32(v, raw + 1, count, stride);
    llg_sv4_export_vpi32(old, oldraw + 1, count, stride);
    CHECK(!memcmp(raw, oldraw, bytes));
    for (size_t i = 0; i < count; ++i) {
        uint32_t a, b;
        memcpy(&a, raw + 1 + i * stride, 4);
        memcpy(&b, raw + 5 + i * stride, 4);
        for (unsigned bit = 0; bit < 32; ++bit) {
            size_t index = i * 32 + bit;
            unsigned s = index < v.width ? state(v, (uint32_t)index) : 0;
            CHECK(((a >> bit) & 1u) == (s == 1 || s == 2));
            CHECK(((b >> bit) & 1u) == (s >= 2));
        }
        a ^= UINT32_C(0xb17a9e2d);
        b ^= UINT32_C(0x55aa6cc3);
        memcpy(raw + 1 + i * stride, &a, 4);
        memcpy(raw + 5 + i * stride, &b, 4);
    }
    g4_t out = llg_gmp_sv4_clone(&v);
    llg_gmp_sv4_import_vpi32(&out, raw + 1, count, stride);
    llg_sv4_import_vpi32(&old, raw + 1, count, stride);
    for (uint32_t bit = 0; bit < v.width; ++bit) {
        unsigned expected = state(v, bit);
        if (bit / 32u < count) {
            uint32_t a, b;
            memcpy(&a, raw + 1 + bit / 32u * stride, 4);
            memcpy(&b, raw + 5 + bit / 32u * stride, 4);
            a = (a >> (bit % 32u)) & 1u;
            b = (b >> (bit % 32u)) & 1u;
            expected = b ? (a ? 2u : 3u) : a;
        }
        CHECK(state(out, bit) == expected);
    }
    dispose(&old, &out);
    free(raw);
    free(oldraw);
    llg_gmp_sv4_export_vpi32(v, NULL, 0, stride);
    llg_gmp_sv4_import_vpi32(&v, NULL, 0, stride);
}
static void coercion(g4_t v) {
    g4_t out = llg_gmp_sv4_clone(&v);
    sv4_t old = legacy(v);
    llg_gmp_sv4_two_state_inplace(&out);
    llg_sv4_two_state_inplace(&old);
    for (uint32_t bit = 0; bit < v.width; ++bit)
        CHECK(state(out, bit) == (state(v, bit) == 1));
    dispose(&old, &out);
    out = llg_gmp_sv4_clone(&v);
    old = legacy(v);
    if (v.width % 64u) {
        size_t top = llg_gmp_sv4_words(v) - 1;
        uint64_t padding = ~LLG_GMP_MASK(v.width % 64u);
        old.bits[top] |= padding;
        old.x[top] |= padding;
        old.z[top] |= padding;
        if (out.width <= 64) {
            out.data.small.a |= padding;
            out.data.small.b |= padding;
        } else {
            out.data.wide.a[top] |= padding;
            if (out.data.wide.b)
                out.data.wide.b[top] |= padding;
        }
    }
    llg_gmp_sv4_mask_top(&out);
    llg_sv4_mask_top(&old);
    dispose(&old, &out);
}
static void aliases(g4_t v, g4_t source) {
    for (unsigned which = 0; which < 3; ++which) {
        g4_t out = llg_gmp_sv4_clone(&v);
        sv4_t old = legacy(v), os = legacy(source);
        if (which == 0) {
            llg_gmp_sv4_masked_merge(&out, source, out);
            llg_sv4_masked_merge(&old, os, old);
        } else if (which == 1) {
            llg_gmp_sv4_masked_copy(&out, source, out);
            llg_sv4_masked_copy(&old, os, old);
        } else {
            llg_gmp_sv4_masked_copy(&out, out, out);
            llg_sv4_masked_copy(&old, old, old);
        }
        dispose(&old, &out);
        sv4_destroy(&os);
    }
    g4_t out = llg_gmp_sv4_clone(&v);
    sv4_t old = legacy(v);
    llg_gmp_sv4_range_copy(&out, 0, out);
    llg_sv4_range_copy(&old, 0, old);
    CHECK(llg_gmp_sv4_range_same(out, 0, out));
    dispose(&old, &out);
    g4_t known = llg_gmp_sv4_fill(1, v.width, v.is_signed);
    out = llg_gmp_sv4_clone(&known);
    old = legacy(known);
    llg_gmp_sv4_mask_remove(&out, out);
    llg_sv4_mask_remove(&old, old);
    CHECK(!llg_gmp_sv4_to_bool(out));
    dispose(&old, &out);
    out = llg_gmp_sv4_clone(&known);
    old = legacy(known);
    g4_t removed = llg_gmp_sv4_from_u64(UINT64_C(0x815eadff), 32, 0);
    sv4_t or = legacy(removed);
    llg_gmp_sv4_mask_remove(&out, removed);
    llg_sv4_mask_remove(&old, or);
    for (uint32_t bit = 0; bit < out.width; ++bit)
        CHECK(state(out, bit) == !state(removed, bit));
    dispose(&old, &out);
    sv4_destroy(& or);
    llg_gmp_sv4_destroy(&removed);
    llg_gmp_sv4_destroy(&known);
}
static void small(void) {
    for (uint32_t wa = 0; wa <= 3; ++wa)
        for (unsigned ca = 0; ca < (1u << (2 * wa)); ++ca) {
            g4_t a = states(wa, ca);
            for (uint32_t wb = 0; wb <= 3; ++wb)
                for (unsigned cb = 0; cb < (1u << (2 * wb)); ++cb) {
                    g4_t b = states(wb, cb);
                    for (unsigned cm = 0; cm < 64; ++cm) {
                        g4_t mask = states(3, cm);
                        masks(a, b, mask);
                    }
                    for (uint32_t offset = 0; offset <= wa; ++offset)
                        if (wb <= wa - offset)
                            ranges(a, b, offset);
                    aliases(a, b);
                    llg_gmp_sv4_destroy(&b);
                }
            llg_gmp_sv4_destroy(&a);
        }
    for (uint32_t w = 0; w <= 4; ++w)
        for (unsigned code = 0; code < (1u << (2 * w)); ++code) {
            g4_t v = states(w, code);
            for (unsigned take = 1; take <= 4; ++take)
                for (unsigned s = 0; s < 4; ++s)
                    for (unsigned digit = 0; digit < 16; ++digit)
                        append(v, take, s, digit);
            for (uint32_t low = 0; low <= w + 1; ++low)
                for (uint32_t count = 0; count <= w + 1; ++count)
                    for (unsigned s = 0; s < 4; ++s)
                        fill(v, low, count, s);
            coercion(v);
            snapshots(v);
            for (size_t count = 0; count < 4; ++count)
                foreign(v, count, 9);
            llg_gmp_sv4_destroy(&v);
        }
    for (uint32_t w = 0; w <= 8; ++w)
        for (unsigned bits = 0; bits < (1u << w); ++bits) {
            g4_t v = llg_gmp_sv4_from_u64(bits, w, bits & 1);
            for (unsigned factor = 0; factor < 16; ++factor)
                for (unsigned add = 0; add < 16; ++add)
                    numeric(v, factor, add);
        }
}
static uint64_t random_word(void) {
    static uint64_t seed = UINT64_C(0x780dea4f89117333);
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}
static g4_t pattern(uint32_t width, unsigned unknown) {
    g4_t v = llg_gmp_sv4_zero(width, unknown & 1);
    size_t n = llg_gmp_sv4_words(v);
    llg_gmp_sv4_vpi_word_t* raw = calloc(n ? n : 1, sizeof(*raw));
    CHECK(raw);
    for (size_t i = 0; i < n; ++i) {
        raw[i].aval = random_word();
        raw[i].bval = unknown ? random_word() : 0;
    }
    llg_gmp_sv4_import_vpi_words(&v, 0, raw, n);
    free(raw);
    return v;
}
static void wide(void) {
    uint32_t widths[] = {1,    2,    7,    8,    31,   32,   33,    63,    64,     65,
                         127,  128,  129,  255,  256,  257,  511,   512,   513,    1023,
                         1024, 1025, 4096, 8191, 8192, 8193, 16384, 65535, 1048575};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (unsigned unknown = 0; unknown < 2; ++unknown) {
            uint32_t w = widths[k];
            g4_t a = pattern(w, unknown), b = pattern(w, !unknown), mask = pattern(w, unknown);
            masks(a, b, mask);
            aliases(a, b);
            coercion(a);
            snapshots(a);
            g4_t ones = llg_gmp_sv4_fill(1, w, 0);
            masks(a, b, ones);
            uint32_t lows[] = {0, 1, 31, 32, 63, 64, w - 1, w, UINT32_MAX};
            for (size_t j = 0; j < sizeof(lows) / sizeof(lows[0]); ++j) {
                uint32_t low = lows[j];
                for (unsigned s = 0; s < 4; ++s)
                    fill(a, low, UINT32_MAX, s);
                if (low < w) {
                    g4_t src = pattern(w - low, !unknown);
                    ranges(a, src, low);
                    llg_gmp_sv4_destroy(&src);
                }
            }
            g4_t shorter = pattern(w - 1, !unknown);
            masks(a, shorter, ones);
            masks(shorter, a, ones);
            llg_gmp_sv4_destroy(&shorter);
            for (unsigned take = 1; take <= 4; ++take)
                for (unsigned s = 0; s < 4; ++s)
                    append(a, take, s, 13);
            size_t records = ((size_t)w + 31u) / 32u;
            size_t counts[] = {0, 1, 2, 3, records / 2, records, records + 3};
            for (size_t j = 0; j < sizeof(counts) / sizeof(counts[0]); ++j)
                for (size_t stride = 8; stride <= 13; stride += 5)
                    foreign(a, counts[j], stride);
            g4_t known = pattern(w, 0);
            uint32_t factors[] = {0, 1, 10, UINT32_MAX};
            for (size_t j = 0; j < sizeof(factors) / sizeof(factors[0]); ++j)
                numeric(known, factors[j], UINT32_MAX);
            llg_gmp_sv4_destroy(&known);
            llg_gmp_sv4_destroy(&ones);
            llg_gmp_sv4_destroy(&a);
            llg_gmp_sv4_destroy(&b);
            llg_gmp_sv4_destroy(&mask);
        }
}
int main(int argc, char** argv) {
    if (argc != 2)
        return 2;
    if (!strcmp(argv[1], "small"))
        small();
    else if (!strcmp(argv[1], "wide"))
        wide();
    else
        return 2;
    printf("compact V06 bridge %s: %lu differential results plus independent oracles\n", argv[1],
           checks);
    return 0;
}
