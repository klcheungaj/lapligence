/* Companion of A19-D71_mailbox_semaphore_waiters.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void waiters(const char* name) {
    vpiHandle variable = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpiHandle object = variable ? vpi_handle(vpiClassObj, variable) : NULL;
    vpiHandle it, thread;
    const char* class_name;
    int count = 0, threads = 1;
    if (!object) {
        vpi_printf("%s: no object error=%d\n", name, level() != 0);
        return;
    }
    class_name = vpi_get_str(vpiName, object);
    it = vpi_iterate(vpiWaitingProcesses, object);
    while (it && (thread = vpi_scan(it))) {
        ++count;
        threads = threads && vpi_get(vpiType, thread) == vpiThread;
    }
    vpi_printf("%s: object type=%d name=%s waiting=%d threads=%d error=%d\n", name,
               (int)vpi_get(vpiType, object), class_name ? class_name : "NULL", count, threads,
               level() != 0);
}

static PLI_INT32 d71(PLI_BYTE8* user) {
    (void)user;
    vpi_printf("$d71:\n");
    waiters("tb.mb");
    waiters("tb.full");
    waiters("tb.sm");
    return 0;
}

static void register_d71(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d71";
    tf.calltf = d71;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d71, 0};
