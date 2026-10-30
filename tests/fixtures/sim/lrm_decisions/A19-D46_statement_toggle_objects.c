/* Decision A19-D46: statement and toggle coverage through the VPI.
 * Companion of A19-D46_statement_toggle_objects.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static void status(const char* what, int value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s -> %d%s\n", what, value, level ? " (error)" : "");
}

static void counts(const char* what, vpiHandle object) {
    vpi_printf("%s %s covered=%d max=%d count=%d\n", what,
               (const char*)vpi_get_str(vpiType, object), (int)vpi_get(vpiCovered, object),
               (int)vpi_get(vpiCoverMax, object), (int)vpi_get(vpiCoveredCount, object));
}

static PLI_INT32 d46(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value phase;
    phase.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &phase);
    vpi_free_object(args);
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpiHandle u = vpi_handle_by_name("tb.u", NULL);
    vpi_printf("phase %d: tb statements %d toggles %d, u statements %d toggles %d\n",
               (int)phase.value.integer, (int)vpi_get(vpiStatementCoverage, tb),
               (int)vpi_get(vpiToggleCoverage, tb), (int)vpi_get(vpiStatementCoverage, u),
               (int)vpi_get(vpiToggleCoverage, u));
    if (phase.value.integer == 0) {
        status("check statements u", (int)vpi_control(vpiCoverageCheck, vpiStatementCoverage, u));
        status("start statements u", (int)vpi_control(vpiCoverageStart, vpiStatementCoverage, u));
        status("start toggles u", (int)vpi_control(vpiCoverageStart, vpiToggleCoverage, u));
        return 0;
    }
    vpiHandle processes = vpi_iterate(vpiProcess, u), process;
    while (processes && (process = vpi_scan(processes))) {
        vpiHandle control = vpi_handle(vpiStmt, process);
        counts("event control", control);
        vpiHandle block = vpi_handle(vpiStmt, control);
        counts(" block", block);
        vpiHandle statements = vpi_iterate(vpiStmt, block), statement;
        while (statements && (statement = vpi_scan(statements))) {
            counts("  statement", statement);
            if (vpi_get(vpiType, statement) == vpiIf) counts("   then", vpi_handle(vpiStmt, statement));
        }
        status("start statements on a statement",
               (int)vpi_control(vpiCoverageStart, vpiStatementCoverage, block));
    }
    vpiHandle n = vpi_handle_by_name("tb.u.n", NULL);
    counts("n", n);
    counts("n[1]", vpi_handle_by_index(n, 1));
    counts("go", vpi_handle_by_name("tb.u.go", NULL));
    counts("r", vpi_handle_by_name("tb.u.r", NULL));
    vpi_printf("n count64 %lld\n", (long long)vpi_get64(vpiCoveredCount, n));
    status("reset toggles on a signal", (int)vpi_control(vpiCoverageReset, vpiToggleCoverage, n));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d46";
    task.calltf = d46;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
