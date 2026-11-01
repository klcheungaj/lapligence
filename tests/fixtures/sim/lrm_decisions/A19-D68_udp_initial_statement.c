/* Companion of A19-D68_udp_initial_statement.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle definition(const char* instance) {
    vpiHandle primitive = vpi_handle_by_name((PLI_BYTE8*)instance, NULL);
    return primitive ? vpi_handle(vpiUdpDefn, primitive) : NULL;
}

static void show(const char* instance) {
    vpiHandle udp = definition(instance), initial, again, stmt, lhs, rhs;
    s_vpi_value value;
    (void)level();
    initial = udp ? vpi_handle(vpiInitial, udp) : NULL;
    vpi_printf("%s %s: initial=%d", instance, udp ? vpi_get_str(vpiDefName, udp) : "?",
               initial ? (int)vpi_get(vpiType, initial) : 0);
    vpi_printf(" error=%d\n", level() != 0);
    if (!initial) return;
    again = vpi_handle(vpiInitial, udp);
    vpi_printf("  compare=%d\n", (int)vpi_compare_objects(initial, again));
    stmt = vpi_handle(vpiStmt, initial);
    if (!stmt) {
        vpi_printf("  stmt NULL error=%d\n", level() != 0);
        return;
    }
    vpi_printf("  stmt type=%d optype=%d blocking=%d\n", (int)vpi_get(vpiType, stmt),
               (int)vpi_get(vpiOpType, stmt), (int)vpi_get(vpiBlocking, stmt));
    lhs = vpi_handle(vpiLhs, stmt);
    rhs = vpi_handle(vpiRhs, stmt);
    if (lhs)
        vpi_printf("  lhs type=%d name=%s direction=%d\n", (int)vpi_get(vpiType, lhs),
                   vpi_get_str(vpiName, lhs), (int)vpi_get(vpiDirection, lhs));
    if (rhs) {
        value.format = vpiIntVal;
        vpi_get_value(rhs, &value);
        vpi_printf("  rhs type=%d const=%d size=%d int=%d", (int)vpi_get(vpiType, rhs),
                   (int)vpi_get(vpiConstType, rhs), (int)vpi_get(vpiSize, rhs),
                   (int)value.value.integer);
        value.format = vpiBinStrVal;
        vpi_get_value(rhs, &value);
        vpi_printf(" bin=%s\n", value.value.str);
    }
    vpi_printf("  error=%d\n", level() != 0);
}

static PLI_INT32 d68(PLI_BYTE8* data) {
    (void)data;
    show("tb.u_ff");
    show("tb.u_latch");
    show("tb.u_inv");
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d68";
    task.calltf = d68;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
