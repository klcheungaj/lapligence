/* Companion of A19-D80_class_event_arrays.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* name_of(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiName, h) : NULL;
    return name ? name : "NULL";
}

/* The names an iteration of `type` from `ref` returns, then its count. */
static void names(const char* label, vpiHandle ref, PLI_INT32 type) {
    vpiHandle it = vpi_iterate(type, ref), x;
    int count = 0, error = level() != 0;
    vpi_printf("  %s:", label);
    while (it && (x = vpi_scan(it)) != NULL) {
        vpi_printf(" %s", name_of(x));
        ++count;
    }
    vpi_printf(" (%d) error=%d\n", count, error);
}

static void array(vpiHandle a) {
    vpiHandle it, r, left, right;
    s_vpi_value value;
    const char* full;
    int error;
    memset(&value, 0, sizeof value);
    vpi_printf("%s type=%d size=%d\n", name_of(a), (int)vpi_get(vpiType, a),
               (int)vpi_get(vpiSize, a));
    full = vpi_get_str(vpiFullName, a);
    error = level() != 0;
    vpi_printf("  full=%s error=%d\n", full ? full : "NULL", error);
    it = vpi_iterate(vpiRange, a);
    while (it && (r = vpi_scan(it)) != NULL) {
        int l = 0, h = 0;
        left = vpi_handle(vpiLeftRange, r);
        right = vpi_handle(vpiRightRange, r);
        value.format = vpiIntVal;
        if (left) {
            vpi_get_value(left, &value);
            l = (int)value.value.integer;
        }
        if (right) {
            vpi_get_value(right, &value);
            h = (int)value.value.integer;
        }
        vpi_printf("  range [%d:%d]\n", l, h);
    }
    value.format = vpiIntVal;
    vpi_get_value(a, &value);
    vpi_printf("  value error=%d\n", level() != 0);
}

static void element(const char* label, vpiHandle e) {
    vpiHandle parent, index, it;
    s_vpi_value value;
    int count = 0, error = level() != 0;
    memset(&value, 0, sizeof value);
    if (!e) {
        vpi_printf("%s: NULL error=%d\n", label, error);
        return;
    }
    vpi_printf("%s: %s type=%d array_member=%d\n", label, name_of(e), (int)vpi_get(vpiType, e),
               (int)vpi_get(vpiArrayMember, e));
    parent = vpi_handle(vpiParent, e);
    vpi_printf("  parent=%s\n", name_of(parent));
    index = vpi_handle(vpiIndex, e);
    value.format = vpiIntVal;
    if (index) vpi_get_value(index, &value);
    vpi_printf("  index=%d\n", index ? (int)value.value.integer : -1);
    it = vpi_iterate(vpiWaitingProcesses, e);
    while (it && vpi_scan(it)) ++count;
    vpi_printf("  waiting=%d\n", count);
}

/* The class definition of an object: some simulators answer vpiClassDefn
 * of the object, others reach it through its class typespec (37.29). */
static vpiHandle class_defn(vpiHandle object) {
    vpiHandle defn = vpi_handle(vpiClassDefn, object);
    (void)level();
    if (!defn) defn = vpi_handle(vpiClassDefn, vpi_handle(vpiClassTypespec, object));
    (void)level();
    return defn;
}

static PLI_INT32 d80(PLI_BYTE8* user) {
    vpiHandle k, ko, kd, it, a, kev = NULL, e, dobj, dd;
    PLI_INT32 indices[1];
    s_vpi_value value;
    (void)user;
    memset(&value, 0, sizeof value);
    k = vpi_handle_by_name((PLI_BYTE8*)"tb.k", NULL);
    ko = vpi_handle(vpiClassObj, k);
    names("k arrays", ko, vpiNamedEventArray);
    names("k events", ko, vpiNamedEvent);
    it = vpi_iterate(vpiNamedEventArray, ko);
    while (it && (a = vpi_scan(it)) != NULL) {
        array(a);
        names("events", a, vpiNamedEvent);
        if (!kev) kev = a;
    }
    element("kev by index 1", vpi_handle_by_index(kev, 1));
    element("kev by index 2", vpi_handle_by_index(kev, 2));
    indices[0] = 0;
    element("kev by indices 0", vpi_handle_by_multi_index(kev, 1, indices));
    e = vpi_handle_by_index(kev, 1);
    value.format = vpiIntVal;
    vpi_put_value(e, &value, NULL, vpiNoDelay);
    vpi_printf("put kev[1] error=%d\n", level() != 0);
    kd = class_defn(ko);
    names("K defn arrays", kd, vpiNamedEventArray);
    it = vpi_iterate(vpiNamedEventArray, kd);
    a = it ? vpi_scan(it) : NULL;
    if (a) {
        vpi_printf("defn %s size=%d\n", name_of(a), (int)vpi_get(vpiSize, a));
        it = vpi_iterate(vpiNamedEvent, a);
        vpi_printf("  defn events=%s error=%d\n", it ? "list" : "NULL", level() != 0);
    }
    dobj = vpi_handle(vpiClassObj, vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL));
    names("d arrays", dobj, vpiNamedEventArray);
    dd = class_defn(dobj);
    names("D defn arrays", dd, vpiNamedEventArray);
    return 0;
}

static void register_d80(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d80";
    task.calltf = d80;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d80, NULL};
