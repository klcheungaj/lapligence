/* ADV-019 family 1: walks the members of packed and unpacked structures and
 * unions in members.sv (IEEE 1800-2009 37.17, 37.24), reads every member,
 * finds members by name, watches one with cbValueChange and writes members
 * with vpi_put_value; the design prints the written values. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void show_value(vpiHandle h) {
    s_vpi_value v;
    v.format = vpi_get(vpiType, h) == vpiRealVar ? vpiRealVal : vpiHexStrVal;
    vpi_get_value(h, &v);
    int level = vpi_chk_error(NULL);
    if (level)
        vpi_printf(" value: level=%d", level);
    else if (v.format == vpiRealVal)
        vpi_printf(" value=%g", v.value.real);
    else
        vpi_printf(" value=%s", v.value.str);
}

static void members(vpiHandle h, int depth) {
    vpiHandle it = vpi_iterate(vpiMember, h), m;
    while ((m = vpi_scan(it))) {
        int type = vpi_get(vpiType, m);
        vpi_printf("%*s%s", 2 * depth, "", str(vpiType, m));
        vpi_printf(" %s", str(vpiFullName, m));
        vpi_printf(" name=%s size=%d member=%d", str(vpiName, m), vpi_get(vpiSize, m),
                             vpi_get(vpiStructUnionMember, m));
        if (type == vpiStructVar || type == vpiUnionVar)
            vpi_printf(" packed=%d", vpi_get(vpiPacked, m));
        show_value(m);
        vpiHandle parent = vpi_handle(vpiParent, m);
        vpi_printf(" parent=%s", str(vpiName, parent));
        vpi_printf(" typespec=%s\n", str(vpiType, vpi_handle(vpiTypespec, m)));
        if (type == vpiStructVar || type == vpiUnionVar) members(m, depth + 1);
    }
}

static void object(vpiHandle h) {
    int type = vpi_get(vpiType, h);
    vpi_printf("%s %s size=%d", str(vpiType, h), str(vpiFullName, h), vpi_get(vpiSize, h));
    if (type == vpiStructVar || type == vpiUnionVar)
        vpi_printf(" packed=%d", vpi_get(vpiPacked, h));
    show_value(h);
    vpi_printf("\n");
    members(h, 1);
}

static void put(const char* name, PLI_INT32 value) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = value;
    vpi_put_value(h, &v, NULL, vpiNoDelay);
    vpi_printf("put %s = %d level=%d\n", name, value, vpi_chk_error(NULL));
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("change %s = %s\n", str(vpiFullName, data->obj), data->value->value.str);
    return 0;
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    object(vpi_handle_by_name("tb.ns", NULL));
    object(vpi_handle_by_name("tb.pu", NULL));
    object(vpi_handle_by_name("tb.up", NULL));
    vpiHandle array = vpi_handle_by_name("tb.uarr", NULL);
    vpi_printf("%s %s size=%d\n", str(vpiType, array), str(vpiFullName, array),
               vpi_get(vpiSize, array));
    for (int i = 0; i < 2; ++i) object(vpi_handle_by_index(array, i));

    static const char* names[] = {"tb.ns.inner.lo", "tb.uarr[1].b", "tb.up.p.hi", "tb.ns.missing",
                                                                NULL};
    for (int i = 0; names[i]; ++i) {
        vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        int level = vpi_chk_error(NULL);
        vpi_printf("by name %s: %s level=%d", names[i], h ? str(vpiFullName, h) : "NULL", level);
        if (h) vpi_printf(" parent=%s", str(vpiFullName, vpi_handle(vpiParent, h)));
        vpi_printf("\n");
    }
    vpiHandle plain = vpi_handle_by_name("tb.plain", NULL);
    vpiHandle none = vpi_iterate(vpiMember, plain);
    vpi_printf("members of plain: %s level=%d\n", none ? "iterator" : "NULL", vpi_chk_error(NULL));

    static s_vpi_value format;
    static s_vpi_time when;
    format.format = vpiHexStrVal;
    when.type = vpiSuppressTime;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = vpi_handle_by_name("tb.ns.inner.hi", NULL);
    cb.time = &when;
    cb.value = &format;
    vpi_register_cb(&cb);
    vpi_printf("callback level=%d\n", vpi_chk_error(NULL));

    put("tb.ns.inner.lo", 9);
    put("tb.ns.s", -1);
    put("tb.pu.p.hi", 0);
    put("tb.up.a", -4);
    put("tb.up.p.lo", 1);
    put("tb.uarr[1].b", 0x5a);
    put("tb.uarr[0]", 3);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$members";
    task.calltf = walk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
