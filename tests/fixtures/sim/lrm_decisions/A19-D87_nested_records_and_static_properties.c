/* Companion of A19-D87_nested_records_and_static_properties.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int error(void) { return vpi_chk_error(NULL) != 0; }

/* A copy: the string vpi_get_str returns lasts until the next call. */
static const char* str(int property, vpiHandle h) {
    static char buffers[4][256];
    static int next;
    char* buffer = buffers[next++ & 3];
    const char* text = h ? vpi_get_str(property, h) : NULL;
    snprintf(buffer, sizeof buffers[0], "%s", text ? text : "NULL");
    return buffer;
}

static vpiHandle find(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    int failed = error();
    if (!h) vpi_printf("%s: NULL error=%d\n", name, failed);
    return h;
}

static int integer(vpiHandle h) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(h, &value);
    return (int)value.value.integer;
}

static void indices(vpiHandle element) {
    vpiHandle it = vpi_iterate(vpiIndex, element), x;
    vpi_printf("  %s indices:", str(vpiFullName, element));
    while (it && (x = vpi_scan(it))) vpi_printf(" %d", integer(x));
    vpiHandle one = vpi_handle(vpiIndex, element);
    int failed = error();
    vpi_printf("; vpiIndex %s error=%d\n", one ? "handle" : "NULL", failed);
}

static void nested(void) {
    vpiHandle qq = find("tb.qq"), it, x;
    vpi_printf("tb.qq size=%d\n", (int)vpi_get(vpiSize, qq));
    it = vpi_iterate(vpiReg, qq);
    while (it && (x = vpi_scan(it)))
        vpi_printf("  %s=%d parent=%s\n", str(vpiName, x), integer(x),
                   str(vpiName, vpi_handle(vpiParent, x)));
    x = vpi_handle_by_index(qq, 0);
    int failed = error();
    vpi_printf("  by index 0: %s error=%d\n", x ? "handle" : "NULL", failed);
    PLI_INT32 both[2] = {1, 0};
    x = vpi_handle_by_multi_index(qq, 2, both);
    failed = error();
    vpi_printf("  by indices 1 0: %s error=%d\n", x ? "handle" : "NULL", failed);
    find("tb.qq[1]");
    x = find("tb.qq[1][0]");
    if (x) {
        indices(x);
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        value.value.integer = 30;
        vpi_put_value(x, &value, NULL, vpiNoDelay);
    }
    x = find("tb.s2[1][2]");
    if (x) indices(x);
}

static void records(void) {
    vpiHandle rq = find("tb.rq");
    vpiHandle element = rq ? vpi_handle_by_index(rq, 0) : NULL, it, x;
    vpi_printf("%s struct=%d size=%d automatic=%d\n", str(vpiFullName, element),
               vpi_get(vpiType, element) == vpiStructVar, (int)vpi_get(vpiSize, element),
               (int)vpi_get(vpiAutomatic, element));
    it = vpi_iterate(vpiMember, element);
    while (it && (x = vpi_scan(it)))
        vpi_printf("  member %s %s parent=%s\n", str(vpiName, x), str(vpiFullName, x),
                   str(vpiName, vpi_handle(vpiParent, x)));
    x = find("tb.rq[0].a");
    if (x) {
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        value.value.integer = 50;
        vpi_put_value(x, &value, NULL, vpiNoDelay);
    }
}

static void list(vpiHandle h, int relation, const char* what) {
    vpiHandle it = vpi_iterate(relation, h), x;
    vpi_printf("  %s:", what);
    while (it && (x = vpi_scan(it)))
        vpi_printf(" %s=%s/%d", str(vpiName, x), str(vpiFullName, x),
                   (int)vpi_get(vpiAutomatic, x));
    vpi_printf("\n");
}

static void statics(void) {
    vpiHandle object = vpi_handle(vpiClassObj, find("tb.p"));
    vpi_printf("object of D\n");
    list(object, vpiVariables, "variables");
    list(object, vpiNamedEvent, "events");
    vpiHandle classes = vpi_iterate(vpiClassDefn, find("tb")), defn;
    while (classes && (defn = vpi_scan(classes))) {
        vpi_printf("defn %s\n", str(vpiFullName, defn));
        list(defn, vpiVariables, "variables");
        list(defn, vpiNamedEvent, "events");
    }
    vpiHandle si = find("tb.p.si");
    vpi_printf("tb.p.si: %s=%d\n", str(vpiFullName, si), integer(si));
    vpiHandle sa1 = find("tb.C::sa[1]");
    vpi_printf("tb.C::sa[1]: %s=%d automatic=%d\n", str(vpiFullName, sa1), integer(sa1),
               (int)vpi_get(vpiAutomatic, sa1));
    vpiHandle da = find("tb.p.da");
    vpiHandle da0 = da ? vpi_handle_by_index(da, 0) : NULL;
    vpi_printf("tb.p.da[0] automatic=%d\n", (int)vpi_get(vpiAutomatic, da0));
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = 40;
    vpi_put_value(find("tb.C::si"), &value, NULL, vpiNoDelay);
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)"dd";
    vpi_put_value(find("tb.D::ds"), &value, NULL, vpiNoDelay);
    value.format = vpiSuppressVal;
    vpi_put_value(find("tb.C::se"), &value, NULL, vpiNoDelay);
    vpi_printf("put error=%d\n", error());
}

static PLI_INT32 d87(PLI_BYTE8* user_data) {
    (void)user_data;
    nested();
    records();
    statics();
    return 0;
}

static void register_d87(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$d87";
    data.calltf = d87;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d87, 0};
