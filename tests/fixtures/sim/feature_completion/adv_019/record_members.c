/* ADV-019 structure members (see record_members.sv): $rmembers(1) lists the
 * members of two unpacked structures, reads, finds and writes them and
 * their elements and selects, and registers value-change callbacks;
 * $rmembers(2) reads the values the design and the callbacks left. Unpacked
 * array members of strings and structures are refused. "!" marks an answer
 * that raised an error. */
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

static const char* value_of(vpiHandle x) {
    static char buffer[128];
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    int type = get(vpiType, x);
    if (type == vpiStringVar) {
        value.format = vpiStringVal;
    } else if (type == vpiRealVar) {
        value.format = vpiRealVal;
    } else if (type == vpiClassVar || type == vpiArrayVar || type == vpiRegArray) {
        return "-";
    } else {
        value.format = vpiHexStrVal;
    }
    vpi_get_value(x, &value);
    int error = failed();
    if (value.format == vpiRealVal)
        snprintf(buffer, sizeof buffer, "%g%s", value.value.real, error ? "!" : "");
    else
        snprintf(buffer, sizeof buffer, "%s%s", value.value.str ? value.value.str : "NULL",
                 error ? "!" : "");
    return buffer;
}

static void show(const char* label, vpiHandle x) {
    if (!x) {
        vpi_printf("%s: NULL%s\n", label, failed() ? "!" : "");
        return;
    }
    vpiHandle parent = vpi_handle(vpiParent, x);
    vpi_printf("%s: type=%d %s %s size=%d member=%d element=%d value=%s parent=%s\n", label,
               get(vpiType, x), str(vpiName, x), str(vpiFullName, x), get(vpiSize, x),
               get(vpiStructUnionMember, x), get(vpiArrayMember, x), value_of(x),
               parent ? str(vpiName, parent) : "NULL");
}

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static void members(const char* name) {
    vpiHandle s = by_name(name);
    vpiHandle it = s ? vpi_iterate(vpiMember, s) : NULL;
    vpiHandle m;
    int refused = failed();
    vpi_printf("%s members:", name);
    while (it && (m = vpi_scan(it))) vpi_printf(" %s(%d)", str(vpiName, m), get(vpiType, m));
    vpi_printf("%s\n", refused ? " !" : "");
}

static void array(const char* name) {
    vpiHandle a = by_name(name);
    if (!a) {
        vpi_printf("%s: NULL%s\n", name, failed() ? "!" : "");
        return;
    }
    vpi_printf("%s: type=%d size=%d array_type=%d member=%d", str(vpiFullName, a),
               get(vpiType, a), get(vpiSize, a), get(vpiArrayType, a),
               get(vpiStructUnionMember, a));
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
    if (!ranges) (void)failed();
    vpi_printf("\n");
    vpiHandle elements = vpi_iterate(vpiReg, a);
    vpiHandle e;
    int n = 0;
    while (elements && (e = vpi_scan(elements))) {
        if (n < 2) show("  first", e);
        ++n;
    }
    vpi_printf("  elements=%d\n", n);
}

static void put_int(const char* name, int v) {
    vpiHandle x = by_name(name);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = v;
    if (x) vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=%d%s\n", name, v, !x || failed() ? "!" : "");
}

static void put_text(const char* name, const char* text) {
    vpiHandle x = by_name(name);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)text;
    if (x) vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=\"%s\"%s\n", name, text, !x || failed() ? "!" : "");
}

static void put_real(const char* name, double v) {
    vpiHandle x = by_name(name);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiRealVal;
    value.value.real = v;
    if (x) vpi_put_value(x, &value, NULL, vpiNoDelay);
    vpi_printf("put %s=%g%s\n", name, v, !x || failed() ? "!" : "");
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("cb %s =%s at %d\n", str(vpiFullName, data->obj), data->value->value.str,
               (int)data->time->low);
    return 0;
}

static void watch(const char* name) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value value = {vpiHexStrVal, {0}};
    vpiHandle x = by_name(name);
    s_cb_data cb;
    memset(&cb, 0, sizeof cb);
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = x;
    cb.time = &time;
    cb.value = &value;
    vpiHandle r = x ? vpi_register_cb(&cb) : NULL;
    vpi_printf("watch %s: %s%s\n", name, r ? "ok" : "NULL", failed() ? "!" : "");
}

static void step1(void) {
    members("tb.r");
    members("tb.cr");
    members("tb.lr");
    show("tb.lr.sa[1]", by_name("tb.lr.sa[1]"));
    show("tb.lr.ia[1].x", by_name("tb.lr.ia[1].x"));
    show("tb.r.s", by_name("tb.r.s"));
    vpiHandle h = by_name("tb.r.h");
    show("tb.r.h", h);
    vpi_printf("tb.r.h object=%s\n", h && vpi_handle(vpiClassObj, h) ? "yes" : "NULL");
    array("tb.r.arr");
    array("tb.r.m");
    array("tb.r.ra");
    array("tb.r.arr20");
    array("tb.r.q");
    array("tb.r.da");
    array("tb.cr.big");
    show("tb.r.q[1]", by_name("tb.r.q[1]"));
    show("tb.r.m[1][2]", by_name("tb.r.m[1][2]"));
    show("tb.r.arr[3]", by_name("tb.r.arr[3]"));
    show("tb.cr.big[4999]", by_name("tb.cr.big[4999]"));
    show("tb.cr.w", by_name("tb.cr.w"));
    show("tb.cr.x", by_name("tb.cr.x"));
    show("index arr 2", vpi_handle_by_index(by_name("tb.r.arr"), 2));
    int indices[2] = {1, 2};
    show("multi m [1][2]", vpi_handle_by_multi_index(by_name("tb.r.m"), 2, indices));
    show("tb.r.f[1]", by_name("tb.r.f[1]"));
    show("tb.r.f[3:0]", by_name("tb.r.f[3:0]"));
    show("tb.cr.w[4]", by_name("tb.cr.w[4]"));
    show("tb.cr.w[7:4]", by_name("tb.cr.w[7:4]"));
    show("tb.r.m[1][2][3]", by_name("tb.r.m[1][2][3]"));
    put_int("tb.r.arr[1]", 66);
    put_text("tb.r.s", "yo");
    put_int("tb.r.q[0]", 33);
    put_int("tb.r.da[1]", 99);
    put_int("tb.r.f[0]", 0);
    put_int("tb.r.arr20[19]", 190);
    put_real("tb.r.ra[0]", 4.25);
    put_int("tb.r.m[0][1]", 21);
    put_int("tb.cr.big[4999]", 4242);
    put_int("tb.cr.w[0]", 1);
    put_real("tb.cr.x", 0.5);
    watch("tb.r.arr[0]");
    watch("tb.cr.big[7]");
}

static void step2(void) {
    show("tb.r.arr[0]", by_name("tb.r.arr[0]"));
    show("tb.r.arr[1]", by_name("tb.r.arr[1]"));
    show("tb.cr.big[7]", by_name("tb.cr.big[7]"));
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
    vpi_printf("== rmembers %d\n", (int)value.value.integer);
    if (value.value.integer == 1)
        step1();
    else
        step2();
    return 0;
}

static void register_rmembers(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$rmembers";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_rmembers, 0};
