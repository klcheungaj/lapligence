/* ADV-019 family 2: an independent walker of the structure objects of
 * structure.sv (IEEE 1800-2009 37.5-37.15, 37.25, 37.31, 37.37, 37.43,
 * 37.58). From the top modules it prints every instance with its ports,
 * named events, processes, continuous assignments, tasks and functions,
 * primitives, instance arrays and modports, then checks that each relation
 * leads back to its origin (vpi_compare_objects) and that exhausted
 * iterators are freed. $fire triggers the named event through
 * vpi_put_value; a cbValueChange on it reports every trigger. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failures;

static const char* text(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void check(int ok, const char* what) {
    if (!ok) {
        ++failures;
        vpi_printf("  CHECK FAILED: %s\n", what);
    }
}

/* Type, name and (when readable) binary value of a connection. */
static void describe(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf(" %s=NULL", label);
        return;
    }
    vpi_printf(" %s=%s:%s", label, text(vpiType, h), text(vpiFullName, h));
    s_vpi_value value;
    value.format = vpiBinStrVal;
    vpi_get_value(h, &value);
    if (vpi_chk_error(NULL) == 0) vpi_printf("=%s", value.value.str);
}

static void value_of(vpiHandle h) {
    s_vpi_value value;
    value.format = vpiBinStrVal;
    vpi_get_value(h, &value);
    if (vpi_chk_error(NULL) == 0) vpi_printf(" value=%s", value.value.str);
    else vpi_printf(" value=error");
}

/* vpi_iterate(type, ref) must be freed once vpi_scan returns NULL. */
static void exhausted(vpiHandle iterator) {
    vpi_get(vpiType, iterator);
    check(vpi_chk_error(NULL) != 0, "exhausted iterator is released");
}

static void ports(vpiHandle module) {
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    if (!it) return;
    while ((port = vpi_scan(it))) {
        PLI_INT32 port_type = vpi_get(vpiPortType, port);
        vpi_printf("  port %s index=%d dir=%d type=%s", text(vpiName, port),
                   vpi_get(vpiPortIndex, port), vpi_get(vpiDirection, port),
                   port_type == vpiPort ? "port" : port_type == vpiInterfacePort ? "interface" : "modport");
        if (port_type != vpiPort) {
            vpiHandle low = vpi_handle(vpiLowConn, port), high = vpi_handle(vpiHighConn, port);
            vpi_printf(" low=%s:%s def=%s", text(vpiType, low), text(vpiName, low), text(vpiDefName, low));
            vpi_printf(" high=%s:%s", text(vpiType, high), text(vpiName, high));
            vpiHandle actual = vpi_handle(vpiActual, low);
            vpi_printf(" actual=%s:%s\n", text(vpiType, actual), text(vpiFullName, actual));
            check(vpi_compare_objects(actual, vpi_handle(vpiActual, high)), "ref objects share the actual");
            continue;
        }
        vpi_printf(" size=%d scalar=%d", vpi_get(vpiSize, port), vpi_get(vpiScalar, port));
        describe("low", vpi_handle(vpiLowConn, port));
        describe("high", vpi_handle(vpiHighConn, port));
        vpi_printf("\n");
        vpiHandle bits = vpi_iterate(vpiBit, port), bit;
        int count = 0;
        while ((bit = vpi_scan(bits))) {
            if (count == 0) {
                vpi_printf("    bit0 %s dir=%d size=%d", text(vpiType, bit), vpi_get(vpiDirection, bit),
                           vpi_get(vpiSize, bit));
                describe("low", vpi_handle(vpiLowConn, bit));
                describe("high", vpi_handle(vpiHighConn, bit));
                vpi_printf("\n");
                check(vpi_compare_objects(vpi_handle(vpiParent, bit), port), "port bit parent");
            }
            ++count;
        }
        vpi_printf("    bits=%d\n", count);
    }
    exhausted(it);
}

static void processes(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiProcess, scope), process;
    while (it && (process = vpi_scan(it))) {
        PLI_INT32 type = vpi_get(vpiType, process);
        vpi_printf("  process %s line=%d", text(vpiType, process), vpi_get(vpiLineNo, process));
        if (type == vpiAlways) vpi_printf(" always_type=%d", vpi_get(vpiAlwaysType, process));
        vpi_printf("\n");
        check(vpi_compare_objects(vpi_handle(vpiModule, process), scope), "process module");
    }
}

static void assigns(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiContAssign, scope), assign;
    while (it && (assign = vpi_scan(it))) {
        vpi_printf("  assign line=%d size=%d net_decl=%d", vpi_get(vpiLineNo, assign),
                   vpi_get(vpiSize, assign), vpi_get(vpiNetDeclAssign, assign));
        describe("lhs", vpi_handle(vpiLhs, assign));
        describe("rhs", vpi_handle(vpiRhs, assign));
        value_of(assign);
        vpi_printf("\n");
        check(vpi_compare_objects(vpi_handle(vpiModule, assign), scope), "assign module");
    }
}

static void taskfuncs(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiTaskFunc, scope), tf;
    while (it && (tf = vpi_scan(it))) {
        vpi_printf("  %s %s auto=%d", text(vpiType, tf), text(vpiFullName, tf), vpi_get(vpiAutomatic, tf));
        if (vpi_get(vpiType, tf) == vpiFunction)
            vpi_printf(" func_type=%d size=%d signed=%d", vpi_get(vpiFuncType, tf), vpi_get(vpiSize, tf),
                       vpi_get(vpiSigned, tf));
        vpi_printf("\n");
        check(vpi_compare_objects(vpi_handle(vpiScope, tf), scope), "task/function scope");
        check(vpi_compare_objects(vpi_handle_by_name((PLI_BYTE8*)text(vpiFullName, tf), NULL), tf),
              "task/function by name");
    }
}

static void events(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiNamedEvent, scope), event;
    while (it && (event = vpi_scan(it))) {
        vpi_printf("  event %s array_member=%d", text(vpiFullName, event), vpi_get(vpiArrayMember, event));
        value_of(event);
        vpi_printf("\n");
    }
}

static void primitive(vpiHandle prim) {
    vpi_printf("  %s %s def=%s prim_type=%d size=%d", text(vpiType, prim), text(vpiFullName, prim),
               text(vpiDefName, prim), vpi_get(vpiPrimType, prim), vpi_get(vpiSize, prim));
    value_of(prim);
    vpiHandle index = vpi_handle(vpiIndex, prim);
    if (index) {
        s_vpi_value value;
        value.format = vpiIntVal;
        vpi_get_value(index, &value);
        vpi_printf(" index=%d", value.value.integer);
    }
    vpi_printf("\n");
    vpiHandle terms = vpi_iterate(vpiPrimTerm, prim), term;
    while ((term = vpi_scan(terms))) {
        vpi_printf("    term %d dir=%d", vpi_get(vpiTermIndex, term), vpi_get(vpiDirection, term));
        describe("expr", vpi_handle(vpiExpr, term));
        value_of(term);
        vpi_printf("\n");
        check(vpi_compare_objects(vpi_handle(vpiParent, term), prim), "terminal parent");
    }
}

static void primitives(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiPrimitive, scope), prim;
    while (it && (prim = vpi_scan(it))) {
        primitive(prim);
        if (vpi_get_str(vpiName, prim))
            check(vpi_compare_objects(vpi_handle_by_name((PLI_BYTE8*)text(vpiFullName, prim), NULL), prim),
                  "primitive by name");
    }
}

static void arrays(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiInstanceArray, scope), array;
    while (it && (array = vpi_scan(it))) {
        vpi_printf("  %s %s size=%d", text(vpiType, array), text(vpiFullName, array), vpi_get(vpiSize, array));
        vpiHandle ranges = vpi_iterate(vpiRange, array), range;
        while ((range = vpi_scan(ranges))) {
            s_vpi_value l, r;
            l.format = r.format = vpiIntVal;
            vpi_get_value(vpi_handle(vpiLeftRange, range), &l);
            vpi_get_value(vpi_handle(vpiRightRange, range), &r);
            vpi_printf(" [%d:%d]", l.value.integer, r.value.integer);
        }
        vpi_printf(" elements");
        vpiHandle elements = vpi_iterate(vpiInstance, array), element;
        while ((element = vpi_scan(elements))) {
            s_vpi_value value;
            value.format = vpiIntVal;
            vpi_get_value(vpi_handle(vpiIndex, element), &value);
            vpi_printf(" %s(%d,member=%d)", text(vpiName, element), value.value.integer,
                       vpi_get(vpiArrayMember, element));
            check(vpi_compare_objects(vpi_handle_by_index(array, value.value.integer), element),
                  "element by index");
        }
        vpi_printf("\n");
    }
}

static void walk(vpiHandle scope, int depth) {
    vpi_printf("%s %s def=%s top=%d\n", text(vpiType, scope), text(vpiFullName, scope),
               text(vpiDefName, scope), vpi_get(vpiType, scope) == vpiModule ? vpi_get(vpiTopModule, scope) : 0);
    if (vpi_get(vpiType, scope) == vpiInterface) {
        vpiHandle it = vpi_iterate(vpiModport, scope), modport;
        while (it && (modport = vpi_scan(it))) {
            vpi_printf("  modport %s\n", text(vpiFullName, modport));
            check(vpi_compare_objects(vpi_handle(vpiParent, modport), scope), "modport parent");
        }
    }
    ports(scope);
    events(scope);
    processes(scope);
    assigns(scope);
    taskfuncs(scope);
    primitives(scope);
    arrays(scope);
    static const PLI_INT32 kinds[] = {vpiInterface, vpiModule};
    for (int k = 0; k < 2; ++k) {
        vpiHandle it = vpi_iterate(kinds[k], scope), child;
        while (it && (child = vpi_scan(it))) {
            check(vpi_compare_objects(vpi_handle(vpiParent, child), scope), "instance parent");
            walk(child, depth + 1);
        }
    }
}

static PLI_INT32 on_event(p_cb_data data) {
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("cb %s at %u value=%s\n", text(vpiFullName, data->obj), now.low,
               data->value ? "set" : "NULL");
    return 0;
}

static PLI_INT32 structure(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tops = vpi_iterate(vpiModule, NULL), top;
    while ((top = vpi_scan(tops))) walk(top, 0);
    vpiHandle ev = vpi_handle_by_name((PLI_BYTE8*)"tb.ev", NULL);
    s_cb_data cb;
    s_vpi_time time;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSimTime;
    cb.reason = cbValueChange;
    cb.cb_rtn = on_event;
    cb.obj = ev;
    cb.time = &time;
    vpi_register_cb(&cb);
    vpi_printf("failures=%d\n", failures);
    return 0;
}

static PLI_INT32 fire(PLI_BYTE8* data) {
    (void)data;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.ev", NULL), NULL, NULL, vpiNoDelay);
    vpi_printf("fire level=%d\n", vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$structure";
    task.calltf = structure;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$fire";
    task.calltf = fire;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
