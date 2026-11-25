/* Reads and writes every object of wrong_vpi_formats.sv in every value
 * format: `.` is served, `E` an error. A refused write that changes the value
 * of its object is reported. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "vpi_user.h"
#include "sv_vpi_user.h"

#define COUNT(array) ((int32_t)(sizeof array / sizeof array[0]))

static const char* const names[] = {
    "tb.r8", "tb.r8[2]", "tb.r1", "tb.w8", "tb.w1", "tb.su", "tb.ig", "tb.i32", "tb.by", "tb.big", "tb.s8",
    "tb.up", "tb.tm", "tb.re", "tb.sr", "tb.st", "tb.en", "tb.ps", "tb.ch", "tb.ev", "tb.mem", "tb.mem[1]",
    "tb.us", "tb.us.a", "tb.us.b", "tb.dyn", "tb.dyn[1]", "tb.qq", "tb.as", "tb.h", "tb.ci", "tb.P", "tb.LP",
    "tb.RP", "tb", "tb.fn", "tb.tk",
};

static const PLI_INT32 formats[] = {
    vpiBinStrVal, vpiOctStrVal, vpiDecStrVal,   vpiHexStrVal, vpiScalarVal, vpiIntVal,   vpiRealVal,
    vpiStringVal, vpiVectorVal, vpiStrengthVal, vpiTimeVal,   vpiObjTypeVal, vpiSuppressVal, 99,
};

static const PLI_INT32 flags[] = {
    vpiNoDelay, vpiInertialDelay, vpiTransportDelay, vpiPureTransportDelay, vpiForceFlag, vpiReleaseFlag, 77,
};

static int32_t failed(void) { return vpi_chk_error(NULL) != 0; }

static int32_t read_value(vpiHandle object, PLI_INT32 format, char* text, size_t size) {
    s_vpi_value value;
    s_vpi_time time;
    memset(&value, 0, sizeof value);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    value.format = format;
    if (format == vpiTimeVal) value.value.time = &time;
    vpi_chk_error(NULL);
    vpi_get_value(object, &value);
    if (failed()) return 0;
    if (text) snprintf(text, size, "%s", value.value.str);
    return 1;
}

static int32_t write_value(vpiHandle object, PLI_INT32 format, PLI_INT32 flag) {
    static s_vpi_vecval vector[4];
    static s_vpi_strengthval strengths[128];
    s_vpi_value value;
    s_vpi_time time;
    char text[] = "1";
    memset(&value, 0, sizeof value);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = 1;
    value.format = format;
    switch (format) {
        case vpiScalarVal: value.value.scalar = vpi1; break;
        case vpiRealVal: value.value.real = 1.0; break;
        case vpiVectorVal: value.value.vector = vector; break;
        case vpiTimeVal: value.value.time = &time; break;
        case vpiStrengthVal:
            for (int32_t i = 0; i < COUNT(strengths); i++) {
                strengths[i].logic = vpi1;
                strengths[i].s0 = 0;
                strengths[i].s1 = vpiStrongDrive;
            }
            value.value.strength = strengths;
            break;
        case vpiBinStrVal:
        case vpiOctStrVal:
        case vpiDecStrVal:
        case vpiHexStrVal:
        case vpiStringVal: value.value.str = text; break;
        default: value.value.integer = 1; break;
    }
    vpi_chk_error(NULL);
    vpi_put_value(object, &value, &time, flag);
    return !failed();
}

static void row(const char* name) {
    char reads[COUNT(formats) + 1] = "", writes[COUNT(formats) + 1] = "", modes[COUNT(flags) + 1] = "";
    char before[256], after[256];
    vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    if (!object) {
        vpi_printf("%s has no handle\n", name);
        vpi_chk_error(NULL);
        return;
    }
    for (int32_t i = 0; i < COUNT(formats); i++)
        reads[i] = read_value(object, formats[i], NULL, 0) ? '.' : 'E';
    for (int32_t i = 0; i < COUNT(formats); i++) {
        int32_t readable = read_value(object, vpiBinStrVal, before, sizeof before);
        writes[i] = write_value(object, formats[i], vpiNoDelay) ? '.' : 'E';
        if (writes[i] == 'E' && readable && read_value(object, vpiBinStrVal, after, sizeof after) &&
            strcmp(before, after))
            vpi_printf("%s changed by a refused write\n", name);
    }
    for (int32_t i = 0; i < COUNT(flags); i++) modes[i] = write_value(object, vpiIntVal, flags[i]) ? '.' : 'E';
    vpi_printf("%-10s type %3d get %s put %s flags %s\n", name, (int)vpi_get(vpiType, object), reads, writes, modes);
    vpi_chk_error(NULL);
}

static PLI_INT32 at_5(p_cb_data data) {
    (void)data;
    for (int32_t i = 0; i < COUNT(names); i++) row(names[i]);
    return 0;
}

static void startup(void) {
    s_cb_data data;
    s_vpi_time time;
    memset(&data, 0, sizeof data);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = 5;
    data.reason = cbAfterDelay;
    data.cb_rtn = at_5;
    data.time = &time;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
