/* Companion of A19-D69_named_event_arrays.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* text(vpiHandle h, int property) {
    const char* s = h ? vpi_get_str(property, h) : NULL;
    return s ? s : "NULL";
}

static void iterate(const char* label, vpiHandle h, int relation) {
    vpiHandle it = vpi_iterate(relation, h), item;
    int error = level() != 0, count = 0;
    vpi_printf("  %s:", label);
    while (it && (item = vpi_scan(it))) {
        ++count;
        if (relation == vpiRange) {
            s_vpi_value v;
            vpiHandle l = vpi_handle(vpiLeftRange, item), r = vpi_handle(vpiRightRange, item);
            v.format = vpiIntVal;
            vpi_get_value(l, &v);
            vpi_printf(" [%d", (int)v.value.integer);
            vpi_get_value(r, &v);
            vpi_printf(":%d]", (int)v.value.integer);
        } else if (relation == vpiIndex) {
            s_vpi_value v;
            v.format = vpiIntVal;
            vpi_get_value(item, &v);
            vpi_printf(" %d", (int)v.value.integer);
        } else {
            vpi_printf(" %s", text(item, vpiName));
        }
    }
    vpi_printf(" (%d) error=%d\n", count, error);
}

static void element(const char* label, vpiHandle e) {
    vpiHandle parent, index;
    s_vpi_value v;
    if (!e) {
        vpi_printf("%s: NULL error=%d\n", label, level() != 0);
        return;
    }
    vpi_printf("%s: %s type=%d full=%s array_member=%d\n", label, text(e, vpiName),
               (int)vpi_get(vpiType, e), text(e, vpiFullName), (int)vpi_get(vpiArrayMember, e));
    parent = vpi_handle(vpiParent, e);
    vpi_printf("  parent=%s\n", text(parent, vpiFullName));
    index = vpi_handle(vpiIndex, e);
    if (index) {
        v.format = vpiIntVal;
        vpi_get_value(index, &v);
        vpi_printf("  index=%d\n", (int)v.value.integer);
    } else {
        vpi_printf("  index NULL error=%d\n", level() != 0);
    }
    iterate("indices", e, vpiIndex);
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("  value change on %s\n", text(data->obj, vpiFullName));
    return 0;
}

static PLI_INT32 d69(PLI_BYTE8* user) {
    vpiHandle tb = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle ev = vpi_handle_by_name((PLI_BYTE8*)"tb.ev", NULL);
    vpiHandle ev2 = vpi_handle_by_name((PLI_BYTE8*)"tb.ev2", NULL);
    vpiHandle e1, waiter;
    PLI_INT32 indices[2];
    s_cb_data cb;
    s_vpi_time time;
    s_vpi_value value;
    (void)user;
    iterate("tb named event arrays", tb, vpiNamedEventArray);
    iterate("tb named events", tb, vpiNamedEvent);
    vpi_printf("ev: type=%d size=%d module=%s\n", (int)vpi_get(vpiType, ev),
               (int)vpi_get(vpiSize, ev), text(vpi_handle(vpiModule, ev), vpiName));
    iterate("ev ranges", ev, vpiRange);
    iterate("ev elements", ev, vpiNamedEvent);
    iterate("ev2 ranges", ev2, vpiRange);
    iterate("ev2 elements", ev2, vpiNamedEvent);
    e1 = vpi_handle_by_index(ev, 1);
    element("ev by index 1", e1);
    element("ev by index 5", vpi_handle_by_index(ev, 5));
    indices[0] = 0;
    indices[1] = 1;
    element("ev2 by indices 0 1", vpi_handle_by_multi_index(ev2, 2, indices));
    element("ev2 by index 1", vpi_handle_by_index(ev2, 1));
    element("ev2[1][0] by name", vpi_handle_by_name((PLI_BYTE8*)"tb.ev2[1][0]", NULL));
    vpi_printf("by name compares=%d\n",
               (int)vpi_compare_objects(e1, vpi_handle_by_name((PLI_BYTE8*)"tb.ev[1]", NULL)));
    vpi_printf("array value:");
    value.format = vpiIntVal;
    vpi_get_value(ev, &value);
    vpi_printf(" error=%d\n", level() != 0);
    {
        vpiHandle it = vpi_iterate(vpiWaitingProcesses, e1);
        int count = 0;
        while (it && (waiter = vpi_scan(it))) ++count;
        vpi_printf("ev[1] waiting=%d error=%d\n", count, level() != 0);
    }
    memset(&cb, 0, sizeof cb);
    time.type = vpiSuppressTime;
    value.format = vpiSuppressVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = e1;
    cb.time = &time;
    cb.value = &value;
    vpi_printf("callback=%d\n", vpi_register_cb(&cb) != NULL);
    value.format = vpiIntVal;
    value.value.integer = 0;
    vpi_put_value(e1, &value, NULL, vpiNoDelay);
    vpi_printf("put error=%d\n", level() != 0);
    return 0;
}

static void register_d69(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d69";
    tf.calltf = d69;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d69, 0};
