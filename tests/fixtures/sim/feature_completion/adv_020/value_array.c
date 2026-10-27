/* ADV-020: vpi_get_value_array and vpi_put_value_array (IEEE 1800-2009
 * 38.16, 38.35). Companion of value_array.sv. Every call prints its result
 * and the vpi_chk_error level and code it left. */
#include "vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void report(const char* label) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: level=%d code=%s\n", label, (int)level,
               level && info.code ? (const char*)info.code : "-");
}

static vpiHandle named(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

/* Raw bytes of `count` elements of `groups` byte groups, most significant
 * byte first per group. */
static void print_raw(const PLI_BYTE8* raw, int count, int groups, int four) {
    for (int e = 0; e < count; ++e) {
        const unsigned char* element = (const unsigned char*)raw + e * groups * (four ? 2 : 1);
        vpi_printf("  [%d] A=", e);
        for (int g = groups; g-- > 0;) vpi_printf("%02x", element[g]);
        if (four) {
            vpi_printf(" B=");
            for (int g = groups; g-- > 0;) vpi_printf("%02x", element[groups + g]);
        }
        vpi_printf("\n");
    }
}

/* A failing get: the level, the code and whether the value pointer is now
 * NULL (38.16: a NULL value pointer reports the error). */
static void get_error(const char* label, vpiHandle handle, PLI_UINT32 format, PLI_UINT32 flags,
                      void* buffer, PLI_INT32* index, PLI_UINT32 num) {
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = format;
    array.flags = flags;
    array.value.rawvals = (PLI_BYTE8*)buffer;
    vpi_get_value_array(handle, &array, index, num);
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: level=%d code=%s value=%s\n", label, (int)level,
               level && info.code ? (const char*)info.code : "-",
               array.value.rawvals ? "set" : "NULL");
}

static void put_error(const char* label, vpiHandle handle, PLI_UINT32 format, PLI_UINT32 flags,
                      void* buffer, PLI_INT32* index, PLI_UINT32 num) {
    s_vpi_arrayvalue array;
    memset(&array, 0, sizeof(array));
    array.format = format;
    array.flags = flags;
    array.value.rawvals = (PLI_BYTE8*)buffer;
    vpi_put_value_array(handle, &array, index, num);
    report(label);
}

static PLI_INT32 va_get(PLI_BYTE8* data) {
    (void)data;
    vpiHandle a = named("tb.a"), b = named("tb.b");
    s_vpi_arrayvalue array;
    PLI_INT32 index[2];

    /* 38.16 example: five elements of a[2:0][3:5] from a[1][4], in an
     * application buffer. */
    PLI_BYTE8 raw[60];
    memset(raw, 0x55, sizeof(raw));
    memset(&array, 0, sizeof(array));
    array.format = vpiRawFourStateVal;
    array.flags = vpiUserAllocFlag;
    array.value.rawvals = raw;
    index[0] = 1;
    index[1] = 4;
    vpi_get_value_array(a, &array, index, 5);
    report("a raw4 from [1][4]");
    vpi_printf("  buffer=%s\n", array.value.rawvals == raw ? "same" : "other");
    print_raw(raw, 5, 6, 1);

    /* Simulator storage: vpiVectorVal, two words per 42-bit element. */
    memset(&array, 0, sizeof(array));
    array.format = vpiVectorVal;
    index[0] = 2;
    index[1] = 3;
    vpi_get_value_array(a, &array, index, 3);
    report("a vector from [2][3]");
    for (int e = 0; e < 3; ++e)
        vpi_printf("  [%d] %08x/%08x %08x/%08x\n", e, (unsigned)array.value.vectors[2 * e + 1].aval,
                   (unsigned)array.value.vectors[2 * e + 1].bval,
                   (unsigned)array.value.vectors[2 * e].aval,
                   (unsigned)array.value.vectors[2 * e].bval);

    /* Two-state view of a four-state array: x and z read as 0. */
    memset(raw, 0x55, sizeof(raw));
    memset(&array, 0, sizeof(array));
    array.format = vpiRawTwoStateVal;
    array.flags = vpiUserAllocFlag;
    array.value.rawvals = raw;
    index[0] = 1;
    index[1] = 5;
    vpi_get_value_array(a, &array, index, 2);
    report("a raw2 from [1][5]");
    print_raw(raw, 2, 6, 0);

    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    index[0] = 0;
    vpi_get_value_array(b, &array, index, 4);
    report("b int from [0]");
    vpi_printf("  %d %d %d %d\n", (int)array.value.integers[0], (int)array.value.integers[1],
               (int)array.value.integers[2], (int)array.value.integers[3]);

    memset(&array, 0, sizeof(array));
    array.format = vpiRawFourStateVal;
    index[0] = 2;
    vpi_get_value_array(b, &array, index, 2);
    report("b raw4 from [2]");
    print_raw(array.value.rawvals, 2, 2, 1);

    PLI_INT16 shorts[3];
    memset(&array, 0, sizeof(array));
    array.format = vpiShortIntVal;
    array.flags = vpiUserAllocFlag;
    array.value.shortints = shorts;
    index[0] = 0;
    vpi_get_value_array(named("tb.by"), &array, index, 3);
    report("by shortint");
    vpi_printf("  %d %d %d\n", shorts[0], shorts[1], shorts[2]);
    vpi_get_value_array(named("tb.si"), &array, index, 2);
    report("si shortint");
    vpi_printf("  %d %d\n", shorts[0], shorts[1]);

    PLI_INT64 longs[3];
    memset(&array, 0, sizeof(array));
    array.format = vpiLongIntVal;
    array.flags = vpiUserAllocFlag;
    array.value.longints = longs;
    vpi_get_value_array(named("tb.by"), &array, index, 3);
    report("by longint");
    vpi_printf("  %lld %lld %lld\n", (long long)longs[0], (long long)longs[1], (long long)longs[2]);
    vpi_get_value_array(named("tb.si"), &array, index, 2);
    report("si longint");
    vpi_printf("  %lld %lld\n", (long long)longs[0], (long long)longs[1]);
    vpi_get_value_array(named("tb.li"), &array, index, 2);
    report("li longint");
    vpi_printf("  %lld %lld\n", (long long)longs[0], (long long)longs[1]);

    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    index[0] = 1;
    vpi_get_value_array(named("tb.iv"), &array, index, 2);
    report("iv int from [1]");
    vpi_printf("  %d %d\n", (int)array.value.integers[0], (int)array.value.integers[1]);

    memset(&array, 0, sizeof(array));
    array.format = vpiRealVal;
    index[0] = 0;
    vpi_get_value_array(named("tb.r"), &array, index, 3);
    report("r real");
    vpi_printf("  %g %g %g\n", array.value.reals[0], array.value.reals[1], array.value.reals[2]);
    memset(&array, 0, sizeof(array));
    array.format = vpiShortRealVal;
    vpi_get_value_array(named("tb.sr"), &array, index, 2);
    report("sr shortreal");
    vpi_printf("  %g %g\n", (double)array.value.shortreals[0], (double)array.value.shortreals[1]);
    memset(&array, 0, sizeof(array));
    array.format = vpiRealVal;
    vpi_get_value_array(named("tb.sr"), &array, index, 2);
    report("sr real");
    vpi_printf("  %g %g\n", array.value.reals[0], array.value.reals[1]);

    memset(&array, 0, sizeof(array));
    array.format = vpiTimeVal;
    vpi_get_value_array(named("tb.t"), &array, index, 2);
    report("t time");
    for (int e = 0; e < 2; ++e)
        vpi_printf("  type=%d high=%u low=%u\n", (int)array.value.times[e].type,
                   (unsigned)array.value.times[e].high, (unsigned)array.value.times[e].low);

    memset(&array, 0, sizeof(array));
    array.format = vpiVectorVal;
    vpi_get_value_array(named("tb.ig"), &array, index, 2);
    report("ig vector");
    vpi_printf("  %08x/%08x %08x/%08x\n", (unsigned)array.value.vectors[0].aval,
               (unsigned)array.value.vectors[0].bval, (unsigned)array.value.vectors[1].aval,
               (unsigned)array.value.vectors[1].bval);

    memset(&array, 0, sizeof(array));
    array.format = vpiVectorVal;
    vpi_get_value_array(named("tb.n"), &array, index, 2);
    report("n vector");
    vpi_printf("  %x/%x %x/%x\n", (unsigned)array.value.vectors[0].aval,
               (unsigned)array.value.vectors[0].bval, (unsigned)array.value.vectors[1].aval,
               (unsigned)array.value.vectors[1].bval);

    /* Errors. */
    PLI_INT32 buffer[8];
    PLI_INT32 first[2] = {0, 3};
    get_error("not an array", named("tb.w"), vpiIntVal, 0, NULL, first, 1);
    get_error("an element", named("tb.a[0][3]"), vpiIntVal, 0, NULL, first, 1);
    get_error("unpacked struct elements", named("tb.us"), vpiIntVal, 0, NULL, first, 1);
    PLI_INT32 outside[2] = {3, 3};
    get_error("index outside", a, vpiIntVal, vpiUserAllocFlag, buffer, outside, 1);
    get_error("num 0", a, vpiIntVal, vpiUserAllocFlag, buffer, first, 0);
    PLI_INT32 late[2] = {0, 4};
    get_error("past the end", a, vpiIntVal, vpiUserAllocFlag, buffer, late, 3);
    get_error("null index", a, vpiIntVal, vpiUserAllocFlag, buffer, NULL, 1);
    get_error("vpiOneValue flag", a, vpiIntVal, vpiUserAllocFlag | vpiOneValue, buffer, first, 1);
    get_error("user buffer NULL", a, vpiIntVal, vpiUserAllocFlag, NULL, first, 1);
    get_error("vpiBinStrVal", a, vpiBinStrVal, 0, NULL, first, 1);
    get_error("shortint of int", named("tb.iv"), vpiShortIntVal, 0, NULL, first, 1);
    get_error("longint of int", named("tb.iv"), vpiLongIntVal, 0, NULL, first, 1);
    get_error("real of bit", b, vpiRealVal, 0, NULL, first, 1);
    get_error("shortreal of real", named("tb.r"), vpiShortRealVal, 0, NULL, first, 1);
    get_error("int of real", named("tb.r"), vpiIntVal, 0, NULL, first, 1);
    vpi_get_value_array(a, NULL, first, 1);
    report("null arrayvalue_p");
    return 0;
}

static PLI_INT32 va_put(PLI_BYTE8* data) {
    (void)data;
    vpiHandle a = named("tb.a"), b = named("tb.b");
    s_vpi_arrayvalue array;
    PLI_INT32 index[2];

    /* 38.35 example: five 42-bit elements from a[1][4]; the n-th has its
     * low n bytes set to 1 and the last is x in its low byte. */
    PLI_BYTE8 raw[60];
    memset(raw, 0, sizeof(raw));
    int offset = 0;
    for (int e = 1; e <= 5; ++e) {
        for (int i = 0; i < e; ++i) raw[offset + i] = 1;
        offset += 12;
    }
    offset -= 12;
    raw[offset] = (PLI_BYTE8)0xff;
    raw[offset + 6] = (PLI_BYTE8)0xff;
    memset(&array, 0, sizeof(array));
    array.format = vpiRawFourStateVal;
    array.flags = vpiPropagateOff;
    array.value.rawvals = raw;
    index[0] = 1;
    index[1] = 4;
    vpi_put_value_array(a, &array, index, 5);
    report("a raw4 to [1][4] propagate off");

    PLI_INT32 one = 0xabc;
    memset(&array, 0, sizeof(array));
    array.format = vpiIntVal;
    array.flags = vpiOneValue | vpiPropagateOff;
    array.value.integers = &one;
    index[0] = 0;
    vpi_put_value_array(b, &array, index, 4);
    report("b one value");

    /* A two-state array ignores the B bytes. */
    unsigned char two[4] = {0x34, 0x02, 0xff, 0x0f};
    memset(&array, 0, sizeof(array));
    array.format = vpiRawFourStateVal;
    array.value.rawvals = (PLI_BYTE8*)two;
    index[0] = 2;
    vpi_put_value_array(b, &array, index, 1);
    report("b raw4 to [2]");

    /* A four-state array takes B as 0 from vpiRawTwoStateVal. */
    unsigned char six[6] = {0x01, 0x02, 0, 0, 0, 0x80};
    memset(&array, 0, sizeof(array));
    array.format = vpiRawTwoStateVal;
    array.value.rawvals = (PLI_BYTE8*)six;
    index[0] = 2;
    index[1] = 3;
    vpi_put_value_array(a, &array, index, 1);
    report("a raw2 to [2][3]");

    PLI_INT16 shorts[2] = {-1, 300};
    memset(&array, 0, sizeof(array));
    array.format = vpiShortIntVal;
    array.value.shortints = shorts;
    index[0] = 1;
    vpi_put_value_array(named("tb.iv"), &array, index, 2);
    report("iv shortint from [1]");

    PLI_INT64 longs[2] = {-1, 42};
    memset(&array, 0, sizeof(array));
    array.format = vpiLongIntVal;
    array.value.longints = longs;
    index[0] = 0;
    vpi_put_value_array(named("tb.li"), &array, index, 2);
    report("li longint");

    double reals[2] = {3.25, -0.5};
    memset(&array, 0, sizeof(array));
    array.format = vpiRealVal;
    array.value.reals = reals;
    index[0] = 1;
    vpi_put_value_array(named("tb.r"), &array, index, 2);
    report("r real from [1]");
    float floats[2] = {1.25f, 2.5f};
    memset(&array, 0, sizeof(array));
    array.format = vpiShortRealVal;
    array.value.shortreals = floats;
    index[0] = 0;
    vpi_put_value_array(named("tb.sr"), &array, index, 2);
    report("sr shortreal");

    s_vpi_time times[2];
    memset(times, 0, sizeof(times));
    times[0].type = vpiSimTime;
    times[0].high = 1;
    times[1].type = vpiSimTime;
    times[1].low = 7;
    memset(&array, 0, sizeof(array));
    array.format = vpiTimeVal;
    array.value.times = times;
    vpi_put_value_array(named("tb.t"), &array, index, 2);
    report("t time");

    s_vpi_vecval vectors[2] = {{5, 0}, {0, 0xf}};
    memset(&array, 0, sizeof(array));
    array.format = vpiVectorVal;
    array.value.vectors = vectors;
    vpi_put_value_array(named("tb.ig"), &array, index, 2);
    report("ig vector");

    /* Errors: nothing is written. */
    PLI_INT32 values[4] = {1, 2, 3, 4};
    PLI_INT32 first[2] = {0, 3};
    put_error("net array", named("tb.n"), vpiIntVal, 0, values, first, 1);
    put_error("vpiUserAllocFlag", b, vpiIntVal, vpiUserAllocFlag, values, first, 1);
    put_error("vpiInertialDelay", b, vpiIntVal, vpiInertialDelay, values, first, 1);
    put_error("null buffer", b, vpiIntVal, 0, NULL, first, 1);
    PLI_INT32 third[1] = {2};
    put_error("past the end", b, vpiIntVal, 0, values, third, 3);
    put_error("shortint to byte", named("tb.by"), vpiShortIntVal, 0, values, first, 1);
    put_error("longint to shortint", named("tb.si"), vpiLongIntVal, 0, values, first, 1);
    put_error("unpacked struct elements", named("tb.us"), vpiIntVal, 0, values, first, 1);
    put_error("vpiRawFourStateVal to real", named("tb.r"), vpiRawFourStateVal, 0, values, first,
              1);
    vpi_put_value_array(b, NULL, first, 1);
    report("null arrayvalue_p");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$va_get";
    task.calltf = va_get;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$va_put";
    task.calltf = va_put;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
