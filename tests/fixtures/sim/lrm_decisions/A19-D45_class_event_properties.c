/* Decision A19-D45 companion: event properties of a class object. */
#include "sv_vpi_user.h"
#include <string.h>

static void list(const char* tag, vpiHandle reference, PLI_INT32 type) {
    vpiHandle it = vpi_iterate(type, reference), item;
    vpi_printf("%s:", tag);
    while (it && (item = vpi_scan(it)))
        vpi_printf(" %s(%d)", vpi_get_str(vpiName, item), (int)vpi_get(vpiType, item));
    vpi_printf("\n");
}

static PLI_INT32 d45(PLI_BYTE8* data) {
    (void)data;
    vpiHandle var = vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL);
    vpiHandle obj = vpi_handle(vpiClassObj, var);
    list("variables", obj, vpiVariables);
    list("events", obj, vpiNamedEvent);
    vpiHandle defn = vpi_handle(vpiClassDefn, vpi_handle(vpiClassTypespec, obj));
    list("defn events", defn, vpiNamedEvent);
    vpiHandle e = vpi_handle_by_name((PLI_BYTE8*)"tb.d.e", NULL);
    vpi_printf("e type=%d array=%d member=%d automatic=%d\n", (int)vpi_get(vpiType, e),
               (int)vpi_get(vpiArray, e), (int)vpi_get(vpiArrayMember, e),
               (int)vpi_get(vpiAutomatic, e));
    int waiting = 0;
    vpiHandle it = vpi_iterate(vpiWaitingProcesses, e);
    while (it && vpi_scan(it)) ++waiting;
    vpi_printf("e waiting=%d\n", waiting);
    vpi_put_value(e, NULL, NULL, vpiNoDelay);
    vpi_printf("put error=%d\n", vpi_chk_error(NULL) != 0);
    vpiHandle f = vpi_handle_by_name((PLI_BYTE8*)"tb.d.f", NULL);
    s_vpi_value value;
    value.format = vpiSuppressVal;
    vpi_put_value(f, &value, NULL, vpiNoDelay);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d45";
    task.calltf = d45;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
