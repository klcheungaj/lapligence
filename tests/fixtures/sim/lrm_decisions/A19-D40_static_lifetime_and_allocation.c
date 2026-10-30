/* Decision A19-D40: lifetime and memory allocation properties of static
 * objects (IEEE 1800-2009 37.3.7). See
 * A19-D40_static_lifetime_and_allocation.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static void show(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf("%s missing\n", label);
        return;
    }
    int automatic = (int)vpi_get(vpiAutomatic, h);
    int automatic_error = level();
    int scheme = (int)vpi_get(vpiAllocScheme, h);
    int scheme_error = level();
    vpi_printf("%s vpiAutomatic %d level=%d vpiAllocScheme %d level=%d\n", label, automatic,
               automatic_error, scheme, scheme_error);
}

static vpiHandle first(vpiHandle h, int relation) {
    vpiHandle it = h ? vpi_iterate(relation, h) : NULL;
    vpiHandle item = it ? vpi_scan(it) : NULL;
    if (item) vpi_release_handle(it);
    return item;
}

static PLI_INT32 d40(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb.v",  "tb.b",  "tb.i",  "tb.ig",  "tb.si",  "tb.li",
                           "tb.by", "tb.t",  "tb.r",  "tb.sr",  "tb.pa",  "tb.ps",
                           "tb.mem", "tb.mem[1]", "tb.ev"};
    for (unsigned k = 0; k < sizeof names / sizeof names[0]; ++k) show(names[k], get(names[k]));
    show("tb.v bit", first(get("tb.v"), vpiBit));
    show("tb.mem[1] bit", first(get("tb.mem[1]"), vpiBit));
    show("tb.ps member", first(get("tb.ps"), vpiMember));
    /* A net has neither property (37.16). */
    show("tb.w", get("tb.w"));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d40";
    task.calltf = d40;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
