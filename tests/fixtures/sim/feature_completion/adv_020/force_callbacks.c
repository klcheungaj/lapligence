/* ADV-020: cbForce, cbRelease, cbAssign and cbDeassign (IEEE 1800-2009
 * 38.36.1). Companion of force_callbacks.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle saved;
static vpiHandle any_force;

static void report(const char* label) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: level=%d code=%s\n", label, (int)level,
               level && info.code ? (const char*)info.code : "-");
}

static PLI_INT32 on_statement(p_cb_data data) {
    const char* label = (const char*)data->user_data;
    PLI_BYTE8* type = vpi_get_str(vpiType, data->obj);
    vpiHandle lhs = vpi_handle(vpiLhs, data->obj);
    PLI_INT32 level = vpi_chk_error(NULL);
    const char* name = lhs ? (const char*)vpi_get_str(vpiFullName, lhs) : "none";
    char value[128];
    if (!data->value)
        snprintf(value, sizeof(value), "NULL");
    else if (data->value->format == vpiRealVal)
        snprintf(value, sizeof(value), "%g", data->value->value.real);
    else if (data->value->format == vpiIntVal)
        snprintf(value, sizeof(value), "%d", (int)data->value->value.integer);
    else if (data->value->format == vpiVectorVal)
        snprintf(value, sizeof(value), "vector %x/%x", (unsigned)data->value->value.vector[0].aval,
                 (unsigned)data->value->value.vector[0].bval);
    else
        snprintf(value, sizeof(value), "%s", (const char*)data->value->value.str);
    vpi_printf("%s at %u: %s lhs=%s%s value=%s\n", label, (unsigned)data->time->low,
               type ? (const char*)type : "?", name, level ? " (lhs error)" : "", value);
    if (lhs) vpi_release_handle(lhs);
    saved = data->obj;
    return 0;
}

static vpiHandle add(const char* label, PLI_INT32 reason, const char* object, PLI_INT32 format) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value values[8];
    static int used;
    s_vpi_value* value = &values[used++ % 8];
    value->format = format;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = on_statement;
    cb.obj = object ? vpi_handle_by_name((PLI_BYTE8*)object, NULL) : NULL;
    cb.time = &time;
    cb.value = value;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    char text[96];
    snprintf(text, sizeof(text), "register %s %s", label, handle ? "handle" : "NULL");
    report(text);
    return handle;
}

static PLI_INT32 watch(PLI_BYTE8* data) {
    (void)data;
    any_force = add("any-force", cbForce, NULL, vpiBinStrVal);
    add("any-release", cbRelease, NULL, vpiHexStrVal);
    add("n[7:4]-force", cbForce, "tb.n[7:4]", vpiHexStrVal);
    add("na-release", cbRelease, "tb.na", vpiIntVal);
    add("r-force", cbForce, "tb.r", vpiRealVal);
    add("n-release", cbRelease, "tb.n", vpiObjTypeVal);
    add("n[0]-force", cbForce, "tb.n[0]", vpiBinStrVal);
    add("P-force", cbForce, "tb.P", vpiBinStrVal);
    add("any-assign", cbAssign, NULL, vpiBinStrVal);
    add("v-deassign", cbDeassign, "tb.v", vpiBinStrVal);
    return 0;
}

static PLI_INT32 vpi_force(PLI_BYTE8* data) {
    (void)data;
    vpiHandle w = vpi_handle_by_name((PLI_BYTE8*)"tb.w", NULL);
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 5;
    vpi_put_value(w, &value, NULL, vpiForceFlag);
    report("vpi force");
    vpi_put_value(w, &value, NULL, vpiReleaseFlag);
    report("vpi release");
    return 0;
}

static PLI_INT32 stale(PLI_BYTE8* data) {
    (void)data;
    vpi_get(vpiType, saved);
    report("statement handle after its callback");
    vpi_printf("remove any-force: %d\n", (int)vpi_remove_cb(any_force));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$watch";
    task.calltf = watch;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$vpi_force";
    task.calltf = vpi_force;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$stale";
    task.calltf = stale;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
