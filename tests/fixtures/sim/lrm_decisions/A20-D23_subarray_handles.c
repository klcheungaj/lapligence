/* Companion of A20-D23_subarray_handles.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void show(const char* label, vpiHandle handle) {
    PLI_INT32 level = vpi_chk_error(NULL);
    if (!handle) {
        vpi_printf("%s: NULL level=%d\n", label, (int)level);
        return;
    }
    s_vpi_value value;
    value.format = vpiHexStrVal;
    vpi_get_value(handle, &value);
    vpi_printf("%s: %s size=%d value=%s level=%d\n", label, vpi_get_str(vpiFullName, handle),
               (int)vpi_get(vpiSize, handle), value.value.str, (int)level);
}

static void multi(const char* label, const char* name, PLI_INT32 count, PLI_INT32* indices) {
    vpiHandle array = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    show(label, vpi_handle_by_multi_index(array, count, indices));
}

static PLI_INT32 d23(PLI_BYTE8* data) {
    (void)data;
    PLI_INT32 one[1] = {1};
    PLI_INT32 two[2] = {1, 2};
    PLI_INT32 bad[1] = {9};
    vpiHandle m = vpi_handle_by_name((PLI_BYTE8*)"tb.m", NULL);
    show("m by_index 1", vpi_handle_by_index(m, 1));
    multi("m multi {1}", "tb.m", 1, one);
    multi("m multi {9}", "tb.m", 1, bad);
    show("m by_name m[1]", vpi_handle_by_name((PLI_BYTE8*)"tb.m[1]", NULL));
    multi("m multi {1,2}", "tb.m", 2, two);
    PLI_INT32 m3_two[2] = {1, 0};
    multi("m3 multi {1,0}", "tb.m3", 2, m3_two);
    multi("m3 multi {1}", "tb.m3", 1, one);
    show("m3 by_name m3[1][0]", vpi_handle_by_name((PLI_BYTE8*)"tb.m3[1][0]", NULL));
    multi("w multi {1}", "tb.w", 1, one);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d23";
    task.calltf = d23;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
