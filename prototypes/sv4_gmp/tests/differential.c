#include "gmp4.h"
#include "../golden/llg_value.h"
#include "check.h"
#include <inttypes.h>
#include <string.h>

static uint64_t seed = UINT64_C(0x857cdf931be4a620);
static uint64_t checks;
static uint64_t random_word(void) {
    seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed;
}
static uint64_t mask(uint32_t width, size_t i) {
    size_t start = i * 64u;
    if (start >= width) return 0;
    size_t bits = width - start;
    return bits >= 64 ? UINT64_MAX : UINT64_MAX >> (64u - bits);
}
static void compare(const char *operation, sv4_t old, gmp4_t fresh) {
    if (old.width != fresh.width || old.is_signed != fresh.is_signed) {
        fprintf(stderr, "%s shape mismatch %u/%d vs %u/%d\n", operation,
                old.width, old.is_signed, fresh.width, fresh.is_signed); exit(1);
    }
    for (size_t i = 0; i < ((size_t)old.width + 63u) / 64u; ++i) {
        uint64_t expected[3] = { old.bits[i], old.x[i], old.z[i] };
        for (unsigned plane = 0; plane < 3; ++plane) {
            if (expected[plane] != gmp4_word(fresh, i, plane)) {
                fprintf(stderr, "%s width=%u word=%zu plane=%u old=%016" PRIx64
                        " new=%016" PRIx64 " seed=%016" PRIx64 "\n", operation,
                        old.width, i, plane, expected[plane],
                        gmp4_word(fresh, i, plane), seed); exit(1);
            }
        }
    }
    ++checks;
    sv4_destroy(&old); gmp4_destroy(&fresh);
}
static void input(uint32_t width, int8_t sign, int unknown, sv4_t *old, gmp4_t *fresh) {
    size_t n = ((size_t)width + 63u) / 64u;
    uint64_t *words = n ? calloc(3u * n, sizeof(uint64_t)) : NULL;
    CHECK(!n || words);
    uint64_t *x = n ? words + n : NULL, *z = n ? words + 2u * n : NULL;
    for (size_t i = 0; i < n; ++i) {
        if (unknown) { x[i] = random_word() & random_word(); z[i] = random_word() & ~x[i]; }
        words[i] = random_word() & ~(x[i] | z[i]);
        words[i] &= mask(width, i); x[i] &= mask(width, i); z[i] &= mask(width, i);
    }
    *old = sv4_from_limbs(words, x, z, width, sign);
    *fresh = gmp4_from_limbs(words, x, z, width, sign);
    free(words);
}
#define BINARY(name) compare(#name, sv4_##name(a,b), gmp4_##name(ga,gb))
#define UNARY(name) compare(#name, sv4_##name(a), gmp4_##name(ga))
static void pair(uint32_t wa, uint32_t wb, int8_t sa, int8_t sb, int unknown) {
    sv4_t a, b; gmp4_t ga, gb;
    input(wa, sa, unknown, &a, &ga); input(wb, sb, unknown, &b, &gb);
    compare("import", sv4_clone(&a), gmp4_clone(&ga));
    BINARY(add); BINARY(sub); BINARY(mul); BINARY(and); BINARY(or);
    BINARY(xor); BINARY(xnor); BINARY(eq); BINARY(case_eq);
    BINARY(logand); BINARY(logor); UNARY(neg); UNARY(bitneg); UNARY(lognot);
    UNARY(to_two_state);
    CHECK(sv4_is_unknown(a) == gmp4_is_unknown(ga));
    CHECK(sv4_to_bool(a) == gmp4_to_bool(ga));
    for (int8_t s = 0; s <= 1; ++s) {
        compare("cast", sv4_cast(a, wb, s), gmp4_cast(ga, wb, s));
        compare("resize", sv4_resize(a, wb, s), gmp4_resize(ga, wb, s));
    }
    for (unsigned state = 0; state < 4; ++state) {
        sv4_t sel = sv4_fill((uint8_t)state, 1, 0);
        gmp4_t gsel = gmp4_fill((uint8_t)state, 1, 0);
        compare("mux", sv4_mux(sel, a, b), gmp4_mux(gsel, ga, gb));
        sv4_destroy(&sel); gmp4_destroy(&gsel);
    }
    if (wa == wb) {
        const sv4_t *ds[3] = { &a, NULL, &b };
        const gmp4_t *gs[3] = { &ga, NULL, &gb };
        compare("wire", sv4_resolve(ds, 3, wa, sa, LLG_RESOLVE_WIRE),
                gmp4_resolve_wire(gs, 3, wa, sa));
    }
    gmp4_t reuse = gmp4_clone(&ga);
    gmp4_add_into(&reuse, reuse, gb);
    compare("add_into alias", sv4_add(a, b), reuse);
    reuse = gmp4_clone(&ga);
    gmp4_workspace_t scratch = GMP4_WORKSPACE_EMPTY;
    gmp4_mul_into(&reuse, reuse, gb, &scratch);
    compare("mul_into alias", sv4_mul(a, b), reuse);
    gmp4_workspace_destroy(&scratch);
    sv4_destroy(&a); sv4_destroy(&b); gmp4_destroy(&ga); gmp4_destroy(&gb);
}
static void independent_truth_tables(void) {
    static const unsigned and_table[4][4] = {{0,0,0,0},{0,1,2,2},{0,2,2,2},{0,2,2,2}};
    static const unsigned or_table[4][4] = {{0,1,2,2},{1,1,1,1},{2,1,2,2},{2,1,2,2}};
    static const unsigned wire_table[4][4] = {{0,2,2,0},{2,1,2,1},{2,2,2,2},{0,1,2,3}};
    for (unsigned x = 0; x < 4; ++x) for (unsigned y = 0; y < 4; ++y) {
        gmp4_t a = gmp4_fill((uint8_t)x, 1, 0), b = gmp4_fill((uint8_t)y, 1, 0);
        gmp4_t r = gmp4_and(a,b); CHECK(gmp4_get_bit(r,0) == and_table[x][y]); gmp4_destroy(&r);
        r = gmp4_or(a,b); CHECK(gmp4_get_bit(r,0) == or_table[x][y]); gmp4_destroy(&r);
        r = gmp4_xor(a,b); CHECK(gmp4_get_bit(r,0) == (x > 1 || y > 1 ? 2 : x ^ y)); gmp4_destroy(&r);
        const gmp4_t *drivers[2] = { &a, &b };
        r = gmp4_resolve_wire(drivers,2,1,0); CHECK(gmp4_get_bit(r,0) == wire_table[x][y]); gmp4_destroy(&r);
        gmp4_t sel = gmp4_x(1,0);
        r = gmp4_mux(sel,a,b); CHECK(gmp4_get_bit(r,0) == (x == y && x < 2 ? x : 2)); gmp4_destroy(&r);
        gmp4_destroy(&a); gmp4_destroy(&b); gmp4_destroy(&sel);
    }
    gmp4_t a = gmp4_from_masks(2,1,0,2,0), b = gmp4_from_u64(0,2,0);
    gmp4_t r = gmp4_eq(a,b); CHECK(gmp4_get_bit(r,0) == 0);
    CHECK(gmp4_to_bool(a)); gmp4_destroy(&r); gmp4_destroy(&a); gmp4_destroy(&b);
}
static void uniform_patterns(void) {
    static const uint32_t widths[] = {0,1,31,32,63,64,65,127,128,129,257,1024};
    for (size_t w = 0; w < sizeof(widths)/sizeof(widths[0]); ++w)
        for (unsigned state = 0; state < 4; ++state) for (int8_t sign=0; sign<2; ++sign) {
            uint32_t width=widths[w];
            sv4_t a=sv4_fill((uint8_t)state,width,sign); gmp4_t ga=gmp4_fill((uint8_t)state,width,sign);
            compare("fill",sv4_clone(&a),gmp4_clone(&ga));
            compare("extend fill",sv4_cast(a, width+67, !sign),gmp4_cast(ga,width+67,!sign));
            sv4_destroy(&a); gmp4_destroy(&ga);
            compare("from_i64",sv4_from_i64(-1,width),gmp4_from_i64(-1,width));
        }
    /* Exact modulo-width carry chain and signed mixed-width normalization. */
    for (size_t w=1; w<sizeof(widths)/sizeof(widths[0]); ++w) {
        uint32_t width=widths[w];
        gmp4_t ones=gmp4_fill(1,width,0), one=gmp4_from_u64(1,width,0);
        gmp4_t zero=gmp4_add(ones,one); CHECK(!gmp4_to_bool(zero));
        gmp4_destroy(&ones); gmp4_destroy(&one); gmp4_destroy(&zero);
    }
}
int main(void) {
    independent_truth_tables(); uniform_patterns();
    static const uint32_t widths[] = {0,1,2,7,8,15,31,32,33,63,64,65,127,128,129,255,256,257,1023,1024,4096};
    size_t nw = sizeof(widths)/sizeof(widths[0]);
    for (size_t a=0; a<nw; ++a) for (size_t b=0; b<nw; ++b)
        for (unsigned signs=0; signs<4; ++signs) for (int xz=0; xz<2; ++xz)
            pair(widths[a],widths[b],signs&1,(signs>>1)&1,xz);
    for (unsigned i=0; i<2500; ++i) {
        uint32_t a=(uint32_t)(random_word()%2050), b=(uint32_t)(random_word()%2050);
        unsigned s=(unsigned)random_word(); pair(a,b,s&1,(s>>1)&1,(s>>2)&1);
    }
    pair(16384,16384,1,1,0); pair(65535,65535,0,0,1);
    printf("differential result comparisons: %" PRIu64 "; independent truth tables PASS\n", checks);
    return 0;
}
