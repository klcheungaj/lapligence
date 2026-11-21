/* Companion of A19-D6_generate_block_primitive_names.sv: a VPI application
   (vlog_startup_routines). The module's primitives are printed sorted by
   name, since the iteration order is not specified. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static int by_text(const void* left, const void* right) {
    return strcmp((const char*)left, (const char*)right);
}

static PLI_INT32 d6g(PLI_BYTE8* data) {
    (void)data;
    char rows[16][96];
    int count = 0;
    vpiHandle prim;
    vpiHandle it = vpi_iterate(vpiPrimitive, vpi_handle_by_name("tb", NULL));
    while (it && (prim = vpi_scan(it))) {
        const char* name = text(vpi_get_str(vpiName, prim));
        if (strchr(name, '.')) continue; /* a named primitive of a generate scope */
        if (count < 16)
            snprintf(rows[count++], sizeof rows[0], "%s full=%s def=%s", name,
                     text(vpi_get_str(vpiFullName, prim)), text(vpi_get_str(vpiDefName, prim)));
    }
    qsort(rows, (size_t)count, sizeof rows[0], by_text);
    for (int i = 0; i < count; ++i) vpi_printf("tb: %s\n", rows[i]);
    static const char* names[] = {"tb.udp$1",   "tb.udp$2",   "tb.t.udp$2", "tb.xor$3",
                                  "tb.and$4",   "tb.t.and$4", "tb.nand$5",  "tb.or$6",
                                  "tb.t.n.or$6", "tb.nor$7",  "tb.buf$8",   "tb.l[1].buf$9",
                                  "tb.t.g_named", "tb.g_named"};
    for (size_t i = 0; i < sizeof names / sizeof names[0]; ++i) {
        vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_chk_error(NULL);
        vpi_printf("by name %s: %s\n", names[i], h ? text(vpi_get_str(vpiFullName, h)) : "NULL");
    }
    return 0;
}

static void reg(void) {
    s_vpi_systf_data d = {vpiSysTask, 0, (PLI_BYTE8*)"$d6g", d6g, NULL, NULL, NULL};
    vpi_register_systf(&d);
}

void (*vlog_startup_routines[])(void) = {reg, NULL};
