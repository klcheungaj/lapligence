/* Companion of A20-D8_value_array_net_array.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d8(PLI_BYTE8* data) {
    (void)data;
    vpiHandle n = vpi_handle_by_name((PLI_BYTE8*)"tb.n", NULL);
    PLI_INT32 index[1] = {0};
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    vpi_get_value_array(n, &array, index, 2);
    vpi_printf("get level=%d values=%d %d\n", (int)vpi_chk_error(NULL),
               (int)array.value.integers[0], (int)array.value.integers[1]);
    PLI_INT32 values[2] = {1, 2};
    array.value.integers = values;
    vpi_put_value_array(n, &array, index, 2);
    vpi_printf("put level=%d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d8";
    task.calltf = d8;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
