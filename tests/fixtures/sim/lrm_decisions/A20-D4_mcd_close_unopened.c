/* Companion of A20-D4_mcd_close_unopened.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void reg(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*)) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = calltf;
    if (!vpi_register_systf(&data)) vpi_printf("registration of %s failed\n", name);
}

static void close_report(const char* label, PLI_UINT32 mcd, PLI_UINT32 expected_left) {
    PLI_UINT32 left = vpi_mcd_close(mcd);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("close %s: %s level=%d\n", label,
               left == 0 ? "0" : (left == expected_left ? "unclosed bits" : "other"), (int)level);
}

static void flush_report(const char* label, PLI_UINT32 mcd) {
    PLI_INT32 result = vpi_mcd_flush(mcd);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("flush %s: %s level=%d\n", label, result ? "nonzero" : "0", (int)level);
}

static PLI_INT32 d4(PLI_BYTE8* data) {
    (void)data;
    PLI_UINT32 unopened = UINT32_C(1) << 30;
    PLI_UINT32 channel = vpi_mcd_open((PLI_BYTE8*)"a20_d4.txt");
    flush_report("0", 0);
    flush_report("unopened", unopened);
    close_report("1", 1, 1);
    PLI_INT32 written = vpi_mcd_printf(1, (PLI_BYTE8*)"channel 1 still writes\n");
    vpi_printf("printf 1 after close: %d\n", (int)written);
    close_report("0", 0, 0);
    close_report("unopened", unopened, unopened);
    close_report("open|unopened", channel | unopened, unopened);
    close_report("closed channel", channel, channel);
    PLI_UINT32 again = vpi_mcd_open((PLI_BYTE8*)"a20_d4.txt");
    close_report("reopened channel", again, again);
    return 0;
}

static void register_all(void) { reg("$d4", d4); }

void (*vlog_startup_routines[])(void) = {register_all, NULL};
