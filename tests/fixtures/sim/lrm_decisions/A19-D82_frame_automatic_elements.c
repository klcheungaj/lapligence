/* Companion of A19-D82_frame_automatic_elements.sv: a VPI application (vlog_startup_routines). */
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

static vpiHandle held; /* an element kept past the end of its frame */

static void element(vpiHandle e) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiBinStrVal;
    /* vpi_get_str and vpiBinStrVal text may share one buffer: print each
     * before the next call. */
    vpi_printf("    %s", str(vpiName, e));
    vpi_printf(" %s", str(vpiFullName, e));
    vpi_printf(" size=%d", (int)vpi_get(vpiSize, e));
    vpi_get_value(e, &value);
    vpi_printf(" bin=%s\n", value.value.str ? value.value.str : "NULL");
}

static void automatic(vpiHandle a) {
    char name[32];
    int type = (int)vpi_get(vpiType, a);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    snprintf(name, sizeof name, "%s", str(vpiName, a));
    vpi_printf("  %s size=%d\n", name, (int)vpi_get(vpiSize, a));
    if (!strcmp(name, "rs")) {
        value.format = vpiStringVal;
        vpi_get_value(a, &value);
        vpi_printf("    value=%s\n", value.value.str ? value.value.str : "NULL");
        value.value.str = (PLI_BYTE8*)"put";
        vpi_put_value(a, &value, NULL, vpiNoDelay);
        vpi_printf("    put error=%d\n", level() != 0);
    } else if (!strcmp(name, "rr")) {
        value.format = vpiRealVal;
        vpi_get_value(a, &value);
        vpi_printf("    value=%.2f\n", value.value.real);
        value.value.real = 2.25;
        vpi_put_value(a, &value, NULL, vpiNoDelay);
        vpi_printf("    put error=%d\n", level() != 0);
    } else if (!strcmp(name, "ri")) {
        value.format = vpiIntVal;
        vpi_get_value(a, &value);
        vpi_printf("    value error=%d\n", level() != 0);
    } else if (type == vpiArrayVar) {
        vpiHandle it = vpi_iterate(vpiReg, a), e;
        while (it && (e = vpi_scan(it)) != NULL) element(e);
        /* A string-keyed array has no integer index. */
        if (!strcmp(name, "aa")) return;
        e = vpi_handle_by_index(a, 1);
        vpi_printf("    by index 1:");
        if (e) {
            vpi_printf("\n");
            element(e);
        } else {
            vpi_printf(" NULL\n");
        }
        if (!strcmp(name, "q") && e) {
            value.format = vpiIntVal;
            value.value.integer = 7;
            vpi_put_value(e, &value, NULL, vpiNoDelay);
            vpi_printf("    put q[1] error=%d\n", level() != 0);
        }
        if (!strcmp(name, "d")) held = e;
    }
}

static PLI_INT32 d82(PLI_BYTE8* user_data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    (void)user_data;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_release_handle(args);
    vpi_printf("step %d\n", (int)value.value.integer);
    if (value.value.integer == 1) {
        vpiHandle frame = vpi_handle(vpiFrame, NULL);
        vpiHandle it = frame ? vpi_iterate(vpiAutomatics, frame) : NULL, a;
        vpi_printf("  frame %s\n", str(vpiFullName, frame));
        while (it && (a = vpi_scan(it)) != NULL) automatic(a);
    } else if (held) {
        s_vpi_value stale;
        memset(&stale, 0, sizeof stale);
        stale.format = vpiIntVal;
        vpi_get_value(held, &stale);
        vpi_printf("  held d[1] after its frame ended: error=%d\n", level() != 0);
    }
    return 0;
}

static void reg(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d82";
    data.calltf = d82;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {reg, 0};
