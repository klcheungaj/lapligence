/* Decision A19-D77: handles to the automatics of an ended frame (see the
 * .sv). Step 1 of the first activation keeps a handle to `loc`; every later
 * step reads it, reads its type and writes it. */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle held;

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    vpi_free_object(args);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(arg, &value);
    int step = (int)value.value.integer;
    vpi_printf("step %d\n", step);
    if (step == 1 && !held) {
        vpiHandle it = vpi_iterate(vpiAutomatics, vpi_handle(vpiFrame, NULL)), x;
        while (it && (x = vpi_scan(it)))
            if (!held && vpi_get_str(vpiName, x) && !strcmp(vpi_get_str(vpiName, x), "loc"))
                held = x;
    }
    if (!held) return 0;
    s_vpi_value read;
    memset(&read, 0, sizeof read);
    read.format = vpiIntVal;
    vpi_get_value(held, &read);
    int read_error = failed();
    vpi_printf("  held loc: value %d (error %d)", read_error ? -1 : (int)read.value.integer,
               read_error);
    int type = (int)vpi_get(vpiType, held);
    vpi_printf(" type %d (error %d)", type, failed());
    s_vpi_value write;
    memset(&write, 0, sizeof write);
    write.format = vpiIntVal;
    write.value.integer = 7;
    vpi_put_value(held, &write, NULL, vpiNoDelay);
    vpi_printf(" put (error %d)\n", failed());
    return 0;
}

static void register_d77(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$d77";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d77, 0};
