/* Statement and toggle coverage of a model built with --code-coverage,
 * read through the VPI at the end of the run. Companion of
 * option_kinds.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 report_calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpiHandle u = vpi_handle_by_name("tb.u", NULL);
    vpi_printf("tb statements %d toggles %d, u statements %d toggles %d\n",
               (int)vpi_get(vpiStatementCoverage, tb), (int)vpi_get(vpiToggleCoverage, tb),
               (int)vpi_get(vpiStatementCoverage, u), (int)vpi_get(vpiToggleCoverage, u));
    vpiHandle processes = vpi_iterate(vpiProcess, u);
    vpiHandle process = processes ? vpi_scan(processes) : NULL;
    vpiHandle control = process ? vpi_handle(vpiStmt, process) : NULL;
    vpi_printf("u always covered=%d max=%d count=%d\n", (int)vpi_get(vpiCovered, control),
               (int)vpi_get(vpiCoverMax, control), (int)vpi_get(vpiCoveredCount, control));
    if (processes) vpi_release_handle(processes);
    fflush(stdout);
    return 0;
}

static void register_report(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$cov_report";
    data.calltf = report_calltf;
    vpi_register_systf(&data);
}

PLI_DLLESPEC void (*vlog_startup_routines[])(void) = {register_report, 0};
