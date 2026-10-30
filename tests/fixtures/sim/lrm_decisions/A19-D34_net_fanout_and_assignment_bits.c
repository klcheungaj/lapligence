/* Decision A19-D34: terminals, assignments and paths of nets and
 * variables, continuous assignment bits and vpiSimNet (IEEE 1800-2009
 * 37.16, 37.43). See A19-D34_net_fanout_and_assignment_bits.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static void describe(vpiHandle h, char* out, size_t size) {
    int type = (int)vpi_get(vpiType, h);
    int line = (int)vpi_get(vpiLineNo, h);
    const char* name = NULL;
    if (type == vpiContAssignBit) {
        snprintf(out, size, "type%d@%d.bit%d", type, line, (int)vpi_get(vpiOffset, h));
        return;
    }
    if (type == vpiPrimTerm) {
        snprintf(out, size, "type%d@%d.term%d", type, line, (int)vpi_get(vpiTermIndex, h));
        return;
    }
    if (type != vpiContAssign && type != vpiPathTerm && type != vpiTchkTerm)
        name = vpi_get_str(vpiFullName, h);
    (void)level();
    if (name) snprintf(out, size, "type%d:%s", type, name);
    else snprintf(out, size, "type%d@%d", type, line);
}

static void iteration(const char* name, vpiHandle h, const char* label, int type) {
    vpiHandle it = vpi_iterate(type, h);
    int error = level();
    vpi_printf("  %s %s level=%d:", name, label, error);
    for (vpiHandle item; it && (item = vpi_scan(it));) {
        char text[160];
        describe(item, text, sizeof text);
        vpi_printf(" %s", text);
    }
    vpi_printf("\n");
}

static void relation(const char* name, vpiHandle h, const char* label, int type) {
    vpiHandle r = vpi_handle(type, h);
    int error = level();
    char text[160] = "NULL";
    if (r) describe(r, text, sizeof text);
    vpi_printf("  %s %s %s level=%d\n", name, label, text, error);
}

static void property(const char* name, vpiHandle h, const char* label, int property) {
    int value = (int)vpi_get(property, h);
    int error = level();
    vpi_printf("  %s %s %d level=%d\n", name, label, value, error);
}

static void fanout(const char* name, vpiHandle h) {
    vpi_printf("%s\n", name);
    iteration(name, h, "vpiPrimTerm", vpiPrimTerm);
    iteration(name, h, "vpiContAssign", vpiContAssign);
    iteration(name, h, "vpiPathTerm", vpiPathTerm);
    iteration(name, h, "vpiTchkTerm", vpiTchkTerm);
    relation(name, h, "vpiSimNet", vpiSimNet);
}

static PLI_INT32 d34(PLI_BYTE8* data) {
    (void)data;
    static const char* const names[] = {"tb.x", "tb.y", "tb.w", "tb.q", "tb.r", "tb.rv",
                                        "tb.l1.i", "tb.l1.o"};
    for (size_t i = 0; i < sizeof names / sizeof names[0]; ++i) fanout(names[i], get(names[i]));
    static const char* const vectors[] = {"tb.w", "tb.q", "tb.rv"};
    for (size_t i = 0; i < sizeof vectors / sizeof vectors[0]; ++i) {
        vpiHandle bits = vpi_iterate(vpiBit, get(vectors[i]));
        for (vpiHandle bit; bits && (bit = vpi_scan(bits));) {
            s_vpi_value index = {vpiIntVal, {0}};
            vpiHandle ix = vpi_handle(vpiIndex, bit);
            if (ix) vpi_get_value(ix, &index);
            char name[64];
            snprintf(name, sizeof name, "%s[%d]", vectors[i], (int)index.value.integer);
            fanout(name, bit);
        }
    }
    vpiHandle assigns = vpi_iterate(vpiContAssign, get("tb"));
    for (vpiHandle assign; assigns && (assign = vpi_scan(assigns));) {
        char name[64];
        snprintf(name, sizeof name, "assign@%d", (int)vpi_get(vpiLineNo, assign));
        vpi_printf("%s\n", name);
        vpiHandle bits = vpi_iterate(vpiBit, assign);
        vpi_printf("  %s vpiBit level=%d\n", name, level());
        for (vpiHandle bit; bits && (bit = vpi_scan(bits));) {
            char label[80];
            snprintf(label, sizeof label, "%s.bit%d", name, (int)vpi_get(vpiOffset, bit));
            relation(label, bit, "vpiParent", vpiParent);
            relation(label, bit, "vpiLhs", vpiLhs);
            relation(label, bit, "vpiRhs", vpiRhs);
            property(label, bit, "vpiLineNo", vpiLineNo);
            property(label, bit, "vpiNetDeclAssign", vpiNetDeclAssign);
            property(label, bit, "vpiStrength0", vpiStrength0);
            property(label, bit, "vpiStrength1", vpiStrength1);
            property(label, bit, "vpiSize", vpiSize);
            s_vpi_time times[2];
            s_vpi_delay delay;
            memset(times, 0, sizeof times);
            memset(&delay, 0, sizeof delay);
            delay.da = times;
            delay.no_of_delays = 2;
            delay.time_type = vpiSimTime;
            vpi_get_delays(bit, &delay);
            vpi_printf("  %s delays %u %u level=%d\n", label, (unsigned)times[0].low,
                       (unsigned)times[1].low, level());
            s_vpi_value value = {vpiBinStrVal, {0}};
            vpi_get_value(bit, &value);
            vpi_printf("  %s value %s level=%d\n", label, value.value.str ? value.value.str : "NULL",
                       level());
        }
    }
    vpiHandle terms = vpi_iterate(vpiPrimTerm, get("tb.l1.b0"));
    for (vpiHandle term; terms && (term = vpi_scan(terms));)
        relation("tb.l1.b0 term", term, "vpiPrimitive", vpiPrimitive);
    vpiHandle paths = vpi_iterate(vpiModPath, get("tb.l1"));
    for (vpiHandle path; paths && (path = vpi_scan(paths));) {
        vpiHandle ins = vpi_iterate(vpiModPathIn, path);
        for (vpiHandle term; ins && (term = vpi_scan(ins));)
            relation("path in", term, "vpiModPath", vpiModPath);
    }
    vpiHandle checks = vpi_iterate(vpiTchk, get("tb.l1"));
    for (vpiHandle check; checks && (check = vpi_scan(checks));) {
        vpiHandle term = vpi_handle(vpiTchkRefTerm, check);
        if (term) relation("tchk ref", term, "vpiTchk", vpiTchk);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d34";
    task.calltf = d34;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
