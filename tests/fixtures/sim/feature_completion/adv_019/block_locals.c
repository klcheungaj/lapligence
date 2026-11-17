/* ADV-019 stored automatics (see block_locals.sv): $locals(1) runs in block
 * blk of automatic task t, $locals(2) in the body of its for loop and
 * $locals(3) after both blocks ended. "!" marks an answer that raised an
 * error. */
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

static const char* text(vpiHandle x) {
    static char buffer[128];
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = get(vpiType, x) == vpiStringVar ? vpiStringVal : vpiHexStrVal;
    vpi_get_value(x, &value);
    snprintf(buffer, sizeof buffer, "%s%s", value.value.str ? value.value.str : "NULL",
             failed() ? "!" : "");
    return buffer;
}

static void show(const char* label, vpiHandle x) {
    if (!x) {
        vpi_printf("%s: NULL%s\n", label, failed() ? "!" : "");
        return;
    }
    int type = get(vpiType, x);
    vpiHandle parent = vpi_handle(vpiParent, x);
    (void)failed();
    vpi_printf("%s: type=%d name=%s full=%s size=%d automatic=%d", label, type, str(vpiName, x),
               str(vpiFullName, x), get(vpiSize, x), get(vpiAutomatic, x));
    if (type == vpiArrayVar) vpi_printf(" array_type=%d", get(vpiArrayType, x));
    if (type != vpiStructVar && type != vpiArrayVar) vpi_printf(" value=%s", text(x));
    vpi_printf(" parent=%s\n", parent ? str(vpiName, parent) : "NULL");
}

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static void found(const char* name) { show(name, by_name(name)); }

static void children(const char* label, vpiHandle x, int type, int limit) {
    vpiHandle items = x ? vpi_iterate(type, x) : NULL;
    if (failed()) vpi_printf("  %s: !\n", label);
    vpiHandle item;
    int n = 0;
    while (items && (item = vpi_scan(items))) {
        if (n++ < limit) show(label, item);
    }
    vpi_printf("  %s count=%d\n", label, n);
}

static void put_int(const char* name, int v) {
    vpiHandle x = by_name(name);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = v;
    if (x) vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=%d%s%s\n", name, v, x ? "" : " (no handle)", failed() ? "!" : "");
}

static void put_text(const char* name, const char* v) {
    vpiHandle x = by_name(name);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)v;
    if (x) vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=\"%s\"%s%s\n", name, v, x ? "" : " (no handle)", failed() ? "!" : "");
}

static void step1(void) {
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    vpi_printf("frame %s\n", frame ? str(vpiFullName, frame) : "NULL");
    children("automatic", frame, vpiAutomatics, 8);
    found("tb.t.wide");
    found("tb.t.wide[1]");
    found("tb.t.big[4000]");
    found("tb.t.one");
    found("tb.t.one.s");
    children("member", by_name("tb.t.one"), vpiMember, 4);
    found("tb.t.ua.arr");
    found("tb.t.ua.arr[1]");
    children("element", by_name("tb.t.ua.arr"), vpiReg, 4);
    found("tb.t.blk.bx");
    found("tb.t.blk.bs");
    found("tb.t.bx");
    children("block variable", by_name("tb.t.blk"), vpiVariables, 4);
    put_int("tb.t.blk.bx", 99);
    put_text("tb.t.blk.bs", "put");
    put_text("tb.t.one.s", "yo");
    put_int("tb.t.one.a", 42);
    put_int("tb.t.ua.arr[1]", 70);
    put_int("tb.t.big[3]", 33);
    put_int("tb.t.wide[2]", 0x5a);
    found("tb.t.blk.bs");
}

static void step2(void) {
    found("tb.t.unnamed$$_0.unnamed$$_1.li");
    found("tb.t.blk.bx");
}

static void step3(void) {
    found("tb.t.unnamed$$_0.unnamed$$_1.li");
    found("tb.t.blk.bx");
    children("block variable", by_name("tb.t.blk"), vpiVariables, 4);
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
    vpi_printf("== locals %d\n", (int)value.value.integer);
    if (value.value.integer == 1)
        step1();
    else if (value.value.integer == 2)
        step2();
    else
        step3();
    return 0;
}

static void register_locals(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$locals";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_locals, 0};
