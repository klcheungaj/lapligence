/* ADV-019 family 3: an independent walker of the module paths and timing
 * checks of timing.sv (IEEE 1800-2009 37.35, 37.36). For every instance it
 * prints each module path with its type, polarities, ifnone flag, terminals
 * (expression, direction, edge), data terminal and condition, and each
 * timing check with its type, reference and data terminals (edge,
 * condition), notifier and arguments; then checks that each terminal leads
 * back to its path or check and that exhausted iterators are freed. */
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
        vpi_printf("  CHECK FAILED: %s\n", what);
    }
}

static void exhausted(vpiHandle iterator) {
    vpi_get(vpiType, iterator);
    check(vpi_chk_error(NULL) != 0, "exhausted iterator is released");
}

/* An expression: an object by name, else its type and (constants) value. */
static void expression(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf(" %s=NULL", label);
        return;
    }
    PLI_INT32 type = vpi_get(vpiType, h);
    if (type == vpiConstant || type == vpiOperation) {
        s_vpi_value value;
        value.format = vpiIntVal;
        vpi_get_value(h, &value);
        vpi_printf(" %s=%s", label, text(vpiType, h));
        if (vpi_chk_error(NULL) == 0) vpi_printf("(%d)", (int)value.value.integer);
        return;
    }
    vpi_printf(" %s=%s:%s", label, text(vpiType, h), text(vpiFullName, h));
}

static void term(const char* label, vpiHandle owner, vpiHandle t) {
    if (!t) {
        vpi_printf("    %s NULL\n", label);
        return;
    }
    vpi_printf("    %s %s edge=0x%02x", label, text(vpiType, t), vpi_get(vpiEdge, t));
    if (vpi_get(vpiType, t) == vpiPathTerm) vpi_printf(" dir=%d", vpi_get(vpiDirection, t));
    expression("expr", vpi_handle(vpiExpr, t));
    if (vpi_get(vpiType, t) == vpiTchkTerm) expression("cond", vpi_handle(vpiCondition, t));
    vpi_printf("\n");
    check(vpi_compare_objects(vpi_handle(vpiParent, t), owner), "a terminal leads back to its owner");
}

static void paths(vpiHandle module) {
    vpiHandle it = vpi_iterate(vpiModPath, module), path;
    if (!it) return;
    while ((path = vpi_scan(it))) {
        vpi_printf("  %s line=%d name=%s type=%d polarity=%d data_polarity=%d ifnone=%d\n",
                   text(vpiType, path), vpi_get(vpiLineNo, path), text(vpiName, path),
                   vpi_get(vpiPathType, path), vpi_get(vpiPolarity, path),
                   vpi_get(vpiDataPolarity, path), vpi_get(vpiModPathHasIfNone, path));
        static const PLI_INT32 kinds[] = {vpiModPathIn, vpiModPathOut};
        static const char* labels[] = {"in", "out"};
        for (int k = 0; k < 2; ++k) {
            vpiHandle terms = vpi_iterate(kinds[k], path), t;
            while ((t = vpi_scan(terms))) term(labels[k], path, t);
            if (terms) exhausted(terms);
        }
        term("data", path, vpi_handle(vpiModDataPathIn, path));
        vpi_printf("   ");
        expression("condition", vpi_handle(vpiCondition, path));
        vpi_printf("\n");
        check(vpi_compare_objects(vpi_handle(vpiModule, path), module), "a path's module");
        PLI_BYTE8* name = vpi_get_str(vpiName, path);
        check(name && !vpi_handle_by_name(name, NULL), "a path is named but not found by name");
    }
    exhausted(it);
}

static void checks(vpiHandle module) {
    vpiHandle it = vpi_iterate(vpiTchk, module), tchk;
    if (!it) return;
    while ((tchk = vpi_scan(it))) {
        vpi_printf("  %s line=%d tchk_type=%d\n", text(vpiType, tchk), vpi_get(vpiLineNo, tchk),
                   vpi_get(vpiTchkType, tchk));
        term("ref", tchk, vpi_handle(vpiTchkRefTerm, tchk));
        term("data", tchk, vpi_handle(vpiTchkDataTerm, tchk));
        vpi_printf("   ");
        expression("notifier", vpi_handle(vpiTchkNotifier, tchk));
        vpi_printf("\n    args");
        vpiHandle args = vpi_iterate(vpiExpr, tchk), arg;
        while ((arg = vpi_scan(args))) {
            if (vpi_get(vpiType, arg) == vpiTchkTerm) vpi_printf(" tchk_term");
            else expression("", arg);
        }
        vpi_printf("\n");
        if (args) exhausted(args);
        check(vpi_compare_objects(vpi_handle(vpiModule, tchk), module), "a check's module");
    }
    exhausted(it);
}

static void walk(vpiHandle module) {
    vpi_printf("%s %s\n", text(vpiType, module), text(vpiFullName, module));
    paths(module);
    checks(module);
    vpiHandle it = vpi_iterate(vpiModule, module), child;
    while ((child = vpi_scan(it))) walk(child);
}

static PLI_INT32 timing(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tops = vpi_iterate(vpiModule, NULL), top;
    while ((top = vpi_scan(tops))) walk(top);
    /* Relations that do not apply. */
    vpiHandle path = vpi_scan(vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL)));
    vpi_iterate(vpiExpr, path);
    vpi_printf("path vpiExpr level=%d\n", vpi_chk_error(NULL));
    vpi_get(vpiTchkType, path);
    vpi_printf("path vpiTchkType level=%d\n", vpi_chk_error(NULL));
    vpi_printf("failures=%d\n", failures);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$timing";
    task.calltf = timing;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
