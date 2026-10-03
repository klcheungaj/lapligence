#include "compact_adapters_test.h"
#include <time.h>

static volatile uint64_t checksum;
static double measure(int compact, unsigned op, unsigned iterations, sv4_t* old, g4_t* v,
                        uint32_t width, unsigned unknown) {
    const sv4_t* od[3] = {&old[0], &old[1], &old[2]};
    const g4_t* nd[3] = {&v[0], &v[1], &v[2]};
    uint8_t s0[3] = {6, 3, 5}, s1[3] = {3, 6, 5}; int indices[2] = {2, 0};
    const uint8_t rows[] = {1, 7, 1, 7, 2, 0};
    const sv4_t* oi[] = {&old[4], &old[5]}; const g4_t* ni[] = {&v[4], &v[5]};
    char buf[4098];
    clock_t start = clock();
    for (unsigned i = 0; i < iterations; ++i) {
        if (compact) {
            g4_t out = LLG_GMP_SV4_EMPTY;
            int64_t host = 0;
            switch (op) {
            case 0: out = llg_gmp_sv4_resolve(nd, 3, width, 0, 0); break;
            case 1: out = llg_gmp_sv4_resolve_strengths(nd, s0, s1, 3, width, 0, 0); break;
            case 2: out = llg_gmp_sv4_resolve_strengths_range(nd, s0, s1, indices, 2, width,
                          width > 64 ? 63 : 0, width > 64 ? width - 63 : width, 0, 0); break;
            case 3: out = llg_gmp_sv4_enum_navigate(v[0], v[4], v, 3, v[1], 1); break;
            case 4: out = llg_gmp_sv4_udp_eval(rows, 2, 2, ni); break;
            case 5: out = llg_gmp_sv4_from_real(unknown ? NAN : 0x1.abcdefp63, width, 1); break;
            case 6: out = llg_gmp_sv4_rtoi(unknown ? NAN : 1234.75); break;
            case 7: out = llg_gmp_sv4_realtobits(unknown ? NAN : 1234.75); break;
            case 8: out = llg_gmp_sv4_shortrealtobits(unknown ? NAN : 1234.75); break;
            case 9: checksum += (uint64_t)isinf(llg_gmp_sv4_to_real(v[0])); break;
            case 10: checksum += (uint64_t)isnan(llg_gmp_sv4_bitstoreal(v[0])); break;
            case 11: checksum += (uint64_t)isnan(llg_gmp_sv4_bitstoshortreal(v[0])); break;
            case 12: checksum += llg_gmp_sv4_delay_ticks(v[3], 1); break;
            case 13: checksum += llg_gmp_sv4_real_delay_ticks(1234.75, 1000, 1); break;
            case 14: case 15: case 16: case 17:
                llg_gmp_sv4_format("dhbo"[op - 14], v[0], buf, sizeof(buf)); checksum += (unsigned char)buf[0]; break;
            case 18: checksum += (uint64_t)llg_gmp_sv4_to_i64(v[0]); break;
            case 19: checksum += llg_gmp_sv4_to_index(v[0]); break;
            case 20: checksum += (uint64_t)llg_gmp_sv4_to_index_i64(v[0], &host) + (uint64_t)host; break;
            case 21: checksum += (uint64_t)llg_gmp_sv4_fits_i64(v[0]); break;
            case 22: checksum += llg_gmp_sv4_checked_width(v[3]); break;
            }
            if (op <= 8) checksum += llg_gmp_sv4_to_u64(out);
            llg_gmp_sv4_destroy(&out);
        } else {
            sv4_t out = SV4_EMPTY;
            int64_t host = 0;
            switch (op) {
            case 0: out = sv4_resolve(od, 3, width, 0, 0); break;
            case 1: out = sv4_resolve_strengths(od, s0, s1, 3, width, 0, 0); break;
            case 2: out = sv4_resolve_strengths_range(od, s0, s1, indices, 2, width,
                          width > 64 ? 63 : 0, width > 64 ? width - 63 : width, 0, 0); break;
            case 3: out = sv4_enum_navigate(old[0], old[4], old, 3, old[1], 1); break;
            case 4: out = sv4_udp_eval(rows, 2, 2, oi); break;
            case 5: out = sv4_from_real(unknown ? NAN : 0x1.abcdefp63, width, 1); break;
            case 6: out = sv4_rtoi(unknown ? NAN : 1234.75); break;
            case 7: out = sv4_realtobits(unknown ? NAN : 1234.75); break;
            case 8: out = sv4_shortrealtobits(unknown ? NAN : 1234.75); break;
            case 9: checksum += (uint64_t)isinf(sv4_to_real(old[0])); break;
            case 10: checksum += (uint64_t)isnan(sv4_bitstoreal(old[0])); break;
            case 11: checksum += (uint64_t)isnan(sv4_bitstoshortreal(old[0])); break;
            case 12: checksum += sv4_delay_ticks(old[3], 1); break;
            case 13: checksum += sv4_real_delay_ticks(1234.75, 1000, 1); break;
            case 14: case 15: case 16: case 17:
                sv4_format("dhbo"[op - 14], old[0], buf, sizeof(buf)); checksum += (unsigned char)buf[0]; break;
            case 18: checksum += (uint64_t)sv4_to_i64(old[0]); break;
            case 19: checksum += sv4_to_index(old[0]); break;
            case 20: checksum += (uint64_t)sv4_to_index_i64(old[0], &host) + (uint64_t)host; break;
            case 21: checksum += (uint64_t)sv4_fits_i64(old[0]); break;
            case 22: checksum += sv4_checked_width(old[3]); break;
            }
            if (op <= 8) checksum += sv4_to_u64(out);
            sv4_destroy(&out);
        }
    }
    return (double)(clock() - start) * 1e9 / CLOCKS_PER_SEC / iterations;
}
static int order(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b; return (x > y) - (x < y);
}
int main(int argc, char** argv) {
    int smoke = argc > 1 && !strcmp(argv[1], "--smoke");
    uint32_t widths[] = {1, 64, 65, 256, 4096};
    const char* names[] = {"resolve", "strength", "range", "enum", "udp", "from_real", "rtoi",
        "realtobits", "shortrealtobits", "to_real", "bitstoreal", "bitstoshortreal", "delay",
        "real_delay", "decimal", "hex", "binary", "octal", "to_i64", "index", "index_i64", "fits_i64", "checked_width"};
    puts("width,state,operation,backend,ns_median,ns_min,ns_max");
    for (unsigned w = 0; w < 5; ++w) for (unsigned unknown = 0; unknown < 2; ++unknown) {
        uint32_t width = widths[w]; sv4_t old[6]; g4_t v[6];
        for (unsigned d = 0; d < 6; ++d) {
            uint32_t dw = d >= 4 ? 1 : width;
            old[d] = sv4_from_u64(d + 1u, dw, 0); v[d] = llg_gmp_sv4_from_u64(d + 1u, dw, 0);
            if (d < 3 && width > 1) {
                llg_sv4_set_state(&old[d], width - 1u, 1); llg_gmp_sv4_set_state(&v[d], width - 1u, 1);
            }
            if (unknown && d != 3) {
                llg_sv4_set_state(&old[d], 0, d & 1 ? 3 : 2); llg_gmp_sv4_set_state(&v[d], 0, d & 1 ? 3 : 2);
            }
        }
        unsigned iterations = smoke ? 1 : width >= 4096 ? 128 : 5000;
        for (unsigned op = 0; op < 23; ++op) {
            if (op == 22 && unknown) continue;
            double values[2][7];
            for (unsigned sample = 0; sample < 7; ++sample)
                for (unsigned turn = 0; turn < 2; ++turn) {
                    unsigned backend = (sample + turn) % 2;
                    values[backend][sample] = measure((int)backend, op, iterations, old, v, width, unknown);
                }
            for (unsigned backend = 0; backend < 2; ++backend) {
                qsort(values[backend], 7, sizeof(double), order);
                printf("%u,%s,%s,%s,%.2f,%.2f,%.2f\n", width, unknown ? "xz" : "known", names[op],
                    backend ? "compact" : "legacy", values[backend][3], values[backend][0], values[backend][6]);
            }
        }
        sv4_destroy_array(old, 6); llg_gmp_sv4_destroy_array(v, 6);
    }
    fprintf(stderr, "checksum=%llu\n", (unsigned long long)checksum); return 0;
}
