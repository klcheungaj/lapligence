/* A measured value-storage workload, not a whole-simulator speed benchmark. */
#include "probe.h"
#include <errno.h>
#include <time.h>
#include <string.h>

static size_t positive_count(const char* text) {
    char* end = NULL;
    errno = 0;
    unsigned long long count = strtoull(text, &end, 10);
    if (errno || !*text || *end || text[0] == '-' || count == 0 || count > 1000000) {
        fputs("counts must be integers in 1..1000000\n", stderr);
        exit(2);
    }
    return (size_t)count;
}

int main(int argc, char** argv) {
    size_t slots = 4096, rounds = 32;
    for (int i = 1; i < argc; ++i) {
        if (i + 1 == argc) return 2;
        if (strcmp(argv[i], "--slots") == 0) slots = positive_count(argv[++i]);
        else if (strcmp(argv[i], "--rounds") == 0) rounds = positive_count(argv[++i]);
        else return 2;
    }
    CHECK(slots + 1u <= SIZE_MAX / sizeof(sv4_t));
    sv4_t* values = (sv4_t*)calloc(slots + 1u, sizeof(*values));
    CHECK(values != NULL);
    const uint32_t widths[] = {0, 1, 8, 31, 32, 33, 64, 65, 129, 257, 1025, 4097};
    size_t expected_bytes = 0, expected_live = 0, empty_slots = 0;
    for (size_t i = 0; i < slots; ++i) {
        uint32_t width = widths[i % (sizeof(widths) / sizeof(*widths))];
        sv4_replace(&values[i], sv4_from_u64((uint64_t)i, width, 0));
        expected_bytes += 24u * (((size_t)width + 63u) / 64u);
        expected_live += width != 0;
        empty_slots += width == 0;
    }
    /* A single legal wide declaration must not inflate all the other cells. */
    sv4_replace(&values[slots], sv4_zero(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 0));
    const size_t widest_payload = sv4_bytes(&values[slots]);
    expected_bytes += widest_payload;
    ++expected_live;
    CHECK(value_test_bytes() == expected_bytes && value_test_live() == expected_live);
    const uint64_t fixed_reference = (uint64_t)(slots + 1u) * (uint64_t)widest_payload;
    value_test_reset_stats();
    clock_t start = clock();
    for (size_t round = 0; round < rounds; ++round) {
        for (size_t i = 0; i < slots; ++i) {
            switch (round % 4u) {
                case 0:
                    sv4_replace(&values[i], sv4_clone(&values[i]));
                    break;
                case 1:
                    sv4_replace(&values[i], sv4_add(values[i], values[i]));
                    break;
                case 2:
                    sv4_replace(&values[i], sv4_resize(values[i], values[i].width, 0));
                    break;
                default: {
                    sv4_t owned = sv4_clone(&values[i]);
                    sv4_move(&values[i], &owned);
                    sv4_destroy(&owned);
                    break;
                }
            }
        }
        sv4_replace(&values[slots], sv4_clone(&values[slots]));
        CHECK(value_test_live() == expected_live);
        CHECK(value_test_bytes() == expected_bytes);
        CHECK(value_test_peak_live() <= expected_live + 1u);
        CHECK(value_test_peak_bytes() <= expected_bytes + widest_payload);
    }
    clock_t finish = clock();
    size_t peak_bytes = value_test_peak_bytes(), peak_live = value_test_peak_live();
    size_t allocations = value_test_allocations();
    CHECK(allocations == rounds * expected_live);
    uint64_t checksum = 0;
    for (size_t i = 0; i < slots; ++i) checksum ^= sv4_to_u64(values[i]) + (uint64_t)i;
    sv4_destroy_array(values, slots + 1u);
    free(values);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    printf("{\"schema\":\"llg.value-lifetime/v1\",\"slots\":%zu,\"rounds\":%zu,"
           "\"descriptor_size\":%zu,\"descriptor_bytes\":%zu,\"zero_width_slots\":%zu,"
           "\"steady_payload_bytes\":%zu,\"peak_payload_bytes\":%zu,"
           "\"steady_live_allocations\":%zu,\"peak_live_allocations\":%zu,"
           "\"allocations_during_cycles\":%zu,\"fixed_capacity_reference_bytes\":%llu,"
           "\"ending_live_allocations\":%zu,\"ending_payload_bytes\":%zu,"
           "\"checksum\":%llu,\"cpu_seconds\":",
           slots, rounds, sizeof(sv4_t), (slots + 1u) * sizeof(sv4_t), empty_slots,
           expected_bytes, peak_bytes, expected_live, peak_live, allocations,
           (unsigned long long)fixed_reference, value_test_live(), value_test_bytes(),
           (unsigned long long)checksum);
    if (start == (clock_t)-1 || finish == (clock_t)-1 || finish < start) printf("null");
    else printf("%.9f", (double)(finish - start) / (double)CLOCKS_PER_SEC);
    puts("}");
    return 0;
}
