//! Standalone coverage for scheduler-independent C container storage.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

const CONTAINER_PROBE: &str = r#"
#include "llg_container.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CHECK(condition)                                                     \
    do {                                                                     \
        if (!(condition)) {                                                  \
            fprintf(stderr, "container check failed at line %d: %s\n",    \
                    __LINE__, #condition);                                   \
            return 1;                                                        \
        }                                                                    \
    } while (0)

static int changes;

static void notify(sv4_t* contents, sv4_t* shape, int change) {
    (void)contents;
    (void)shape;
    if (change & LLG_CONTAINER_CHANGED_CONTENTS) ++changes;
}

static uint64_t real_bits(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

static void eval_gt_two(sv4_t* out, sv4_t item, sv4_t index, void* context) {
    (void)index;
    (void)context;
    *out = sv4_gt(item, sv4_from_i64(2, 32));
}

static void eval_eq_one(sv4_t* out, sv4_t item, sv4_t index, void* context) {
    (void)index;
    (void)context;
    *out = sv4_eq(item, sv4_from_i64(1, 32));
}

static void eval_scale_257(sv4_t* out, sv4_t item, sv4_t index, void* context) {
    (void)index;
    (void)context;
    *out = sv4_mul(item, sv4_from_u64(257, 64, 0));
}

static void eval_identity(sv4_t* out, sv4_t item, sv4_t index, void* context) {
    (void)index;
    (void)context;
    *out = item;
}

static void eval_index(sv4_t* out, sv4_t item, sv4_t index, void* context) {
    (void)item;
    (void)context;
    *out = index;
}

static int check_packed_conversions(void) {
    llg_dyn_array_t source;
    llg_dyn_array_t destination;
    llg_dyn_init(&source, 8, 1, 0);
    llg_dyn_init(&destination, 16, 1, 0);
    sv4_t values[2] = {sv4_from_i64(-2, 8), sv4_from_u64(0x7f, 8, 0)};
    llg_dyn_assign_values(&source, values, 2);
    llg_dyn_new(&destination, sv4_from_u64(2, 32, 0), &source);
    CHECK(sv4_to_u64(llg_dyn_get(&destination, sv4_from_u64(0, 32, 0))) ==
          UINT64_C(0xfffe));

    llg_queue_t source_queue;
    llg_queue_t destination_queue;
    llg_queue_init(&source_queue, 8, 0, 0, UINT64_MAX);
    llg_queue_init(&destination_queue, 8, 0, 1, UINT64_MAX);
    values[0] = sv4_x(8, 0);
    values[1] = sv4_from_u64(0xff, 8, 0);
    llg_queue_assign_values(&source_queue, values, 2);
    llg_queue_copy(&destination_queue, &source_queue);
    CHECK(sv4_to_u64(llg_queue_get(
              &destination_queue, sv4_from_u64(0, 32, 0))) == 0);
    CHECK(sv4_to_u64(llg_queue_get(
              &destination_queue, sv4_from_u64(1, 32, 0))) == 0xff);

    llg_queue_t wide_queue;
    llg_queue_t narrow_slice;
    llg_queue_init(&wide_queue, 16, 0, 0, UINT64_MAX);
    llg_queue_init(&narrow_slice, 8, 0, 1, UINT64_MAX);
    sv4_t wide_values[3] = {
        sv4_from_u64(0xaaaa, 16, 0),
        sv4_from_u64(0x12fe, 16, 0),
        sv4_from_u64(0x0123, 16, 0)
    };
    llg_queue_assign_values(&wide_queue, wide_values, 3);
    llg_queue_source_t slice = {
        .queue = &wide_queue,
        .left = sv4_from_u64(1, 32, 0),
        .right = sv4_from_u64(2, 32, 0)
    };
    llg_queue_assign_sources(&narrow_slice, &slice, 1);
    CHECK(narrow_slice.size == 2);
    CHECK(sv4_to_u64(llg_queue_get(
              &narrow_slice, sv4_from_u64(0, 32, 0))) == 0xfe);
    CHECK(sv4_to_u64(llg_queue_get(
              &narrow_slice, sv4_from_u64(1, 32, 0))) == 0x23);

    llg_queue_destroy(&narrow_slice);
    llg_queue_destroy(&wide_queue);
    llg_queue_destroy(&destination_queue);
    llg_queue_destroy(&source_queue);
    llg_dyn_destroy(&destination);
    llg_dyn_destroy(&source);
    return 0;
}

static int check_queue_references(void) {
    llg_queue_t queue;
    llg_queue_init(&queue, 32, 1, 0, UINT64_MAX);
    sv4_t values[3] = {
        sv4_from_i64(1, 32), sv4_from_i64(2, 32), sv4_from_i64(3, 32)
    };
    llg_queue_assign_values(&queue, values, 3);
    uint64_t surviving = llg_queue_ref_identity(&queue, 1);
    uint64_t removed = llg_queue_ref_identity(&queue, 2);
    llg_queue_push_front(&queue, sv4_from_i64(0, 32));
    CHECK(llg_queue_delete_index(&queue, sv4_from_u64(3, 32, 0)));
    CHECK(llg_queue_ref_write(&queue, surviving, sv4_from_i64(22, 32)));
    CHECK(!llg_queue_ref_write(&queue, removed, sv4_from_i64(99, 32)));
    CHECK(sv4_to_i64(llg_queue_get(&queue, sv4_from_u64(2, 32, 0))) == 22);
    llg_queue_destroy(&queue);

    llg_queue_init(&queue, 32, 1, 0, 3);
    llg_queue_assign_values(&queue, values, 3);
    surviving = llg_queue_ref_identity(&queue, 1);
    removed = llg_queue_ref_identity(&queue, 2);
    llg_queue_push_front(&queue, sv4_from_i64(0, 32));
    CHECK(llg_queue_ref_write(&queue, surviving, sv4_from_i64(44, 32)));
    CHECK(!llg_queue_ref_write(&queue, removed, sv4_from_i64(99, 32)));
    llg_queue_destroy(&queue);
    return 0;
}

static int check_array_methods(void) {
    sv4_t values[5] = {
        sv4_from_i64(1, 32), sv4_from_i64(3, 32), sv4_from_i64(2, 32),
        sv4_from_i64(3, 32), sv4_from_i64(4, 32)
    };
    llg_queue_t source;
    llg_queue_t result;
    llg_queue_init(&source, 32, 1, 0, UINT64_MAX);
    llg_queue_init(&result, 32, 1, 0, UINT64_MAX);
    llg_queue_assign_values(&source, values, 5);

    llg_queue_method_assign(&result, &source, LLG_CONTAINER_METHOD_FIND,
                            eval_gt_two, NULL);
    CHECK(result.size == 3 && sv4_to_i64(result.data[0]) == 3 &&
          sv4_to_i64(result.data[1]) == 3 && sv4_to_i64(result.data[2]) == 4);
    llg_queue_method_assign(&result, &source,
                            LLG_CONTAINER_METHOD_FIND_LAST_INDEX, eval_gt_two,
                            NULL);
    CHECK(result.size == 1 && sv4_to_i64(result.data[0]) == 4);
    llg_queue_method_assign(&result, &source,
                            LLG_CONTAINER_METHOD_FIND_LAST_INDEX, eval_eq_one,
                            NULL);
    CHECK(result.size == 1 && sv4_to_i64(result.data[0]) == 0);
    llg_queue_method_assign(&result, &source, LLG_CONTAINER_METHOD_MIN,
                            eval_scale_257, NULL);
    CHECK(result.size == 1 && sv4_to_i64(result.data[0]) == 1);
    llg_queue_method_assign(&result, &source,
                            LLG_CONTAINER_METHOD_UNIQUE_INDEX, NULL, NULL);
    CHECK(result.size == 4 && sv4_to_i64(result.data[0]) == 0 &&
          sv4_to_i64(result.data[1]) == 1 && sv4_to_i64(result.data[2]) == 2 &&
          sv4_to_i64(result.data[3]) == 4);

    sv4_t reduced = llg_queue_reduce_with(
        &source, LLG_CONTAINER_REDUCE_SUM, 64, 0, 0, eval_scale_257, NULL);
    CHECK(reduced.width == 64 && sv4_to_u64(reduced) == UINT64_C(3341));
    reduced = llg_queue_reduce_with(
        &source, LLG_CONTAINER_REDUCE_SUM, 32, 1, 0, eval_index, NULL);
    CHECK(reduced.width == 32 && sv4_to_i64(reduced) == 10);

    llg_queue_method(&source, LLG_CONTAINER_METHOD_SORT, eval_identity, NULL);
    CHECK(sv4_to_i64(source.data[0]) == 1 && sv4_to_i64(source.data[1]) == 2 &&
          sv4_to_i64(source.data[2]) == 3 && sv4_to_i64(source.data[3]) == 3 &&
          sv4_to_i64(source.data[4]) == 4);
    llg_queue_method(&source, LLG_CONTAINER_METHOD_REVERSE, NULL, NULL);
    CHECK(sv4_to_i64(source.data[0]) == 4 && sv4_to_i64(source.data[4]) == 1);

    llg_queue_t shuffled_a;
    llg_queue_t shuffled_b;
    llg_queue_init(&shuffled_a, 32, 1, 0, UINT64_MAX);
    llg_queue_init(&shuffled_b, 32, 1, 0, UINT64_MAX);
    llg_queue_assign_values(&shuffled_a, values, 5);
    llg_queue_assign_values(&shuffled_b, values, 5);
    llg_container_seed(UINT64_C(123));
    llg_queue_method(&shuffled_a, LLG_CONTAINER_METHOD_SHUFFLE, NULL, NULL);
    llg_container_seed(UINT64_C(123));
    llg_queue_method(&shuffled_b, LLG_CONTAINER_METHOD_SHUFFLE, NULL, NULL);
    for (size_t i = 0; i < 5; ++i)
        CHECK(sv4_same(shuffled_a.data[i], shuffled_b.data[i]));

    llg_dyn_array_t dynamic;
    llg_dyn_init(&dynamic, 32, 1, 0);
    llg_dyn_assign_values(&dynamic, values, 5);
    llg_dyn_method_assign(&result, &dynamic,
                          LLG_CONTAINER_METHOD_FIND_FIRST_INDEX, eval_gt_two,
                          NULL);
    CHECK(result.size == 1 && sv4_to_i64(result.data[0]) == 1);

    llg_assoc_t associative;
    llg_assoc_init_integral(&associative, 32, 1, 0, 8, 0, 0);
    for (uint64_t i = 0; i < 5; ++i)
        CHECK(llg_assoc_set_integral(&associative,
                                     sv4_from_u64(i, 8, 0), values[i]));
    llg_assoc_method_assign(&result, &associative,
                            LLG_CONTAINER_METHOD_FIND_INDEX, eval_gt_two,
                            NULL);
    CHECK(result.size == 3 && sv4_to_i64(result.data[0]) == 1 &&
          sv4_to_i64(result.data[1]) == 3 && sv4_to_i64(result.data[2]) == 4);
    llg_assoc_method_assign(&result, &associative,
                            LLG_CONTAINER_METHOD_UNIQUE, NULL, NULL);
    CHECK(result.size == 4 && sv4_to_i64(result.data[0]) == 1 &&
          sv4_to_i64(result.data[1]) == 3 && sv4_to_i64(result.data[2]) == 2 &&
          sv4_to_i64(result.data[3]) == 4);
    reduced = llg_assoc_reduce_with(
        &associative, LLG_CONTAINER_REDUCE_SUM, 64, 0, 0, eval_scale_257,
        NULL);
    CHECK(reduced.width == 64 && sv4_to_u64(reduced) == UINT64_C(3341));

    llg_assoc_destroy(&associative);
    llg_dyn_destroy(&dynamic);
    llg_queue_destroy(&shuffled_b);
    llg_queue_destroy(&shuffled_a);
    llg_queue_destroy(&result);
    llg_queue_destroy(&source);
    return 0;
}

static int check_recursive_values(void) {
    static const llg_value_desc_t real_desc = {
        .kind = LLG_VALUE_REAL
    };
    static const llg_value_desc_t shortreal_desc = {
        .kind = LLG_VALUE_REAL, .real_short = 1
    };
    static const llg_value_desc_t string_desc = {
        .kind = LLG_VALUE_STRING
    };
    static const llg_value_desc_t chandle_desc = {
        .kind = LLG_VALUE_CHANDLE
    };

    llg_dyn_value_array_t real_source;
    llg_dyn_value_array_t array;
    llg_dyn_value_init(&real_source, &real_desc);
    llg_dyn_value_init(&array, &shortreal_desc);
    double precise = 1.0 + 0x1p-25;
    llg_dyn_value_assign_reals(&real_source, &precise, 1);
    llg_dyn_value_copy(&array, &real_source);
    CHECK(real_bits(llg_dyn_value_get_real(
              &array, sv4_from_u64(0, 32, 0))) ==
          real_bits((double)(float)precise));
    array.notify = notify;
    changes = 0;
    CHECK(llg_dyn_value_set_real(&array, sv4_from_u64(0, 32, 0), -0.0));
    CHECK(changes == 1);
    CHECK(real_bits(llg_dyn_value_get_real(
              &array, sv4_from_u64(0, 32, 0))) == UINT64_C(0x8000000000000000));
    double nan;
    uint64_t nan_bits = UINT64_C(0x7ff8000000000001);
    memcpy(&nan, &nan_bits, sizeof(nan));
    CHECK(llg_dyn_value_set_real(&array, sv4_from_u64(0, 32, 0), nan));
    CHECK(changes == 2);
    CHECK(llg_dyn_value_set_real(&array, sv4_from_u64(0, 32, 0), nan));
    CHECK(changes == 2);
    llg_dyn_value_destroy(&array);
    llg_dyn_value_destroy(&real_source);

    llg_queue_value_array_t real_queue;
    llg_queue_value_array_t string_queue;
    llg_queue_value_array_t handle_queue;
    llg_queue_value_init(&real_queue, &real_desc, UINT64_MAX);
    llg_queue_value_init(&string_queue, &string_desc, UINT64_MAX);
    llg_queue_value_init(&handle_queue, &chandle_desc, UINT64_MAX);
    CHECK(llg_queue_value_set_real(
        &real_queue, sv4_from_u64(0, 32, 0), 2.5));
    CHECK(llg_queue_value_set_string(
        &string_queue, sv4_from_u64(0, 32, 0), llg_string_bytes("ok", 2)));
    CHECK(llg_queue_value_set_chandle(
        &handle_queue, sv4_from_u64(0, 32, 0), NULL));
    CHECK(real_queue.size == 1 && string_queue.size == 1 && handle_queue.size == 1);
    CHECK(llg_queue_value_get_real(
              &real_queue, sv4_from_u64(0, 32, 0)) == 2.5);
    llg_string_t text = llg_queue_value_get_string(
        &string_queue, sv4_from_u64(0, 32, 0));
    CHECK(text.len == 2 && memcmp(text.data, "ok", 2) == 0);
    llg_string_destroy(&text);

    llg_queue_value_array_t bounded_real_queue;
    llg_queue_value_init(&bounded_real_queue, &real_desc, 1);
    CHECK(llg_queue_value_set_real(
        &bounded_real_queue, sv4_from_u64(0, 32, 0), 1.25));
    CHECK(!llg_queue_value_set_real(
        &bounded_real_queue, sv4_from_u64(1, 32, 0), 9.5));
    CHECK(bounded_real_queue.size == 1);
    CHECK(llg_queue_value_get_real(
              &bounded_real_queue, sv4_from_u64(0, 32, 0)) == 1.25);
    llg_queue_value_destroy(&bounded_real_queue);

    llg_queue_value_destroy(&handle_queue);
    llg_queue_value_destroy(&string_queue);
    llg_queue_value_destroy(&real_queue);
    return 0;
}

int main(void) {
    CHECK(check_packed_conversions() == 0);
    CHECK(check_queue_references() == 0);
    CHECK(check_array_methods() == 0);
    CHECK(check_recursive_values() == 0);
    puts("runtime container isolation ok");
    return 0;
}
"#;

#[test]
fn container_runtime_compiles_and_runs_without_scheduler() {
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    if Command::new(&compiler).arg("--version").output().is_err() {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-containers").expect("create temp directory");
    let (value_header, value_implementation) = llg::sim::rt::value_sources();
    let (string_header, string_implementation) = llg::sim::rt::string_sources();
    let (container_header, container_implementation) = llg::sim::rt::container_sources();
    for (name, contents) in [
        ("llg_value.h", value_header),
        ("llg_value.c", value_implementation),
        ("llg_string.h", string_header),
        ("llg_string.c", string_implementation),
        ("llg_container.h", container_header),
        ("llg_container.c", container_implementation),
        ("runtime_containers_probe.c", CONTAINER_PROBE),
    ] {
        std::fs::write(dir.path().join(name), contents).expect("write runtime source");
    }

    let executable = dir.path().join("runtime_containers_probe");
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
            "llg_string.c",
            "llg_container.c",
            "runtime_containers_probe.c",
            "-lm",
            "-o",
        ])
        .arg(&executable);
    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone container runtime must compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let stdout = sim_harness::run_executable(&executable).expect("container probe should run");
    assert_eq!(stdout, "runtime container isolation ok\n");
}
