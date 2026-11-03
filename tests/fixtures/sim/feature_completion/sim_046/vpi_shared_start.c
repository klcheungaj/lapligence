/* Statement coverage of the five leaf instances (one shared body) through
 * vpi_control: started, stopped and reset per instance after time 0.
 * Companion of vpi_shared_start.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle leaf(int index) {
    char name[16];
    snprintf(name, sizeof(name), "tb.u%d", index);
    return vpi_handle_by_name(name, NULL);
}

static void report(int phase) {
    for (int i = 0; i < 5; ++i) {
        vpiHandle u = leaf(i);
        vpiHandle processes = vpi_iterate(vpiProcess, u);
        vpiHandle process = processes ? vpi_scan(processes) : NULL;
        vpiHandle control = process ? vpi_handle(vpiStmt, process) : NULL;
        vpi_printf("phase %d u%d statements %d covered=%d max=%d count=%d\n", phase, i,
                   (int)vpi_get(vpiStatementCoverage, u), (int)vpi_get(vpiCovered, control),
                   (int)vpi_get(vpiCoverMax, control), (int)vpi_get(vpiCoveredCount, control));
        if (processes) vpi_release_handle(processes);
    }
}

static void control(const char* what, int operation, int index) {
    vpi_printf("%s u%d -> %d\n", what, index,
               (int)vpi_control(operation, vpiStatementCoverage, leaf(index)));
}

static PLI_INT32 phase_calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value phase;
    phase.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &phase);
    vpi_release_handle(args);
    report((int)phase.value.integer);
    switch (phase.value.integer) {
    case 0:
        control("start", vpiCoverageStart, 1);
        control("start", vpiCoverageStart, 3);
        break;
    case 1:
        control("stop", vpiCoverageStop, 1);
        control("start", vpiCoverageStart, 4);
        break;
    case 2:
        control("reset", vpiCoverageReset, 3);
        break;
    default:
        break;
    }
    fflush(stdout);
    return 0;
}

static void register_phase(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$cov_phase";
    data.calltf = phase_calltf;
    vpi_register_systf(&data);
}

PLI_DLLESPEC void (*vlog_startup_routines[])(void) = {register_phase, 0};
