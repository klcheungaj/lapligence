//! Direct runtime boundary coverage for the functional-coverage store
//! (IEEE 1800-2009 19.5.2, 19.7 and 19.11). Expected percentages are
//! computed by hand from the clause formulas.

use std::process::Command;
use std::time::Duration;

use crate::sim_harness;

use llg::sim;

const COVERAGE_PROBE: &str = r#"
#define LLG_MODEL_PROCESS_ABI 3
#include "llg_rt.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static int failed;

#define CHECK(condition)                                                      \
    do {                                                                       \
        if (!(condition)) {                                                    \
            fprintf(stderr, "coverage check failed at line %d: %s\n",         \
                    __LINE__, #condition);                                    \
            failed = 1;                                                        \
        }                                                                      \
    } while (0)
#define NEAR(value, expected) CHECK(fabs((value) - (expected)) < 1e-9)

static void hit(void* inst, uint32_t point, int64_t value, uint32_t width) {
    sv4_t v = sv4_from_i64(value, width);
    llg_cov_hit(inst, point, &v);
    sv4_destroy(&v);
}

static void option(void* inst, int32_t point, uint32_t which, int64_t value) {
    sv4_t v = sv4_from_i64(value, 32);
    llg_cov_option(inst, point, which, v, "probe");
    sv4_destroy(&v);
}

/* Coverpoints: unsigned 2-bit auto, signed 3-bit auto, unsigned 3-bit auto,
 * enum {0, 1, 5}, explicit {b0 = [0:3], b1 = [2:5], b2 = <no value>}, and a
 * 16-bit auto coverpoint for sparse storage. */
static const uint64_t enum_keys[] = {0u, 1u, 5u};
static const llg_cov_segment_t segments[] = {
    {0u, 1u, 0u, 1u}, {2u, 3u, 1u, 2u}, {4u, 5u, 3u, 1u}
};
static const uint32_t segment_bins[] = {0u, 0u, 1u, 1u};
static const uint8_t valued[] = {1u, 1u, 0u};
static const llg_cov_point_t points[] = {
    {"u2", 2u, 0u, LLG_COV_POINT_AUTO, 0u, NULL, 0u, NULL, NULL, NULL, 0u},
    {"s3", 3u, 1u, LLG_COV_POINT_AUTO, 0u, NULL, 0u, NULL, NULL, NULL, 0u},
    {"u3", 3u, 0u, LLG_COV_POINT_AUTO, 0u, NULL, 0u, NULL, NULL, NULL, 0u},
    {"e", 3u, 0u, LLG_COV_POINT_ENUM, 3u, enum_keys, 0u, NULL, NULL, NULL, 0u},
    {"x", 4u, 0u, LLG_COV_POINT_EXPLICIT, 3u, NULL, 3u, segments, segment_bins, valued, 2u},
    {"wide", 16u, 0u, LLG_COV_POINT_AUTO, 0u, NULL, 0u, NULL, NULL, NULL, 0u},
};
static llg_cov_type_t type_a = {"a", 1, NULL, NULL, 0u, NULL, 0u};
static llg_cov_group_t group_a = {"a", 6u, points, &type_a, NULL, 0u, 0u, NULL, 0u};
static llg_cov_type_t type_b = {"b", 3, NULL, NULL, 0u, NULL, 0u};
static llg_cov_group_t group_b = {"b", 1u, points, &type_b, NULL, 0u, 0u, NULL, 0u};
static const llg_cov_point_t empty_points[] = {
    {"none", 4u, 0u, LLG_COV_POINT_EXPLICIT, 1u, NULL, 0u, NULL, NULL, valued + 2, 0u},
};
static llg_cov_type_t type_empty = {"empty", 1, NULL, NULL, 0u, NULL, 0u};
static llg_cov_group_t group_empty = {"empty", 1u, empty_points, &type_empty, NULL, 0u, 0u, NULL, 0u};

static void object_trace(void* object, llg_gc_tracer_t* tracer) {
    (void)object;
    (void)tracer;
}
static void object_finalize(void* object) { (void)object; }
static const llg_gc_type_t object_type = {"probe", object_trace, NULL, object_finalize, NULL};

int main(void) {
    llg_rt_init();
    NEAR(llg_cov_total_coverage(), 100.0);
    /* No instance: the type weight is nonzero, so 0.0 (19.11). */
    NEAR(llg_cov_type_coverage(&type_a, -1, 1), 0.0);
    NEAR(llg_cov_type_coverage(&type_a, -1, 0), 100.0);

    void* a = llg_cov_new(&group_a);
    option(a, 1, LLG_COV_OPTION_AUTO_BIN_MAX, 3);
    option(a, 2, LLG_COV_OPTION_AUTO_BIN_MAX, 3);
    option(a, 5, LLG_COV_OPTION_AUTO_BIN_MAX, 10000);
    llg_cov_ready(a);
    /* Unsampled: every coverpoint is 0.0. */
    NEAR(llg_cov_inst_coverage(a, -1), 0.0);
    hit(a, 0, 0, 2);
    hit(a, 0, 1, 2);
    hit(a, 0, 1, 2);
    NEAR(llg_cov_inst_coverage(a, 0), 50.0);
    /* 3 bins over 8 values: <0:1>, <2:3>, <4:7> in numeric order. */
    hit(a, 1, 0, 3);
    hit(a, 1, 2, 3);
    NEAR(llg_cov_inst_coverage(a, 1), 100.0 / 3.0);
    hit(a, 2, 0, 3);
    hit(a, 2, 2, 3);
    NEAR(llg_cov_inst_coverage(a, 2), 200.0 / 3.0);
    hit(a, 3, 5, 3);
    hit(a, 3, 3, 3);
    NEAR(llg_cov_inst_coverage(a, 3), 100.0 / 3.0);
    /* Overlapping bins both count a shared value; b2 has no value. */
    hit(a, 4, 2, 4);
    NEAR(llg_cov_inst_coverage(a, 4), 100.0);
    sv4_t unknown = sv4_from_masks(2u, 1u, 0u, 2u, 0);
    llg_cov_hit(a, 0, &unknown);
    sv4_destroy(&unknown);
    NEAR(llg_cov_inst_coverage(a, 0), 50.0);
    hit(a, 5, 3, 16);
    /* 65536 values over 10000 bins of 6 values; the last bin takes the rest. */
    hit(a, 5, 7, 16);
    hit(a, 5, 65535, 16);
    NEAR(llg_cov_inst_coverage(a, 5), 300.0 / 10000.0);
    /* Equal coverpoint weights: (50 + 33.3 + 66.7 + 33.3 + 100 + 0.03) / 6. */
    double expected = (50.0 + 100.0 / 3.0 + 200.0 / 3.0 + 100.0 / 3.0 + 100.0 + 0.03) / 6.0;
    NEAR(llg_cov_inst_coverage(a, -1), expected);
    option(a, 0, LLG_COV_OPTION_WEIGHT, 3);
    NEAR(llg_cov_inst_coverage(a, -1),
         (3.0 * 50.0 + 100.0 / 3.0 + 200.0 / 3.0 + 100.0 / 3.0 + 100.0 + 0.03) / 8.0);

    /* stop()/start() and a group-level at_least default. */
    void* b = llg_cov_new(&group_b);
    option(b, -1, LLG_COV_OPTION_AT_LEAST, 2);
    llg_cov_ready(b);
    llg_cov_control(b, -1, 0);
    hit(b, 0, 3, 2);
    hit(b, 0, 3, 2);
    NEAR(llg_cov_inst_coverage(b, -1), 0.0);
    llg_cov_control(b, -1, 1);
    hit(b, 0, 3, 2);
    NEAR(llg_cov_inst_coverage(b, -1), 0.0);
    hit(b, 0, 3, 2);
    NEAR(llg_cov_inst_coverage(b, -1), 25.0);
    llg_cov_control(b, 0, 0);
    hit(b, 0, 2, 2);
    hit(b, 0, 2, 2);
    NEAR(llg_cov_inst_coverage(b, -1), 25.0);

    /* Two instances of one type: weighted by instance option.weight. */
    void* b2 = llg_cov_new(&group_b);
    option(b2, -1, LLG_COV_OPTION_WEIGHT, 3);
    llg_cov_ready(b2);
    hit(b2, 0, 0, 2);
    hit(b2, 0, 1, 2);
    NEAR(llg_cov_inst_coverage(b2, -1), 50.0);
    NEAR(llg_cov_type_coverage(&type_b, -1, 3), (1.0 * 25.0 + 3.0 * 50.0) / 4.0);

    /* A coverpoint without valued bins is excluded: 0.0, or 100.0 at weight 0. */
    void* e = llg_cov_new(&group_empty);
    llg_cov_ready(e);
    hit(e, 0, 1, 4);
    NEAR(llg_cov_inst_coverage(e, -1), 0.0);
    void* e0 = llg_cov_new(&group_empty);
    option(e0, -1, LLG_COV_OPTION_WEIGHT, 0);
    llg_cov_ready(e0);
    NEAR(llg_cov_inst_coverage(e0, -1), 100.0);
    NEAR(llg_cov_type_coverage(&type_empty, -1, 1), 0.0);

    /* $get_coverage: types weighted by type_option.weight; the empty type
     * does not contribute. */
    double total = (1.0 * llg_cov_type_coverage(&type_a, -1, 1) +
                    3.0 * llg_cov_type_coverage(&type_b, -1, 3)) / 4.0;
    NEAR(llg_cov_total_coverage(), total);

    /* A negative weight is an error that keeps the previous value. */
    option(b2, -1, LLG_COV_OPTION_WEIGHT, -1);
    NEAR(llg_cov_type_coverage(&type_b, -1, 3), (1.0 * 25.0 + 3.0 * 50.0) / 4.0);

    void* object = llg_gc_alloc(sizeof(llg_gc_header_t), &object_type);
    llg_cov_register(&group_a, object);
    llg_cov_register(&group_a, NULL);
    CHECK(llg_cov_registered(&group_a) == 1u);
    CHECK(llg_cov_registered_at(&group_a, 0) == object);
    CHECK(llg_cov_registered_at(&group_a, 1) == NULL);
    llg_gc_collect();
    CHECK(llg_gc_is_object(object));

    llg_rt_cleanup();
    CHECK(type_a.first == NULL && type_a.n_instances == 0u && !type_a.used);
    CHECK(group_a.registry == NULL && group_a.n_registered == 0u);
    NEAR(llg_cov_total_coverage(), 100.0);
    return failed;
}
"#;

#[test]
fn coverage_store_runtime_boundary_compiles_and_runs() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("runtime-coverage").expect("create temp directory");
    let executable =
        sim::build::build_model_cmake(dir.path(), &[("runtime_coverage_probe.c", COVERAGE_PROBE)])
            .expect("coverage runtime probe should compile");
    let output = sim_harness::run_command(&mut Command::new(&executable), Duration::from_secs(60))
        .expect("coverage runtime probe should run");
    assert!(output.status.success(), "coverage probe failed: {output:?}");
}
