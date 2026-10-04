#include "compact_adapters_test.h"
#include <float.h>

static int same_double(double a, double b) {
    uint64_t aa, bb;
    memcpy(&aa, &a, 8); memcpy(&bb, &b, 8);
    return aa == bb || (isnan(a) && isnan(b));
}
static void small_tables(void) {
    for (uint32_t width = 0; width <= 4; ++width) for (unsigned sign = 0; sign < 2; ++sign)
        for (unsigned code = 0; code < (1u << (2u * width)); ++code) {
            sv4_t old = sv4_zero(width, (int8_t)sign);
            g4_t v = llg_gmp_sv4_zero(width, (int8_t)sign);
            uint64_t known = 0;
            for (uint32_t i = 0; i < width; ++i) {
                unsigned state = (code >> (2u * i)) & 3;
                llg_sv4_set_state(&old, i, state); llg_gmp_sv4_set_state(&v, i, state);
                if (state == 1) known |= UINT64_C(1) << i;
            }
            int64_t numeric = (int64_t)known;
            if (sign && width && ((known >> (width - 1u)) & 1)) numeric -= INT64_C(1) << width;
            CHECK(llg_gmp_sv4_to_real(v) == (double)numeric);
            CHECK(same_double(sv4_to_real(old), llg_gmp_sv4_to_real(v)));
            CHECK(same_double(sv4_bitstoreal(old), llg_gmp_sv4_bitstoreal(v)));
            CHECK(same_double(sv4_bitstoshortreal(old), llg_gmp_sv4_bitstoshortreal(v)));
            uint64_t delay = llg_gmp_sv4_is_unknown(v) ? 0 : (uint64_t)numeric;
            CHECK(llg_gmp_sv4_delay_ticks(v, 1) == delay);
            CHECK(sv4_delay_ticks(old, 1) == llg_gmp_sv4_delay_ticks(v, 1));
            sv4_destroy(&old);
        }
}
static void conversions(void) {
    const uint64_t payloads[] = {0, UINT64_C(0x8000000000000000), 1, UINT64_MAX,
        UINT64_C(0x7ff0000000000000), UINT64_C(0x7ff0000000000001),
        UINT64_C(0x7ff8123456789abc), UINT64_C(0xfff8123456789abc)};
    for (size_t i = 0; i < sizeof(payloads) / sizeof(payloads[0]); ++i) {
        g4_t bits = llg_gmp_sv4_from_u64(payloads[i], 64, 0);
        double value = llg_gmp_sv4_bitstoreal(bits);
        CHECK(llg_gmp_sv4_to_u64(llg_gmp_sv4_realtobits(value)) == payloads[i]);
    }
    double values[] = {0, -0.0, 0.49, -0.49, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5,
        2147483647.5, -2147483648.5, 4294967295.75, -4294967295.75,
        0x1.fffffffffffffp63, 0x1p64, -0x1p64, 0x1.fffffffffffffp128,
        -0x1.fffffffffffffp128, DBL_MAX, -DBL_MAX, DBL_MIN, -DBL_MIN, INFINITY, -INFINITY, NAN};
    for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); ++i) {
        double value = values[i];
        compare(sv4_rtoi(value), llg_gmp_sv4_rtoi(value));
        compare(sv4_realtobits(value), llg_gmp_sv4_realtobits(value));
        compare(sv4_shortrealtobits(value), llg_gmp_sv4_shortrealtobits(value));
        CHECK(llg_real_to_bool(value) == llg_gmp_real_to_bool(value));
        CHECK(llg_gmp_real_to_bool(value) == (value != 0.0));
        g4_t bits = llg_gmp_sv4_realtobits(value);
        CHECK(same_double(value, llg_gmp_sv4_bitstoreal(bits)));
        bits = llg_gmp_sv4_shortrealtobits(value);
        CHECK(same_double((double)(float)value, llg_gmp_sv4_bitstoshortreal(bits)));
        for (size_t w = 0; w < sizeof(adapter_widths) / sizeof(adapter_widths[0]); ++w)
            for (int8_t sign = 0; sign < 2; ++sign)
                compare(sv4_from_real(value, adapter_widths[w], sign),
                        llg_gmp_sv4_from_real(value, adapter_widths[w], sign));
    }
    for (int i = -1000; i <= 1000; ++i) {
        double value = (double)i / 4.0;
        int64_t rounded = i < 0 ? -((-(int64_t)i + 2) / 4) : ((int64_t)i + 2) / 4;
        g4_t v = llg_gmp_sv4_from_real(value, 64, 1);
        CHECK(llg_gmp_sv4_to_u64(v) == (uint64_t)rounded);
        CHECK(llg_gmp_sv4_to_u64(llg_gmp_sv4_rtoi(value)) == ((uint64_t)(i / 4) & UINT32_MAX));
    }
    uint64_t seed = UINT64_C(0x87787bb987bd);
    for (unsigned sample = 0; sample < 2048; ++sample) {
        uint64_t bits = next_word(&seed);
        double value; memcpy(&value, &bits, 8);
        uint32_t width = adapter_widths[sample % 24u];
        compare(sv4_from_real(value, width, (int8_t)(sample & 1)),
                llg_gmp_sv4_from_real(value, width, (int8_t)(sample & 1)));
        compare(sv4_rtoi(value), llg_gmp_sv4_rtoi(value));
    }
    for (size_t w = 1; w < sizeof(adapter_widths) / sizeof(adapter_widths[0]); ++w) {
        uint32_t width = adapter_widths[w]; size_t n = ((size_t)width + 63u) / 64u;
        uint64_t* p = (uint64_t*)calloc(3u * n, 8); CHECK(p);
        for (unsigned pattern = 0; pattern < 4; ++pattern) {
            for (size_t i = 0; i < n; ++i) {
                p[i] = next_word(&seed); p[n + i] = pattern >= 2 ? next_word(&seed) : 0;
                p[2u * n + i] = pattern >= 2 ? next_word(&seed) & ~p[n + i] : 0;
            }
            sv4_t old = sv4_from_limbs(p, p + n, p + 2u * n, width, (int8_t)(pattern & 1));
            g4_t v = llg_gmp_sv4_from_limbs(p, p + n, p + 2u * n, width, (int8_t)(pattern & 1));
            CHECK(same_double(sv4_to_real(old), llg_gmp_sv4_to_real(v)));
            CHECK(same_double(sv4_bitstoreal(old), llg_gmp_sv4_bitstoreal(v)));
            CHECK(same_double(sv4_bitstoshortreal(old), llg_gmp_sv4_bitstoshortreal(v)));
            sv4_destroy(&old); llg_gmp_sv4_destroy(&v);
        }
        free(p);
    }
}
static void delays(void) {
    const uint64_t units[] = {1, 10, 1000, 1000000}, precisions[] = {1, 10, 1000};
    for (unsigned u = 0; u < 4; ++u) for (unsigned p = 0; p < 3; ++p)
        for (unsigned i = 0; i < 100; ++i) {
            double value = (double)i / 4.0;
            CHECK(sv4_real_delay_ticks(value, units[u], precisions[p]) ==
                  llg_gmp_sv4_real_delay_ticks(value, units[u], precisions[p]));
        }
    CHECK(llg_gmp_sv4_real_delay_ticks(0.5, 1000, 1000) == 1000);
    CHECK(llg_gmp_sv4_real_delay_ticks(0.49, 1000, 1000) == 0);
    CHECK(llg_gmp_sv4_real_delay_ticks(-0.0, 1, 1) == 0);
    // Negative delays are two's-complement unsigned time after rounding.
    for (unsigned u = 0; u < 4; ++u)
        for (unsigned i = 1; i < 100; ++i) {
            double value = -(double)i / 4.0;
            CHECK(sv4_real_delay_ticks(value, units[u], 1) ==
                  llg_gmp_sv4_real_delay_ticks(value, units[u], 1));
        }
    CHECK(llg_gmp_sv4_real_delay_ticks(-0.4, 1, 1) == 0);
    CHECK(llg_gmp_sv4_real_delay_ticks(-1.0, 1, 1) == UINT64_MAX);
    CHECK(llg_gmp_sv4_real_delay_ticks(-0.25, 10, 1) == UINT64_MAX - 2u);
    CHECK(llg_gmp_sv4_real_delay_ticks(-0x1p63, 1, 1) == UINT64_C(1) << 63);
    for (size_t w = 1; w < sizeof(adapter_widths) / sizeof(adapter_widths[0]); ++w) {
        g4_t value = llg_gmp_sv4_from_i64(-1, adapter_widths[w]);
        sv4_t old = sv4_from_i64(-1, adapter_widths[w]);
        CHECK(sv4_delay_ticks(old, 1) == llg_gmp_sv4_delay_ticks(value, 1));
        sv4_destroy(&old); llg_gmp_sv4_destroy(&value);
    }
    CHECK(llg_gmp_sv4_delay_ticks(llg_gmp_sv4_from_u64(UINT64_MAX, 64, 0), 1) == UINT64_MAX);
}
int main(int argc, char** argv) {
    if (argc > 1) {
        if (!strcmp(argv[1], "packed-wide")) {
            g4_t v = llg_gmp_sv4_zero(65, 0); llg_gmp_sv4_set_state(&v, 64, 1);
            (void)llg_gmp_sv4_delay_ticks(v, 1);
        } else if (!strcmp(argv[1], "packed-overflow"))
            (void)llg_gmp_sv4_delay_ticks(llg_gmp_sv4_from_u64(UINT64_MAX, 64, 0), 2);
        else if (!strcmp(argv[1], "packed-scale"))
            (void)llg_gmp_sv4_delay_ticks(llg_gmp_sv4_zero(1, 0), 0);
        else if (!strcmp(argv[1], "real-negative"))
            (void)llg_gmp_sv4_real_delay_ticks(-0x1p64, 1, 1);
        else if (!strcmp(argv[1], "real-nan"))
            (void)llg_gmp_sv4_real_delay_ticks(NAN, 1, 1);
        else if (!strcmp(argv[1], "real-scale"))
            (void)llg_gmp_sv4_real_delay_ticks(0, 1, 0);
        else
            (void)llg_gmp_sv4_real_delay_ticks(0x1p64, 1, 1);
        return 1;
    }
    small_tables(); conversions(); delays();
    printf("real/time: %zu checks passed\n", checks);
    return 0;
}
