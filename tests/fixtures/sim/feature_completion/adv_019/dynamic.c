/* ADV-019 family 3: walks the dynamic objects of dynamic.sv (IEEE 1800-2009
 * 37.17, 37.27-37.29): string, chandle and class variables, the class
 * objects they refer to and their properties, the class definitions and
 * typespecs, and the elements of dynamic arrays, queues and associative
 * arrays; finds objects by name and writes values with vpi_put_value. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void show_value(vpiHandle h) {
    s_vpi_value v;
    int type = vpi_get(vpiType, h);
    v.format = type == vpiRealVar ? vpiRealVal : type == vpiStringVar ? vpiStringVal : vpiIntVal;
    vpi_get_value(h, &v);
    int level = vpi_chk_error(NULL);
    if (level)
        vpi_printf(" value: level=%d", level);
    else if (v.format == vpiRealVal)
        vpi_printf(" value=%g", v.value.real);
    else if (v.format == vpiIntVal)
        vpi_printf(" value=%d", (int)v.value.integer);
    else
        vpi_printf(" value=%s", v.value.str);
}

static void object(vpiHandle obj, int depth);

static void elements(vpiHandle array, int depth) {
    vpiHandle it = vpi_iterate(vpiMemoryWord, array), e;
    while ((e = vpi_scan(it))) {
        vpi_printf("%*s%s %s", 2 * depth, "", str(vpiType, e), str(vpiName, e));
        vpiHandle index = vpi_handle(vpiIndex, e);
        if (index) {
            s_vpi_value v;
            v.format = vpiIntVal;
            vpi_get_value(index, &v);
            vpi_printf(" index=%d", (int)v.value.integer);
        } else {
            vpi_printf(" index: level=%d", vpi_chk_error(NULL));
        }
        vpi_printf(" member=%d", vpi_get(vpiArrayMember, e));
        if (vpi_get(vpiType, e) == vpiClassVar) {
            vpi_printf(" objid=%lld", (long long)vpi_get64(vpiObjId, e));
            vpiHandle target = vpi_handle(vpiClassObj, e);
            vpi_printf(" obj=%s\n", target ? "set" : "null");
            if (target) vpi_release_handle(target);
            continue;
        }
        show_value(e);
        vpi_printf("\n");
    }
}

static void variable(vpiHandle h, int depth) {
    int type = vpi_get(vpiType, h);
    vpi_printf("%*s%s %s", 2 * depth, "", str(vpiType, h), str(vpiName, h));
    const char* full = vpi_get_str(vpiFullName, h);
    if (full)
        vpi_printf(" full=%s", full);
    else
        vpi_printf(" full: level=%d", vpi_chk_error(NULL));
    if (type == vpiArrayVar) {
        vpi_printf(" size=%d arraytype=%d\n", vpi_get(vpiSize, h), vpi_get(vpiArrayType, h));
        elements(h, depth + 1);
        return;
    }
    if (type == vpiClassVar) {
        show_value(h);
        vpi_printf(" objid=%lld declared=%s", (long long)vpi_get64(vpiObjId, h),
                   str(vpiName, vpi_handle(vpiClassTypespec, h)));
        vpiHandle target = vpi_handle(vpiClassObj, h);
        if (!target) {
            vpi_printf(" obj=null\n");
            return;
        }
        vpi_printf("\n");
        if (depth < 2) object(target, depth + 1);
        return;
    }
    if (type == vpiStringVar || vpi_get(vpiSize, h) != vpiUndefined)
        vpi_printf(" size=%d", vpi_get(vpiSize, h));
    vpi_chk_error(NULL);
    show_value(h);
    vpi_printf("\n");
}

static void object(vpiHandle obj, int depth) {
    vpi_printf("%*s%s id=%lld created=%s alloc=%d automatic=%d\n", 2 * depth, "", str(vpiType, obj),
               (long long)vpi_get64(vpiObjId, obj), str(vpiName, vpi_handle(vpiClassTypespec, obj)),
               vpi_get(vpiAllocScheme, obj), vpi_get(vpiAutomatic, obj));
    vpiHandle it = vpi_iterate(vpiVariables, obj), p;
    while ((p = vpi_scan(it))) variable(p, depth + 1);
}

static void definition(const char* name, vpiHandle ts) {
    vpiHandle defn = vpi_handle(vpiClassDefn, ts);
    vpi_printf("%s %s classtype=%d automatic=%d defn=%s %s\n", name, str(vpiType, ts),
               vpi_get(vpiClassType, ts), vpi_get(vpiAutomatic, ts), str(vpiType, defn),
               str(vpiName, defn));
    vpiHandle extends = vpi_handle(vpiExtends, ts);
    if (extends)
        vpi_printf("  %s base=%s\n", str(vpiType, extends),
                   str(vpiName, vpi_handle(vpiClassTypespec, extends)));
    else
        vpi_printf("  extends=null level=%d\n", vpi_chk_error(NULL));
    vpiHandle it = vpi_iterate(vpiVariables, defn), p;
    while ((p = vpi_scan(it))) {
        vpi_printf("  declares %s %s", str(vpiType, p), str(vpiName, p));
        show_value(p);
        vpi_printf("\n");
    }
    it = vpi_iterate(vpiDerivedClasses, defn);
    while ((p = vpi_scan(it))) vpi_printf("  derived %s\n", str(vpiName, p));
}

static void put_int(const char* name, PLI_INT32 value) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = value;
    vpi_put_value(h, &v, NULL, vpiNoDelay);
    vpi_printf("put %s = %d level=%d\n", name, (int)value, vpi_chk_error(NULL));
}

static vpiHandle held, first;

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    static const char* const names[] = {"tb.s", "tb.ch", "tb.p", "tb.none", "tb.ring",
                                        "tb.da", "tb.q", "tb.aa", "tb.sa", NULL};
    for (int i = 0; names[i]; ++i) variable(vpi_handle_by_name((PLI_BYTE8*)names[i], NULL), 0);

    vpiHandle p = vpi_handle_by_name("tb.p", NULL);
    vpiHandle obj = vpi_handle(vpiClassObj, p);
    vpiHandle again = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.ring[0]", NULL));
    vpi_printf("same object=%d\n", vpi_compare_objects(obj, again));
    definition("LinkedPacket", vpi_handle(vpiClassTypespec, obj));
    definition("Packet", vpi_handle(vpiClassTypespec, p));

    s_vpi_value v;
    v.format = vpiHexStrVal;
    vpi_get_value(vpi_handle_by_name("tb.s", NULL), &v);
    vpi_printf("tb.s hex=%s\n", v.value.str);
    vpiHandle id = vpi_handle_by_name("tb.p.id", NULL);
    vpi_printf("by name %s %s", str(vpiType, id), str(vpiName, id));
    show_value(id);
    vpi_printf("\n");
    vpi_printf("missing tb.da[3]=%s tb.aa[6]=%s tb.p.nope=%s\n",
               vpi_handle_by_name("tb.da[3]", NULL) ? "set" : "null",
               vpi_handle_by_name("tb.aa[6]", NULL) ? "set" : "null",
               vpi_handle_by_name("tb.p.nope", NULL) ? "set" : "null");

    put_int("tb.p.id", 42);
    put_int("tb.q[0]", -5);
    put_int("tb.aa[5]", 70);
    put_int("tb.p.next.id", 44);
    vpiHandle element = vpi_handle_by_index(vpi_handle_by_name("tb.da", NULL), 2);
    v.format = vpiIntVal;
    v.value.integer = 33;
    vpi_put_value(element, &v, NULL, vpiNoDelay);
    vpi_printf("put tb.da[2] level=%d\n", vpi_chk_error(NULL));
    vpi_put_value(vpi_handle_by_name("tb.s", NULL), &v, NULL, vpiNoDelay);
    vpi_printf("put int to tb.s level=%d\n", vpi_chk_error(NULL));
    v.format = vpiStringVal;
    v.value.str = "world";
    vpi_put_value(vpi_handle_by_name("tb.s", NULL), &v, NULL, vpiNoDelay);
    vpi_printf("put tb.s level=%d\n", vpi_chk_error(NULL));
    vpi_put_value(vpi_handle(vpiClassObj, p), &v, NULL, vpiNoDelay);
    vpi_printf("put to class obj level=%d\n", vpi_chk_error(NULL));
    held = vpi_handle_by_name("tb.q[1]", NULL);
    first = vpi_handle_by_name("tb.q[0]", NULL);
    return 0;
}

static PLI_INT32 stale(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(held, &v);
    vpi_printf("tb.q[1] after delete level=%d\n", vpi_chk_error(NULL));
    vpi_get_value(first, &v);
    vpi_printf("tb.q[0]=%d size=%d\n", (int)v.value.integer,
               vpi_get(vpiSize, vpi_handle_by_name("tb.q", NULL)));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$walk";
    task.calltf = walk;
    vpi_register_systf(&task);
    task.tfname = "$stale";
    task.calltf = stale;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
