/* Decision A19-D75: the origin and statement of a frame (see the .sv).
 * $d75(n) prints, for each frame from the active one outwards, the type and
 * line of its origin, the subroutine the origin calls and the type and line
 * of its statement. */
#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* type_of(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiType, h) : NULL;
    return name ? name : "NULL";
}

static const char* full_name(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "-";
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
    vpi_printf("step %d\n", (int)value.value.integer);
    for (vpiHandle frame = vpi_handle(vpiFrame, NULL); frame;
         frame = vpi_handle(vpiParent, frame)) {
        if (!subprogram_frame(frame)) continue;
        vpiHandle origin = vpi_handle(vpiOrigin, frame);
        int origin_error = failed();
        vpi_printf("  %s origin %s", full_name(frame), type_of(origin));
        if (origin) {
            int type = (int)vpi_get(vpiType, origin);
            vpiHandle callee = vpi_handle(
                type == vpiTaskCall || type == vpiMethodTaskCall ? vpiTask : vpiFunction, origin);
            int callee_error = failed();
            vpi_printf("@%d calls %s (error %d)", (int)vpi_get(vpiLineNo, origin),
                       full_name(callee), callee_error);
        }
        vpi_printf(" (error %d)", origin_error);
        vpiHandle stmt = vpi_handle(vpiStmt, frame);
        int stmt_error = failed();
        vpi_printf(" stmt %s", type_of(stmt));
        if (stmt) vpi_printf("@%d", (int)vpi_get(vpiLineNo, stmt));
        vpi_printf(" (error %d)\n", stmt_error);
    }
    return 0;
}

static void register_d75(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$d75";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d75, 0};
