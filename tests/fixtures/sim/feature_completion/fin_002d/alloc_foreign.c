/* Allocation failure matrix: the DPI-C routines and the VPI application of
 * alloc_foreign.sv. */
#include "svdpi.h"
#include "sv_vpi_user.h"
#include "vpi_user.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void sv_note(const char* message, int value);
extern int sv_wait(int cycles);

DPI_DLLESPEC int c_sum(const svOpenArrayHandle values)
{
    int sum = 0;
    int index;
    for (index = svLow(values, 1); index <= svHigh(values, 1); index++)
        sum += *(const int*)svGetArrElemPtr1(values, index);
    return sum;
}

DPI_DLLESPEC const char* c_join(const char* left, const char* right)
{
    static char joined[64];
    snprintf(joined, sizeof joined, "%s+%s", left, right);
    return joined;
}

DPI_DLLESPEC void c_wide(const svLogicVecVal* in, svLogicVecVal* out)
{
    int word;
    for (word = 0; word < 4; word++) {
        out[word].aval = in[3 - word].aval;
        out[word].bval = 0;
    }
    out[3].aval &= 0xf;
}

DPI_DLLESPEC int c_walk(int steps)
{
    int step;
    for (step = 0; step < steps; step++) {
        sv_note(svGetNameFromScope(svGetScope()), step + 1);
        if (sv_wait(1)) return 1;
    }
    return 0;
}

static int sessions;

DPI_DLLESPEC void* c_open(const char* name)
{
    sessions += (int)strlen(name);
    return &sessions;
}

DPI_DLLESPEC int c_close(void* handle)
{
    return handle == &sessions ? sessions : -1;
}

static vpiHandle first_argument(void)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = arguments ? vpi_scan(arguments) : NULL;
    if (argument) vpi_free_object(arguments);
    return argument;
}

static vpiHandle named_object(void)
{
    vpiHandle argument = first_argument();
    s_vpi_value text;
    text.format = vpiStringVal;
    if (!argument) return NULL;
    vpi_get_value(argument, &text);
    return vpi_handle_by_name(text.value.str, NULL);
}

/* $inspect(scope): the variables of a scope with their sizes and values in
 * three formats. */
static PLI_INT32 inspect(PLI_BYTE8* user)
{
    vpiHandle scope = named_object();
    vpiHandle iterator;
    vpiHandle object;
    int objects = 0;
    (void)user;
    if (!scope) {
        vpi_printf("inspect: no scope\n");
        return 0;
    }
    iterator = vpi_iterate(vpiVariables, scope);
    while (iterator && (object = vpi_scan(iterator)) != NULL) {
        s_vpi_value value;
        objects++;
        if (vpi_get(vpiType, object) != vpiLogicVar && vpi_get(vpiType, object) != vpiStringVar)
            continue;
        value.format = vpi_get(vpiType, object) == vpiStringVar ? vpiStringVal : vpiHexStrVal;
        vpi_get_value(object, &value);
        vpi_printf("inspect %s %d %s\n", vpi_get_str(vpiFullName, object),
                   (int)vpi_get(vpiSize, object), value.value.str);
        if (vpi_get(vpiSize, object) == 100) {
            value.format = vpiVectorVal;
            vpi_get_value(object, &value);
            vpi_printf("inspect vector %08x\n", (unsigned)value.value.vector[0].aval);
            value.format = vpiDecStrVal;
            vpi_get_value(object, &value);
            vpi_printf("inspect decimal %s\n", value.value.str);
        }
    }
    iterator = vpi_iterate(vpiModule, NULL);
    while (iterator && (object = vpi_scan(iterator)) != NULL)
        vpi_printf("inspect top %s %s\n", vpi_get_str(vpiName, object),
                   vpi_get_str(vpiDefName, object));
    vpi_printf("inspect objects %d\n", objects);
    return 0;
}

static int changes;

static PLI_INT32 changed(p_cb_data data)
{
    changes++;
    if (changes == 3)
        vpi_printf("watch %s %d\n", vpi_get_str(vpiName, data->obj),
                   (int)data->value->value.integer);
    return 0;
}

static PLI_INT32 watch(PLI_BYTE8* user)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data request;
    (void)user;
    memset(&request, 0, sizeof request);
    time.type = vpiSimTime;
    value.format = vpiIntVal;
    request.reason = cbValueChange;
    request.cb_rtn = changed;
    request.obj = named_object();
    request.time = &time;
    request.value = &value;
    if (request.obj) vpi_register_cb(&request);
    return 0;
}

static PLI_INT32 poke(PLI_BYTE8* user)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle path = vpi_scan(arguments);
    vpiHandle operand = vpi_scan(arguments);
    vpiHandle object;
    s_vpi_value text;
    s_vpi_value number;
    (void)user;
    text.format = vpiStringVal;
    number.format = vpiIntVal;
    vpi_get_value(path, &text);
    object = vpi_handle_by_name(text.value.str, NULL);
    vpi_get_value(operand, &number);
    vpi_free_object(arguments);
    if (object)
        vpi_put_value(object, &number, NULL, vpiNoDelay);
    else
        vpi_printf("poke: no object\n");
    return 0;
}

static PLI_INT32 fired(p_cb_data data)
{
    vpi_printf("later at %u after %d changes\n", (unsigned)data->time->low, changes);
    return 0;
}

static PLI_INT32 later(PLI_BYTE8* user)
{
    static s_vpi_time time;
    vpiHandle argument = first_argument();
    s_vpi_value number;
    s_cb_data request;
    (void)user;
    number.format = vpiIntVal;
    number.value.integer = 1;
    if (argument) vpi_get_value(argument, &number);
    memset(&request, 0, sizeof request);
    time.type = vpiSimTime;
    time.low = (PLI_UINT32)number.value.integer;
    request.reason = cbAfterDelay;
    request.cb_rtn = fired;
    request.time = &time;
    vpi_register_cb(&request);
    return 0;
}

static PLI_INT32 ended(p_cb_data data)
{
    (void)data;
    vpi_printf("end of simulation after %d changes\n", changes);
    return 0;
}

static void systf(const char* name, PLI_INT32 (*call)(PLI_BYTE8*))
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = call;
    vpi_register_systf(&data);
}

static void startup(void)
{
    s_cb_data request;
    systf("$inspect", inspect);
    systf("$watch", watch);
    systf("$poke", poke);
    systf("$later", later);
    memset(&request, 0, sizeof request);
    request.reason = cbEndOfSimulation;
    request.cb_rtn = ended;
    vpi_register_cb(&request);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
