/* FIN-002 VPI application shared by tb_force_clocking.sv, tb_vif_swap.sv
 * and gl_pad_force.sv: $vforce/$vrelease/$vput
 * write a named object with the force, release and no-delay flags, $vget
 * reads one, $vwatch registers a value-change callback and $vreport prints
 * what the callback saw. */
#include "vpi_user.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define MAX_SEEN 64

static int changes;
static int seen[MAX_SEEN];

static vpiHandle argument(vpiHandle arguments)
{
    return arguments ? vpi_scan(arguments) : NULL;
}

/* The object named by the first (string literal) argument and, when
 * `value` is not NULL, the integer second argument. */
static vpiHandle named_object(int* value)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle path = argument(arguments);
    vpiHandle object = NULL;
    s_vpi_value text;
    text.format = vpiStringVal;
    if (path) {
        vpi_get_value(path, &text);
        object = vpi_handle_by_name(text.value.str, NULL);
        if (!object) vpi_printf("vpi: no object %s\n", text.value.str);
    }
    if (value) {
        vpiHandle operand = argument(arguments);
        s_vpi_value number;
        number.format = vpiIntVal;
        *value = 0;
        if (operand) {
            vpi_get_value(operand, &number);
            *value = number.value.integer;
        }
        arguments = operand ? arguments : NULL;
    }
    if (arguments && path) vpi_free_object(arguments);
    return object;
}

static PLI_INT32 write_object(PLI_BYTE8* user)
{
    PLI_INT32 flag = (PLI_INT32)(intptr_t)user;
    int number = 0;
    vpiHandle object = named_object(flag == vpiReleaseFlag ? NULL : &number);
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = number;
    if (object) vpi_put_value(object, &value, NULL, flag);
    return 0;
}

static PLI_INT32 get_object(PLI_BYTE8* user)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle object = named_object(NULL);
    s_vpi_value value;
    (void)user;
    value.format = vpiIntVal;
    value.value.integer = -1;
    if (object) vpi_get_value(object, &value);
    vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 changed(p_cb_data data)
{
    if (changes < MAX_SEEN) seen[changes] = (int)data->value->value.integer;
    changes++;
    return 0;
}

static PLI_INT32 watch_object(PLI_BYTE8* user)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data watch;
    (void)user;
    memset(&watch, 0, sizeof(watch));
    time.type = vpiSuppressTime;
    value.format = vpiIntVal;
    watch.reason = cbValueChange;
    watch.cb_rtn = changed;
    watch.obj = named_object(NULL);
    watch.time = &time;
    watch.value = &value;
    if (watch.obj) {
        vpi_printf("vpi watch %s size=%d\n", vpi_get_str(vpiFullName, watch.obj),
                   (int)vpi_get(vpiSize, watch.obj));
        vpi_register_cb(&watch);
    }
    return 0;
}

static PLI_INT32 report(PLI_BYTE8* user)
{
    int index;
    (void)user;
    vpi_printf("vpi changes=%d:", changes);
    for (index = 0; index < changes && index < MAX_SEEN; index++)
        vpi_printf(" %d", seen[index]);
    vpi_printf("\n");
    return 0;
}

static PLI_INT32 int_size(PLI_BYTE8* user)
{
    (void)user;
    return 32;
}

static void systf(PLI_INT32 type, const char* name, PLI_INT32 (*call)(PLI_BYTE8*), intptr_t user)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = type;
    data.sysfunctype = type == vpiSysFunc ? vpiSizedFunc : 0;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = call;
    data.sizetf = type == vpiSysFunc ? int_size : NULL;
    data.user_data = (PLI_BYTE8*)user;
    vpi_register_systf(&data);
}

static void startup(void)
{
    systf(vpiSysTask, "$vforce", write_object, vpiForceFlag);
    systf(vpiSysTask, "$vrelease", write_object, vpiReleaseFlag);
    systf(vpiSysTask, "$vput", write_object, vpiNoDelay);
    systf(vpiSysFunc, "$vget", get_object, 0);
    systf(vpiSysTask, "$vwatch", watch_object, 0);
    systf(vpiSysTask, "$vreport", report, 0);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
