/* Decision A17-D13: FSM objects of the VPI. Companion of
 * A17-D13_fsm_objects.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static void status(const char* what, int value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s -> %d%s\n", what, value, level ? " (error)" : "");
}

static PLI_INT32 d13(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpiHandle f = vpi_handle_by_name("tb.f", NULL);
    status("vpi_iterate(vpiFsm, tb) found", vpi_iterate(vpiFsm, tb) != NULL);
    status("vpi_iterate(vpiFsm, NULL) found", vpi_iterate(vpiFsm, NULL) != NULL);
    vpi_printf("tb.f state coverage %d\n", (int)vpi_get(vpiFsmStateCoverage, f));
    vpiHandle fsms = vpi_iterate(vpiFsm, f), fsm;
    while (fsms && (fsm = vpi_scan(fsms))) {
        vpi_printf("%s %s covered=%d max=%d count=%d\n", (const char*)vpi_get_str(vpiType, fsm),
                   (const char*)vpi_get_str(vpiFullName, fsm), (int)vpi_get(vpiCovered, fsm),
                   (int)vpi_get(vpiCoverMax, fsm), (int)vpi_get(vpiCoveredCount, fsm));
        vpiHandle expression = vpi_handle(vpiFsmStateExpression, fsm);
        if (expression)
            vpi_printf("  expression %s\n", (const char*)vpi_get_str(vpiFullName, expression));
        else
            status("  vpiFsmStateExpression found", 0);
        vpiHandle states = vpi_iterate(vpiFsmStates, fsm), state;
        while (states && (state = vpi_scan(states))) {
            s_vpi_value value;
            value.format = vpiBinStrVal;
            vpi_get_value(state, &value);
            vpi_printf("  %s %s value=%s covered=%d max=%d count=%d\n",
                       (const char*)vpi_get_str(vpiType, state),
                       (const char*)vpi_get_str(vpiName, state), value.value.str,
                       (int)vpi_get(vpiCovered, state), (int)vpi_get(vpiCoverMax, state),
                       (int)vpi_get(vpiCoveredCount, state));
        }
        status("  vpi_control(vpiCoverageReset, vpiFsmStateCoverage, fsm)",
               (int)vpi_control(vpiCoverageReset, vpiFsmStateCoverage, fsm));
    }
    status("vpi_control(vpiCoverageCheck, vpiFsmStateCoverage, tb.f)",
           (int)vpi_control(vpiCoverageCheck, vpiFsmStateCoverage, f));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d13";
    task.calltf = d13;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
