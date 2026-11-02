/* ADV-019 frame records (see frame_records.sv): $frames(n) prints the
 * automatics of each subprogram frame, from the active one outwards: name,
 * type, line, size and value. */
#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* text(const char* s) { return s ? s : "?"; }

static void print_value(vpiHandle x) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    int type = (int)vpi_get(vpiType, x);
    if (type == vpiStringVar) {
        value.format = vpiStringVal;
        vpi_get_value(x, &value);
        vpi_printf("\"%s\"", text(value.value.str));
    } else if (type == vpiRealVar) {
        value.format = vpiRealVal;
        vpi_get_value(x, &value);
        vpi_printf("%g", value.value.real);
    } else if (type == vpiClassVar) {
        vpiHandle object = vpi_handle(vpiClassObj, x);
        vpi_printf("<%s>", object ? text(vpi_get_str(vpiType, object)) : "null");
    } else if (type == vpiArrayVar) {
        vpi_printf("<array %d>", (int)vpi_get(vpiArrayType, x));
    } else {
        value.format = vpiDecStrVal;
        vpi_get_value(x, &value);
        vpi_printf("%s", text(value.value.str));
    }
    vpi_printf("%s", failed() ? " (error)" : "");
}

/* Frames of subprograms only: one named as the subprogram its origin call
 * calls, or as a task or function when no call activated it. Another
 * simulator also lists frames of blocks, which llg does not model (decision
 * A19-D44). */
static int subprogram_frame(vpiHandle frame) {
    const char* full = vpi_get_str(vpiFullName, frame);
    if (!full) return 0;
    char name[256];
    size_t length = 0;
    while (full[length] && length + 1 < sizeof name) {
        name[length] = full[length];
        ++length;
    }
    name[length] = '\0';
    vpiHandle origin = vpi_handle(vpiOrigin, frame);
    vpi_chk_error(NULL);
    if (origin) {
        int type = (int)vpi_get(vpiType, origin);
        vpiHandle callee = vpi_handle(
            type == vpiTaskCall || type == vpiMethodTaskCall ? vpiTask : vpiFunction, origin);
        vpi_chk_error(NULL);
        const char* called = callee ? vpi_get_str(vpiFullName, callee) : NULL;
        return called && !strcmp(called, name);
    }
    vpiHandle scope = vpi_handle_by_name(name, NULL);
    vpi_chk_error(NULL);
    int type = scope ? (int)vpi_get(vpiType, scope) : 0;
    return type == vpiTask || type == vpiFunction;
}

/* Type names as 37.17 spells the variable types (vpiReg and vpiLogicVar
 * share one code). */
static const char* type_name(int type) {
    switch (type) {
        case vpiIntVar: return "vpiIntVar";
        case vpiLogicVar: return "vpiLogicVar";
        case vpiStringVar: return "vpiStringVar";
        case vpiRealVar: return "vpiRealVar";
        case vpiClassVar: return "vpiClassVar";
        case vpiArrayVar: return "vpiArrayVar";
        case vpiStructVar: return "vpiStructVar";
        default: return "other";
    }
}

static vpiHandle find(vpiHandle frame, const char* name) {
    vpiHandle it = vpi_iterate(vpiAutomatics, frame), x, found = NULL;
    while (it && (x = vpi_scan(it)))
        if (!found && !strcmp(text(vpi_get_str(vpiName, x)), name)) found = x;
    return found;
}

static void put_int(vpiHandle x, int v) {
    if (!x) {
        vpi_printf("  put: no such automatic\n");
        return;
    }
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = v;
    vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("  put %s=%d (error %d)\n", text(vpi_get_str(vpiName, x)), v, failed());
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    vpi_free_object(args);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(arg, &value);
    int step = (int)value.value.integer;
    vpi_printf("step %d\n", step);
    vpiHandle own = NULL;
    for (vpiHandle frame = vpi_handle(vpiFrame, NULL); frame;
         frame = vpi_handle(vpiParent, frame)) {
        if (!subprogram_frame(frame)) continue;
        if (!own) own = frame;
        vpi_printf("  %s:", text(vpi_get_str(vpiFullName, frame)));
        vpiHandle it = vpi_iterate(vpiAutomatics, frame), x;
        if (!it) vpi_printf(" none (error %d)", failed());
        while (it && (x = vpi_scan(it))) {
            vpi_printf(" %s:%s@%d/%d=", text(vpi_get_str(vpiName, x)),
                       type_name((int)vpi_get(vpiType, x)), (int)vpi_get(vpiLineNo, x),
                       (int)vpi_get(vpiSize, x));
            failed();
            print_value(x);
        }
        vpi_printf("\n");
    }
    if (step == 2) put_int(find(own, "r"), 41);
    if (step == 5) put_int(find(own, "y"), 100);
    return 0;
}

static void register_frames(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$frames";
    data.calltf = calltf;
    data.sizetf = NULL;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_frames, 0};
