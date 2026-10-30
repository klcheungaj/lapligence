/* Companion of A19-D26_wrong_object_queries.sv: a VPI application
 * (vlog_startup_routines). Each line prints what a query returned and the
 * vpi_chk_error level right after it (3 is vpiError). */
#include "sv_vpi_user.h"
#include <string.h>

static void got_int(const char* label, PLI_INT32 value) {
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s value=%d level=%d\n", label, (int)value, (int)level);
}

static void got_handle(const char* label, vpiHandle h) {
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s %s level=%d\n", label, h ? "handle" : "NULL", (int)level);
}

static void got_str(const char* label, const PLI_BYTE8* s) {
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s %s level=%d\n", label, s ? s : "NULL", (int)level);
}

/* The first statement of type `type` in the named block. */
static vpiHandle statement(vpiHandle block, PLI_INT32 type) {
    vpiHandle it = vpi_iterate(vpiStmt, block), s, found = NULL;
    while (it && (s = vpi_scan(it)))
        if (!found && vpi_get(vpiType, s) == type) found = s;
    return found;
}

static PLI_INT32 d26(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle n = vpi_handle_by_name((PLI_BYTE8*)"tb.n", NULL);
    vpiHandle mem = vpi_handle_by_name((PLI_BYTE8*)"tb.mem", NULL);
    vpiHandle blk = vpi_handle_by_name((PLI_BYTE8*)"tb.blk", NULL);
    /* Defined: answers. */
    got_int("net vpiSize", vpi_get(vpiSize, n));
    got_int("array vpiArray", vpi_get(vpiArray, mem));
    /* Not defined for the object: errors. */
    got_int("net vpiValid", vpi_get(vpiValid, n));
    got_int("module vpiNetType", vpi_get(vpiNetType, tb));
    got_str("net vpiDefFile", vpi_get_str(vpiDefFile, n));
    got_handle("module vpiCondition", vpi_handle(vpiCondition, tb));
    got_handle("net vpiOperand", vpi_handle(vpiOperand, n));
    got_handle("net vpiNet iteration", vpi_iterate(vpiNet, n));
    got_handle("array vpiNet iteration", vpi_iterate(vpiNet, mem));
    got_handle("named begin vpiStmt handle", vpi_handle(vpiStmt, blk));
    /* Defined, with no objects: NULL without an error. */
    got_handle("module vpiTchk iteration", vpi_iterate(vpiTchk, tb));
    got_handle("named begin vpiStmt iteration", vpi_iterate(vpiStmt, blk));
    vpiHandle assertions = vpi_iterate(vpiAssertion, tb);
    vpiHandle assertion = assertions ? vpi_scan(assertions) : NULL;
    if (assertion) vpi_release_handle(assertions);
    got_int("immediate assert vpiIsDeferred", vpi_get(vpiIsDeferred, assertion));
    vpiHandle loop = statement(blk, vpiFor);
    got_handle("for vpiForInitStmt", vpi_handle(vpiForInitStmt, loop));
    got_handle("for vpiOperand", vpi_handle(vpiOperand, loop));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d26";
    task.calltf = d26;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
