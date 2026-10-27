/* Companion of A20-D6_value_array_raw_two_state.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d6(PLI_BYTE8* data) {
    (void)data;
    vpiHandle q = vpi_handle_by_name((PLI_BYTE8*)"tb.q", NULL);
    PLI_INT32 index[1] = {0};
    unsigned char raw[4];
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = vpiRawTwoStateVal;
    array.flags = vpiUserAllocFlag;
    array.value.rawvals = (PLI_BYTE8*)raw;
    vpi_get_value_array(q, &array, index, 2);
    vpi_printf("q two-state: %02x %02x level=%d\n", raw[0], raw[1], (int)vpi_chk_error(NULL));
    array.format = vpiRawFourStateVal;
    vpi_get_value_array(q, &array, index, 2);
    vpi_printf("q four-state: A=%02x B=%02x A=%02x B=%02x level=%d\n", raw[0], raw[1], raw[2],
               raw[3], (int)vpi_chk_error(NULL));

    unsigned char four[2] = {0x3c, 0xff};
    memset(&array, 0, sizeof(array));
    array.format = vpiRawFourStateVal;
    array.value.rawvals = (PLI_BYTE8*)four;
    vpi_put_value_array(vpi_handle_by_name((PLI_BYTE8*)"tb.b", NULL), &array, index, 1);
    vpi_printf("b four-state put level=%d\n", (int)vpi_chk_error(NULL));
    unsigned char two[1] = {0xc3};
    array.format = vpiRawTwoStateVal;
    array.value.rawvals = (PLI_BYTE8*)two;
    PLI_INT32 second[1] = {1};
    vpi_put_value_array(q, &array, second, 1);
    vpi_printf("q two-state put level=%d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d6";
    task.calltf = d6;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
