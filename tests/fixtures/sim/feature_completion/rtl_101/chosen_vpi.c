#include "vpi_user.h"
#include "sv_vpi_user.h"

static PLI_INT32 changed(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiHexStrVal;
    vpi_get_value(data->obj, &value);
    vpi_printf("cb %s=%s\n", vpi_get_str(vpiFullName, data->obj), value.value.str);
    return 0;
}

static void show(const char* name) {
    vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%s %s\n", name, object ? vpi_get_str(vpiType, object) : "NULL");
    if (!object) return;
    vpi_printf("  size %d\n", vpi_get(vpiSize, object));
    vpiHandle members = vpi_iterate(vpiMember, object);
    if (!members) return;
    for (vpiHandle member = vpi_scan(members); member; member = vpi_scan(members))
        vpi_printf("  member %s %s size %d\n", vpi_get_str(vpiFullName, member),
                   vpi_get_str(vpiType, member), vpi_get(vpiSize, member));
}

static PLI_INT32 chosen_vpi(PLI_BYTE8* unused) {
    (void)unused;
    show("tb.r");
    show("tb.r.tag");
    show("tb.r.n");
    show("tb.r.w");
    show("tb.r.s");
    show("tb.r.a");
    show("tb.r.a[3]");
    vpiHandle tag = vpi_handle_by_name((PLI_BYTE8*)"tb.r.tag", NULL);
    s_vpi_value value;
    value.format = vpiHexStrVal;
    vpi_get_value(tag, &value);
    vpi_printf("tag=%s\n", value.value.str);
    s_vpi_time time = {vpiSimTime, 0, 0, 0};
    s_vpi_value watched = {vpiHexStrVal, {0}};
    s_cb_data callback = {cbValueChange, changed, tag, &time, &watched, 0, 0};
    vpi_register_cb(&callback);
    s_vpi_value put;
    put.format = vpiIntVal;
    put.value.integer = 42;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.r.n", NULL), &put, NULL, vpiNoDelay);
    return 0;
}

static void chosen_vpi_register(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$chosen_vpi", chosen_vpi, 0, 0, 0};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {chosen_vpi_register, 0};
