/* Decision A19-D27: unnamed blocks, for and foreach statements as scopes
 * (IEEE 1800-2009 37.12 details 1-3). See A19-D27_statement_scopes.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

/* The error level of the last call (0: none). */
static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void scope_line(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf("%s: NULL level=%d\n", label, level());
        return;
    }
    const char* type = vpi_get_str(vpiType, h);
    const char* name = vpi_get_str(vpiName, h);
    const char* full = vpi_get_str(vpiFullName, h);
    vpiHandle scope = vpi_handle(vpiScope, h);
    vpi_printf("%s: %s name=%s full=%s scope=%s\n", label, text(type), text(name), text(full),
               scope ? text(vpi_get_str(vpiFullName, scope)) : "NULL");
}

static int count(int relation, vpiHandle h, int* error) {
    vpiHandle it = vpi_iterate(relation, h);
    *error = level();
    int n = 0;
    if (it)
        while (vpi_scan(it)) ++n;
    return n;
}

static PLI_INT32 d27(PLI_BYTE8* data) {
    (void)data;
    static const char* const names[] = {
        "tb.unnamed$$_0",
        "tb.unnamed$$_0.a",
        "tb.unnamed$$_0.unnamed$$_1",
        "tb.unnamed$$_0.unnamed$$_1.c",
        "tb.unnamed$$_0.unnamed$$_4.b",
        "tb.t.par",
    };
    for (size_t i = 0; i < sizeof names / sizeof names[0]; ++i)
        scope_line(names[i], vpi_handle_by_name((PLI_BYTE8*)names[i], NULL));
    int error;
    vpiHandle outer = vpi_handle_by_name((PLI_BYTE8*)"tb.unnamed$$_0", NULL);
    int scopes = count(vpiInternalScope, outer, &error);
    vpi_printf("tb.unnamed$$_0 internal scopes=%d level=%d\n", scopes, error);
    int variables = count(vpiVariables, outer, &error);
    vpi_printf("tb.unnamed$$_0 variables=%d level=%d\n", variables, error);
    /* The statements of the outer block, in source order. */
    vpiHandle it = vpi_iterate(vpiStmt, outer), stmt;
    int index = 0;
    while (it && (stmt = vpi_scan(it))) {
        const char* type = vpi_get_str(vpiType, stmt);
        const char* full = vpi_get_str(vpiFullName, stmt);
        int full_level = level();
        int t = vpi_get(vpiType, stmt);
        vpi_printf("stmt %d: %s full=%s", index++, text(type), text(full));
        /* A block that is not a scope has no vpiFullName (A19-D26). */
        if (t == vpiBegin || t == vpiFork) vpi_printf(" level=%d", full_level);
        /* The loop variables of a for or foreach scope are automatic: llg
         * gives automatic variables no objects, so their count is not shown. */
        if (t == vpiBegin || t == vpiFork || (t == vpiFor && !full)) {
            int vars = count(vpiVariables, stmt, &error);
            vpi_printf(" variables=%d level=%d", vars, error);
        }
        vpi_printf("\n");
        if (vpi_get(vpiType, stmt) == vpiBegin && vpi_get_str(vpiFullName, stmt)) {
            vpiHandle inner = vpi_iterate(vpiStmt, stmt), first = inner ? vpi_scan(inner) : NULL;
            vpiHandle scope = first ? vpi_handle(vpiScope, first) : NULL;
            vpi_printf("  first statement scope=%s\n",
                       scope ? text(vpi_get_str(vpiFullName, scope)) : "NULL");
            if (inner && first) vpi_release_handle(inner);
        }
        level();
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d27";
    task.calltf = d27;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
