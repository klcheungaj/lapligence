//! Standalone coverage for the scheduler-independent random-stream service.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

const RANDOM_PROBE: &str = r#"
#include "llg_rng.h"

#include <stdint.h>
#include <stdio.h>

#define CHECK(condition)                                                       \
    do {                                                                       \
        if (!(condition)) {                                                    \
            fprintf(stderr, "random check failed at line %d: %s\n",          \
                    __LINE__, #condition);                                     \
            return 1;                                                          \
        }                                                                      \
    } while (0)

static int same_state(const llg_rng_state_t* left,
                      const llg_rng_state_t* right) {
    return left->state == right->state &&
           left->increment == right->increment &&
           left->child_count == right->child_count;
}

static int check_sibling_stability(void) {
    llg_rng_state_t with_draw;
    llg_rng_state_t without_draw;
    llg_rng_state_t with_draw_first;
    llg_rng_state_t with_draw_second;
    llg_rng_state_t without_draw_first;
    llg_rng_state_t without_draw_second;
    llg_rng_state_seed(&with_draw, UINT64_C(0x12345678));
    llg_rng_state_seed(&without_draw, UINT64_C(0x12345678));
    llg_rng_state_child(&with_draw, &with_draw_first);
    (void)llg_rng_state_next(&with_draw);
    llg_rng_state_child(&with_draw, &with_draw_second);
    llg_rng_state_child(&without_draw, &without_draw_first);
    llg_rng_state_child(&without_draw, &without_draw_second);

    /* Parent draws are not part of stream identity, so child ordinal one is
     * unchanged even when an unrelated parent draw happens first. */
    for (unsigned i = 0; i < 8; ++i) {
        CHECK(llg_rng_state_next(&with_draw_first) ==
              llg_rng_state_next(&without_draw_first));
        CHECK(llg_rng_state_next(&with_draw_second) ==
              llg_rng_state_next(&without_draw_second));
    }
    return 0;
}

static int check_sibling_streams(void) {
    llg_rng_state_t parent;
    llg_rng_state_t first;
    llg_rng_state_t second;
    llg_rng_state_seed(&parent, UINT64_C(0xfeedface));
    llg_rng_state_child(&parent, &first);
    llg_rng_state_child(&parent, &second);
    int different = 0;
    for (unsigned i = 0; i < 8; ++i)
        different |= llg_rng_state_next(&first) != llg_rng_state_next(&second);
    CHECK(different);
    return 0;
}

static int check_state_replay(void) {
    llg_rng_state_t stream;
    llg_rng_state_seed(&stream, UINT64_C(42));
    (void)llg_rng_state_next(&stream);
    llg_string_t saved = llg_rng_state_get(&stream);
    uint32_t expected = llg_rng_state_next(&stream);
    CHECK(llg_rng_state_set(&stream, &saved));
    CHECK(llg_rng_state_next(&stream) == expected);
    llg_string_destroy(&saved);

    llg_rng_state_t before = stream;
    llg_string_t invalid = llg_string_bytes("LLG_RNG_V1:bad", 14);
    CHECK(!llg_rng_state_set(&stream, &invalid));
    CHECK(same_state(&stream, &before));
    llg_string_destroy(&invalid);

    llg_rng_state_t parent;
    llg_rng_state_t first_child;
    llg_rng_state_t expected_child;
    llg_rng_state_t actual_child;
    llg_rng_state_seed(&parent, UINT64_C(77));
    llg_rng_state_child(&parent, &first_child);
    (void)first_child;
    llg_string_t parent_saved = llg_rng_state_get(&parent);
    llg_rng_state_child(&parent, &expected_child);
    CHECK(llg_rng_state_set(&parent, &parent_saved));
    llg_rng_state_child(&parent, &actual_child);
    CHECK(llg_rng_state_next(&expected_child) == llg_rng_state_next(&actual_child));
    llg_string_destroy(&parent_saved);
    return 0;
}

static int check_inclusive_ranges(void) {
    llg_rng_state_t stream;
    llg_rng_state_seed(&stream, UINT64_C(99));
    for (unsigned i = 0; i < 10000; ++i) {
        uint32_t forward = llg_rng_state_uniform(&stream, 3, 7);
        uint32_t reverse = llg_rng_state_uniform(&stream, 7, 3);
        CHECK(forward >= 3 && forward <= 7);
        CHECK(reverse >= 3 && reverse <= 7);
        CHECK(llg_rng_state_uniform(&stream, 11, 11) == 11);
    }
    CHECK(llg_rng_state_uniform(&stream, 0, UINT32_MAX) <= UINT32_MAX);
    return 0;
}

int main(void) {
    CHECK(check_sibling_stability() == 0);
    CHECK(check_sibling_streams() == 0);
    CHECK(check_state_replay() == 0);
    CHECK(check_inclusive_ranges() == 0);
    puts("runtime random streams ok");
    return 0;
}
"#;

#[test]
fn random_runtime_compiles_and_runs_without_scheduler() {
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    if Command::new(&compiler).arg("--version").output().is_err() {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-random").expect("create temp directory");
    let (header, implementation) = llg::sim::rt::rng_sources();
    let (string_header, string_implementation) = llg::sim::rt::string_sources();
    let (value_header, value_implementation) = llg::sim::rt::value_sources();
    for (name, contents) in [
        ("llg_value.h", value_header),
        ("llg_value.c", value_implementation),
        ("llg_rng.h", header),
        ("llg_rng.c", implementation),
        ("llg_string.h", string_header),
        ("llg_string.c", string_implementation),
        ("runtime_random_probe.c", RANDOM_PROBE),
    ] {
        std::fs::write(dir.path().join(name), contents).expect("write runtime source");
    }

    let executable = dir.path().join("runtime_random_probe");
    let mut command = Command::new(&compiler);
    command
        .current_dir(dir.path())
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I."]);
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command
        .args([
            "llg_value.c",
            "llg_rng.c",
            "llg_string.c",
            "runtime_random_probe.c",
            "-lm",
            "-o",
        ])
        .arg(&executable);
    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone random runtime must compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let stdout = sim_harness::run_executable(&executable).expect("random probe should run");
    assert_eq!(stdout, "runtime random streams ok\n");
}
