/* Decision A19-D62: the process of a statement and the statement of
 * `wait fork`. Companion of A19-D62_statement_processes.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

/* Prints each statement under `stmt` with the process vpiProcess gives;
 * blocks are flattened, since their nesting is not part of the decision. */
static void statements(vpiHandle stmt) {
    int type = (int)vpi_get(vpiType, stmt);
    int block = type == vpiBegin || type == vpiNamedBegin || type == vpiFork || type == vpiNamedFork;
    if (!block) {
        vpiHandle process = vpi_handle(vpiProcess, stmt);
        int error = failed();
        vpi_printf("  %s line %d process %s%s", (const char*)vpi_get_str(vpiType, stmt),
                   (int)vpi_get(vpiLineNo, stmt),
                   process ? (const char*)vpi_get_str(vpiType, process) : "NULL", error ? "(error)" : "");
        if (process) vpi_printf(" line %d", (int)vpi_get(vpiLineNo, process));
        if (type == vpiAssignment) {
            /* A function call belongs to the tf call class of 37.56. */
            vpiHandle rhs = vpi_handle(vpiRhs, stmt);
            if (rhs && vpi_get(vpiType, rhs) == vpiFuncCall) {
                vpiHandle owner = vpi_handle(vpiProcess, rhs);
                error = failed();
                vpi_printf(" call process %s%s", owner ? (const char*)vpi_get_str(vpiType, owner) : "NULL",
                           error ? "(error)" : "");
            }
        }
        if (type == vpiWaitFork) {
            vpiHandle body = vpi_handle(vpiStmt, stmt);
            error = failed();
            vpi_printf(" stmt %s%s", body ? (const char*)vpi_get_str(vpiType, body) : "NULL",
                       error ? "(error)" : "");
        }
        vpi_printf("\n");
    }
    if (block) {
        vpiHandle it = vpi_iterate(vpiStmt, stmt), child;
        while (it && (child = vpi_scan(it))) statements(child);
        return;
    }
    vpiHandle child = vpi_handle(vpiStmt, stmt);
    (void)failed();
    if (child) statements(child);
}

static PLI_INT32 d62(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle it = vpi_iterate(vpiProcess, tb), process;
    while (it && (process = vpi_scan(it))) {
        vpi_printf("%s line %d\n", (const char*)vpi_get_str(vpiType, process), (int)vpi_get(vpiLineNo, process));
        statements(vpi_handle(vpiStmt, process));
    }
    vpiHandle task = vpi_handle_by_name((PLI_BYTE8*)"tb.bump", NULL);
    vpi_printf("task bump\n");
    statements(vpi_handle(vpiStmt, task));
    return 0;
}

static void register_d62(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d62", d62, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d62, NULL};
