/* Decision S46-D10: VPI coverage control and queries (see the .sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle find(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static PLI_INT32 d10_calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    vpi_release_handle(arguments);
    vpiHandle tb = find("tb");
    vpiHandle s = find("tb.s");
    vpiHandle ok = find("tb.s.ok");
    if (value.value.integer == 0) {
        /* Stop tb's own assertion `top`; tb.s.ok keeps collecting. */
        int stop = vpi_control(vpiCoverageStop, vpiAssertCoverage, tb);
        int check = vpi_control(vpiCoverageCheck, vpiAssertCoverage, s);
        /* The action of tb.s.ok is a statement; a model built for VPI has
         * statement coverage available (A19-D46). */
        int statement = vpi_control(vpiCoverageCheck, vpiStatementCoverage, s);
        vpi_printf((PLI_BYTE8*)"stop tb %d check s %d statement %d\n", stop, check, statement);
    } else {
        int covered_s = vpi_get(vpiAssertCoverage, s);
        int covered_tb = vpi_get(vpiAssertCoverage, tb);
        int covered_ok = vpi_get(vpiCovered, ok);
        vpi_printf((PLI_BYTE8*)"s %d tb %d ok covered %d\n", covered_s, covered_tb, covered_ok);
        int reset = vpi_control(vpiCoverageReset, vpiAssertCoverage, ok);
        int after = vpi_get(vpiAssertCoverage, s);
        vpi_printf((PLI_BYTE8*)"reset ok %d s %d\n", reset, after);
    }
    return 0;
}

static void d10_register(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d10";
    data.calltf = d10_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {d10_register, 0};
