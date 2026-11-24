/* Companion of CS-D2_ambiguous_known_strength.sv: a VPI application
 * (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d2(PLI_BYTE8* data)
{
    static const char* names[] = {"tb.low", "tb.high", "tb.one"};
    (void)data;
    for (int i = 0; i < 3; ++i) {
        s_vpi_value value;
        value.format = vpiStrengthVal;
        vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)names[i], NULL), &value);
        vpi_printf("%s: logic=%d s0=%d s1=%d\n", names[i], (int)value.value.strength[0].logic,
                   (int)value.value.strength[0].s0, (int)value.value.strength[0].s1);
    }
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d2";
    task.calltf = d2;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
