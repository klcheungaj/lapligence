/* Companion of A19-D83_fixed_automatic_elements.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof info);
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* str(int property, vpiHandle h) {
    const char* text = h ? vpi_get_str(property, h) : NULL;
    return text ? text : "NULL";
}

/* Own names: some simulators spell vpiLogicVar as vpiReg. */
static const char* type_name(int type) {
    switch (type) {
        case vpiLogicVar: return "logic";
        case vpiIntVar: return "int";
        case vpiRealVar: return "real";
        case vpiStringVar: return "string";
        case vpiArrayVar: return "array";
        default: return "other";
    }
}

static vpiHandle held; /* an element kept past the end of its frame */

/* The element's name, type, flags, parent and value, each printed before
 * the next call (vpi_get_str and value text may share a buffer). */
static void element(vpiHandle e) {
    s_vpi_value value;
    int type = (int)vpi_get(vpiType, e);
    vpiHandle parent = vpi_handle(vpiParent, e);
    memset(&value, 0, sizeof value);
    vpi_printf("    %s", str(vpiName, e));
    vpi_printf(" %s", str(vpiFullName, e));
    vpi_printf(" %s member=%d auto=%d", type_name(type), (int)vpi_get(vpiArrayMember, e),
               (int)vpi_get(vpiAutomatic, e));
    vpi_printf(" parent=%s", str(vpiName, parent));
    if (type == vpiRealVar) {
        value.format = vpiRealVal;
        vpi_get_value(e, &value);
        vpi_printf(" value=%0.2f\n", value.value.real);
    } else if (type == vpiStringVar) {
        value.format = vpiStringVal;
        vpi_get_value(e, &value);
        vpi_printf(" value=%s\n", value.value.str ? value.value.str : "NULL");
    } else {
        vpi_printf(" size=%d", (int)vpi_get(vpiSize, e));
        value.format = vpiBinStrVal;
        vpi_get_value(e, &value);
        vpi_printf(" bin=%s\n", value.value.str ? value.value.str : "NULL");
    }
}

static void by_index(vpiHandle a, int index) {
    vpiHandle e = vpi_handle_by_index(a, index);
    int error = level();
    vpi_printf("    by index %d:", index);
    if (e) {
        vpi_printf("\n");
        element(e);
    } else {
        vpi_printf(" NULL error=%d\n", error != 0);
    }
}

static void put_int(vpiHandle e, int number) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = number;
    vpi_put_value(e, &value, NULL, vpiNoDelay);
    vpi_printf("    put %s=%d error=%d\n", str(vpiName, e), number, level() != 0);
}

/* One array automatic, found by name. */
static void array(const char* name, int first, int iterate) {
    vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)name, NULL), r, e, it;
    if (!a) {
        vpi_printf("  %s NULL\n", name);
        return;
    }
    vpi_printf("  %s %s size=%d\n", str(vpiFullName, a), type_name((int)vpi_get(vpiType, a)),
               (int)vpi_get(vpiSize, a));
    /* The ranges of a fixed-size array. */
    it = vpi_get(vpiArrayType, a) == vpiStaticArray ? vpi_iterate(vpiRange, a) : NULL;
    while (it && (r = vpi_scan(it)) != NULL)
        vpi_printf("    range size=%d\n", (int)vpi_get(vpiSize, r));
    it = vpi_iterate(iterate, a);
    while (it && (e = vpi_scan(it)) != NULL) element(e);
    by_index(a, first);
    by_index(a, 7);
}

static void d83(int step) {
    s_vpi_value value;
    vpiHandle e, it;
    memset(&value, 0, sizeof value);
    vpi_printf("step %d\n", step);
    if (step == 1) {
        PLI_INT32 indices[2] = {1, 2};
        array("tb.t.arr", 1, vpiReg);
        array("tb.t.lar", 3, vpiVarSelect);
        array("tb.t.m2", 0, vpiReg);
        array("tb.t.fa", 2, vpiReg);
        array("tb.t.ra", 0, vpiVarSelect);
        array("tb.t.fo", 1, vpiReg);
        array("tb.t.rd", 1, vpiReg);
        array("tb.t.sa", 0, vpiVarSelect);
        array("tb.t.q", 0, vpiVarSelect);
        /* A static queue's elements by vpiVarSelect. */
        it = vpi_iterate(vpiVarSelect, vpi_handle_by_name((PLI_BYTE8*)"tb.dq", NULL));
        while (it && (e = vpi_scan(it)) != NULL) vpi_printf("  tb.dq var select %s\n", str(vpiFullName, e));
        e = vpi_handle_by_multi_index(vpi_handle_by_name((PLI_BYTE8*)"tb.t.m2", NULL), 2, indices);
        vpi_printf("  m2 by multi index [1][2]:\n");
        if (e) element(e);
        vpi_printf("  by name:\n");
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.m2[1][2]", NULL);
        if (e) {
            element(e);
            put_int(e, 66);
        }
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.arr[1]", NULL);
        if (e) {
            element(e);
            put_int(e, 93);
            held = e;
        }
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.lar[2]", NULL);
        if (e) {
            value.format = vpiHexStrVal;
            value.value.str = (PLI_BYTE8*)"5a";
            vpi_put_value(e, &value, NULL, vpiNoDelay);
            vpi_printf("    put lar[2]=5a error=%d\n", level() != 0);
        }
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.fa[0]", NULL);
        if (e) put_int(e, 8);
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.ra[1]", NULL);
        if (e) put_int(e, 92);
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.fo[1]", NULL);
        if (e) put_int(e, 82);
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.rd[1]", NULL);
        if (e) {
            value.format = vpiRealVal;
            value.value.real = 9.75;
            vpi_put_value(e, &value, NULL, vpiNoDelay);
            vpi_printf("    put rd[1]=9.75 error=%d\n", level() != 0);
        }
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.sa[1]", NULL);
        if (e) {
            value.format = vpiStringVal;
            value.value.str = (PLI_BYTE8*)"P";
            vpi_put_value(e, &value, NULL, vpiNoDelay);
            vpi_printf("    put sa[1]=P error=%d\n", level() != 0);
        }
    } else {
        e = vpi_handle_by_name((PLI_BYTE8*)"tb.t.arr", NULL);
        vpi_printf("  tb.t.arr by name after its frame ended: %s error=%d\n", e ? "found" : "NULL",
                   level() != 0);
        if (held) {
            value.format = vpiIntVal;
            vpi_get_value(held, &value);
            vpi_printf("  held arr[1] after its frame ended: error=%d\n", level() != 0);
        }
    }
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    (void)user_data;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_release_handle(args);
    d83((int)value.value.integer);
    return 0;
}

static void reg(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d83";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {reg, 0};
