#include "llg_vpi.h"
#include <stdio.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "VPI array metadata check failed at line %d\n", __LINE__); \
        return 1; \
    } \
} while (0)

int main(void) {
    llg_rt_init();
    llg_vpi_model_object_t objects[] = {
        {.type = vpiModule, .name = "tb", .full_name = "tb"},
        {.type = vpiRegArray, .name = "packed", .full_name = "tb.packed",
         .width = 8, .parent = &objects[0]},
        {.type = vpiRegArray, .name = "reals", .full_name = "tb.reals",
         .is_real = 1, .parent = &objects[0]},
    };
    CHECK(llg_vpi_model_init("tb", objects, 3));
    CHECK(vpi_get(vpiType, vpi_handle_by_name((PLI_BYTE8*)"tb.reals", NULL)) ==
          vpiRegArray);
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();

    objects[1].width = 0;
    CHECK(!llg_vpi_model_init("tb", objects, 3));
    CHECK(vpi_chk_error(NULL));
    llg_vpi_shutdown();
    objects[1].width = 8;

    objects[2].width = 8;
    CHECK(!llg_vpi_model_init("tb", objects, 3));
    CHECK(vpi_chk_error(NULL));
    llg_vpi_shutdown();

    objects[2].width = 0;
    CHECK(llg_vpi_model_init("tb", objects, 3));
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();
    llg_rt_cleanup();
    puts("vpi array metadata ok");
    return 0;
}
