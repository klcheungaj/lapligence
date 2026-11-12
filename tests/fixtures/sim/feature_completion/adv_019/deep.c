/* ADV-019 deeper dynamic objects (see deep.sv): $deep(1) walks the module's
 * arrays and static class properties and writes some of them; $deep(2)
 * walks the automatics of task t. Each line prints what the reading
 * routines answer and whether they reported an error ("!"). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* text(const char* s) { return s ? s : "NULL"; }

/* vpi_get_str with the error status sampled before the next call. */
static const char* str(int property, vpiHandle x) {
    static char buffers[4][256];
    static int next;
    char* buffer = buffers[next++ & 3];
    const char* s = vpi_get_str(property, x);
    snprintf(buffer, sizeof buffers[0], "%s%s", text(s), failed() ? "!" : "");
    return buffer;
}

static int get(int property, vpiHandle x) {
    int v = (int)vpi_get(property, x);
    return failed() ? -99 : v;
}

static const char* type_name(int type) {
    switch (type) {
        case vpiIntVar: return "int";
        case vpiByteVar: return "byte";
        case vpiStringVar: return "string";
        case vpiClassVar: return "class";
        case vpiArrayVar: return "array";
        case vpiStructVar: return "struct";
        case vpiNamedEvent: return "event";
        default: return "other";
    }
}

static void print_value(vpiHandle x, int type) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    int error;
    if (type == vpiStringVar) {
        value.format = vpiStringVal;
        vpi_get_value(x, &value);
        error = failed();
        vpi_printf(" =\"%s\"", text(value.value.str));
    } else if (type == vpiClassVar) {
        vpiHandle object = vpi_handle(vpiClassObj, x);
        error = failed();
        vpi_printf(" =%s", object ? "object" : "null");
    } else if (type == vpiIntVar || type == vpiByteVar) {
        value.format = vpiIntVal;
        vpi_get_value(x, &value);
        error = failed();
        vpi_printf(" =%d", (int)value.value.integer);
    } else {
        return;
    }
    vpi_printf("%s", error ? "!" : "");
}

static void show(vpiHandle x, int depth);

static void list(vpiHandle x, int relation, const char* tag, int depth) {
    vpiHandle it = vpi_iterate(relation, x), y;
    int error = failed();
    if (!it) {
        vpi_printf("%*s%s: none%s\n", depth * 2, "", tag, error ? "!" : "");
        return;
    }
    while ((y = vpi_scan(it))) {
        vpi_printf("%*s%s ", depth * 2, "", tag);
        show(y, depth);
    }
}

static void show(vpiHandle x, int depth) {
    int type = get(vpiType, x);
    vpi_printf("%s %s %s", type_name(type), str(vpiName, x), str(vpiFullName, x));
    if (type != vpiClassVar && type != vpiNamedEvent) vpi_printf(" size=%d", get(vpiSize, x));
    vpi_printf(" auto=%d", get(vpiAutomatic, x));
    print_value(x, type);
    vpiHandle parent = vpi_handle(vpiParent, x);
    if (parent && (type == vpiStructVar || type == vpiIntVar || type == vpiStringVar ||
                   type == vpiByteVar || type == vpiClassVar))
        vpi_printf(" parent=%s", str(vpiName, parent));
    failed();
    vpi_printf("\n");
    if (type == vpiStructVar) list(x, vpiMember, "member", depth + 1);
}

static void indices(vpiHandle x) {
    const char* name = str(vpiName, x);
    vpiHandle it = vpi_iterate(vpiIndex, x), y;
    int error = failed();
    vpi_printf("  index of %s:", name);
    if (!it) vpi_printf(" none%s", error ? "!" : "");
    s_vpi_value value;
    while (it && (y = vpi_scan(it))) {
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        vpi_get_value(y, &value);
        vpi_printf(" %d", (int)value.value.integer);
    }
    vpiHandle one = vpi_handle(vpiIndex, x);
    error = failed();
    vpi_printf("; vpiIndex %s%s\n", one ? "handle" : "NULL", error ? "!" : "");
}

static vpiHandle by_name(const char* name) {
    vpiHandle x = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    int error = failed();
    vpi_printf("  [%s] ", name);
    if (x)
        show(x, 1);
    else
        vpi_printf("NULL%s\n", error ? "!" : "");
    return x;
}

static void by_index(vpiHandle array, int index) {
    vpiHandle x = vpi_handle_by_index(array, index);
    int error = failed();
    vpi_printf("  %s by index %d: ", str(vpiName, array), index);
    if (x)
        show(x, 1);
    else
        vpi_printf("NULL%s\n", error ? "!" : "");
}

static void by_indices(vpiHandle array, int count, int* list_of) {
    vpiHandle x = vpi_handle_by_multi_index(array, count, list_of);
    int error = failed();
    vpi_printf("  %s by indices", str(vpiName, array));
    for (int i = 0; i < count; ++i) vpi_printf(" %d", list_of[i]);
    vpi_printf(": ");
    if (x)
        show(x, 1);
    else
        vpi_printf("NULL%s\n", error ? "!" : "");
}

static void ranges(vpiHandle array) {
    vpiHandle it = vpi_iterate(vpiRange, array), y;
    int error = failed();
    vpi_printf("  ranges of %s:", str(vpiName, array));
    while (it && (y = vpi_scan(it))) {
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        vpiHandle left = vpi_handle(vpiLeftRange, y), right = vpi_handle(vpiRightRange, y);
        vpi_get_value(left, &value);
        int l = (int)value.value.integer;
        vpi_get_value(right, &value);
        vpi_printf(" [%d:%d]", l, (int)value.value.integer);
    }
    vpi_printf("%s\n", error ? "!" : "");
}

static void put_int(vpiHandle x, int v) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = v;
    vpi_put_value(x, &value, NULL, vpiNoDelay);
    int error = failed();
    vpi_printf("  put %s=%d%s\n", str(vpiFullName, x), v, error ? "!" : "");
}

static void put_string(vpiHandle x, const char* s) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)s;
    vpi_put_value(x, &value, NULL, vpiNoDelay);
    int error = failed();
    vpi_printf("  put %s=\"%s\"%s\n", str(vpiFullName, x), s, error ? "!" : "");
}

static vpiHandle member(vpiHandle record, const char* name) {
    vpiHandle it = vpi_iterate(vpiMember, record), y, found = NULL;
    while (it && (y = vpi_scan(it)))
        if (!found && !strcmp(text(vpi_get_str(vpiName, y)), name)) found = y;
    return found;
}

static vpiHandle find(vpiHandle frame, const char* name) {
    vpiHandle it = vpi_iterate(vpiAutomatics, frame), y, found = NULL;
    while (it && (y = vpi_scan(it)))
        if (!found && !strcmp(text(vpi_get_str(vpiName, y)), name)) found = y;
    return found;
}

static void module_objects(void) {
    vpi_printf("fixed arrays of more dimensions\n");
    vpiHandle s2 = by_name("tb.s2");
    ranges(s2);
    list(s2, vpiReg, "element", 1);
    int i12[2] = {1, 2};
    by_indices(s2, 2, i12);
    by_index(s2, 1);
    vpiHandle e = by_name("tb.s2[0][2]");
    indices(e);
    put_string(e, "w02");
    by_name("tb.s2[1]");
    vpiHandle h2 = by_name("tb.h2");
    list(h2, vpiReg, "element", 1);

    vpi_printf("nested containers\n");
    vpiHandle qq = by_name("tb.qq");
    list(qq, vpiReg, "leaf", 1);
    e = by_name("tb.qq[1][0]");
    indices(e);
    put_int(e, 30);
    by_name("tb.qq[0]");
    by_index(qq, 0);
    int i01[2] = {0, 1};
    by_indices(qq, 2, i01);
    vpiHandle aq = by_name("tb.aq");
    list(aq, vpiReg, "leaf", 1);
    put_int(by_name("tb.aq[5][1]"), 52);

    vpi_printf("arrays of records\n");
    vpiHandle rq = by_name("tb.rq");
    list(rq, vpiReg, "element", 1);
    e = vpi_handle_by_index(rq, 1);
    put_int(member(e, "a"), 33);
    vpiHandle in = member(vpi_handle_by_index(rq, 1), "in");
    put_int(member(in, "b"), 44);
    put_string(member(vpi_handle_by_index(rq, 0), "s"), "uno");
    by_name("tb.rf[1]");

    vpi_printf("static class properties\n");
    vpiHandle object = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.p", NULL));
    list(object, vpiVariables, "variable", 1);
    list(object, vpiNamedEvent, "event", 1);
    by_name("tb.p.si");
    vpiHandle sa = by_name("tb.C::sa");
    ranges(sa);
    put_int(by_name("tb.C::sa[0]"), 20);
    put_int(by_name("tb.C::si"), 40);
    put_string(by_name("tb.p.ss"), "ts");
    put_int(by_name("tb.D::ds"), 90);
    vpiHandle se = by_name("tb.C::se");
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiSuppressVal;
    vpi_put_value(se, &value, NULL, vpiNoDelay);
    int error = failed();
    vpi_printf("  trigger %s%s\n", str(vpiFullName, se), error ? "!" : "");
}

static vpiHandle element_at(vpiHandle array, int position) {
    vpiHandle it = vpi_iterate(vpiReg, array), y, found = NULL;
    for (int i = 0; it && (y = vpi_scan(it)); ++i)
        if (i == position) found = y;
    return found;
}

static void frame_objects(void) {
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    vpi_printf("automatics of %s\n", str(vpiFullName, frame));
    vpiHandle sm = find(frame, "sm");
    list(sm, vpiReg, "element", 1);
    int i10[2] = {1, 0};
    by_indices(sm, 2, i10);
    put_string(element_at(sm, 2), "w10");
    vpiHandle nq = find(frame, "nq");
    list(nq, vpiReg, "leaf", 1);
    indices(element_at(nq, 1));
    put_int(element_at(nq, 1), 60);
    vpiHandle rr = find(frame, "rr");
    list(rr, vpiReg, "element", 1);
    vpiHandle in = member(element_at(rr, 0), "in");
    put_int(member(in, "b"), 70);
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    vpi_free_object(args);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(arg, &value);
    if (value.value.integer == 1)
        module_objects();
    else
        frame_objects();
    return 0;
}

static void register_deep(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$deep";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_deep, 0};
