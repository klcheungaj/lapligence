/* Companion of A19-D85_method_imports_and_definition_events.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof info);
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* str(int property, vpiHandle h) {
    const char* text = h ? vpi_get_str(property, h) : NULL;
    return text ? text : "NULL";
}

static void event_by_index(vpiHandle array, int index) {
    vpiHandle e = vpi_handle_by_index(array, index), it;
    s_vpi_value value;
    int error = level();
    vpi_printf("    %s by index %d:", str(vpiName, array), index);
    if (!e) {
        vpi_printf(" NULL error=%d\n", error != 0);
        return;
    }
    vpi_printf(" %s named_event=%d", str(vpiName, e), vpi_get(vpiType, e) == vpiNamedEvent);
    vpi_printf(" parent=%s\n", str(vpiName, vpi_handle(vpiParent, e)));
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(e, &value);
    vpi_printf("      value error=%d", level() != 0);
    it = vpi_iterate(vpiWaitingProcesses, e);
    vpi_printf(" waiting=%s error=%d\n", it ? "iter" : "NULL", level() != 0);
}

static PLI_INT32 d85(PLI_BYTE8* user_data) {
    vpiHandle top = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle classes = top ? vpi_iterate(vpiClassDefn, top) : NULL;
    vpiHandle defn = classes ? vpi_scan(classes) : NULL, it, x, y;
    (void)user_data;
    if (classes && defn) vpi_release_handle(classes);
    vpi_printf("class %s\n", str(vpiName, defn));
    if (!defn) return 0;
    it = vpi_iterate(vpiMethods, defn);
    while (it && (x = vpi_scan(it)) != NULL) {
        vpiHandle imports = vpi_iterate(vpiImport, x);
        int error = level();
        vpi_printf("  method %s\n", str(vpiName, x));
        vpi_printf("    imports %s error=%d\n", imports ? "iter" : "NULL", error != 0);
        while (imports && (y = vpi_scan(imports)) != NULL)
            vpi_printf("    import %s int_var=%d\n", str(vpiName, y), vpi_get(vpiType, y) == vpiIntVar);
    }
    it = vpi_iterate(vpiNamedEventArray, defn);
    while (it && (x = vpi_scan(it)) != NULL) {
        vpiHandle events = vpi_iterate(vpiNamedEvent, x);
        int error = level();
        vpi_printf("  event array %s size=%d\n", str(vpiName, x), (int)vpi_get(vpiSize, x));
        vpi_printf("    vpiNamedEvent %s error=%d\n", events ? "iter" : "NULL", error != 0);
        event_by_index(x, 0);
        event_by_index(x, 2);
        event_by_index(x, 3);
    }
    return 0;
}

static void reg(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d85";
    data.calltf = d85;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {reg, 0};
