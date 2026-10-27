/* ADV-020 A01: every Annex K/M VPI routine resolves, and the unprovided ones
 * report an error instead of a guessed success. Companion of symbols.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

typedef void (*any_routine)(void);

/* The 41 routines of the Annex K vpi_user.h listing, in its order, then the
 * Annex M routine. Taking each address makes the loader (or the static
 * link) resolve every one of them. */
static const struct {
    const char* name;
    any_routine address;
} routines[] = {
    {"vpi_register_cb", (any_routine)vpi_register_cb},
    {"vpi_remove_cb", (any_routine)vpi_remove_cb},
    {"vpi_get_cb_info", (any_routine)vpi_get_cb_info},
    {"vpi_register_systf", (any_routine)vpi_register_systf},
    {"vpi_get_systf_info", (any_routine)vpi_get_systf_info},
    {"vpi_handle_by_name", (any_routine)vpi_handle_by_name},
    {"vpi_handle_by_index", (any_routine)vpi_handle_by_index},
    {"vpi_handle", (any_routine)vpi_handle},
    {"vpi_handle_multi", (any_routine)vpi_handle_multi},
    {"vpi_iterate", (any_routine)vpi_iterate},
    {"vpi_scan", (any_routine)vpi_scan},
    {"vpi_get", (any_routine)vpi_get},
    {"vpi_get64", (any_routine)vpi_get64},
    {"vpi_get_str", (any_routine)vpi_get_str},
    {"vpi_get_delays", (any_routine)vpi_get_delays},
    {"vpi_put_delays", (any_routine)vpi_put_delays},
    {"vpi_get_value", (any_routine)vpi_get_value},
    {"vpi_put_value", (any_routine)vpi_put_value},
    {"vpi_get_value_array", (any_routine)vpi_get_value_array},
    {"vpi_put_value_array", (any_routine)vpi_put_value_array},
    {"vpi_get_time", (any_routine)vpi_get_time},
    {"vpi_mcd_open", (any_routine)vpi_mcd_open},
    {"vpi_mcd_close", (any_routine)vpi_mcd_close},
    {"vpi_mcd_name", (any_routine)vpi_mcd_name},
    {"vpi_mcd_printf", (any_routine)vpi_mcd_printf},
    {"vpi_printf", (any_routine)vpi_printf},
    {"vpi_compare_objects", (any_routine)vpi_compare_objects},
    {"vpi_chk_error", (any_routine)vpi_chk_error},
    {"vpi_free_object", (any_routine)vpi_free_object},
    {"vpi_release_handle", (any_routine)vpi_release_handle},
    {"vpi_get_vlog_info", (any_routine)vpi_get_vlog_info},
    {"vpi_get_data", (any_routine)vpi_get_data},
    {"vpi_put_data", (any_routine)vpi_put_data},
    {"vpi_get_userdata", (any_routine)vpi_get_userdata},
    {"vpi_put_userdata", (any_routine)vpi_put_userdata},
    {"vpi_vprintf", (any_routine)vpi_vprintf},
    {"vpi_mcd_vprintf", (any_routine)vpi_mcd_vprintf},
    {"vpi_flush", (any_routine)vpi_flush},
    {"vpi_mcd_flush", (any_routine)vpi_mcd_flush},
    {"vpi_control", (any_routine)vpi_control},
    {"vpi_handle_by_multi_index", (any_routine)vpi_handle_by_multi_index},
    {"vpi_register_assertion_cb", (any_routine)vpi_register_assertion_cb},
};

/* Print the error the last call left: level, code and message. */
static void report(const char* label, const char* result) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: %s level=%d code=%s\n", label, result, (int)level,
               level && info.code ? (const char*)info.code : "-");
    if (level && info.message) vpi_printf("  %s\n", (const char*)info.message);
}

static PLI_INT32 assertion_cb(PLI_INT32 reason, p_vpi_time cb_time, vpiHandle assertion,
                              p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)cb_time;
    (void)assertion;
    (void)info;
    (void)user_data;
    vpi_printf("assertion callback ran\n");
    return 0;
}

static PLI_INT32 symbols(PLI_BYTE8* data) {
    (void)data;
    size_t count = sizeof(routines) / sizeof(routines[0]);
    size_t resolved = 0;
    for (size_t i = 0; i < count; ++i) {
        if (routines[i].address)
            ++resolved;
        else
            vpi_printf("unresolved: %s\n", routines[i].name);
    }
    vpi_printf("routines: %d of %d resolved\n", (int)resolved, (int)count);

    vpiHandle top = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    report("lookup tb", top ? "handle" : "NULL");

    report("vpi_handle_multi", vpi_handle_multi(vpiPort, top, top) ? "handle" : "NULL");

    /* The delay record must come back untouched. */
    s_vpi_time times[3];
    memset(times, 0, sizeof(times));
    times[0].type = vpiSimTime;
    times[0].low = 77;
    s_vpi_delay delay;
    memset(&delay, 0, sizeof(delay));
    delay.da = times;
    delay.no_of_delays = 1;
    delay.time_type = vpiSimTime;
    vpi_get_delays(top, &delay);
    report("vpi_get_delays", "void");
    vpi_put_delays(top, &delay);
    report("vpi_put_delays", "void");
    vpi_printf("delay record: da=%s no_of_delays=%d time_type=%d low=%u\n",
               delay.da == times ? "same" : "changed", (int)delay.no_of_delays,
               (int)delay.time_type, (unsigned)times[0].low);

    PLI_INT32 values[2] = {11, 22};
    PLI_INT32 index[1] = {0};
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    array.flags = vpiUserAllocFlag;
    array.value.integers = values;
    vpi_get_value_array(top, &array, index, 2);
    report("vpi_get_value_array", "void");
    vpi_put_value_array(top, &array, index, 2);
    report("vpi_put_value_array", "void");
    vpi_printf("array record: values=%d,%d integers=%s\n", (int)values[0], (int)values[1],
               array.value.integers == values ? "same" : "changed");

    char buffer[4] = {'a', 'b', 'c', 0};
    report("vpi_get_data", vpi_get_data(1, buffer, 3) == 0 ? "0" : "nonzero");
    report("vpi_put_data", vpi_put_data(1, buffer, 3) == 0 ? "0" : "nonzero");
    vpi_printf("data buffer: %s\n", buffer);

    report("vpi_register_assertion_cb",
           vpi_register_assertion_cb(top, cbAssertionFailure, assertion_cb, NULL) ? "handle"
                                                                                 : "NULL");

    /* 38.2: the next routine clears the error status. */
    report("vpi_get after the errors", vpi_get(vpiType, top) == vpiModule ? "vpiModule" : "other");
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$symbols";
    data.calltf = symbols;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
