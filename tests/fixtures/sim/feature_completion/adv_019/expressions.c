/* ADV-019 family 3: an independent walker of the expression objects of
 * expressions.sv (IEEE 1800-2009 37.54, 37.55, 37.19, 37.38). $exprs prints
 * the tree of every argument and of the right-hand side of every continuous
 * assignment and port connection of tb: object type, operator or constant
 * type, size, vpiDecompile and value, then the operands, select ranges and
 * call arguments. $again re-reads saved handles after the design changes
 * the variables they read. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failures;
static vpiHandle saved[32];
static int saved_count;

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

/* The value in its natural format: a real prints as %g, everything else
 * as a binary string; an expression without a value prints `error`. */
static void value_of(vpiHandle h) {
    s_vpi_value value;
    value.format = vpiObjTypeVal;
    vpi_get_value(h, &value);
    if (vpi_chk_error(NULL)) {
        vpi_printf(" value=error");
        return;
    }
    if (value.format == vpiRealVal) {
        vpi_printf(" value=%g", value.value.real);
        return;
    }
    value.format = vpiBinStrVal;
    vpi_get_value(h, &value);
    vpi_printf(" value=%s", value.value.str);
}

static void walk(const char* label, vpiHandle h, int depth);

static void walk_iteration(const char* label, PLI_INT32 relation, vpiHandle h, int depth) {
    vpiHandle it = vpi_iterate(relation, h), child;
    if (!it) return;
    while ((child = vpi_scan(it))) {
        walk(label, child, depth);
    }
}

static void walk_one(const char* label, PLI_INT32 relation, vpiHandle h, int depth) {
    vpiHandle child = vpi_handle(relation, h);
    if (child) walk(label, child, depth);
}

static void walk(const char* label, vpiHandle h, int depth) {
    PLI_INT32 type = vpi_get(vpiType, h);
    vpi_printf("%*s%s %s", depth * 2, "", label, text(vpiType, h));
    if (type == vpiOperation) vpi_printf(" op=%d", vpi_get(vpiOpType, h));
    if (type == vpiConstant) vpi_printf(" const=%d", vpi_get(vpiConstType, h));
    const char* name = vpi_get_str(vpiName, h);
    vpi_chk_error(NULL);
    if (name) vpi_printf(" name=%s", name);
    vpi_printf(" size=%d", vpi_get(vpiSize, h));
    if (type == vpiBitSelect || type == vpiPartSelect || type == vpiIndexedPartSelect ||
        type == vpiVarSelect)
        vpi_printf(" constant_select=%d", vpi_get(vpiConstantSelect, h));
    if (type == vpiIndexedPartSelect)
        vpi_printf(" indexed=%d", vpi_get(vpiIndexedPartSelectType, h));
    vpi_printf(" dec=\"%s\"", text(vpiDecompile, h));
    value_of(h);
    vpi_printf("\n");
    switch (type) {
        case vpiOperation:
            if (vpi_get(vpiOpType, h) == vpiCastOp) {
                vpiHandle typespec = vpi_handle(vpiTypespec, h);
                vpi_printf("%*s  typespec %s\n", depth * 2, "", text(vpiType, typespec));
            }
            walk_iteration("operand", vpiOperand, h, depth + 1);
            break;
        case vpiBitSelect:
            walk_one("parent", vpiParent, h, depth + 1);
            walk_one("index", vpiIndex, h, depth + 1);
            break;
        case vpiPartSelect:
            walk_one("parent", vpiParent, h, depth + 1);
            walk_one("left", vpiLeftRange, h, depth + 1);
            walk_one("right", vpiRightRange, h, depth + 1);
            break;
        case vpiIndexedPartSelect:
            walk_one("parent", vpiParent, h, depth + 1);
            walk_one("base", vpiBaseExpr, h, depth + 1);
            walk_one("width", vpiWidthExpr, h, depth + 1);
            break;
        case vpiVarSelect:
            walk_one("parent", vpiParent, h, depth + 1);
            walk_iteration("index", vpiIndex, h, depth + 1);
            break;
        case vpiFuncCall: {
            vpiHandle function = vpi_handle(vpiFunction, h);
            vpi_printf("%*s  function %s func_type=%d\n", depth * 2, "", text(vpiFullName, function),
                       vpi_get(vpiFuncType, h));
            walk_iteration("argument", vpiArgument, h, depth + 1);
            break;
        }
        case vpiSysFuncCall:
            vpi_printf("%*s  user_defn=%d\n", depth * 2, "", vpi_get(vpiUserDefn, h));
            walk_iteration("argument", vpiArgument, h, depth + 1);
            break;
        default:
            break;
    }
}

static PLI_INT32 exprs(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle it = vpi_iterate(vpiArgument, call), arg;
    int index = 0;
    while ((arg = vpi_scan(it))) {
        char label[32];
        snprintf(label, sizeof label, "arg%d", index++);
        walk(label, arg, 0);
    }
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    it = vpi_iterate(vpiContAssign, tb);
    vpiHandle assign;
    while ((assign = vpi_scan(it))) {
        vpiHandle lhs = vpi_handle(vpiLhs, assign), rhs = vpi_handle(vpiRhs, assign);
        vpi_printf("assign %s line=%d\n", text(vpiName, lhs), vpi_get(vpiLineNo, assign));
        walk("rhs", rhs, 1);
        if (saved_count < 32) saved[saved_count++] = rhs;
        /* The assignment's value is its right-hand side converted to the
         * left-hand side size (decision A19-D5). */
        vpi_printf("  assign");
        value_of(assign);
        vpi_printf("\n");
    }
    vpiHandle u = vpi_handle_by_name("tb.u", NULL);
    vpiHandle ports = vpi_iterate(vpiPort, u);
    vpiHandle first = vpi_scan(ports);
    vpi_free_object(ports);
    walk("high", vpi_handle(vpiHighConn, first), 0);
    /* Selects and their parents point back at the same objects. */
    vpiHandle w = vpi_handle_by_name("tb.w", NULL);
    vpiHandle part = saved[4];
    if (part) check(vpi_compare_objects(vpi_handle(vpiParent, part), w), "w[i*4 +: 8] parent is tb.w");
    vpi_printf("failures=%d\n", failures);
    return 0;
}

static PLI_INT32 again(PLI_BYTE8* user_data) {
    (void)user_data;
    for (int i = 0; i < saved_count; ++i) {
        vpi_printf("again %s", text(vpiDecompile, saved[i]));
        value_of(saved[i]);
        vpi_printf("\n");
    }
    return 0;
}

static void register_task(const char* name, PLI_INT32 (*routine)(PLI_BYTE8*)) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = routine;
    vpi_register_systf(&data);
}

static void startup(void) {
    register_task("$exprs", exprs);
    register_task("$again", again);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
