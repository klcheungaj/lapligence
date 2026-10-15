/* SIM-046: assertion handles, coverage properties and coverage control
 * through VPI (IEEE 1800-2009 39.3, 40.5). Called from the HDL as
 * $vpi_cov(phase). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* text(const char* value) { return value ? value : "-"; }

static const char* error_code(void) {
    static char code[64];
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (!vpi_chk_error(&info)) return "none";
    snprintf(code, sizeof(code), "%s", info.code ? (const char*)info.code : "?");
    return code;
}

static vpiHandle find(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static void describe(vpiHandle assertion) {
    vpiHandle scope = vpi_handle(vpiScope, assertion);
    printf("%s name=%s full=%s scope=%s def=%s deferred=%d\n",
           vpi_get_str(vpiType, assertion), text(vpi_get_str(vpiName, assertion)),
           text(vpi_get_str(vpiFullName, assertion)),
           scope ? vpi_get_str(vpiFullName, scope) : "-",
           text(vpi_get_str(vpiDefName, assertion)), (int)vpi_get(vpiIsDeferred, assertion));
}

static void counters(vpiHandle assertion) {
    printf("%s attempts=%d success=%d vacuous=%d failure=%d disable=%d kill=%d covered=%d "
           "max=%d count=%lld\n",
           text(vpi_get_str(vpiFullName, assertion)), (int)vpi_get(vpiAssertAttemptCovered, assertion),
           (int)vpi_get(vpiAssertSuccessCovered, assertion),
           (int)vpi_get(vpiAssertVacuousSuccessCovered, assertion),
           (int)vpi_get(vpiAssertFailureCovered, assertion),
           (int)vpi_get(vpiAssertDisableCovered, assertion),
           (int)vpi_get(vpiAssertKillCovered, assertion), (int)vpi_get(vpiCovered, assertion),
           (int)vpi_get(vpiCoverMax, assertion),
           (long long)vpi_get64(vpiCoveredCount, assertion));
}

static void each_assertion(void (*visit)(vpiHandle)) {
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL);
    vpiHandle assertion;
    while (iterator && (assertion = vpi_scan(iterator))) {
        visit(assertion);
        vpi_release_handle(assertion);
    }
}

static void phase_start(void) {
    each_assertion(describe);
    vpiHandle u1 = find("tb.u1");
    vpiHandle u2 = find("tb.u2");
    int count = 0;
    vpiHandle iterator = vpi_iterate(vpiAssertion, u1);
    vpiHandle assertion;
    while (iterator && (assertion = vpi_scan(iterator))) {
        ++count;
        vpi_release_handle(assertion);
    }
    printf("u1 items=%d\n", count);
    vpiHandle p = find("tb.u1.p");
    vpiHandle p_in_u1 = vpi_handle_by_name((PLI_BYTE8*)"p", u1);
    printf("by name %s same=%d\n", vpi_get_str(vpiType, p), (int)vpi_compare_objects(p, p_in_u1));
    vpi_handle_by_name((PLI_BYTE8*)"tb.u1.nosuch", NULL);
    printf("missing %s\n", error_code());
    /* Stop u1 (an instance: that instance only) and u2.c (one assertion). */
    vpiHandle c2 = find("tb.u2.c");
    /* Each call is its own statement: argument evaluation order is
     * unspecified in C. */
    int stop_u1 = vpi_control(vpiCoverageStop, vpiAssertCoverage, u1);
    int stop_c2 = vpi_control(vpiCoverageStop, vpiAssertCoverage, c2);
    int check_u1 = vpi_control(vpiCoverageCheck, vpiAssertCoverage, u1);
    printf("stop u1 %d stop u2.c %d check u1 %d\n", stop_u1, stop_c2, check_u1);
    int check = vpi_control(vpiCoverageCheck, vpiStatementCoverage, u1);
    int stop = vpi_control(vpiCoverageStop, vpiStatementCoverage, u1);
    printf("statement check %d stop %d\n", check, stop);
    int bad = vpi_control(vpiCoverageCheck, 999, u1);
    printf("bad type %d %s\n", bad, error_code());
    vpi_release_handle(p);
    vpi_release_handle(p_in_u1);
    vpi_release_handle(c2);
    (void)u2;
}

static void phase_end(void) {
    each_assertion(counters);
    vpiHandle u1 = find("tb.u1");
    vpiHandle u2 = find("tb.u2");
    vpiHandle tb = find("tb");
    int covered_u1 = vpi_get(vpiAssertCoverage, u1);
    int covered_u2 = vpi_get(vpiAssertCoverage, u2);
    int covered_tb = vpi_get(vpiAssertCoverage, tb);
    printf("assert coverage u1=%d u2=%d tb=%d\n", covered_u1, covered_u2, covered_tb);
    int toggle = vpi_get(vpiToggleCoverage, u1);
    printf("toggle coverage %d %s\n", toggle, error_code());
    int save = vpi_control(vpiCoverageSave, vpiAssertCoverage, "vpi_coverage.db");
    int merge = vpi_control(vpiCoverageMerge, vpiAssertCoverage, "vpi_coverage.db");
    printf("save %d merge %d\n", save, merge);
    vpiHandle p2 = find("tb.u2.p");
    counters(p2);
    vpi_release_handle(p2);
}

static PLI_INT32 vpi_cov_calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = arguments ? vpi_scan(arguments) : NULL;
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(argument, &value);
    if (arguments) vpi_release_handle(arguments);
    if (value.value.integer == 0) phase_start();
    else phase_end();
    fflush(stdout);
    return 0;
}

static void register_vpi_cov(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$vpi_cov";
    data.calltf = vpi_cov_calltf;
    vpi_register_systf(&data);
}

PLI_DLLESPEC void (*vlog_startup_routines[])(void) = {register_vpi_cov, 0};
