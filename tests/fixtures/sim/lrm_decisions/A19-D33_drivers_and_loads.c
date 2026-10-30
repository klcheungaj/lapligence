/* Decision A19-D33: drivers and loads of nets and variables (IEEE
 * 1800-2009 37.16, 37.21, 37.42). See A19-D33_drivers_and_loads.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

/* A description independent of handle identity: the object's kind and
 * where it sits. */
static void describe(vpiHandle h, char* out, size_t size) {
    int type = (int)vpi_get(vpiType, h);
    int line = (int)vpi_get(vpiLineNo, h);
    switch (type) {
        case vpiContAssign:
            snprintf(out, size, "assign@%d", line);
            return;
        case vpiContAssignBit:
            snprintf(out, size, "assign@%d.bit%d", line, (int)vpi_get(vpiOffset, h));
            return;
        case vpiForce:
            snprintf(out, size, "force@%d", line);
            return;
        case vpiPrimTerm: {
            vpiHandle prim = vpi_handle(vpiPrimitive, h);
            if (!prim) prim = vpi_handle(vpiParent, h);
            (void)level();
            snprintf(out, size, "term%d:%s", (int)vpi_get(vpiTermIndex, h),
                     prim ? vpi_get_str(vpiFullName, prim) : "?");
            return;
        }
        case vpiPort: {
            vpiHandle module = vpi_handle(vpiModule, h);
            snprintf(out, size, "port:%s.%s", module ? vpi_get_str(vpiFullName, module) : "?",
                     vpi_get_str(vpiName, h));
            return;
        }
        case vpiPortBit: {
            vpiHandle port = vpi_handle(vpiParent, h);
            char parent[128];
            int bit = -1, index = 0;
            describe(port, parent, sizeof parent);
            vpiHandle bits = vpi_iterate(vpiBit, port);
            for (vpiHandle other; bits && (other = vpi_scan(bits)); ++index)
                if (bit < 0 && vpi_compare_objects(other, h)) bit = index;
            snprintf(out, size, "%s.bit%d", parent, bit);
            return;
        }
        default: {
            const char* name = vpi_get_str(vpiFullName, h);
            snprintf(out, size, "type%d:%s", type, name ? name : "?");
            return;
        }
    }
}

static void relation(const char* name, vpiHandle h, const char* label, int type) {
    vpiHandle it = vpi_iterate(type, h);
    int error = level();
    char line[1024] = "";
    size_t used = 0;
    int count = 0;
    for (vpiHandle item; it && (item = vpi_scan(it)); ++count) {
        char text[192];
        describe(item, text, sizeof text);
        used += (size_t)snprintf(line + used, sizeof line - used, " %s", text);
        if (used >= sizeof line) used = sizeof line - 1;
    }
    vpi_printf("  %s %s level=%d n=%d:%s\n", name, label, error, count, line);
}

static void connectivity(const char* name, vpiHandle h, int local) {
    relation(name, h, "vpiDriver", vpiDriver);
    relation(name, h, "vpiLoad", vpiLoad);
    if (local) {
        relation(name, h, "vpiLocalDriver", vpiLocalDriver);
        relation(name, h, "vpiLocalLoad", vpiLocalLoad);
    }
    relation(name, h, "vpiPortInst", vpiPortInst);
    relation(name, h, "vpiPort", vpiPort);
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static PLI_INT32 d33(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value pass = {vpiIntVal, {0}};
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    if (arg) vpi_get_value(arg, &pass);
    if (args && arg) vpi_free_object(args);
    vpi_printf("pass %d\n", (int)pass.value.integer);
    static const char* const nets[] = {"tb.a", "tb.b", "tb.c", "tb.v", "tb.u1.ci", "tb.u1.co",
                                       "tb.u2.ci", "tb.u2.cv"};
    for (size_t i = 0; i < sizeof nets / sizeof nets[0]; ++i) {
        vpi_printf("%s\n", nets[i]);
        connectivity(nets[i], get(nets[i]), 1);
    }
    vpiHandle v = get("tb.v");
    vpiHandle bits = vpi_iterate(vpiBit, v);
    for (vpiHandle bit; bits && (bit = vpi_scan(bits));) {
        char name[64];
        s_vpi_value index = {vpiIntVal, {0}};
        vpiHandle ix = vpi_handle(vpiIndex, bit);
        if (ix) vpi_get_value(ix, &index);
        snprintf(name, sizeof name, "tb.v[%d]", (int)index.value.integer);
        vpi_printf("%s\n", name);
        connectivity(name, bit, 1);
    }
    static const char* const vars[] = {"tb.r", "tb.lv", "tb.u1.cr"};
    for (size_t i = 0; i < sizeof vars / sizeof vars[0]; ++i) {
        vpi_printf("%s\n", vars[i]);
        connectivity(vars[i], get(vars[i]), 0);
        vpiHandle h = get(vars[i]);
        relation(vars[i], h, "vpiLocalDriver", vpiLocalDriver);
        relation(vars[i], h, "vpiLocalLoad", vpiLocalLoad);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d33";
    task.calltf = d33;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
