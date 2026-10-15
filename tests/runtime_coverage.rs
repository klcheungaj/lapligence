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
    llg_cov_hit(inst, point, &v, NULL);
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
static const llg_cov_item_t items[] = {
    {.lo = 0u, .hi = 3u, .lo_kind = LLG_COV_BOUND_KEY, .hi_kind = LLG_COV_BOUND_KEY},
    {.lo = 2u, .hi = 5u, .lo_kind = LLG_COV_BOUND_KEY, .hi_kind = LLG_COV_BOUND_KEY},
};
static const llg_cov_bin_t bins[] = {
    {.name = "b0", .site = "probe", .guard = -1, .first_item = 0u, .n_items = 1u},
    {.name = "b1", .site = "probe", .guard = -1, .first_item = 1u, .n_items = 1u},
    {.name = "b2", .site = "probe", .guard = -1, .first_item = 2u, .n_items = 0u},
};
static const llg_cov_point_t points[] = {
    {.name = "u2", .width = 2u, .kind = LLG_COV_POINT_AUTO},
    {.name = "s3", .width = 3u, .is_signed = 1u, .kind = LLG_COV_POINT_AUTO},
    {.name = "u3", .width = 3u, .kind = LLG_COV_POINT_AUTO},
    {.name = "e", .width = 3u, .kind = LLG_COV_POINT_ENUM, .n_keys = 3u, .keys = enum_keys},
    {.name = "x", .width = 4u, .kind = LLG_COV_POINT_EXPLICIT, .n_bins = 3u, .bins = bins,
     .items = items},
    {.name = "wide", .width = 16u, .kind = LLG_COV_POINT_AUTO},
};
static llg_cov_type_t type_a = {.name = "a", .weight = 1};
static llg_cov_group_t group_a = {.name = "a", .n_points = 6u, .points = points, .type = &type_a};
static llg_cov_type_t type_b = {.name = "b", .weight = 3};
static llg_cov_group_t group_b = {.name = "b", .n_points = 1u, .points = points, .type = &type_b};
static const llg_cov_point_t empty_points[] = {
    {.name = "none", .width = 4u, .kind = LLG_COV_POINT_EXPLICIT, .n_bins = 1u, .bins = bins + 2,
     .items = items},
};
static llg_cov_type_t type_empty = {.name = "empty", .weight = 1};
static llg_cov_group_t group_empty = {.name = "empty", .n_points = 1u, .points = empty_points,
                                      .type = &type_empty};

/* Bins that read argument slot 0, resolved per instance by 19.5.6:
 * `bins lim = {[0:arg]}; bins high = {[arg:$]};`. */
static const llg_cov_item_t slot_items[] = {
    {.lo = 0u, .hi = 0u, .lo_kind = LLG_COV_BOUND_KEY, .hi_kind = LLG_COV_BOUND_SLOT,
     .site = "probe:1:1"},
    {.lo = 0u, .hi = 0u, .lo_kind = LLG_COV_BOUND_SLOT, .hi_kind = LLG_COV_BOUND_MAX,
     .site = "probe:2:1"},
};
static const llg_cov_bin_t slot_bins[] = {
    {.name = "lim", .site = "probe", .guard = -1, .first_item = 0u, .n_items = 1u},
    {.name = "high", .site = "probe", .guard = -1, .first_item = 1u, .n_items = 1u},
};
static const llg_cov_point_t slot_points[] = {
    {.name = "s", .width = 4u, .kind = LLG_COV_POINT_EXPLICIT, .dynamic = 1u, .n_bins = 2u,
     .bins = slot_bins, .items = slot_items},
};
static llg_cov_type_t type_slot = {.name = "slot", .weight = 0};
static llg_cov_group_t group_slot = {.name = "slot", .n_points = 1u, .points = slot_points,
                                     .n_slots = 1u, .type = &type_slot};

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
    llg_cov_hit(a, 0, &unknown, NULL);
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

    /* Per-instance layouts: slot 0 is 3 in one instance and 12 in another;
     * `high` starts at the slot value, so 3 is in both bins of the first. */
    void* s3 = llg_cov_new(&group_slot);
    sv4_t three = sv4_from_u64(3u, 4u, 0);
    llg_cov_slot(s3, 0u, three);
    sv4_destroy(&three);
    llg_cov_ready(s3);
    void* s12 = llg_cov_new(&group_slot);
    sv4_t twelve = sv4_from_u64(12u, 4u, 0);
    llg_cov_slot(s12, 0u, twelve);
    sv4_destroy(&twelve);
    llg_cov_ready(s12);
    hit(s3, 0, 3, 4);
    hit(s12, 0, 13, 4);
    NEAR(llg_cov_inst_coverage(s3, 0), 100.0);
    NEAR(llg_cov_inst_coverage(s12, 0), 50.0);

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
