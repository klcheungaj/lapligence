/* Companion of A20-D5_value_array_errors.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void get(const char* label, const char* name, PLI_UINT32 format, PLI_UINT32 flags,
                PLI_INT32 start, PLI_UINT32 num) {
    PLI_INT32 buffer[8];
    PLI_INT32 index[1] = {start};
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = format;
    array.flags = flags | vpiUserAllocFlag;
    array.value.integers = buffer;
    vpi_get_value_array(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &array, index, num);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("get %s: level=%d value=%s", label, (int)level,
               array.value.integers ? "set" : "NULL");
    if (array.value.integers && format == vpiIntVal)
        for (PLI_UINT32 i = 0; i < num; ++i) vpi_printf(" %d", (int)buffer[i]);
    vpi_printf("\n");
}

static void put(const char* label, PLI_UINT32 flags, PLI_INT32 start, PLI_UINT32 num) {
    PLI_INT32 values[4] = {91, 92, 93, 94};
    PLI_INT32 index[1] = {start};
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    array.flags = flags;
    array.value.integers = values;
    vpi_put_value_array(vpi_handle_by_name((PLI_BYTE8*)"tb.b", NULL), &array, index, num);
    vpi_printf("put %s: level=%d\n", label, (int)vpi_chk_error(NULL));
}

static PLI_INT32 d5(PLI_BYTE8* data) {
    (void)data;
    get("b[1..2]", "tb.b", vpiIntVal, 0, 1, 2);
    get("count 0", "tb.b", vpiIntVal, 0, 0, 0);
    get("past the end", "tb.b", vpiIntVal, 0, 2, 3);
    get("index 4", "tb.b", vpiIntVal, 0, 4, 1);
    get("vpiOneValue", "tb.b", vpiIntVal, vpiOneValue, 0, 1);
    get("vpiPropagateOff", "tb.b", vpiIntVal, vpiPropagateOff, 0, 1);
    get("vpiRealVal of bit", "tb.b", vpiRealVal, 0, 0, 1);
    get("vpiShortIntVal of int", "tb.iv", vpiShortIntVal, 0, 0, 1);
    get("vpiBinStrVal", "tb.b", vpiBinStrVal, 0, 0, 1);
    put("count 0", 0, 0, 0);
    put("past the end", 0, 2, 3);
    put("vpiUserAllocFlag", vpiUserAllocFlag, 0, 1);
    put("vpiInertialDelay", vpiInertialDelay, 0, 1);
    put("vpiNoDelay b[3]", vpiNoDelay, 3, 1);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d5";
    task.calltf = d5;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
