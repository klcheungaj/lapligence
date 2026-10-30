/* Decision A19-D38 companion: dump the UDP definitions of tb. */
#include "sv_vpi_user.h"
#include <string.h>

static void show_defn(vpiHandle defn) {
    vpi_printf("defn %s size=%d protected=%d seq=%d\n", vpi_get_str(vpiDefName, defn),
               (int)vpi_get(vpiSize, defn), (int)vpi_get(vpiProtected, defn),
               vpi_get(vpiPrimType, defn) == vpiSeqPrim);
    vpiHandle it = vpi_iterate(vpiIODecl, defn), io;
    while (it && (io = vpi_scan(it)))
        vpi_printf("  io %s dir=%d size=%d\n", vpi_get_str(vpiName, io),
                   (int)vpi_get(vpiDirection, io), (int)vpi_get(vpiSize, io));
    it = vpi_iterate(vpiTableEntry, defn);
    vpiHandle entry;
    while (it && (entry = vpi_scan(it))) {
        s_vpi_value value;
        value.format = vpiStringVal;
        vpi_get_value(entry, &value);
        vpi_printf("  entry size=%d %s", (int)vpi_get(vpiSize, entry), value.value.str);
        int words = ((int)vpi_get(vpiSize, entry) - 1) / 4 + 1;
        value.format = vpiVectorVal;
        vpi_get_value(entry, &value);
        for (int i = 0; i < words; ++i)
            vpi_printf(" %08x/%08x", (unsigned)value.value.vector[i].aval,
                       (unsigned)value.value.vector[i].bval);
        value.format = vpiIntVal;
        vpi_get_value(entry, &value);
        vpi_printf(" int-error=%d\n", vpi_chk_error(NULL) != 0);
    }
}

static PLI_INT32 d38(PLI_BYTE8* data) {
    (void)data;
    int count = 0;
    vpiHandle it = vpi_iterate(vpiUdpDefn, NULL), defn;
    while (it && (defn = vpi_scan(it))) ++count;
    vpi_printf("definitions %d\n", count);
    const char* names[] = {"tb.g1", "tb.f1"};
    for (int i = 0; i < 2; ++i) {
        vpiHandle prim = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        show_defn(vpi_handle(vpiUdpDefn, prim));
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d38";
    task.calltf = d38;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
