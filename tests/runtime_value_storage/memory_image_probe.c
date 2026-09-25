#include "llg_rt.h"
#include "probe.h"
#include <string.h>

static void write_input(const char* name, const char* data) {
    FILE* file = fopen(name, "wb");
    CHECK(file != NULL);
    CHECK(fwrite(data, 1, strlen(data), file) == strlen(data));
    CHECK(fclose(file) == 0);
}

static unsigned bit_state(sv4_t value, uint32_t bit) {
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    if (value.x[bit / 64u] & mask) return 2;
    if (value.z[bit / 64u] & mask) return 3;
    return (value.bits[bit / 64u] & mask) != 0;
}

static void load(const char* name, sv4_t* memory, uint32_t count, uint32_t width,
                 int8_t sign, int8_t two_state, const int32_t* dims, int policy,
                 const sv4_t* enums, uint32_t enum_count, int radix) {
    const uint64_t strides[] = {1};
    const sv4_t omitted = SV4_EMPTY;
    llg_memory_read_view(llg_string_bytes(name, strlen(name)), memory, count,
                        width, sign, two_state, dims, 1, strides, 0, count,
                        omitted, omitted, 0, 0, policy, enums, enum_count, radix);
}

static void token_values(void) {
    const uint32_t widths[] = {1, 7, 8, 65, 129};
    const int32_t dims[] = {0, 7};
    write_input("tokens.hex", "x z 0x x7 0z z0 f 1ff\n");
    write_input("tokens.bin", "x z 0x x1 0z z0 1 101\n");
    for (size_t w = 0; w < sizeof(widths) / sizeof(widths[0]); ++w) {
        for (int radix = 2; radix <= 16; radix += 14) {
            for (int sign = 0; sign <= 1; ++sign) {
                uint32_t width = widths[w];
                sv4_t memory[8];
                for (int i = 0; i < 8; ++i) memory[i] = sv4_zero(width, (int8_t)sign);
                load(radix == 2 ? "tokens.bin" : "tokens.hex", memory, 8, width,
                     (int8_t)sign, 0, dims, LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009,
                     NULL, 0, radix);
                for (uint32_t bit = 0; bit < width; ++bit) {
                    uint32_t digit = radix == 2 ? 1u : 4u;
                    CHECK(bit_state(memory[0], bit) == 2);
                    CHECK(bit_state(memory[1], bit) == 3);
                    CHECK(bit_state(memory[2], bit) == (bit < digit ? 2u : 0u));
                    unsigned low = radix == 2 ? 1u : 7u;
                    CHECK(bit_state(memory[3], bit) == (bit < digit ? ((low >> bit) & 1u) : 2u));
                    CHECK(bit_state(memory[4], bit) == (bit < digit ? 3u : 0u));
                    CHECK(bit_state(memory[5], bit) == (bit < digit ? 0u : 3u));
                    CHECK(bit_state(memory[6], bit) == (bit < digit ? 1u : 0u));
                    uint64_t number = radix == 2 ? 5u : 511u;
                    CHECK(bit_state(memory[7], bit) == (bit < 64 ? ((number >> bit) & 1u) : 0u));
                }
                for (int i = 0; i < 8; ++i) {
                    CHECK(memory[i].is_signed == sign);
                    sv4_destroy(&memory[i]);
                }
            }
        }
    }
}

static void enum_overflow(void) {
    const int32_t dims[] = {0, 2};
    sv4_t values[] = {sv4_from_u64(0, 2, 0), sv4_from_u64(1, 2, 0)};
    sv4_t memory[] = {sv4_from_u64(0, 2, 0), sv4_from_u64(1, 2, 0), sv4_from_u64(1, 2, 0)};
    write_input("enum.hex", "1 4x 0\n");
    load("enum.hex", memory, 3, 2, 0, 1, dims,
         LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009, values, 2, 16);
    for (int i = 0; i < 3; ++i) {
        CHECK(sv4_to_u64(memory[i]) == 1u && !sv4_is_unknown(memory[i]));
        sv4_destroy(&memory[i]);
    }
    for (int i = 0; i < 2; ++i) sv4_destroy(&values[i]);
}

static void sparse(int policy) {
    const int32_t dims[] = {3, 0};
    sv4_t memory[4];
    for (int i = 0; i < 4; ++i) memory[i] = sv4_from_u64(7, 8, 0);
    write_input("sparse.hex", "@2 aa @0 bb\n");
    load("sparse.hex", memory, 4, 8, 0, 0, dims, policy, NULL, 0, 16);
    const uint64_t expected[] = {7, 0xaa, 7, 0xbb};
    for (int i = 0; i < 4; ++i) {
        CHECK(sv4_to_u64(memory[i]) == expected[i]);
        sv4_destroy(&memory[i]);
    }
}

static void repeated_addresses(int policy) {
    const int32_t dims[] = {0, 1};
    sv4_t memory[] = {sv4_zero(8, 0), sv4_zero(8, 0)};
    write_input("repeated.hex", "@0 aa @0 bb @0 cc\n");
    load("repeated.hex", memory, 2, 8, 0, 0, dims, policy, NULL, 0, 16);
    CHECK(sv4_to_u64(memory[0]) == 0xcc && sv4_to_u64(memory[1]) == 0);
    sv4_destroy(&memory[0]);
    sv4_destroy(&memory[1]);
}

static void conversion_controls(void) {
    const int32_t dims[] = {0, 1};
    sv4_t memory[] = {sv4_zero(7, 0), sv4_zero(7, 0)};
    write_input("two_state.hex", "x7 z1\n");
    load("two_state.hex", memory, 2, 7, 0, 1, dims,
         LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009, NULL, 0, 16);
    CHECK(sv4_to_u64(memory[0]) == 7u && sv4_to_u64(memory[1]) == 1u);
    CHECK(!sv4_is_unknown(memory[0]) && !sv4_is_unknown(memory[1]));
    sv4_destroy(&memory[0]);
    sv4_destroy(&memory[1]);
    const int32_t one[] = {0, 0};
    sv4_t value = sv4_from_u64(7, 3, 1);
    sv4_t target = sv4_zero(3, 1);
    write_input("signed.hex", "ff\n");
    load("signed.hex", &target, 1, 3, 1, 0, one,
         LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009, &value, 1, 16);
    CHECK(sv4_to_i64(target) == -1);
    sv4_destroy(&target);
    sv4_destroy(&value);
}

int main(int argc, char** argv) {
    CHECK(argc == 2);
    llg_rt_init();
    if (strcmp(argv[1], "tokens") == 0) token_values();
    else if (strcmp(argv[1], "enum") == 0) enum_overflow();
    else if (strcmp(argv[1], "sparse2009") == 0) sparse(LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009);
    else if (strcmp(argv[1], "sparse2001") == 0) sparse(LLG_MEMORY_ADDRESSING_VERILOG_2001);
    else if (strcmp(argv[1], "repeated2009") == 0) repeated_addresses(LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009);
    else if (strcmp(argv[1], "repeated2001") == 0) repeated_addresses(LLG_MEMORY_ADDRESSING_VERILOG_2001);
    else if (strcmp(argv[1], "conversion") == 0) conversion_controls();
    else CHECK(0);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    printf("MEMORY_IMAGE_PASS %s\n", argv[1]);
    return 0;
}
