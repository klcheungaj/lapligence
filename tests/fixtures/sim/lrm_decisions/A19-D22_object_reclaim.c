/* Companion of A19-D22_object_reclaim.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle held_obj, held_value, passed;

static vpiHandle property(vpiHandle obj, const char* name) {
    vpiHandle it = vpi_iterate(vpiVariables, obj), p;
    while ((p = vpi_scan(it)))
        if (strcmp(vpi_get_str(vpiName, p), name) == 0) {
            vpi_release_handle(it);
            return p;
        }
    return NULL;
}

static PLI_INT32 on_end(p_cb_data data) {
    vpi_printf("end of object id=%d\n", (int)vpi_get(vpiObjId, data->obj));
    return 0;
}

static PLI_INT32 on_reclaim(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(property(data->obj, "v"), &value);
    vpi_printf("reclaim %s id=%d v=%d time=%s\n", (const char*)data->user_data,
               (int)vpi_get(vpiObjId, data->obj), (int)value.value.integer,
               data->time ? "set" : "NULL");
    vpiHandle target = vpi_handle(vpiClassObj, property(data->obj, "link"));
    vpi_printf("  navigate %s level=%d\n", target ? "set" : "null", vpi_chk_error(NULL));
    value.value.integer = 9;
    vpi_put_value(property(data->obj, "v"), &value, NULL, vpiNoDelay);
    vpi_printf("  put level=%d\n", vpi_chk_error(NULL));
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbEndOfObject;
    cb.cb_rtn = on_end;
    cb.obj = data->obj;
    vpiHandle registered = vpi_register_cb(&cb);
    vpi_printf("  register %s level=%d\n", registered ? "ok" : "refused", vpi_chk_error(NULL));
    passed = data->obj;
    return 0;
}

static void watch(int reason, vpiHandle obj, const char* tag) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = reason == cbEndOfObject ? on_end : on_reclaim;
    cb.obj = obj;
    cb.user_data = (PLI_BYTE8*)tag;
    vpi_register_cb(&cb);
}

static PLI_INT32 setup(PLI_BYTE8* data) {
    (void)data;
    vpiHandle base = vpi_handle(vpiClassTypespec, vpi_handle_by_name("tb.a", NULL));
    watch(cbReclaimObj, base, "Base");
    vpi_printf("setup level=%d\n", vpi_chk_error(NULL));
    return 0;
}

static PLI_INT32 hold(PLI_BYTE8* data) {
    (void)data;
    vpiHandle b = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.b", NULL));
    vpiHandle a = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.a", NULL));
    vpi_printf("first seen b id=%d a id=%d\n", (int)vpi_get(vpiObjId, b), (int)vpi_get(vpiObjId, a));
    held_obj = b;
    held_value = vpi_handle_by_name("tb.b.v", NULL);
    watch(cbEndOfObject, b, "b");
    vpiHandle derived = vpi_iterate(vpiDerivedClasses,
                                    vpi_handle(vpiClassDefn, vpi_handle(vpiClassTypespec, a)));
    watch(cbReclaimObj, vpi_handle(vpiClassTypespec, vpi_scan(derived)), "Derived");
    vpi_release_handle(derived);
    return 0;
}

static PLI_INT32 check(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(held_value, &value);
    vpi_printf("held property level=%d\n", vpi_chk_error(NULL));
    PLI_INT32 id = vpi_get(vpiObjId, held_obj);
    vpi_printf("held object id=%d level=%d\n", (int)id, vpi_chk_error(NULL));
    id = vpi_get(vpiObjId, passed);
    vpi_printf("callback handle id=%d level=%d\n", (int)id, vpi_chk_error(NULL));
    vpiHandle a = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.a", NULL));
    vpi_printf("a id=%d created=%s\n", (int)vpi_get(vpiObjId, a),
               vpi_get_str(vpiName, vpi_handle(vpiClassTypespec, a)));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d22_setup";
    task.calltf = setup;
    vpi_register_systf(&task);
    task.tfname = "$d22_hold";
    task.calltf = hold;
    vpi_register_systf(&task);
    task.tfname = "$d22_check";
    task.calltf = check;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
