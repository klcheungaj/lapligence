//! Standalone coverage for the scheduler-independent C value runtime.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

const VALUE_PROBE: &str = r#"
#include "llg_value.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CHECK(condition)                                                        \
    do {                                                                        \
        if (!(condition)) {                                                     \
            fprintf(stderr, "runtime value check failed at line %d: %s\n",     \
                    __LINE__, #condition);                                      \
            return 1;                                                           \
        }                                                                       \
    } while (0)

static int state_at(sv4_t value, unsigned bit) {
    unsigned limb = bit / 64u;
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    if ((value.x[limb] & mask) != 0) return 2;
    if ((value.z[limb] & mask) != 0) return 3;
    return (value.bits[limb] & mask) != 0;
}

static int check_wide_four_state_ops(void) {
    uint64_t bits[LLG_LIMBS] = {0};
    uint64_t x[LLG_LIMBS] = {0};
    uint64_t z[LLG_LIMBS] = {0};
    for (unsigned i = 0; i < LLG_LIMBS; i++) {
        x[i] = UINT64_C(1) << (i % 61u);
        z[i] = UINT64_C(1) << ((i + 17u) % 61u);
        bits[i] = (UINT64_C(0xa55aa55a01234567) ^ ((uint64_t)i << 32))
                  & ~(x[i] | z[i]);
    }

    sv4_t mixed = sv4_from_limbs(bits, x, z, LLG_MAX_WIDTH, 0);
    sv4_t ones = sv4_fill(1, LLG_MAX_WIDTH, 0);
    sv4_t zeros = sv4_fill(0, LLG_MAX_WIDTH, 0);
    sv4_t and_result = sv4_and(ones, mixed);
    sv4_t or_result = sv4_or(zeros, mixed);
    sv4_t xor_result = sv4_xor(ones, mixed);
    for (unsigned i = 0; i < LLG_LIMBS; i++) {
        uint64_t unknown = x[i] | z[i];
        CHECK(and_result.bits[i] == bits[i]);
        CHECK(and_result.x[i] == unknown);
        CHECK(and_result.z[i] == 0);
        CHECK(or_result.bits[i] == bits[i]);
        CHECK(or_result.x[i] == unknown);
        CHECK(or_result.z[i] == 0);
        CHECK(xor_result.bits[i] == (~bits[i] & ~unknown));
        CHECK(xor_result.x[i] == unknown);
        CHECK(xor_result.z[i] == 0);
    }

    sv4_t one = sv4_from_u64(1, LLG_MAX_WIDTH, 0);
    sv4_t sum = sv4_add(ones, one);
    sv4_t difference = sv4_sub(zeros, one);
    CHECK(sum.width == LLG_MAX_WIDTH && !sv4_is_unknown(sum));
    CHECK(difference.width == LLG_MAX_WIDTH && !sv4_is_unknown(difference));
    for (unsigned i = 0; i < LLG_LIMBS; i++) {
        CHECK(sum.bits[i] == 0);
        CHECK(difference.bits[i] == UINT64_MAX);
    }

    sv4_t unknown_sum = sv4_add(mixed, one);
    for (unsigned i = 0; i < LLG_LIMBS; i++) {
        CHECK(unknown_sum.bits[i] == 0);
        CHECK(unknown_sum.x[i] == UINT64_MAX);
        CHECK(unknown_sum.z[i] == 0);
    }

    sv4_t clamped = sv4_fill(1, UINT16_MAX, 0);
    CHECK(clamped.width == LLG_MAX_WIDTH);
    return 0;
}

static int check_signed_resize(void) {
    sv4_t negative = sv4_from_i64(-2, 8);
    sv4_t extended = sv4_resize(negative, 130, 1);
    CHECK(extended.width == 130 && extended.is_signed);
    CHECK(state_at(extended, 0) == 0);
    for (unsigned bit = 1; bit < 130; bit++) CHECK(state_at(extended, bit) == 1);
    CHECK((extended.bits[2] & ~UINT64_C(3)) == 0);

    uint64_t x_bits[LLG_LIMBS] = {0};
    uint64_t z_bits[LLG_LIMBS] = {0};
    x_bits[0] = UINT64_C(1) << 7;
    z_bits[0] = UINT64_C(1) << 7;
    sv4_t x_sign = sv4_from_limbs(NULL, x_bits, NULL, 8, 1);
    sv4_t z_sign = sv4_from_limbs(NULL, NULL, z_bits, 8, 1);
    x_sign = sv4_resize(x_sign, 130, 1);
    z_sign = sv4_resize(z_sign, 130, 1);
    for (unsigned bit = 7; bit < 130; bit++) {
        CHECK(state_at(x_sign, bit) == 2);
        CHECK(state_at(z_sign, bit) == 3);
    }

    sv4_t signed_source = sv4_from_i64(-128, 8);
    sv4_t unsigned_source = sv4_from_u64(0x80, 8, 0);
    sv4_t cast_unsigned = sv4_cast(signed_source, 16, 0);
    sv4_t cast_signed = sv4_cast(unsigned_source, 16, 1);
    CHECK(cast_unsigned.bits[0] == UINT64_C(0xff80));
    CHECK(cast_unsigned.is_signed == 0);
    CHECK(cast_signed.bits[0] == UINT64_C(0x0080));
    CHECK(cast_signed.is_signed == 1);

    sv4_t target_signed_resize = sv4_resize(unsigned_source, 16, 1);
    CHECK(target_signed_resize.bits[0] == UINT64_C(0xff80));
    return 0;
}

static int check_queries(void) {
    uint64_t bits[LLG_LIMBS] = {0};
    uint64_t x[LLG_LIMBS] = {0};
    uint64_t z[LLG_LIMBS] = {0};
    for (unsigned i = 0; i < LLG_LIMBS; i++) {
        bits[i] = UINT64_C(0x89);
        x[i] = UINT64_C(0x100);
        z[i] = UINT64_C(0x200);
    }
    sv4_t many = sv4_from_limbs(bits, x, z, LLG_MAX_WIDTH, 0);
    CHECK(sv4_to_i64(sv4_countones(many)) == 48);
    CHECK(sv4_to_u64(sv4_onehot(many, 0)) == 0);
    CHECK(sv4_to_u64(sv4_onehot(many, 1)) == 0);
    CHECK(sv4_is_unknown(many));

    sv4_t only_unknown = sv4_from_limbs(NULL, x, z, LLG_MAX_WIDTH, 0);
    CHECK(sv4_to_i64(sv4_countones(only_unknown)) == 0);
    CHECK(sv4_to_u64(sv4_onehot(only_unknown, 0)) == 0);
    CHECK(sv4_to_u64(sv4_onehot(only_unknown, 1)) == 1);
    only_unknown.bits[15] = UINT64_C(1) << 63;
    CHECK(sv4_to_u64(sv4_onehot(only_unknown, 0)) == 1);
    CHECK(sv4_to_u64(sv4_onehot(only_unknown, 1)) == 1);

    uint64_t edge_bits[LLG_LIMBS] = {0};
    edge_bits[1] = UINT64_C(1) | (UINT64_C(1) << 63);
    sv4_t width_65 = sv4_from_limbs(edge_bits, NULL, NULL, 65, 0);
    CHECK(sv4_to_i64(sv4_countones(width_65)) == 1);
    CHECK(!sv4_is_unknown(width_65));
    return 0;
}

static int check_numeric_conversions(void) {
    uint64_t wide_bits[LLG_LIMBS] = {0};
    wide_bits[2] = 1;
    sv4_t wide = sv4_from_limbs(wide_bits, NULL, NULL, 192, 0);
    CHECK(sv4_to_real(wide) == ldexp(1.0, 128));
    CHECK(sv4_to_real(sv4_fill(1, 130, 1)) == -1.0);

    uint64_t packed_bits[LLG_LIMBS] = {UINT64_C(0xf)};
    uint64_t packed_x[LLG_LIMBS] = {UINT64_C(0x2)};
    uint64_t packed_z[LLG_LIMBS] = {UINT64_C(0x8)};
    sv4_t four_state = sv4_from_limbs(packed_bits, packed_x, packed_z, 4, 0);
    CHECK(sv4_to_real(four_state) == 5.0);

    CHECK(sv4_to_i64(sv4_from_real(12.5, 16, 1)) == 13);
    CHECK(sv4_to_i64(sv4_from_real(-2.5, 16, 1)) == -3);
    CHECK(sv4_to_i64(sv4_rtoi(12.75)) == 12);
    CHECK(sv4_to_i64(sv4_rtoi(-12.75)) == -12);
    CHECK(sv4_is_unknown(sv4_from_real(INFINITY, 32, 1)));
    CHECK(sv4_is_unknown(sv4_from_real(NAN, 32, 1)));
    CHECK(sv4_is_unknown(sv4_rtoi(-INFINITY)));

    double real_value = -13.25;
    uint64_t real_bits = 0;
    memcpy(&real_bits, &real_value, sizeof(real_bits));
    sv4_t encoded_real = sv4_realtobits(real_value);
    CHECK(encoded_real.width == 64 && encoded_real.bits[0] == real_bits);
    CHECK(sv4_bitstoreal(encoded_real) == real_value);
    encoded_real.x[0] |= UINT64_C(1);
    encoded_real.z[0] |= UINT64_C(2);
    real_bits &= ~UINT64_C(3);
    double masked_real = 0.0;
    memcpy(&masked_real, &real_bits, sizeof(masked_real));
    CHECK(sv4_bitstoreal(encoded_real) == masked_real);

    double short_value = 1.5;
    float narrowed = (float)short_value;
    uint32_t short_bits = 0;
    memcpy(&short_bits, &narrowed, sizeof(short_bits));
    sv4_t encoded_short = sv4_shortrealtobits(short_value);
    CHECK(encoded_short.width == 32 && encoded_short.bits[0] == short_bits);
    CHECK(sv4_bitstoshortreal(encoded_short) == short_value);
    encoded_short.x[0] |= UINT64_C(1);
    encoded_short.z[0] |= UINT64_C(2);
    short_bits &= ~UINT32_C(3);
    memcpy(&narrowed, &short_bits, sizeof(narrowed));
    CHECK(sv4_bitstoshortreal(encoded_short) == (double)narrowed);

    CHECK(llg_real_to_bool(0.0) == 0);
    CHECK(llg_real_to_bool(-0.0) == 0);
    CHECK(llg_real_to_bool(-0.25) == 1);
    return 0;
}

int main(void) {
    CHECK(check_wide_four_state_ops() == 0);
    CHECK(check_signed_resize() == 0);
    CHECK(check_queries() == 0);
    CHECK(check_numeric_conversions() == 0);
    puts("runtime value isolation ok");
    return 0;
}
"#;

#[test]
fn value_runtime_compiles_and_runs_without_scheduler() {
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    if Command::new(&compiler).arg("--version").output().is_err() {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-values").expect("create temp directory");
    let (header, implementation) = llg::sim::rt::value_sources();
    std::fs::write(dir.path().join("llg_value.h"), header).expect("write value header");
    std::fs::write(dir.path().join("llg_value.c"), implementation)
        .expect("write value implementation");
    std::fs::write(dir.path().join("runtime_values_probe.c"), VALUE_PROBE)
        .expect("write value probe");

    let executable = dir.path().join("runtime_values_probe");
    let mut command = Command::new(&compiler);
    command
        .current_dir(dir.path())
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I."]);
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command
        .args(["llg_value.c", "runtime_values_probe.c", "-lm", "-o"])
        .arg(&executable);

    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone value runtime must compile without scheduler/libaco:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let stdout = sim_harness::run_executable(&executable).expect("value probe should run");
    assert_eq!(stdout, "runtime value isolation ok\n");
}
