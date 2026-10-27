/* Companion of A19-D6_primitive_names.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdlib.h>
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static int by_line(const void* left, const void* right) {
    vpiHandle a = *(const vpiHandle*)left, b = *(const vpiHandle*)right;
    int la = (int)vpi_get(vpiLineNo, a), lb = (int)vpi_get(vpiLineNo, b);
    if (la != lb) return la - lb;
    return strcmp(text(vpi_get_str(vpiFullName, a)), text(vpi_get_str(vpiFullName, b)));
}

static PLI_INT32 d6(PLI_BYTE8* data) {
    (void)data;
    vpiHandle items[8], prim;
    int count = 0;
    vpiHandle it = vpi_iterate(vpiPrimitive, vpi_handle_by_name("tb", NULL));
    while ((prim = vpi_scan(it)) && count < 8) items[count++] = prim;
    qsort(items, (size_t)count, sizeof(items[0]), by_line);
    for (int i = 0; i < count; ++i) {
        prim = items[i];
        const char* name = vpi_get_str(vpiName, prim);
        int name_level = (int)vpi_chk_error(NULL);
        const char* full = vpi_get_str(vpiFullName, prim);
        int full_level = (int)vpi_chk_error(NULL);
        vpi_printf("%s name=%s(%d) full=%s(%d) def=%s prim_type=%d",
                   text(vpi_get_str(vpiType, prim)), text(name), name_level, text(full),
                   full_level, text(vpi_get_str(vpiDefName, prim)), (int)vpi_get(vpiPrimType, prim));
        vpiHandle index = vpi_handle(vpiIndex, prim);
        if (index) {
            s_vpi_value value;
            value.format = vpiIntVal;
            vpi_get_value(index, &value);
            vpi_printf(" index=%d", (int)value.value.integer);
        } else {
            vpi_printf(" index=NULL(%d)", (int)vpi_chk_error(NULL));
        }
        vpiHandle terms = vpi_iterate(vpiPrimTerm, prim), term;
        while ((term = vpi_scan(terms))) {
            s_vpi_value value;
            value.format = vpiBinStrVal;
            vpi_get_value(term, &value);
            vpi_printf(" t%d=%s", (int)vpi_get(vpiTermIndex, term), value.value.str);
        }
        vpi_printf("\n");
    }
    vpiHandle element = vpi_handle_by_name("tb.ga[1]", NULL);
    vpi_printf("by name %s %s\n", text(vpi_get_str(vpiType, element)),
               text(vpi_get_str(vpiFullName, element)));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d6";
    task.calltf = d6;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
