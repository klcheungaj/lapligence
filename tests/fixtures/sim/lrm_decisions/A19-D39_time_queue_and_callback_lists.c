/* Decision A19-D39 companion: list the time queue, the callbacks of each
 * time queue, of NULL and of a variable, the user systfs and the reference
 * of iterators. */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 ignore(p_cb_data data) {
    (void)data;
    return 0;
}

static void place(PLI_INT32 reason, vpiHandle obj, PLI_UINT32 delay, const char* tag) {
    s_cb_data data;
    s_vpi_time time;
    s_vpi_value value;
    memset(&data, 0, sizeof data);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = delay;
    value.format = vpiIntVal;
    data.reason = reason;
    data.cb_rtn = ignore;
    data.obj = obj;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)tag;
    vpi_register_cb(&data);
}

static void list_callbacks(const char* label, vpiHandle reference) {
    vpiHandle it = vpi_iterate(vpiCallback, reference), cb;
    vpi_printf("  callbacks of %s:", label);
    while (it && (cb = vpi_scan(it))) {
        s_cb_data info;
        vpi_get_cb_info(cb, &info);
        vpi_printf(" %s(%d)", info.user_data ? (const char*)info.user_data : "-", (int)info.reason);
    }
    vpi_printf("\n");
}

static PLI_INT32 d39(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_free_object(args);
    int step = value.value.integer;
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("step %d at %u\n", step, (unsigned)now.low);
    vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)"tb.a", NULL);
    if (step == 1) {
        place(cbAfterDelay, NULL, 5, "after5");
        place(cbReadOnlySynch, NULL, 0, "ro0");
        place(cbEndOfSimulation, NULL, 0, "end");
        place(cbValueChange, a, 0, "va");
    }
    vpiHandle it = vpi_iterate(vpiTimeQueue, NULL), queue;
    while (it && (queue = vpi_scan(it))) {
        s_vpi_time time;
        time.type = vpiSimTime;
        vpi_get_time(queue, &time);
        vpi_printf("  queue type=%d", (int)vpi_get(vpiType, queue));
        vpi_printf(" time %u\n", (unsigned)time.low);
        list_callbacks("the queue", queue);
    }
    list_callbacks("NULL", NULL);
    list_callbacks("tb.a", a);
    if (step == 2) {
        it = vpi_iterate(vpiUserSystf, NULL);
        vpiHandle systf;
        while (it && (systf = vpi_scan(it))) {
            s_vpi_systf_data info;
            vpi_get_systf_info(systf, &info);
            if (strcmp((const char*)info.tfname, "$d39") == 0 ||
                strcmp((const char*)info.tfname, "$d39_unused") == 0)
                vpi_printf("  systf %s type=%d\n", (const char*)info.tfname, (int)info.type);
        }
        vpiHandle top = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
        it = vpi_iterate(vpiReg, top);
        vpi_printf("  reg iterator use is tb: %d\n", (int)vpi_compare_objects(vpi_handle(vpiUse, it), top));
        vpi_free_object(it);
        it = vpi_iterate(vpiModule, NULL);
        vpi_printf("  module iterator use: %s\n", vpi_handle(vpiUse, it) ? "object" : "NULL");
        vpi_free_object(it);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d39";
    task.calltf = d39;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$d39_unused";
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
