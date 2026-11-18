/* Decision A19-D91: registers cbSizeChange and the callbacks llg refuses
 * and prints whether each registration returned a handle, its error level
 * and every call with its time and the size the object reports then. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle da_size;

static PLI_INT32 called(p_cb_data data) {
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("t=%u called %s", now.low, (const char*)data->user_data);
    if (data->reason == cbSizeChange)
        vpi_printf(" size=%d value=%s time=%s", (int)vpi_get(vpiSize, data->obj),
                   data->value ? "set" : "NULL", data->time ? "set" : "NULL");
    vpi_printf("\n");
    return 0;
}

static vpiHandle place_at(PLI_INT32 reason, vpiHandle obj, PLI_INT32 format, PLI_INT32 time_type,
                          const char* label) {
    s_vpi_time time;
    time.type = time_type;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = called;
    data.obj = obj;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)label;
    vpiHandle cb = vpi_register_cb(&data);
    int level = vpi_chk_error(NULL);
    vpi_printf("%s: %s level=%d\n", label, cb ? "handle" : "NULL", level);
    return cb;
}

static void place_on(PLI_INT32 reason, vpiHandle obj, PLI_INT32 format, const char* label) {
    place_at(reason, obj, format, vpiSuppressTime, label);
}

static void place(PLI_INT32 reason, const char* name, PLI_INT32 format, const char* label) {
    place_on(reason, vpi_handle_by_name((PLI_BYTE8*)name, NULL), format, label);
}

static PLI_INT32 d91(PLI_BYTE8* user) {
    (void)user;
    vpiHandle systf = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, systf);
    s_vpi_value step;
    step.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &step);
    vpi_free_object(args);
    if (step.value.integer == 1) {
        /* A removed record is not called again. */
        vpi_printf("remove da: %d\n", (int)vpi_remove_cb(da_size));
        return 0;
    }
    static const char* containers[] = {"tb.da", "tb.q", "tb.aa", "tb.s", NULL};
    for (int i = 0; containers[i]; ++i) {
        static char labels[8][48];
        snprintf(labels[2 * i], sizeof(labels[0]), "size %s int", containers[i]);
        snprintf(labels[2 * i + 1], sizeof(labels[0]), "size %s suppress", containers[i]);
        place(cbSizeChange, containers[i], vpiIntVal, labels[2 * i]);
        vpiHandle cb = place_at(cbSizeChange, vpi_handle_by_name((PLI_BYTE8*)containers[i], NULL),
                                vpiSuppressVal, i == 1 ? vpiSimTime : vpiSuppressTime,
                                labels[2 * i + 1]);
        if (i == 0) da_size = cb;
    }
    vpiHandle c = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.c", NULL));
    vpiHandle props = c ? vpi_iterate(vpiVariables, c) : NULL, prop;
    while (props && (prop = vpi_scan(props)))
        place_on(cbSizeChange, prop, vpiSuppressVal, "size c.d suppress");
    place(cbValueChange, "tb.h", vpiHexStrVal, "value chandle hex");
    place(cbValueChange, "tb.h", vpiStrengthVal, "value chandle strength");
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpiHandle it = vpi_iterate(vpiContAssign, tb), object;
    while ((object = vpi_scan(it))) place_on(cbValueChange, object, vpiBinStrVal, "value cont assign");
    it = vpi_iterate(vpiPrimitive, tb);
    while ((object = vpi_scan(it))) place_on(cbValueChange, object, vpiScalarVal, "value gate");
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
