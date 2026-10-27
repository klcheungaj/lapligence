/* Companion of A20-D7_value_array_propagate_off.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d7(PLI_BYTE8* data) {
    (void)data;
    PLI_INT32 values[2] = {5, 9};
    PLI_INT32 index[1] = {0};
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    array.flags = vpiPropagateOff;
    array.value.integers = values;
    vpi_put_value_array(vpi_handle_by_name((PLI_BYTE8*)"tb.m", NULL), &array, index, 2);
    vpi_printf("put level=%d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d7";
    task.calltf = d7;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
