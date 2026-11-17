/* ADV-019 members of structure values and fixed-size class properties (see
 * element_members.sv): $emembers walks the members of structure elements
 * and the properties of a class object, finds elements by name, index and
 * multi-index, writes some of them and asks for the member selects llg
 * refuses. "!" marks an answer that raised an error. */
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
    if (type == vpiArrayVar || type == vpiStructVar) return "-";
    value.format = type == vpiStringVar ? vpiStringVal : vpiHexStrVal;
    vpi_get_value(x, &value);
    snprintf(buffer, sizeof buffer, "%s%s", value.value.str ? value.value.str : "NULL",
             failed() ? "!" : "");
    return buffer;
}

static void show(int depth, const char* label, vpiHandle x);

static void ranges(int depth, vpiHandle x) {
    vpiHandle it = vpi_iterate(vpiRange, x);
    vpiHandle range;
    vpi_printf("%*sranges:", depth * 2, "");
    if (!it) vpi_printf(" none%s", failed() ? "!" : "");
    while (it && (range = vpi_scan(it))) {
        s_vpi_value l, r;
        memset(&l, 0, sizeof l);
        memset(&r, 0, sizeof r);
        l.format = r.format = vpiIntVal;
        vpi_get_value(vpi_handle(vpiLeftRange, range), &l);
        vpi_get_value(vpi_handle(vpiRightRange, range), &r);
        vpi_printf(" [%d:%d]", (int)l.value.integer, (int)r.value.integer);
    }
    vpi_printf("\n");
}

static void children(int depth, int relation, const char* tag, vpiHandle x) {
    vpiHandle it = vpi_iterate(relation, x);
    vpiHandle child;
    if (!it) {
        vpi_printf("%*s%s: none%s\n", depth * 2, "", tag, failed() ? "!" : "");
        return;
    }
    while ((child = vpi_scan(it))) show(depth, tag, child);
}

static void show(int depth, const char* label, vpiHandle x) {
    if (!x) {
        vpi_printf("%*s%s: NULL%s\n", depth * 2, "", label, failed() ? "!" : "");
        return;
    }
    int type = get(vpiType, x);
    vpiHandle parent = vpi_handle(vpiParent, x);
    vpi_printf("%*s%s: type=%d %s %s size=%d member=%d element=%d value=%s parent=%s",
               depth * 2, "", label, type, str(vpiName, x), str(vpiFullName, x),
               get(vpiSize, x), get(vpiStructUnionMember, x), get(vpiArrayMember, x),
               value_of(x), parent ? str(vpiName, parent) : "NULL");
    if (type == vpiArrayVar) vpi_printf(" array_type=%d", get(vpiArrayType, x));
    vpi_printf("\n");
    if (depth > 3) return;
    if (type == vpiStructVar) children(depth + 1, vpiMember, "member", x);
    if (type == vpiArrayVar) {
        ranges(depth + 1, x);
        children(depth + 1, vpiReg, "element", x);
    }
}

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static void indices(const char* name) {
    vpiHandle x = by_name(name);
    vpiHandle it = x ? vpi_iterate(vpiIndex, x) : NULL;
    vpiHandle index;
    vpi_printf("%s indices:", name);
    if (!it) vpi_printf(" none%s", failed() ? "!" : "");
    while (it && (index = vpi_scan(it))) {
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        vpi_get_value(index, &value);
        vpi_printf(" %d", (int)value.value.integer);
    }
    vpiHandle one = x ? vpi_handle(vpiIndex, x) : NULL;
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (one) vpi_get_value(one, &value);
    vpi_printf(" handle=%s\n", one ? (value.value.integer == 1 ? "1" : "other") : failed() ? "NULL!" : "NULL");
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

static void step1(void) {
    show(0, "tb.ea[1]", by_name("tb.ea[1]"));
    show(0, "tb.eq[0].fa", by_name("tb.eq[0].fa"));
    vpiHandle c = by_name("tb.c");
    vpiHandle object = c ? vpi_handle(vpiClassObj, c) : NULL;
    if (object) children(0, vpiVariables, "property", object);
    show(0, "tb.ea[1].fa[1]", by_name("tb.ea[1].fa[1]"));
    show(0, "tb.ea[1].q[0]", by_name("tb.ea[1].q[0]"));
    show(0, "tb.ea[1].m[1][0]", by_name("tb.ea[1].m[1][0]"));
    show(0, "tb.ea[1].fa[2]", by_name("tb.ea[1].fa[2]"));
    show(0, "tb.ea[1].m[1]", by_name("tb.ea[1].m[1]"));
    show(0, "tb.c.fa[1]", by_name("tb.c.fa[1]"));
    show(0, "tb.c.la[1][0]", by_name("tb.c.la[1][0]"));
    vpiHandle fa = by_name("tb.ea[1].fa");
    show(0, "index fa 0", fa ? vpi_handle_by_index(fa, 0) : NULL);
    int pair[2] = {0, 2};
    show(0, "multi m [0][2]", vpi_handle_by_multi_index(by_name("tb.ea[1].m"), 2, pair));
    int pair2[2] = {1, 0};
    show(0, "multi la [1][0]", vpi_handle_by_multi_index(by_name("tb.c.la"), 2, pair2));
    indices("tb.ea[1].fa[1]");
    indices("tb.ea[1].m[0][2]");
    put_int("tb.ea[1].fa[0]", 200);
    put_int("tb.ea[1].q[0]", 220);
    put_text("tb.ea[1].ss[1]", "y");
    put_int("tb.ea[1].m[1][0]", 7);
    put_int("tb.eq[0].fa[1]", 211);
    put_int("tb.eq[0].q[0]", 222);
    put_int("tb.c.fa[0]", 100);
    put_text("tb.c.sa[1]", "zz");
    put_int("tb.c.la[0][1]", 3);
    show(0, "tb.c.pr.fa[1]", by_name("tb.c.pr.fa[1]"));
    put_int("tb.c.pr.k", 9);
    put_int("tb.c.pr.fa[0]", 8);
    show(0, "tb.ea[1].k[1]", by_name("tb.ea[1].k[1]"));
    show(0, "tb.c.pe.k", by_name("tb.c.pe.k"));
}

static PLI_INT32 calltf(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("== emembers\n");
    step1();
    return 0;
}

static void register_emembers(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$emembers";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_emembers, 0};
