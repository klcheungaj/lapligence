#include "compact_adapters_test.h"

static unsigned scalar_resolution(const unsigned* states, const uint8_t* s0,
                                    const uint8_t* s1, unsigned count, int mode) {
    int k0 = -1, k1 = -1, p0 = -1, p1 = -1;
    if (mode >= LLG_RESOLVE_TRI0) {
        int strength = mode <= LLG_RESOLVE_TRI1 ? LLG_STRENGTH_PULL : LLG_STRENGTH_SUPPLY;
        if (mode == LLG_RESOLVE_TRI0 || mode == LLG_RESOLVE_SUPPLY0)
            k0 = p0 = strength;
        else
            k1 = p1 = strength;
    }
    for (unsigned i = 0; i < count; ++i) {
        if ((states[i] == 0 || states[i] == 2) && s0[i]) {
            if (p0 < s0[i]) p0 = s0[i];
            if (states[i] == 0 && k0 < s0[i]) k0 = s0[i];
        }
        if ((states[i] == 1 || states[i] == 2) && s1[i]) {
            if (p1 < s1[i]) p1 = s1[i];
            if (states[i] == 1 && k1 < s1[i]) k1 = s1[i];
        }
    }
    if (p0 < 0 && p1 < 0) return 3;
    if (k0 > p1 || (mode == LLG_RESOLVE_WAND && k0 >= 0 && k0 == p1)) return 0;
    if (k1 > p0 || (mode == LLG_RESOLVE_WOR && k1 >= 0 && k1 == p0)) return 1;
    return 2;
}
static void exhaustive_strengths(void) {
    for (unsigned states = 0; states < 16; ++states) {
        unsigned st[2] = {states % 4, states / 4};
        g4_t v[2] = {llg_gmp_sv4_fill((uint8_t)st[0], 1, 0), llg_gmp_sv4_fill((uint8_t)st[1], 1, 0)};
        sv4_t old[2] = {sv4_fill((uint8_t)st[0], 1, 0), sv4_fill((uint8_t)st[1], 1, 0)};
        const g4_t* vp[2] = {&v[0], &v[1]};
        const sv4_t* op[2] = {&old[0], &old[1]};
        for (unsigned strength = 0; strength < 4096; ++strength) {
            uint8_t s0[2] = {(uint8_t)(strength & 7), (uint8_t)((strength >> 6) & 7)};
            uint8_t s1[2] = {(uint8_t)((strength >> 3) & 7), (uint8_t)((strength >> 9) & 7)};
            for (int mode = 0; mode < 7; ++mode) {
                g4_t r = llg_gmp_sv4_resolve_strengths(vp, s0, s1, 2, 1, 0, mode);
                CHECK(llg_gmp_sv4_state(r, 0) == scalar_resolution(st, s0, s1, 2, mode));
                compare(sv4_resolve_strengths(op, s0, s1, 2, 1, 0, mode), r);
            }
        }
        for (int mode = 0; mode < 7; ++mode)
            compare(sv4_resolve(op, 2, 1, 0, mode), llg_gmp_sv4_resolve(vp, 2, 1, 0, mode));
        sv4_destroy_array(old, 2);
    }
}
static void wide_nets(void) {
    uint64_t seed = UINT64_C(0x123ab87d665);
    for (size_t w = 0; w < sizeof(adapter_widths) / sizeof(adapter_widths[0]); ++w) {
        uint32_t width = adapter_widths[w];
        size_t n = ((size_t)width + 63u) / 64u;
        uint64_t* p = (uint64_t*)calloc(3u * (n ? n : 1u), 8u);
        CHECK(p);
        for (unsigned pattern = 0; pattern < 4; ++pattern) {
            for (size_t i = 0; i < n; ++i) {
                p[i] = next_word(&seed);
                p[n + i] = pattern >= 2 ? next_word(&seed) : 0;
                p[2u * n + i] = pattern >= 2 ? next_word(&seed) & ~p[n + i] : 0;
            }
            sv4_t old[3]; g4_t v[3];
            const sv4_t* op[4] = {&old[0], NULL, &old[1], &old[2]};
            const g4_t* vp[4] = {&v[0], NULL, &v[1], &v[2]};
            for (unsigned d = 0; d < 3; ++d) {
                uint32_t dw = d == 1 && width ? width - 1u : width;
                old[d] = sv4_from_limbs(p, p + n, p + 2u * n, dw, (int8_t)(pattern & 1));
                v[d] = llg_gmp_sv4_from_limbs(p, p + n, p + 2u * n, dw, (int8_t)(pattern & 1));
                if (d == 2 && width) {
                    llg_sv4_set_state(&old[d], 0, pattern);
                    llg_gmp_sv4_set_state(&v[d], 0, pattern);
                }
            }
            uint8_t s0[4] = {6, 3, 5, 7}, s1[4] = {3, 6, 7, 5};
            int indices[3] = {3, 1, 0};
            for (int mode = 0; mode < 7; ++mode) {
                compare(sv4_resolve(op, 4, width, 1, mode), llg_gmp_sv4_resolve(vp, 4, width, 1, mode));
                compare(sv4_resolve_strengths(op, s0, s1, 4, width, 0, mode),
                        llg_gmp_sv4_resolve_strengths(vp, s0, s1, 4, width, 0, mode));
                uint32_t offset = width ? (width > 65 ? 63 : width / 2) : 0;
                uint32_t rw = width - offset;
                for (unsigned strengths = 0; strengths < 2; ++strengths)
                    compare(sv4_resolve_strengths_range(op, strengths ? s0 : NULL, s1, indices,
                                3, width, offset, rw, 1, mode),
                            llg_gmp_sv4_resolve_strengths_range(vp, strengths ? s0 : NULL, s1,
                                indices, 3, width, offset, rw, 1, mode));
                compare(sv4_resolve_strengths_range(op, s0, s1, NULL, 0, width, width, 0, 0, mode),
                        llg_gmp_sv4_resolve_strengths_range(vp, s0, s1, NULL, 0, width, width, 0, 0, mode));
            }
            sv4_destroy_array(old, 3); llg_gmp_sv4_destroy_array(v, 3);
        }
        free(p);
    }
}
static void udp_and_enum(void) {
    unsigned masks[] = {1, 2, 4, 3, 7};
    for (unsigned i = 0; i < 5; ++i) for (unsigned j = 0; j < 5; ++j)
        for (unsigned states = 0; states < 16; ++states) {
            unsigned st[2] = {states % 4, states / 4};
            g4_t v[2] = {llg_gmp_sv4_fill((uint8_t)st[0], 1, 0), llg_gmp_sv4_fill((uint8_t)st[1], 1, 0)};
            sv4_t old[2] = {sv4_fill((uint8_t)st[0], 1, 0), sv4_fill((uint8_t)st[1], 1, 0)};
            const g4_t* vp[2] = {&v[0], &v[1]}; const sv4_t* op[2] = {&old[0], &old[1]};
            uint8_t rows[6] = {(uint8_t)masks[i], (uint8_t)masks[j], 1, 7, 7, 0};
            int match = (masks[i] & (1u << (st[0] >= 2 ? 2 : st[0]))) &&
                        (masks[j] & (1u << (st[1] >= 2 ? 2 : st[1])));
            for (size_t count = 0; count < 3; ++count) {
                g4_t r = llg_gmp_sv4_udp_eval(rows, count, 2, vp);
                CHECK(llg_gmp_sv4_state(r, 0) == (unsigned)(match && count ? 1 : count == 2 ? 0 : 2));
                compare(sv4_udp_eval(rows, count, 2, op), r);
            }
            sv4_destroy_array(old, 2);
        }
    for (unsigned sign = 0; sign < 2; ++sign) {
        g4_t v[4]; sv4_t old[4];
        unsigned members[4] = {7, 2, 7, 9};
        for (unsigned i = 0; i < 4; ++i) {
            v[i] = llg_gmp_sv4_from_u64(members[i], 129, (int8_t)sign);
            old[i] = sv4_from_u64(members[i], 129, (int8_t)sign);
        }
        for (unsigned current = 0; current < 12; ++current) for (unsigned step = 0; step < 10; ++step)
            for (int dir = -1; dir <= 1; ++dir) {
                g4_t c = llg_gmp_sv4_from_u64(current, 129, (int8_t)sign), s = llg_gmp_sv4_from_u64(step, 32, 0);
                sv4_t oc = sv4_from_u64(current, 129, (int8_t)sign), os = sv4_from_u64(step, 32, 0);
                unsigned expected_index = current == 7 ? 2u : current == 2 ? 1u : current == 9 ? 3u : 4u;
                if (expected_index < 4)
                    for (unsigned hop = 0; hop < step; ++hop)
                        expected_index = dir < 0 ? (expected_index ? expected_index - 1u : 3u)
                                                 : (expected_index == 3u ? 0u : expected_index + 1u);
                g4_t result = llg_gmp_sv4_enum_navigate(c, s, v, 4, v[1], dir);
                CHECK(llg_gmp_sv4_to_u64(result) == members[expected_index < 4 ? expected_index : 1u]);
                compare(sv4_enum_navigate(oc, os, old, 4, old[1], dir), result);
                compare(sv4_enum_navigate(oc, os, NULL, 0, old[1], dir),
                        llg_gmp_sv4_enum_navigate(c, s, NULL, 0, v[1], dir));
                sv4_destroy(&oc); sv4_destroy(&os); llg_gmp_sv4_destroy(&c);
            }
        g4_t x = llg_gmp_sv4_x(129, 0), step = llg_gmp_sv4_x(32, 0);
        g4_t r = llg_gmp_sv4_enum_navigate(v[0], step, v, 4, v[1], 1);
        CHECK(llg_gmp_sv4_same(r, v[2]));
        llg_gmp_sv4_set_state(&v[2], 0, 0);
        CHECK(llg_gmp_sv4_state(r, 0) == 1);
        llg_gmp_sv4_destroy(&r);
        r = llg_gmp_sv4_enum_navigate(x, step, v, 4, v[1], 1);
        CHECK(llg_gmp_sv4_same(r, v[1]));
        llg_gmp_sv4_destroy(&r); llg_gmp_sv4_destroy(&x);
        llg_gmp_sv4_destroy_array(v, 4); sv4_destroy_array(old, 4);
    }
}
int main(int argc, char** argv) {
    if (argc > 1) {
        g4_t v = llg_gmp_sv4_zero(65, 0); const g4_t* drivers[] = {&v};
        uint8_t s0[] = {8}, s1[] = {6}; int indices[] = {-1};
        if (!strcmp(argv[1], "strength"))
            v = llg_gmp_sv4_resolve_strengths(drivers, s0, s1, 1, 65, 0, 0);
        else if (!strcmp(argv[1], "index"))
            v = llg_gmp_sv4_resolve_strengths_range(drivers, s0, s1, indices, 1, 65, 0, 65, 0, 0);
        else
            v = llg_gmp_sv4_resolve_strengths_range(drivers, NULL, NULL, NULL, 1, 65, 66, 1, 0, 0);
        llg_gmp_sv4_destroy(&v); return 1;
    }
    exhaustive_strengths(); wide_nets(); udp_and_enum();
    printf("net/strength/UDP/enum: %zu checks passed\n", checks);
    return 0;
}
