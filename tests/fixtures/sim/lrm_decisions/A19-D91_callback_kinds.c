/* Decision A19-D91: registers the callbacks llg refuses and prints whether
 * each registration returned a handle, its error level and every call. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 called(p_cb_data data) {
    vpi_printf("called %s\n", (const char*)data->user_data);
    return 0;
}

static void place(PLI_INT32 reason, const char* name, PLI_INT32 format, const char* label) {
    s_vpi_time time;
    time.type = vpiSuppressTime;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = called;
    data.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)label;
    vpiHandle cb = vpi_register_cb(&data);
    int level = vpi_chk_error(NULL);
    vpi_printf("%s: %s level=%d\n", label, cb ? "handle" : "NULL", level);
}

static PLI_INT32 d91(PLI_BYTE8* user) {
    (void)user;
    static const char* containers[] = {"tb.da", "tb.q", "tb.aa", "tb.s", NULL};
    for (int i = 0; containers[i]; ++i) {
        static char labels[8][48];
        snprintf(labels[2 * i], sizeof(labels[0]), "size %s int", containers[i]);
        snprintf(labels[2 * i + 1], sizeof(labels[0]), "size %s suppress", containers[i]);
        place(cbSizeChange, containers[i], vpiIntVal, labels[2 * i]);
        place(cbSizeChange, containers[i], vpiSuppressVal, labels[2 * i + 1]);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d91";
    task.calltf = d91;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
