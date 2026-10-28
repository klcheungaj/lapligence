/* Companion of A19-D23_dynamic_elements.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle da1, aa5, sak;

static void show(const char* label, vpiHandle h, PLI_INT32 format) {
    s_vpi_value value;
    value.format = format;
    vpi_get_value(h, &value);
    int level = vpi_chk_error(NULL);
    if (level)
        vpi_printf(" %s level=%d", label, level);
    else if (format == vpiIntVal)
        vpi_printf(" %s=%d", label, (int)value.value.integer);
    else
        vpi_printf(" %s=%s", label, value.value.str);
}

static PLI_INT32 on_change(p_cb_data data) {
    (void)data;
    return 0;
}

static const char* watch(vpiHandle obj) {
    s_cb_data cb;
    s_vpi_time time;
    s_vpi_value format;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSuppressTime;
    format.format = vpiIntVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = on_change;
    cb.obj = obj;
    cb.time = &time;
    cb.value = &format;
    return vpi_register_cb(&cb) ? "registered" : "refused";
}

static PLI_INT32 hold(PLI_BYTE8* data) {
    (void)data;
    vpiHandle da = vpi_handle_by_name("tb.da", NULL);
    da1 = vpi_handle_by_index(da, 1);
    aa5 = vpi_handle_by_index(vpi_handle_by_name("tb.aa", NULL), 5);
    vpiHandle it = vpi_iterate(vpiMemoryWord, vpi_handle_by_name("tb.sa", NULL));
    sak = vpi_scan(it);
    vpi_release_handle(it);
    /* vpi_get_str returns a buffer the next call may reuse. */
    vpi_printf("hold %s", vpi_get_str(vpiName, da1));
    vpi_printf(" %s", vpi_get_str(vpiName, aa5));
    vpi_printf(" %s\n", vpi_get_str(vpiName, sak));
    vpiHandle index = vpi_handle(vpiIndex, sak);
    vpi_printf("string key index=%s level=%d\n", index ? "set" : "null", vpi_chk_error(NULL));
    vpiHandle missing = vpi_handle_by_index(vpi_handle_by_name("tb.aa", NULL), 6);
    vpi_printf("missing key=%s level=%d\n", missing ? "set" : "null", vpi_chk_error(NULL));
    s_vpi_value value;
    s_vpi_time delay;
    value.format = vpiIntVal;
    value.value.integer = 99;
    delay.type = vpiSimTime;
    delay.high = 0;
    delay.low = 1;
    vpi_put_value(da1, &value, &delay, vpiInertialDelay);
    vpi_printf("delayed put level=%d\n", vpi_chk_error(NULL));
    const char* result = watch(da);
    vpi_printf("watch array %s level=%d\n", result, vpi_chk_error(NULL));
    result = watch(da1);
    vpi_printf("watch element %s level=%d\n", result, vpi_chk_error(NULL));
    return 0;
}

static PLI_INT32 check(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("check");
    show("da[1]", da1, vpiIntVal);
    show("aa[5]", aa5, vpiIntVal);
    show("sa[k]", sak, vpiStringVal);
    vpi_printf("\n");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d23_hold";
    task.calltf = hold;
    vpi_register_systf(&task);
    task.tfname = "$d23_check";
    task.calltf = check;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
