/* Decision A19-D43 companion: list the uses of tb.v, tb.v[0] and tb.x. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int compare_lines(const void* left, const void* right) {
    return strcmp((const char*)left, (const char*)right);
}

/* The order of the uses is not specified: print them sorted. */
static void list_uses(const char* label, vpiHandle object) {
    char lines[32][64];
    int count = 0;
    vpi_printf("uses of %s:\n", label);
    vpiHandle it = vpi_iterate(vpiUse, object), use;
    while (it && (use = vpi_scan(it))) {
        if (count == 32) continue;
        snprintf(lines[count++], sizeof lines[0], "%s line %d", vpi_get_str(vpiType, use),
                 (int)vpi_get(vpiLineNo, use));
    }
    qsort(lines, (size_t)count, sizeof lines[0], compare_lines);
    for (int i = 0; i < count; ++i) vpi_printf("  %s\n", lines[i]);
}

static PLI_INT32 d43(PLI_BYTE8* data) {
    (void)data;
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    list_uses("tb.v", v);
    list_uses("tb.v[0]", vpi_handle_by_index(v, 0));
    list_uses("tb.x", vpi_handle_by_name((PLI_BYTE8*)"tb.x", NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d43";
    task.calltf = d43;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
