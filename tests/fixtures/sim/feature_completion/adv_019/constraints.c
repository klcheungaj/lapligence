/* ADV-019 family 3: walks the constraints of constraints.sv (IEEE 1800-2009
 * 37.30, 37.34): each constraint's name, full name, qualifiers, state and
 * parent from the class defn and from the object `p` refers to, and the tree
 * of its constraint items with their relations, decompiled expressions and
 * distribution types. */
#include "sv_vpi_user.h"
#include <string.h>

static int get(PLI_INT32 property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    return vpi_chk_error(NULL) ? -99 : value;
}

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return vpi_chk_error(NULL) || !s ? "-" : s;
}

static void indent(int depth) {
    for (int i = 0; i < depth; ++i) vpi_printf("  ");
}

static void tree(const char* role, vpiHandle h, int depth);

static void each(PLI_INT32 relation, const char* role, vpiHandle h, int depth) {
    vpiHandle it = vpi_iterate(relation, h), child;
    if (vpi_chk_error(NULL)) return;
    while (it && (child = vpi_scan(it))) tree(role, child, depth);
}

static void one(PLI_INT32 relation, const char* role, vpiHandle h, int depth) {
    vpiHandle child = vpi_handle(relation, h);
    if (vpi_chk_error(NULL)) {
        indent(depth);
        vpi_printf("%s: error\n", role);
        return;
    }
    if (child) tree(role, child, depth);
}

static void tree(const char* role, vpiHandle h, int depth) {
    int type = (int)vpi_get(vpiType, h);
    indent(depth);
    vpi_printf("%s: %s line=%d", role, str(vpiType, h), get(vpiLineNo, h));
    switch (type) {
        case vpiDistribution:
            vpi_printf("\n");
            one(vpiExpr, "expr", h, depth + 1);
            each(vpiDistItem, "item", h, depth + 1);
            break;
        case vpiDistItem:
            vpi_printf(" disttype=%d\n", get(vpiDistType, h));
            one(vpiValueRange, "range", h, depth + 1);
            one(vpiWeight, "weight", h, depth + 1);
            break;
        case vpiConstraintOrdering:
            vpi_printf("\n");
            each(vpiSolveBefore, "before", h, depth + 1);
            each(vpiSolveAfter, "after", h, depth + 1);
            break;
        case vpiImplication:
        case vpiConstrIf:
        case vpiConstrIfElse:
            vpi_printf("\n");
            one(vpiCondition, "condition", h, depth + 1);
            each(vpiConstraintExpr, "then", h, depth + 1);
            if (type == vpiConstrIfElse) each(vpiElseConst, "else", h, depth + 1);
            break;
        case vpiRange:
            vpi_printf("\n");
            one(vpiLeftRange, "left", h, depth + 1);
            one(vpiRightRange, "right", h, depth + 1);
            break;
        default:
            vpi_printf(" name=%s decompile=%s\n", str(vpiName, h), str(vpiDecompile, h));
            if (type == vpiOperation) each(vpiOperand, "operand", h, depth + 1);
            break;
    }
}

static void constraints(const char* label, vpiHandle owner, int items) {
    vpiHandle it = vpi_iterate(vpiConstraint, owner), c;
    while (it && (c = vpi_scan(it))) {
        vpiHandle parent = vpi_handle(vpiParent, c);
        vpi_printf("%s constraint %s full=%s automatic=%d virtual=%d alloc=%d enabled=%d "
                   "parent=%s line=%d\n",
                   label, str(vpiName, c), str(vpiFullName, c), get(vpiAutomatic, c),
                   get(vpiVirtual, c), get(vpiAllocScheme, c), get(vpiIsConstraintEnabled, c),
                   parent ? str(vpiType, parent) : "null", get(vpiLineNo, c));
        if (items) each(vpiConstraintItem, "item", c, 1);
    }
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiClassDefn, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
    vpiHandle defn = it ? vpi_scan(it) : NULL;
    if (it && defn) vpi_release_handle(it);
    constraints("defn", defn, 1);
    vpiHandle var = vpi_handle_by_name((PLI_BYTE8*)"tb.p", NULL);
    vpiHandle obj = vpi_handle(vpiClassObj, var);
    constraints("object", obj, 0);
    return 0;
}

static PLI_INT32 modes(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiClassDefn, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
    vpiHandle defn = it ? vpi_scan(it) : NULL;
    if (it && defn) vpi_release_handle(it);
    constraints("defn", defn, 0);
    vpiHandle obj = vpi_handle(vpiClassObj, vpi_handle_by_name((PLI_BYTE8*)"tb.p", NULL));
    constraints("object", obj, 0);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$constraints";
    task.calltf = walk;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$modes";
    task.calltf = modes;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
