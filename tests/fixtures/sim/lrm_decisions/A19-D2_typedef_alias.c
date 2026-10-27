/* Companion of A19-D2_typedef_alias.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static void chain(const char* label, vpiHandle typespec) {
    vpi_printf("%s:", label);
    for (int depth = 0; typespec && depth < 4; ++depth) {
        vpi_printf(" %s name=%s", text(vpi_get_str(vpiType, typespec)),
                   text(vpi_get_str(vpiName, typespec)));
        typespec = vpi_handle(vpiTypedefAlias, typespec);
        PLI_INT32 level = vpi_chk_error(NULL);
        vpi_printf(" ->%s", typespec ? "" : " NULL");
        if (level) vpi_printf(" level=%d", (int)level);
    }
    vpi_printf("\n");
}

static PLI_INT32 d2(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.c", "tb.t", "tb.n", "tb.a", "tb.b", "tb.raw", NULL};
    for (int i = 0; names[i]; ++i)
        chain(names[i], vpi_handle(vpiTypespec, vpi_handle_by_name((PLI_BYTE8*)names[i], NULL)));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = "$d2";
    data.calltf = d2;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
