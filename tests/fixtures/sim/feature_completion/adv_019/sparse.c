/* ADV-019 sparse fixed-size arrays (see sparse.sv): $sparse(1) reads, finds
 * and writes elements of arrays llg keeps in a keyed container and
 * registers value-change callbacks; $sparse(2) reads the values the design
 * and the callbacks left. "!" marks an answer that raised an error. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* str(int property, vpiHandle x) {
    static char buffers[4][256];
    static int next;
    char* buffer = buffers[next++ & 3];
    const char* s = vpi_get_str(property, x);
    snprintf(buffer, sizeof buffers[0], "%s%s", s ? s : "NULL", failed() ? "!" : "");
    return buffer;
}

static int get(int property, vpiHandle x) {
    int v = (int)vpi_get(property, x);
    return failed() ? -99 : v;
}

static const char* hex(vpiHandle x) {
    static char buffer[128];
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiHexStrVal;
    vpi_get_value(x, &value);
    snprintf(buffer, sizeof buffer, "%s%s", value.value.str ? value.value.str : "NULL",
             failed() ? "!" : "");
    return buffer;
}

static void element(const char* label, vpiHandle x) {
    if (!x) {
        vpi_printf("%s: NULL%s\n", label, failed() ? "!" : "");
        return;
    }
    vpiHandle parent = vpi_handle(vpiParent, x);
    vpi_printf("%s: type=%d %s size=%d member=%d value=%s parent=%s\n", label, get(vpiType, x),
               str(vpiFullName, x), get(vpiSize, x), get(vpiArrayMember, x), hex(x),
               parent ? str(vpiName, parent) : "NULL");
}

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static void array(const char* name) {
    vpiHandle a = by_name(name);
    if (!a) {
        vpi_printf("%s: NULL%s\n", name, failed() ? "!" : "");
        return;
    }
    vpi_printf("%s: type=%d size=%d memory=%d", str(vpiFullName, a), get(vpiType, a),
               get(vpiSize, a), get(vpiIsMemory, a));
    vpiHandle ranges = vpi_iterate(vpiRange, a);
    vpiHandle range;
    while (ranges && (range = vpi_scan(ranges))) {
        s_vpi_value l, r;
        memset(&l, 0, sizeof l);
        memset(&r, 0, sizeof r);
        l.format = r.format = vpiIntVal;
        vpi_get_value(vpi_handle(vpiLeftRange, range), &l);
        vpi_get_value(vpi_handle(vpiRightRange, range), &r);
        vpi_printf(" [%d:%d]", (int)l.value.integer, (int)r.value.integer);
    }
    vpi_printf("\n");
    vpiHandle elements = vpi_iterate(vpiReg, a);
    vpiHandle e;
    int n = 0;
    while (elements && (e = vpi_scan(elements))) {
        if (n < 2) element("  first", e);
        ++n;
    }
    vpi_printf("  elements=%d\n", n);
}

static void put_int(vpiHandle x, int v) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = v;
    vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=%d%s\n", str(vpiFullName, x), v, failed() ? "!" : "");
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("cb %s index=%d =%s at %d\n", str(vpiFullName, data->obj), (int)data->index,
               data->value->value.str, (int)data->time->low);
    return 0;
}

static void watch(vpiHandle x) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value value = {vpiHexStrVal, {0}};
    s_cb_data cb;
    memset(&cb, 0, sizeof cb);
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = x;
    cb.time = &time;
    cb.value = &value;
    vpiHandle r = vpi_register_cb(&cb);
    vpi_printf("watch %s: %s%s\n", str(vpiFullName, x), r ? "ok" : "NULL", failed() ? "!" : "");
}

static void step1(void) {
    array("tb.mem");
    array("tb.big");
    array("tb.two");
    array("tb.wide");
    element("tb.mem[5]", by_name("tb.mem[5]"));
    element("tb.mem[8191]", by_name("tb.mem[8191]"));
    element("tb.big[4000]", by_name("tb.big[4000]"));
    element("tb.two[3][4]", by_name("tb.two[3][4]"));
    element("tb.wide[7]", by_name("tb.wide[7]"));
    element("tb.mem[8192]", by_name("tb.mem[8192]"));
    vpiHandle mem = by_name("tb.mem");
    element("index 5", vpi_handle_by_index(mem, 5));
    element("index 9000", vpi_handle_by_index(mem, 9000));
    int indices[2] = {3, 4};
    element("multi [3][4]", vpi_handle_by_multi_index(by_name("tb.two"), 2, indices));
    put_int(by_name("tb.mem[6]"), 0x3c);
    put_int(by_name("tb.big[10]"), 1234);
    put_int(by_name("tb.two[1][2]"), 5);
    put_int(by_name("tb.wide[9]"), 0x5a);
    watch(by_name("tb.mem[7]"));
    watch(by_name("tb.big"));
    /* 38.16 and 38.35 across three cells, one of them never written. */
    s_vpi_arrayvalue values;
    memset(&values, 0, sizeof values);
    values.format = vpiIntVal;
    PLI_INT32 put[3] = {0x10, 0x11, 0x12};
    values.value.integers = put;
    PLI_INT32 first = 100;
    vpi_put_value_array(mem, &values, &first, 3);
    vpi_printf("put_value_array mem[100..102]%s\n", failed() ? "!" : "");
    memset(&values, 0, sizeof values);
    values.format = vpiIntVal;
    first = 4;
    vpi_get_value_array(mem, &values, &first, 4);
    if (failed() || !values.value.integers)
        vpi_printf("get_value_array mem[4..7]: !\n");
    else
        vpi_printf("get_value_array mem[4..7]: %x %x %x %x\n", (unsigned)values.value.integers[0],
                   (unsigned)values.value.integers[1], (unsigned)values.value.integers[2],
                   (unsigned)values.value.integers[3]);
}

static void step2(void) {
    element("tb.mem[7]", by_name("tb.mem[7]"));
    element("tb.mem[6]", by_name("tb.mem[6]"));
    element("tb.big[3]", by_name("tb.big[3]"));
    element("tb.big[10]", by_name("tb.big[10]"));
}

static PLI_INT32 calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_free_object(args);
    vpi_printf("== sparse %d\n", (int)value.value.integer);
    if (value.value.integer == 1)
        step1();
    else
        step2();
    return 0;
}

static void register_sparse(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$sparse";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_sparse, 0};
