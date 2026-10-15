#include "llg_vpi.h"
#include <stdio.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "VPI array metadata check failed at line %d\n", __LINE__); \
        return 1; \
    } \
} while (0)

/* Model initialization only records the catalog; it is validated on the
 * first VPI use, which then fails with a vpi_chk_error() error. */
static int rejected(llg_vpi_model_object_t* objects) {
    if (!llg_vpi_model_init("tb", objects, 3)) return 0;
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    int ok = handle == NULL && vpi_chk_error(NULL) == vpiError;
    llg_vpi_shutdown();
    return ok;
}

int main(void) {
    llg_rt_init();
    sv4_t cells[2] = {SV4_EMPTY, SV4_EMPTY};
    double reals[3] = {0.0, 0.0, 0.0};
    static const int64_t packed_dims[] = {0, 1};
    static const int64_t real_dims[] = {2, 0};
    llg_vpi_model_object_t objects[] = {
        {.type = vpiModule, .name = "tb", .full_name = "tb"},
        {.type = vpiRegArray, .name = "packed", .full_name = "tb.packed",
         .width = 8, .packed = cells, .parent = &objects[0], .subtype = vpiReg,
         .dims = packed_dims, .dim_count = 1},
        {.type = vpiRegArray, .name = "reals", .full_name = "tb.reals",
         .is_real = 1, .real = reals, .parent = &objects[0], .subtype = vpiRealVar,
         .dims = real_dims, .dim_count = 1},
    };
    CHECK(llg_vpi_model_init("tb", objects, 3));
    vpiHandle array = vpi_handle_by_name((PLI_BYTE8*)"tb.reals", NULL);
    CHECK(vpi_get(vpiType, array) == vpiRegArray);
    CHECK(vpi_get(vpiSize, array) == 3);
    CHECK(vpi_get(vpiType, vpi_handle_by_index(array, 1)) == vpiRealVar);
    CHECK(vpi_handle_by_index(array, 3) == NULL && vpi_chk_error(NULL) == vpiError);
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();

    objects[1].width = 0;
    CHECK(rejected(objects));
    objects[1].width = 8;

    objects[2].width = 8;
    CHECK(rejected(objects));
    objects[2].width = 0;

    objects[1].dim_count = 0;
    CHECK(rejected(objects));
    objects[1].dim_count = 1;

    CHECK(llg_vpi_model_init("tb", objects, 3));
    CHECK(vpi_handle_by_name((PLI_BYTE8*)"tb.packed", NULL) != NULL);
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();
    llg_rt_cleanup();
    puts("vpi array metadata ok");
    return 0;
}
