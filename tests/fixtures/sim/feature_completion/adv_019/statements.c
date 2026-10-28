/* ADV-019 family 4: an independent walker of the statement objects of
 * statements.sv (IEEE 1800-2009 37.56-37.74, 37.12). $statements prints the
 * statement tree under every process, task and function of tb through
 * vpiStmt: each statement's type, line and file, its properties and the
 * vpiDecompile of the expressions it relates to, then its sub-statements.
 * It ends with the refusals: a cbStmt callback and vpiStmt of a variable. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failures;

static const char* text(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void check(int ok, const char* what) {
    if (!ok) {
        ++failures;
        vpi_printf("CHECK FAILED: %s\n", what);
    }
}

static const char* base_name(const char* path) {
    const char* slash = strrchr(path, '/');
    const char* backslash = strrchr(path, '\\');
    if (backslash && (!slash || backslash > slash)) slash = backslash;
    return slash ? slash + 1 : path;
}

/* An expression as `label=<decompiled>`, or `label=NULL`. */
static void expr(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf(" %s=NULL", label);
        return;
    }
    const char* decompiled = vpi_get_str(vpiDecompile, h);
    if (decompiled) {
        vpi_printf(" %s=\"%s\"", label, decompiled);
    } else {
        vpi_chk_error(NULL);
        vpi_printf(" %s=%s:%s", label, text(vpiType, h), text(vpiFullName, h));
    }
}

static void stmt(const char* label, vpiHandle h, int depth);

static void stmt_one(const char* label, PLI_INT32 relation, vpiHandle h, int depth) {
    vpiHandle child = vpi_handle(relation, h);
    if (child) stmt(label, child, depth);
}

static void stmt_all(const char* label, PLI_INT32 relation, vpiHandle h, int depth) {
    vpiHandle it = vpi_iterate(relation, h), child;
    if (!it) return;
    while ((child = vpi_scan(it))) stmt(label, child, depth);
}

static void control(vpiHandle h) {
    PLI_INT32 type = vpi_get(vpiType, h);
    if (type == vpiDelayControl) {
        expr("delay", vpi_handle(vpiDelay, h));
    } else if (type == vpiEventControl) {
        expr("event", vpi_handle(vpiCondition, h));
    } else if (type == vpiRepeatControl) {
        expr("count", vpi_handle(vpiExpr, h));
        vpiHandle event = vpi_handle(vpiEventControl, h);
        if (event) expr("event", vpi_handle(vpiCondition, event));
    }
}

static void stmt(const char* label, vpiHandle h, int depth) {
    PLI_INT32 type = vpi_get(vpiType, h);
    vpi_printf("%*s%s %s line=%d", depth * 2, "", label, text(vpiType, h), vpi_get(vpiLineNo, h));
    const char* file = vpi_get_str(vpiFile, h);
    vpi_printf(" file=%s", file ? base_name(file) : "(null)");
    const char* name = vpi_get_str(vpiName, h);
    vpi_chk_error(NULL);
    if (name) vpi_printf(" name=%s", name);
    switch (type) {
        case vpiAssignment: {
            vpi_printf(" blocking=%d op=%d", vpi_get(vpiBlocking, h), vpi_get(vpiOpType, h));
            expr("lhs", vpi_handle(vpiLhs, h));
            expr("rhs", vpi_handle(vpiRhs, h));
            vpiHandle timing = vpi_handle(vpiDelayControl, h);
            if (!timing) timing = vpi_handle(vpiEventControl, h);
            if (!timing) timing = vpi_handle(vpiRepeatControl, h);
            if (timing) {
                vpi_printf(" control=%s", text(vpiType, timing));
                control(timing);
                check(vpi_handle(vpiStmt, timing) == NULL, "37.60 d1: control of an assignment has no stmt");
            }
            break;
        }
        case vpiIf: case vpiIfElse:
            vpi_printf(" qualifier=%d", vpi_get(vpiQualifier, h));
            expr("cond", vpi_handle(vpiCondition, h));
            break;
        case vpiCase:
            vpi_printf(" case_type=%d qualifier=%d", vpi_get(vpiCaseType, h), vpi_get(vpiQualifier, h));
            expr("cond", vpi_handle(vpiCondition, h));
            break;
        case vpiWhile: case vpiRepeat: case vpiDoWhile: case vpiWait: case vpiReturnStmt:
            expr("cond", vpi_handle(vpiCondition, h));
            break;
        case vpiFor:
            vpi_printf(" local_vars=%d", vpi_get(vpiLocalVarDecls, h));
            expr("cond", vpi_handle(vpiCondition, h));
            break;
        case vpiForeachStmt: {
            expr("array", vpi_handle(vpiVariables, h));
            vpiHandle it = vpi_iterate(vpiLoopVars, h), var;
            while (it && (var = vpi_scan(it))) vpi_printf(" loop_var=%s:%s", text(vpiType, var), text(vpiName, var));
            break;
        }
        case vpiDelayControl: case vpiEventControl: case vpiRepeatControl:
            control(h);
            break;
        case vpiEventStmt: {
            vpiHandle event = vpi_handle(vpiNamedEvent, h);
            vpi_printf(" event=%s", text(vpiFullName, event));
            break;
        }
        case vpiDisable:
            vpi_printf(" target=%s", text(vpiFullName, vpi_handle(vpiExpr, h)));
            break;
        case vpiForce: case vpiAssignStmt:
            expr("lhs", vpi_handle(vpiLhs, h));
            expr("rhs", vpi_handle(vpiRhs, h));
            break;
        case vpiRelease: case vpiDeassign:
            expr("lhs", vpi_handle(vpiLhs, h));
            break;
        case vpiTaskCall: case vpiSysTaskCall: {
            if (type == vpiTaskCall) vpi_printf(" task=%s", text(vpiFullName, vpi_handle(vpiTask, h)));
            vpiHandle it = vpi_iterate(vpiArgument, h), arg;
            while (it && (arg = vpi_scan(it))) expr("arg", arg);
            break;
        }
        default:
            break;
    }
    vpi_printf("\n");
    switch (type) {
        case vpiBegin: case vpiNamedBegin: case vpiFork: case vpiNamedFork:
            stmt_all("stmt", vpiStmt, h, depth + 1);
            break;
        case vpiIf: case vpiWhile: case vpiRepeat: case vpiDoWhile: case vpiWait: case vpiForever:
        case vpiDelayControl: case vpiEventControl: case vpiForeachStmt:
            stmt_one("stmt", vpiStmt, h, depth + 1);
            break;
        case vpiIfElse:
            stmt_one("then", vpiStmt, h, depth + 1);
            stmt_one("else", vpiElseStmt, h, depth + 1);
            break;
        case vpiFor:
            stmt_all("init", vpiForInitStmt, h, depth + 1);
            stmt_all("inc", vpiForIncStmt, h, depth + 1);
            stmt_one("stmt", vpiStmt, h, depth + 1);
            break;
        case vpiCase: {
            vpiHandle it = vpi_iterate(vpiCaseItem, h), item;
            while (it && (item = vpi_scan(it))) {
                vpi_printf("%*sitem", (depth + 1) * 2, "");
                vpiHandle exprs = vpi_iterate(vpiExpr, item), e;
                if (!exprs) vpi_printf(" default");
                while (exprs && (e = vpi_scan(exprs))) expr("expr", e);
                vpi_printf("\n");
                stmt_one("stmt", vpiStmt, item, depth + 2);
            }
            break;
        }
        default:
            break;
    }
}

static void walk_scope(PLI_INT32 relation, vpiHandle scope) {
    vpiHandle it = vpi_iterate(relation, scope), owner;
    if (!it) return;
    while ((owner = vpi_scan(it))) {
        vpi_printf("%s %s line=%d\n", text(vpiType, owner),
                   relation == vpiProcess ? "-" : text(vpiName, owner), vpi_get(vpiLineNo, owner));
        vpiHandle body = vpi_handle(vpiStmt, owner);
        if (body) stmt("stmt", body, 1);
        else vpi_printf("  stmt NULL level=%d\n", vpi_chk_error(NULL));
    }
}

static PLI_INT32 statements(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    walk_scope(vpiProcess, tb);
    walk_scope(vpiTaskFunc, tb);
    /* A named begin found by name iterates the same statements. */
    vpiHandle main_block = vpi_handle_by_name("tb.main", NULL);
    vpiHandle it = vpi_iterate(vpiStmt, main_block);
    vpiHandle first = it ? vpi_scan(it) : NULL;
    if (it && first) vpi_free_object(it);
    vpi_printf("tb.main first=%s line=%d\n", first ? text(vpiType, first) : "NULL",
               first ? vpi_get(vpiLineNo, first) : 0);
    /* No per-statement callbacks: cbStmt fails with an error. */
    s_cb_data cb;
    memset(&cb, 0, sizeof cb);
    cb.reason = cbStmt;
    cb.obj = first;
    vpiHandle registered = vpi_register_cb(&cb);
    vpi_printf("cbStmt handle=%s level=%d\n", registered ? "set" : "NULL", vpi_chk_error(NULL));
    vpi_handle(vpiStmt, vpi_handle_by_name("tb.a", NULL));
    vpi_printf("vpiStmt of a variable level=%d\n", vpi_chk_error(NULL));
    vpi_printf("failures=%d\n", failures);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$statements";
    data.calltf = statements;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
