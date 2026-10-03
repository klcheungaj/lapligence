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
static void compare(sv4_t old, g4_t value) {
    CHECK(old.width == value.width && old.is_signed == value.is_signed);
    for (size_t i = 0; i < llg_sv4_words(old); ++i)
        for (unsigned p = 0; p < 3; ++p)
            CHECK(llg_sv4_word(old, i, p) == llg_gmp_sv4_word(value, i, p));
    CHECK(
        llg_gmp_sv4_bytes(&value) ==
        (value.width <= 64 ? 0 : 8u * llg_gmp_sv4_words(value) * (sv4_is_unknown(old) ? 2u : 1u)));
    if (value.width % 64) {
        llg_gmp_sv4_vpi_word_t top = llg_gmp_sv4_vpi_word(value, llg_gmp_sv4_words(value) - 1);
        CHECK(!((top.aval | top.bval) & ~LLG_GMP_MASK(value.width % 64)));
    }
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&value);
    ++checks;
}
static sv4_t legacy(g4_t value) {
    sv4_t result = sv4_zero(value.width, value.is_signed);
    for (size_t i = 0; i < llg_sv4_words(result); ++i) {
        llg_gmp_sv4_word_t word;
        llg_gmp_sv4_export_words(value, i, &word, 1);
        llg_sv4_set_word(&result, i, word.bits, word.x, word.z);
    }
    return result;
}
static g4_t states(uint32_t width, unsigned code) {
    g4_t v = llg_gmp_sv4_zero(width, 0);
    for (uint32_t i = 0; i < width; ++i) {
        llg_gmp_sv4_set_state(&v, i, code % 4);
        code /= 4;
    }
    return v;
}
static int state_at(g4_t v, int64_t i) {
    return i < 0 || i >= v.width ? 2 : llg_gmp_sv4_state(v, (uint32_t)i);
}
static void expect_state(g4_t v, uint32_t bit, int state) {
    CHECK(llg_gmp_sv4_state(v, bit) == (unsigned)state);
}
static void reads(g4_t v, sv4_t old) {
    int64_t bounds[] = {
        INT64_MIN, -4, -1, 0, 1, 63, 64, 65, (int64_t)v.width - 1, v.width, (int64_t)v.width + 3,
        INT64_MAX};
    for (size_t k = 0; k < sizeof(bounds) / sizeof(bounds[0]); ++k) {
        int64_t low = bounds[k];
        compare(sv4_bit_select(old, (uint64_t)low), llg_gmp_sv4_bit_select(v, (uint64_t)low));
        if (low > INT64_MAX - 6)
            continue;
        for (unsigned reverse = 0; reverse < 2; ++reverse) {
            int64_t left = reverse ? low : low + 6, right = reverse ? low + 6 : low;
            g4_t r = llg_gmp_sv4_part_select(v, left, right);
            for (uint32_t j = 0; j < 7; ++j)
                expect_state(r, j, state_at(v, low + (reverse ? 6 - j : j)));
            compare(sv4_part_select(old, left, right), r);
        }
        for (unsigned sign = 0; sign < 2; ++sign) {
            g4_t base = llg_gmp_sv4_from_i64(low, 65);
            sv4_t obase = sv4_from_i64(low, 65);
            g4_t r = llg_gmp_sv4_idx_part_select_value(v, base, 7, (int)sign);
            for (uint32_t j = 0; j < 7; ++j) {
                int state = sign && low < 0 ? 2 : state_at(v, low - (sign ? 6 : 0) + j);
                expect_state(r, j, state);
            }
            compare(sv4_idx_part_select_value(old, obase, 7, (int)sign), r);
            compare(sv4_idx_part_select(old, (uint64_t)low, 7, (int)sign),
                    llg_gmp_sv4_idx_part_select(v, (uint64_t)low, 7, (int)sign));
            sv4_destroy(&obase);
            llg_gmp_sv4_destroy(&base);
        }
    }
}
static void writes(g4_t v, sv4_t old, g4_t source) {
    sv4_t osource = legacy(source);
    for (int low = -3; low <= 3; ++low) {
        for (unsigned reverse = 0; reverse < 2; ++reverse) {
            int left = reverse ? low : low + 6, right = reverse ? low + 6 : low;
            g4_t target = llg_gmp_sv4_clone(&v);
            sv4_t otarget = sv4_clone(&old);
            llg_gmp_sv4_part_select_set(&target, left, right, source);
            sv4_part_select_set(&otarget, left, right, osource);
            for (uint32_t j = 0; j < v.width; ++j) {
                int state = state_at(v, j);
                if ((int64_t)j >= low && (int64_t)j < low + 7)
                    state =
                        state_at(source, (int64_t)source.width - 7 +
                                             (reverse ? 6 - ((int64_t)j - low) : (int64_t)j - low));
                expect_state(target, j, state);
            }
            compare(otarget, target);
        }
        for (int neg = 0; neg < 2; ++neg) {
            g4_t target = llg_gmp_sv4_clone(&v), base = llg_gmp_sv4_from_i64(low, 8);
            sv4_t otarget = sv4_clone(&old), obase = sv4_from_i64(low, 8);
            llg_gmp_sv4_idx_part_select_set_value(&target, base, 7, neg, source);
            sv4_idx_part_select_set_value(&otarget, obase, 7, neg, osource);
            int start = low - (neg ? 6 : 0);
            for (uint32_t j = 0; j < v.width; ++j) {
                int state = state_at(v, j);
                if (!(neg && low < 0) && (int64_t)j >= start && (int64_t)j < start + 7)
                    state = state_at(source, (int64_t)j - start);
                expect_state(target, j, state);
            }
            compare(otarget, target);
            target = llg_gmp_sv4_clone(&v);
            otarget = sv4_clone(&old);
            llg_gmp_sv4_idx_part_select_set(&target, (uint64_t)low, 7, neg, source);
            sv4_idx_part_select_set(&otarget, (uint64_t)low, 7, neg, osource);
            compare(otarget, target);
            llg_gmp_sv4_destroy(&base);
            sv4_destroy(&obase);
        }
        g4_t target = llg_gmp_sv4_clone(&v);
        sv4_t otarget = sv4_clone(&old);
        llg_gmp_sv4_bit_select_set(&target, (uint64_t)low, source);
        sv4_bit_select_set(&otarget, (uint64_t)low, osource);
        compare(otarget, target);
    }
    sv4_destroy(&osource);
}
static void streams(g4_t v, sv4_t old) {
    uint32_t slices[] = {1, 2, 3, 7, 63, 64, 65, v.width, v.width + 1};
    for (size_t s = 0; s < sizeof(slices) / sizeof(slices[0]); ++s) {
        uint32_t slice = slices[s];
        if (!slice)
            continue;
        for (int reverse = 0; reverse < 2; ++reverse) {
            g4_t r = llg_gmp_sv4_stream(v, slice, reverse);
            for (uint32_t bit = 0; bit < v.width; ++bit) {
                uint32_t start = (bit / slice) * slice;
                uint32_t take = v.width - start < slice ? v.width - start : slice;
                uint32_t dst = reverse ? v.width - start - take + bit - start : bit;
                expect_state(r, dst, state_at(v, bit));
            }
            g4_t restored = llg_gmp_sv4_unstream(r, slice, reverse);
            for (uint32_t bit = 0; bit < v.width; ++bit)
                expect_state(restored, bit, state_at(v, bit));
            compare(sv4_stream(old, slice, reverse), r);
            sv4_t unsigned_old = sv4_clone(&old);
            unsigned_old.is_signed = 0;
            compare(unsigned_old, restored);
            compare(sv4_unstream(old, slice, reverse), llg_gmp_sv4_unstream(v, slice, reverse));
        }
    }
}
static void plan_compare(sv4_select_plan_t old, llg_gmp_sv4_select_plan_t p) {
    CHECK(old.storage_width == p.storage_width && old.width == p.width &&
          old.storage_lsb == p.storage_lsb && old.value_lsb == p.value_lsb && old.count == p.count);
}
static void plans(g4_t v, sv4_t old) {
    int64_t lows[] = {-80, -3, 0, 2, 63, 64, 65, (int64_t)v.width - 2, INT64_MIN, INT64_MAX};
    for (size_t k = 0; k < sizeof(lows) / sizeof(lows[0]); ++k) {
        g4_t base = llg_gmp_sv4_from_i64(lows[k], 65);
        sv4_t obase = sv4_from_i64(lows[k], 65);
        llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_indexed(v.width, base, 71, 0);
        sv4_select_plan_t op = sv4_select_plan_indexed(v.width, obase, 71, 0);
        plan_compare(op, p);
        int64_t map[71];
        for (uint32_t j = 0; j < 71; ++j)
            map[j] =
                lows[k] >= -71 && lows[k] < v.width && lows[k] + j >= 0 && lows[k] + j < v.width
                    ? lows[k] + j
                    : -1;
        for (unsigned step = 0; step < 3; ++step) {
            g4_t result = llg_gmp_sv4_select_plan_read(v, &p);
            for (uint32_t j = 0; j < p.width; ++j)
                expect_state(result, j, state_at(v, map[j]));
            compare(sv4_select_plan_read(old, &op), result);
            g4_t source = llg_gmp_sv4_resize(v, p.width, 0);
            sv4_t osource = sv4_resize(old, op.width, 0);
            for (int reverse = 0; reverse < 2; ++reverse) {
                g4_t slice = llg_gmp_sv4_select_plan_slice(source, &p, reverse);
                for (uint32_t j = 0; j < p.count; ++j)
                    expect_state(slice, j,
                                 state_at(source, reverse ? p.width - 1 - p.value_lsb - j
                                                          : p.value_lsb + j));
                compare(sv4_select_plan_slice(osource, &op, reverse), slice);
            }
            g4_t target = llg_gmp_sv4_clone(&v);
            sv4_t otarget = sv4_clone(&old);
            llg_gmp_sv4_select_plan_set(&target, &p, source);
            sv4_select_plan_set(&otarget, &op, osource);
            uint32_t cursor = 0;
            while (cursor < p.width && map[cursor] < 0)
                ++cursor;
            for (uint32_t j = 0; j < v.width; ++j) {
                int state = state_at(v, j);
                if (cursor < p.width && map[cursor] == j)
                    state = state_at(source, cursor++);
                expect_state(target, j, state);
            }
            compare(otarget, target);
            llg_gmp_sv4_destroy(&source);
            sv4_destroy(&osource);
            int64_t next = step == 0 ? -2 : 3;
            g4_t index = llg_gmp_sv4_from_i64(next, 8);
            sv4_t oindex = sv4_from_i64(next, 8);
            int64_t next_map[71];
            for (uint32_t j = 0; j < 71; ++j)
                next_map[j] = next + j >= 0 && next + j < p.width ? map[next + j] : -1;
            memcpy(map, next_map, sizeof(map));
            llg_gmp_sv4_select_plan_step(&p, index, 71);
            sv4_select_plan_step(&op, oindex, 71);
            plan_compare(op, p);
            llg_gmp_sv4_destroy(&index);
            sv4_destroy(&oindex);
        }
        for (int neg = 0; neg < 2; ++neg)
            plan_compare(sv4_select_plan_indexed(v.width, obase, 71, neg),
                         llg_gmp_sv4_select_plan_indexed(v.width, base, 71, neg));
        llg_gmp_sv4_destroy(&base);
        sv4_destroy(&obase);
    }
    plan_compare(sv4_select_plan_init(v.width), llg_gmp_sv4_select_plan_init(v.width));
    plan_compare(sv4_select_plan_bit(v.width, UINT64_MAX),
                 llg_gmp_sv4_select_plan_bit(v.width, UINT64_MAX));
    plan_compare(sv4_select_plan_part(v.width, -2, 71),
                 llg_gmp_sv4_select_plan_part(v.width, -2, 71));
    plan_compare(sv4_select_plan_part(v.width, 71, -2),
                 llg_gmp_sv4_select_plan_part(v.width, 71, -2));
}
static void aliases(g4_t v, sv4_t old) {
    for (int reverse = 0; reverse < 2; ++reverse) {
        g4_t t = llg_gmp_sv4_clone(&v);
        sv4_t ot = sv4_clone(&old);
        int64_t left = reverse ? 0 : v.width - 1, right = reverse ? v.width - 1 : 0;
        llg_gmp_sv4_part_select_set(&t, left, right, t);
        sv4_part_select_set(&ot, left, right, ot);
        compare(ot, t);
    }
    g4_t t = llg_gmp_sv4_clone(&v);
    sv4_t ot = sv4_clone(&old);
    llg_gmp_sv4_idx_part_select_set(&t, 3, v.width, 0, t);
    sv4_idx_part_select_set(&ot, 3, old.width, 0, ot);
    compare(ot, t);
    g4_t base = llg_gmp_sv4_from_i64(-3, 8);
    sv4_t obase = sv4_from_i64(-3, 8);
    t = llg_gmp_sv4_clone(&v);
    ot = sv4_clone(&old);
    llg_gmp_sv4_idx_part_select_set_value(&t, base, v.width, 0, t);
    sv4_idx_part_select_set_value(&ot, obase, old.width, 0, ot);
    compare(ot, t);
    llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_indexed(v.width, base, v.width, 0);
    sv4_select_plan_t op = sv4_select_plan_indexed(old.width, obase, old.width, 0);
    t = llg_gmp_sv4_clone(&v);
    ot = sv4_clone(&old);
    llg_gmp_sv4_select_plan_set(&t, &p, t);
    sv4_select_plan_set(&ot, &op, ot);
    compare(ot, t);
    sv4_destroy(&obase);
    llg_gmp_sv4_destroy(&base);
}
static void array_merge(g4_t a, g4_t b, uint32_t stride, unsigned default_code) {
    g4_t def = llg_gmp_sv4_fill((uint8_t)default_code, stride, 0);
    sv4_t oa = legacy(a), ob = legacy(b), od = legacy(def);
    g4_t r = llg_gmp_sv4_array_conditional_merge(a, b, def);
    for (uint32_t start = 0; start < a.width; start += stride) {
        int equal = 1;
        for (uint32_t j = 0; j < stride; ++j) {
            int aa = state_at(a, start + j), bb = state_at(b, start + j);
            equal &= aa < 2 && bb < 2 && aa == bb;
        }
        for (uint32_t j = 0; j < stride; ++j)
            expect_state(r, start + j, equal ? state_at(a, start + j) : (int)default_code);
    }
    compare(sv4_array_conditional_merge(oa, ob, od), r);
    sv4_destroy(&oa);
    sv4_destroy(&ob);
    sv4_destroy(&od);
    llg_gmp_sv4_destroy(&def);
}
static void exhaustive(void) {
    for (uint32_t w = 1; w <= 4; ++w)
        for (unsigned code = 0; code < (1u << (2 * w)); ++code) {
            g4_t v = states(w, code);
            sv4_t old = legacy(v);
            reads(v, old);
            streams(v, old);
            aliases(v, old);
            for (unsigned state = 0; state < 4; ++state) {
                g4_t source = llg_gmp_sv4_fill((uint8_t)state, 3, 0);
                writes(v, old, source);
                llg_gmp_sv4_destroy(&source);
            }
            for (uint64_t n = 0; n <= 3; ++n) {
                g4_t r = llg_gmp_sv4_repeat(v, n);
                for (uint32_t j = 0; j < r.width; ++j)
                    expect_state(r, j, state_at(v, j % w));
                compare(sv4_repeat(old, n), r);
            }
            for (int sign = 0; sign < 2; ++sign) {
                v.is_signed = old.is_signed = (int8_t)sign;
                compare(sv4_repeat_count(old), llg_gmp_sv4_repeat_count(v));
            }
            llg_gmp_sv4_destroy(&v);
            sv4_destroy(&old);
        }
    for (unsigned a = 0; a < 256; ++a)
        for (unsigned b = 0; b < 256; ++b) {
            g4_t va = states(4, a), vb = states(4, b);
            for (uint32_t stride = 1; stride <= 4; stride *= 2)
                for (unsigned def = 0; def < 4; ++def)
                    array_merge(va, vb, stride, def);
            llg_gmp_sv4_destroy(&va);
            llg_gmp_sv4_destroy(&vb);
        }
    for (uint32_t aw = 0; aw <= 3; ++aw)
        for (uint32_t bw = 0; bw <= 3; ++bw)
            for (unsigned a = 0; a < (1u << (2 * aw)); ++a)
                for (unsigned b = 0; b < (1u << (2 * bw)); ++b) {
                    g4_t va = states(aw, a), vb = states(bw, b), r = llg_gmp_sv4_concat(va, vb);
                    sv4_t oa = legacy(va), ob = legacy(vb);
                    for (uint32_t j = 0; j < r.width; ++j)
                        expect_state(r, j, j < bw ? state_at(vb, j) : state_at(va, j - bw));
                    compare(sv4_concat(oa, ob), r);
                    sv4_destroy(&oa);
                    sv4_destroy(&ob);
                    llg_gmp_sv4_destroy(&va);
                    llg_gmp_sv4_destroy(&vb);
                }
}
static void invalid_bases(void) {
    g4_t value = llg_gmp_sv4_from_u64(15, 65, 0);
    sv4_t old = legacy(value);
    g4_t bases[] = {llg_gmp_sv4_fill(2, 65, 0), llg_gmp_sv4_fill(3, 65, 1),
                    llg_gmp_sv4_from_u64(UINT64_MAX, 64, 0), llg_gmp_sv4_zero(129, 0),
                    llg_gmp_sv4_fill(1, 129, 1)};
    llg_gmp_sv4_set_state(&bases[3], 100, 1);
    for (size_t i = 0; i < sizeof(bases) / sizeof(bases[0]); ++i) {
        sv4_t ob = legacy(bases[i]);
        for (int neg = 0; neg < 2; ++neg) {
            compare(sv4_idx_part_select_value(old, ob, 7, neg),
                    llg_gmp_sv4_idx_part_select_value(value, bases[i], 7, neg));
            llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_indexed(65, bases[i], 7, neg);
            sv4_select_plan_t op = sv4_select_plan_indexed(65, ob, 7, neg);
            plan_compare(op, p);
            compare(sv4_select_plan_read(old, &op), llg_gmp_sv4_select_plan_read(value, &p));
            g4_t t = llg_gmp_sv4_clone(&value);
            sv4_t ot = sv4_clone(&old);
            llg_gmp_sv4_idx_part_select_set_value(&t, bases[i], 7, neg, value);
            sv4_idx_part_select_set_value(&ot, ob, 7, neg, old);
            compare(ot, t);
        }
        llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_init(65);
        sv4_select_plan_t op = sv4_select_plan_init(65);
        llg_gmp_sv4_select_plan_step(&p, bases[i], 7);
        sv4_select_plan_step(&op, ob, 7);
        plan_compare(op, p);
        sv4_destroy(&ob);
        llg_gmp_sv4_destroy(&bases[i]);
    }
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&value);
}
static void small_indices_and_empty(void) {
    g4_t v = states(4, 228);
    sv4_t old = legacy(v);
    for (uint32_t w = 0; w <= 4; ++w)
        for (unsigned code = 0; code < (1u << (2 * w)); ++code)
            for (int sign = 0; sign < 2; ++sign)
                for (int neg = 0; neg < 2; ++neg) {
                    g4_t base = states(w, code);
                    base.is_signed = (int8_t)sign;
                    sv4_t obase = legacy(base);
                    int known = 1, number = 0;
                    for (uint32_t j = 0; j < w; ++j) {
                        int bit = (int)((code >> (2 * j)) & 3);
                        known &= bit < 2;
                        if (bit == 1)
                            number += 1 << j;
                    }
                    if (sign && w && ((code >> (2 * (w - 1))) & 3) == 1)
                        number -= 1 << w;
                    g4_t r = llg_gmp_sv4_idx_part_select_value(v, base, 5, neg);
                    for (uint32_t j = 0; j < 5; ++j)
                        expect_state(r, j,
                                     known && !(neg && number < 0)
                                         ? state_at(v, (int64_t)number - (neg ? 4 : 0) + j)
                                         : 2);
                    compare(sv4_idx_part_select_value(old, obase, 5, neg), r);
                    llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_indexed(4, base, 5, neg);
                    sv4_select_plan_t op = sv4_select_plan_indexed(4, obase, 5, neg);
                    plan_compare(op, p);
                    compare(sv4_select_plan_read(old, &op), llg_gmp_sv4_select_plan_read(v, &p));
                    llg_gmp_sv4_destroy(&base);
                    sv4_destroy(&obase);
                }
    g4_t empty = LLG_GMP_SV4_EMPTY;
    sv4_t oe = SV4_EMPTY;
    for (int64_t left = -2; left <= 2; ++left) {
        g4_t target = LLG_GMP_SV4_EMPTY;
        sv4_t otarget = SV4_EMPTY;
        llg_gmp_sv4_part_select_set(&target, left, left + 4, v);
        sv4_part_select_set(&otarget, left, left + 4, old);
        compare(otarget, target);
        target = llg_gmp_sv4_clone(&v);
        otarget = sv4_clone(&old);
        llg_gmp_sv4_part_select_set(&target, left, left + 4, empty);
        sv4_part_select_set(&otarget, left, left + 4, oe);
        for (uint32_t bit = 0; bit < target.width; ++bit)
            expect_state(target, bit,
                         (int64_t)bit >= left && (int64_t)bit <= left + 4 ? 2 : state_at(v, bit));
        compare(otarget, target);
    }
    compare(sv4_part_select(oe, 5, -2), llg_gmp_sv4_part_select(empty, 5, -2));
    compare(sv4_idx_part_select(old, 0, 0, 0), llg_gmp_sv4_idx_part_select(v, 0, 0, 0));
    compare(sv4_repeat(oe, UINT64_MAX), llg_gmp_sv4_repeat(empty, UINT64_MAX));
    compare(sv4_repeat_count(oe), llg_gmp_sv4_repeat_count(empty));
    compare(sv4_stream(oe, 7, 1), llg_gmp_sv4_stream(empty, 7, 1));
    compare(sv4_unstream(oe, 7, 1), llg_gmp_sv4_unstream(empty, 7, 1));
    g4_t index = llg_gmp_sv4_x(65, 0);
    llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_init(4);
    llg_gmp_sv4_select_plan_step(&p, index, 7);
    CHECK(!p.count);
    llg_gmp_sv4_destroy(&index);
    index = llg_gmp_sv4_zero(8, 0);
    llg_gmp_sv4_select_plan_step(&p, index, 4);
    CHECK(!p.count);
    g4_t r = llg_gmp_sv4_select_plan_read(v, &p);
    for (uint32_t j = 0; j < r.width; ++j)
        expect_state(r, j, 2);
    llg_gmp_sv4_select_plan_set(&v, &p, v);
    compare(sv4_clone(&old), llg_gmp_sv4_clone(&v));
    llg_gmp_sv4_destroy(&r);
    llg_gmp_sv4_destroy(&index);
    llg_gmp_sv4_destroy(&v);
    sv4_destroy(&old);
}
static uint64_t seed = UINT64_C(0x621547923bcda985);
static uint64_t random_word(void) {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}
static void word_boundaries(void) {
    uint32_t widths[] = {63, 64, 65, 127, 128, 129, 191, 256, 257};
    uint32_t lengths[] = {1, 7, 31, 63, 64, 65, 127, 128, 129, 193};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (unsigned unknown = 0; unknown < 2; ++unknown) {
            g4_t value = llg_gmp_sv4_zero(widths[k], 0);
            for (size_t j = 0; j < llg_gmp_sv4_words(value); ++j) {
                uint64_t b = unknown ? random_word() : 0;
                llg_gmp_sv4_set_word(&value, j, random_word() & ~b,
                                     b & UINT64_C(0x5555555555555555),
                                     b & UINT64_C(0xaaaaaaaaaaaaaaaa));
            }
            sv4_t old = legacy(value);
            for (uint32_t slice = 1; slice <= 130; ++slice) {
                g4_t streamed = llg_gmp_sv4_stream(value, slice, 1);
                g4_t unstreamed = llg_gmp_sv4_unstream(value, slice, 1);
                for (uint32_t bit = 0; bit < value.width; ++bit) {
                    uint32_t start = bit / slice * slice;
                    uint32_t count = value.width - start < slice ? value.width - start : slice;
                    uint32_t other = value.width - start - count + bit - start;
                    expect_state(streamed, other, state_at(value, bit));
                    expect_state(unstreamed, bit, state_at(value, other));
                }
                compare(sv4_stream(old, slice, 1), streamed);
                compare(sv4_unstream(old, slice, 1), unstreamed);
            }
            for (int64_t left = -65; left <= 130; ++left)
                for (size_t n = 0; n < sizeof(lengths) / sizeof(lengths[0]); ++n) {
                    uint32_t length = lengths[n];
                    int64_t right = left + length - 1;
                    g4_t selected = llg_gmp_sv4_part_select(value, left, right);
                    for (uint32_t bit = 0; bit < length; ++bit)
                        expect_state(selected, bit, state_at(value, right - bit));
                    compare(sv4_part_select(old, left, right), selected);
                    g4_t target = llg_gmp_sv4_clone(&value);
                    sv4_t otarget = sv4_clone(&old);
                    llg_gmp_sv4_part_select_set(&target, left, right, target);
                    sv4_part_select_set(&otarget, left, right, otarget);
                    for (uint32_t bit = 0; bit < target.width; ++bit)
                        expect_state(
                            target, bit,
                            (int64_t)bit >= left && (int64_t)bit <= right
                                ? state_at(value, (int64_t)value.width - 1 - ((int64_t)bit - left))
                                : state_at(value, bit));
                    compare(otarget, target);
                }
            sv4_destroy(&old);
            llg_gmp_sv4_destroy(&value);
        }
}
static void wide(void) {
    uint32_t widths[] = {1,    2,    7,    8,    31,    32,
                         33,   63,   64,   65,   127,   128,
                         129,  255,  256,  257,  1023,  1024,
                         4096, 8191, 8192, 8193, 16384, LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (int unknown = 0; unknown < 2; ++unknown) {
            uint32_t w = widths[k];
            g4_t v = llg_gmp_sv4_zero(w, 1);
            for (size_t j = 0; j < llg_gmp_sv4_words(v); ++j) {
                uint64_t b = unknown ? random_word() : 0;
                llg_gmp_sv4_set_word(&v, j, random_word() & ~b, b & UINT64_C(0x5555555555555555),
                                     b & UINT64_C(0xaaaaaaaaaaaaaaaa));
            }
            sv4_t old = legacy(v);
            reads(v, old);
            streams(v, old);
            plans(v, old);
            aliases(v, old);
            g4_t src = llg_gmp_sv4_from_masks(0x91, 0x44, 0x20, 8, 0);
            writes(v, old, src);
            llg_gmp_sv4_destroy(&src);
            compare(sv4_repeat(old, 0), llg_gmp_sv4_repeat(v, 0));
            if (w < LLG_GMP_SUPPORTED_WIDTH_LIMIT / 3)
                compare(sv4_repeat(old, 3), llg_gmp_sv4_repeat(v, 3));
            g4_t tail = llg_gmp_sv4_from_masks(3, 4, 8, 7, 1);
            sv4_t otail = legacy(tail);
            if (w + 7 < LLG_GMP_SUPPORTED_WIDTH_LIMIT)
                compare(sv4_concat(old, otail), llg_gmp_sv4_concat(v, tail));
            compare(sv4_repeat_count(old), llg_gmp_sv4_repeat_count(v));
            for (unsigned def = 0; def < 4; ++def) {
                array_merge(v, v, w, def);
                array_merge(v, v, 1, def);
            }
            llg_gmp_sv4_destroy(&tail);
            sv4_destroy(&otail);
            llg_gmp_sv4_destroy(&v);
            sv4_destroy(&old);
        }
}
static g4_t retained_read(const void* p) { return llg_gmp_sv4_clone((const g4_t*)p); }
static sv4_t old_retained_read(const void* p) { return sv4_clone((const sv4_t*)p); }
static g4_t queue_read(const llg_queue_t* queue, uint64_t identity) {
    (void)queue;
    return llg_gmp_sv4_from_u64(identity, 65, 1);
}
static sv4_t old_queue_read(const llg_queue_t* queue, uint64_t identity) {
    (void)queue;
    return sv4_from_u64(identity, 65, 1);
}
static void references(void) {
    g4_t v = llg_gmp_sv4_from_masks(0xa1, 4, 8, 8, 1);
    sv4_t old = legacy(v);
    llg_gmp_ref_t ref = {0};
    llg_ref_t oref = {0};
    ref.base = &v;
    oref.base = &old;
    ref.width = oref.width = 8;
    compare(llg_ref_read(NULL), llg_gmp_ref_read(NULL));
    for (uint8_t kind = 0; kind <= 6; ++kind)
        for (unsigned invalid = 0; invalid < 2; ++invalid)
            for (uint8_t two = 0; two < 2; ++two) {
                ref.kind = oref.kind = kind;
                ref.two_state = oref.two_state = two;
                ref.index = oref.index = invalid ? UINT64_MAX : 0;
                ref.array_size = oref.array_size = 1;
                ref.left = oref.left = 7;
                ref.right = oref.right = -2;
                ref.indexed_width = oref.indexed_width = 10;
                llg_gmp_sv4_select_plan_t plan = llg_gmp_sv4_select_plan_bit(8, ref.index);
                sv4_select_plan_t op = sv4_select_plan_bit(8, oref.index);
                ref.retained = &plan;
                oref.retained = &op;
                if (kind == LLG_REF_PACKED_PLAN)
                    ref.width = oref.width = 1;
                else
                    ref.width = oref.width = 8;
                compare(llg_ref_read(&oref), llg_gmp_ref_read(&ref));
            }
    ref.kind = oref.kind = LLG_REF_QUEUE;
    ref.width = oref.width = 71;
    ref.queue_identity = oref.queue_identity = 47;
    ref.queue_read = queue_read;
    oref.queue_read = old_queue_read;
    compare(llg_ref_read(&oref), llg_gmp_ref_read(&ref));
    ref.retained = &v;
    oref.retained = &old;
    ref.retained_read = retained_read;
    oref.retained_read = old_retained_read;
    compare(llg_ref_read(&oref), llg_gmp_ref_read(&ref));
    ref = (llg_gmp_ref_t){0};
    oref = (llg_ref_t){0};
    ref.width = oref.width = 8;
    ref.base = &v;
    oref.base = &old;
    llg_gmp_ref_t* parts[] = {&ref, &ref};
    llg_ref_t* oparts[] = {&oref, &oref};
    llg_gmp_ref_composite_t composite = {2, parts};
    llg_ref_composite_t oc = {2, oparts};
    llg_gmp_ref_t cref = {0};
    llg_ref_t ocref = {0};
    cref.kind = ocref.kind = LLG_REF_COMPOSITE;
    cref.width = ocref.width = 16;
    cref.retained = &composite;
    ocref.retained = &oc;
    compare(llg_ref_read(&ocref), llg_gmp_ref_read(&cref));
    llg_gmp_ref_tag_check_t check = {llg_gmp_sv4_select_plan_init(8), 3, 5, "member"};
    llg_ref_tag_check_t oc_check = {sv4_select_plan_init(8), 3, 5, "member"};
    llg_gmp_ref_view_t view = {&ref, llg_gmp_sv4_select_plan_part(8, 4, 0), 1, &check, "test"};
    llg_ref_view_t ov = {&oref, sv4_select_plan_part(8, 4, 0), 1, &oc_check, "test"};
    llg_gmp_ref_t vref = {0};
    llg_ref_t ovref = {0};
    vref.retained = &view;
    ovref.retained = &ov;
    vref.width = ovref.width = 5;
    for (unsigned tagged = 0; tagged < 2; ++tagged)
        for (unsigned match = 0; match < 3; ++match) {
            vref.kind = ovref.kind = tagged ? LLG_REF_TAGGED_VIEW : LLG_REF_VIEW;
            check.member_index = oc_check.member_index = match == 0 ? 5 : 4;
            if (match == 2) {
                llg_gmp_sv4_set_state(&v, 7, 2);
                llg_sv4_set_state(&old, 7, 2);
            }
            size_t failed = 99, ofailed = 99;
            CHECK(llg_gmp_ref_view_valid(&view, &v, &failed) ==
                  llg_ref_view_valid(&ov, &old, &ofailed));
            CHECK(failed == ofailed);
            for (uint8_t two = 0; two < 2; ++two) {
                vref.two_state = ovref.two_state = two;
                compare(llg_ref_read(&ovref), llg_gmp_ref_read(&vref));
            }
        }
    CHECK(!llg_gmp_ref_view_valid(NULL, &v, NULL));
    check.tag_width = oc_check.tag_width = 9;
    CHECK(!llg_gmp_ref_view_valid(&view, &v, NULL));
    ref.base = NULL;
    oref.base = NULL;
    compare(llg_ref_read(&oref), llg_gmp_ref_read(&ref));
    llg_gmp_sv4_destroy(&v);
    sv4_destroy(&old);
}
static void stream_padding(void) {
    g4_t source = llg_gmp_sv4_fill(1, 67, 1);
    llg_gmp_sv4_set_state(&source, 1, 2);
    llg_gmp_sv4_set_state(&source, 65, 3);
    sv4_t old = legacy(source);
    uint32_t widths[] = {1, 31, 63, 64, 65, 66, 67, 68, 129, 256};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k) {
        uint32_t width = widths[k];
        g4_t padded = llg_gmp_sv4_resize(source, width, 0);
        sv4_t opadded = sv4_resize(old, width, 0);
        for (int reverse = 0; reverse < 2; ++reverse) {
            g4_t result = llg_gmp_sv4_unstream(padded, 7, reverse);
            for (uint32_t bit = 0; bit < width;) {
                uint32_t count = width - bit < 7 ? width - bit : 7;
                uint32_t other = width - bit - count;
                for (uint32_t j = 0; j < count; ++j) {
                    uint32_t input = reverse ? other + j : bit + j;
                    expect_state(result, bit + j,
                                 input < source.width ? state_at(source, input) : 0);
                }
                bit += count;
            }
            compare(sv4_unstream(opadded, 7, reverse), result);
        }
        llg_gmp_sv4_destroy(&padded);
        sv4_destroy(&opadded);
    }
    llg_gmp_sv4_destroy(&source);
    sv4_destroy(&old);
}
static void array_boundaries(void) {
    uint32_t strides[] = {2, 3, 7, 31, 63, 64, 65, 127, 129};
    for (size_t k = 0; k < sizeof(strides) / sizeof(strides[0]); ++k) {
        uint32_t stride = strides[k], width = 3 * stride;
        g4_t a = llg_gmp_sv4_fill(1, width, 1), b = llg_gmp_sv4_clone(&a),
             def = llg_gmp_sv4_zero(stride, 1);
        for (uint32_t bit = 0; bit < stride; ++bit)
            llg_gmp_sv4_set_state(&def, bit, bit % 4);
        llg_gmp_sv4_set_state(&b, stride + 1, 0);
        llg_gmp_sv4_set_state(&a, 2 * stride, 3);
        llg_gmp_sv4_set_state(&b, 2 * stride, 3);
        sv4_t oa = legacy(a), ob = legacy(b), od = legacy(def);
        g4_t r = llg_gmp_sv4_array_conditional_merge(a, b, def);
        for (uint32_t bit = 0; bit < width; ++bit)
            expect_state(r, bit, bit < stride ? 1 : state_at(def, bit % stride));
        compare(sv4_array_conditional_merge(oa, ob, od), r);
        sv4_destroy(&oa);
        sv4_destroy(&ob);
        sv4_destroy(&od);
        llg_gmp_sv4_destroy(&a);
        llg_gmp_sv4_destroy(&b);
        llg_gmp_sv4_destroy(&def);
    }
}
static void result_ownership(void) {
    g4_t v = llg_gmp_sv4_fill(1, 129, 1), base = llg_gmp_sv4_zero(65, 0);
    llg_gmp_sv4_set_state(&v, 1, 2);
    llg_gmp_sv4_set_state(&v, 100, 3);
    sv4_t old = legacy(v), obase = legacy(base);
    llg_gmp_sv4_select_plan_t p = llg_gmp_sv4_select_plan_init(129);
    sv4_select_plan_t op = sv4_select_plan_init(129);
    llg_gmp_ref_t ref = {0};
    llg_ref_t oref = {0};
    ref.base = &v;
    oref.base = &old;
    ref.width = oref.width = 129;
    ref.is_signed = oref.is_signed = 1;
    g4_t results[] = {llg_gmp_sv4_part_select(v, 128, 1),
                      llg_gmp_sv4_idx_part_select_value(v, base, 129, 0),
                      llg_gmp_sv4_select_plan_read(v, &p),
                      llg_gmp_sv4_select_plan_slice(v, &p, 1),
                      llg_gmp_sv4_stream(v, 7, 1),
                      llg_gmp_sv4_unstream(v, 7, 1),
                      llg_gmp_sv4_concat(v, v),
                      llg_gmp_sv4_repeat(v, 3),
                      llg_gmp_sv4_repeat_count(v),
                      llg_gmp_sv4_array_conditional_merge(v, v, v),
                      llg_gmp_ref_read(&ref)};
    sv4_t expected[] = {sv4_part_select(old, 128, 1),
                        sv4_idx_part_select_value(old, obase, 129, 0),
                        sv4_select_plan_read(old, &op),
                        sv4_select_plan_slice(old, &op, 1),
                        sv4_stream(old, 7, 1),
                        sv4_unstream(old, 7, 1),
                        sv4_concat(old, old),
                        sv4_repeat(old, 3),
                        sv4_repeat_count(old),
                        sv4_array_conditional_merge(old, old, old),
                        llg_ref_read(&oref)};
    for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); ++i)
        CHECK(results[i].width <= 64 || results[i].data.wide.a != v.data.wide.a);
    llg_gmp_sv4_set_state(&v, 1, 0);
    llg_gmp_sv4_set_state(&v, 100, 0);
    llg_gmp_sv4_destroy(&v);
    llg_gmp_sv4_destroy(&base);
    sv4_destroy(&old);
    sv4_destroy(&obase);
    for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); ++i)
        compare(expected[i], results[i]);
}
static void reject(const char* mode) {
    g4_t v = llg_gmp_sv4_zero(65, 0), source = llg_gmp_sv4_zero(7, 0);
    llg_gmp_sv4_select_plan_t plan = llg_gmp_sv4_select_plan_init(65);
    if (!strcmp(mode, "concat")) {
        g4_t big = llg_gmp_sv4_zero(LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1, 0);
        v = llg_gmp_sv4_concat(big, source);
    } else if (!strcmp(mode, "repeat-overflow"))
        v = llg_gmp_sv4_repeat(v, UINT64_MAX);
    else if (!strcmp(mode, "repeat-limit"))
        v = llg_gmp_sv4_repeat(v, LLG_GMP_SUPPORTED_WIDTH_LIMIT);
    else if (!strcmp(mode, "part-limit"))
        v = llg_gmp_sv4_part_select(v, INT64_MIN, INT64_MAX);
    else if (!strcmp(mode, "stream-zero"))
        v = llg_gmp_sv4_stream(v, 0, 1);
    else if (!strcmp(mode, "plan-zero"))
        plan = llg_gmp_sv4_select_plan_init(0);
    else if (!strcmp(mode, "plan-indexed-zero")) {
        g4_t base = llg_gmp_sv4_from_u64(INT64_MAX, 64, 0);
        plan = llg_gmp_sv4_select_plan_indexed(65, base, 0, 1);
    } else if (!strcmp(mode, "plan-step-zero"))
        llg_gmp_sv4_select_plan_step(&plan, source, 0);
    else if (!strcmp(mode, "plan-invalid")) {
        plan.count = 66;
        v = llg_gmp_sv4_select_plan_read(v, &plan);
    } else if (!strcmp(mode, "plan-storage"))
        v = llg_gmp_sv4_select_plan_read(source, &plan);
    else if (!strcmp(mode, "plan-source"))
        v = llg_gmp_sv4_select_plan_slice(source, &plan, 0);
    else if (!strcmp(mode, "plan-set"))
        llg_gmp_sv4_select_plan_set(&v, &plan, source);
    else if (!strcmp(mode, "array-shape"))
        v = llg_gmp_sv4_array_conditional_merge(v, v, source);
    else
        CHECK(0);
    llg_gmp_sv4_destroy(&v);
    llg_gmp_sv4_destroy(&source);
    CHECK(0);
}
int main(int argc, char** argv) {
    if (argc > 1)
        reject(argv[1]);
    exhaustive();
    small_indices_and_empty();
    invalid_bases();
    references();
    result_ownership();
    array_boundaries();
    stream_padding();
    word_boundaries();
    wide();
    puts("compact S4/S5 independent and differential checks passed");
    printf("results checked: %lu\n", checks);
    return 0;
}
