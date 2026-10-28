/* Companion of A19-D21_dynamic_values.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static void get(const char* name, PLI_INT32 format, const char* label) {
    s_vpi_value value;
    value.format = format;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &value);
    int level = vpi_chk_error(NULL);
    if (level)
        vpi_printf("%s %s level=%d\n", name, label, level);
    else if (format == vpiIntVal)
        vpi_printf("%s %s=%d\n", name, label, (int)value.value.integer);
    else
        vpi_printf("%s %s=\"%s\"\n", name, label, value.value.str);
}

static PLI_INT32 on_change(p_cb_data data) {
    (void)data;
    return 0;
}

static PLI_INT32 d21(PLI_BYTE8* data) {
    (void)data;
    vpiHandle s = vpi_handle_by_name("tb.s", NULL);
    vpi_printf("tb.s %s size=%d\n", vpi_get_str(vpiType, s), (int)vpi_get(vpiSize, s));
    get("tb.s", vpiStringVal, "string");
    get("tb.s", vpiBinStrVal, "bin");
    get("tb.s", vpiHexStrVal, "hex");
    get("tb.s", vpiIntVal, "int");
    get("tb.e", vpiStringVal, "string");
    get("tb.e", vpiHexStrVal, "hex");
    vpi_printf("tb.e size=%d\n", (int)vpi_get(vpiSize, vpi_handle_by_name("tb.e", NULL)));
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 0x43;
    vpi_put_value(s, &value, NULL, vpiNoDelay);
    vpi_printf("put int level=%d\n", vpi_chk_error(NULL));
    value.format = vpiStringVal;
    value.value.str = "xyz";
    vpi_put_value(s, &value, NULL, vpiNoDelay);
    vpi_printf("put string level=%d", vpi_chk_error(NULL));
    vpi_printf(" size=%d\n", (int)vpi_get(vpiSize, s));
    vpiHandle c = vpi_handle_by_name("tb.c", NULL);
    vpi_printf("tb.c %s\n", vpi_get_str(vpiType, c));
    get("tb.c", vpiIntVal, "int");
    get("tb.c", vpiHexStrVal, "hex");
    value.format = vpiIntVal;
    value.value.integer = 0;
    vpi_put_value(c, &value, NULL, vpiNoDelay);
    vpi_printf("put chandle level=%d\n", vpi_chk_error(NULL));
    s_cb_data cb;
    s_vpi_time time;
    s_vpi_value format;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSuppressTime;
    format.format = vpiSuppressVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = on_change;
    cb.obj = s;
    cb.time = &time;
    cb.value = &format;
    vpiHandle registered = vpi_register_cb(&cb);
    vpi_printf("watch string %s level=%d\n", registered ? "registered" : "refused",
               (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d21";
    task.calltf = d21;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
