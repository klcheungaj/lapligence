/* SIM-042 A02: system task/function registration, compiletf/sizetf/calltf,
 * vpiArgument, user data and return conventions. Companion of systf.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static int compile_calls, size_calls;
static vpiHandle noret_call;
static vpiHandle sum_registration;

static const char* text(PLI_BYTE8* value) { return value ? (const char*)value : "<null>"; }

static const char* copy_str(PLI_INT32 property, vpiHandle handle) {
    static char ring[8][128];
    static unsigned next;
    char* buffer = ring[next++ % 8];
    snprintf(buffer, sizeof(ring[0]), "%s", text(vpi_get_str(property, handle)));
    return buffer;
}

static const char* shown(vpiHandle handle, PLI_INT32 format) {
    static char ring[8][128];
    static unsigned next;
    char* buffer = ring[next++ % 8];
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    vpi_get_value(handle, &value);
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) snprintf(buffer, sizeof(ring[0]), "<error %d>", (int)level);
    else if (format == vpiIntVal) snprintf(buffer, sizeof(ring[0]), "%d", (int)value.value.integer);
    else if (format == vpiRealVal) snprintf(buffer, sizeof(ring[0]), "%.2f", value.value.real);
    else snprintf(buffer, sizeof(ring[0]), "%s", text(value.value.str));
    return buffer;
}

static PLI_INT32 sum_compile(PLI_BYTE8* data) {
    ++compile_calls;
    if (strcmp(data, "sum-data") != 0) return 1;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    int count = 0;
    while (vpi_scan(arguments)) ++count;
    return count == 2 ? 0 : 1;
}

static PLI_INT32 sum_size(PLI_BYTE8* data) {
    (void)data;
    ++size_calls;
    return 12;
}

static PLI_INT32 sum_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    PLI_INT32 total = 0;
    vpiHandle argument;
    while ((argument = vpi_scan(arguments)) != NULL) {
        s_vpi_value value;
        memset(&value, 0, sizeof(value));
        value.format = vpiIntVal;
        vpi_get_value(argument, &value);
        total += value.value.integer;
    }
    vpi_printf("sum call line=%d size=%d functype=%d\n", (int)vpi_get(vpiLineNo, call),
               (int)vpi_get(vpiSize, call), (int)vpi_get(vpiFuncType, call));
    s_vpi_value result;
    memset(&result, 0, sizeof(result));
    result.format = vpiIntVal;
    result.value.integer = total;
    if (vpi_put_value(call, &result, NULL, vpiNoDelay) || vpi_chk_error(NULL))
        vpi_printf("FAIL sum put\n");
    return 0;
}

static PLI_INT32 noret_compile(PLI_BYTE8* data) {
    (void)data;
    ++compile_calls;
    noret_call = vpi_handle(vpiSysTfCall, NULL);
    return 0;
}

static PLI_INT32 noret_call_routine(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("noret calltf puts nothing\n");
    return 0;
}

static PLI_INT32 half_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle x = vpi_scan(arguments);
    vpi_free_object(arguments);
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiRealVal;
    vpi_get_value(x, &value);
    value.value.real /= 2.0;
    s_vpi_value text_value;
    memset(&text_value, 0, sizeof(text_value));
    text_value.format = vpiStringVal;
    text_value.value.str = (PLI_BYTE8*)"9";
    vpiHandle refused = vpi_put_value(call, &text_value, NULL, vpiNoDelay);
    vpi_printf("half string put -> %s level=%d\n", refused ? "handle" : "null",
               (int)vpi_chk_error(NULL));
    (void)vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 big_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    s_vpi_time time;
    memset(&time, 0, sizeof(time));
    time.type = vpiSimTime;
    time.high = 2u;
    time.low = 5u;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiTimeVal;
    value.value.time = &time;
    vpi_printf("big size=%d\n", (int)vpi_get(vpiSize, call));
    (void)vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 args_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpi_printf("args call type=%s scope=%s name=%s\n", copy_str(vpiType, call),
               copy_str(vpiFullName, vpi_handle(vpiScope, call)), copy_str(vpiName, call));
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument;
    int position = 0;
    while ((argument = vpi_scan(arguments)) != NULL) {
        PLI_INT32 type = vpi_get(vpiType, argument);
        vpi_printf("arg%d type=%s", position, copy_str(vpiType, argument));
        if (type == vpiConstant)
            vpi_printf(" const=%d", (int)vpi_get(vpiConstType, argument));
        if (type == vpiRealVar) {
            vpi_printf(" real=%s\n", shown(argument, vpiRealVal));
        } else {
            vpi_printf(" name=%s size=%d bin=%s\n", copy_str(vpiName, argument),
                       (int)vpi_get(vpiSize, argument), shown(argument, vpiBinStrVal));
        }
        if (type == vpiConstant && vpi_get(vpiConstType, argument) == vpiStringConst)
            vpi_printf("arg%d string=%s\n", position, shown(argument, vpiStringVal));
        s_vpi_value value;
        memset(&value, 0, sizeof(value));
        value.format = vpiIntVal;
        value.value.integer = position == 0 ? 9 : 4;
        if (position == 0 || position == 1 || position == 3 || position == 6) {
            (void)vpi_put_value(argument, &value, NULL, vpiNoDelay);
            vpi_printf("arg%d put level=%d\n", position, (int)vpi_chk_error(NULL));
        }
        ++position;
    }
    return 0;
}

static PLI_INT32 count_compile(PLI_BYTE8* data) {
    (void)data;
    ++compile_calls;
    /* Static storage: the application owns user data (38.33). */
    static int counters[8];
    static int used;
    int* counter = used < 8 ? &counters[used++] : NULL;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    if (vpi_get_userdata(call) != NULL) return 1;
    return counter && vpi_put_userdata(call, counter) == 1 ? 0 : 1;
}

static PLI_INT32 count_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    int* counter = (int*)vpi_get_userdata(call);
    vpi_printf("count line=%d calls=%d\n", (int)vpi_get(vpiLineNo, call), counter ? ++*counter : -1);
    return 0;
}

static PLI_INT32 late_put(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    value.value.integer = 99;
    vpiHandle result = vpi_put_value(noret_call, &value, NULL, vpiNoDelay);
    vpi_printf("inactive put -> %s level=%d\n", result ? "handle" : "null", (int)vpi_chk_error(NULL));
    vpi_printf("inactive get=%s\n", shown(noret_call, vpiIntVal));
    vpi_printf("compiletf=%d sizetf=%d\n", compile_calls, size_calls);
    s_vpi_systf_data info;
    memset(&info, 0, sizeof(info));
    vpi_get_systf_info(sum_registration, &info);
    vpi_printf("systf name=%s type=%d functype=%d registered=%s user=%d\n", text(info.tfname),
               (int)info.type, (int)info.sysfunctype, copy_str(vpiName, sum_registration),
               (int)vpi_get(vpiUserDefn, sum_registration));
    vpi_printf("noret systf same=%d\n",
               (int)vpi_compare_objects(vpi_handle(vpiUserSystf, noret_call),
                                        vpi_handle(vpiUserSystf, noret_call)));
    vpi_printf("noret userdata=%s\n", vpi_get_userdata(noret_call) ? "set" : "null");
    return 0;
}

static vpiHandle register_one(PLI_INT32 type, PLI_INT32 functype, const char* name,
                              PLI_INT32 (*calltf)(PLI_BYTE8*), PLI_INT32 (*compiletf)(PLI_BYTE8*),
                              PLI_INT32 (*sizetf)(PLI_BYTE8*), const char* user) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = type;
    data.sysfunctype = functype;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = calltf;
    data.compiletf = compiletf;
    data.sizetf = sizetf;
    data.user_data = (PLI_BYTE8*)user;
    vpiHandle handle = vpi_register_systf(&data);
    if (!handle) vpi_printf("registration of %s failed\n", name);
    return handle;
}

static void register_all(void) {
    sum_registration = register_one(vpiSysFunc, vpiSizedSignedFunc, "$sum", sum_call, sum_compile,
                                    sum_size, "sum-data");
    register_one(vpiSysFunc, vpiIntFunc, "$noret", noret_call_routine, noret_compile, NULL, NULL);
    register_one(vpiSysFunc, vpiRealFunc, "$half", half_call, NULL, NULL, NULL);
    register_one(vpiSysFunc, vpiTimeFunc, "$big", big_call, NULL, NULL, NULL);
    register_one(vpiSysTask, 0, "$noop", NULL, NULL, NULL, NULL);
    register_one(vpiSysTask, 0, "$args", args_call, NULL, NULL, NULL);
    register_one(vpiSysTask, 0, "$count", count_call, count_compile, NULL, NULL);
    register_one(vpiSysTask, 0, "$late_put", late_put, NULL, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
