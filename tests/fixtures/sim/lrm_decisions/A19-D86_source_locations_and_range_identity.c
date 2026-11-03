/* Decision A19-D86: source locations of class properties, container
 * elements, ranges, range bounds and indices, and the identity of ranges
 * and of the constants made per request (37.3.3, 38.3). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* base(const char* file) {
    const char* slash = file ? strrchr(file, '/') : NULL;
    return slash ? slash + 1 : file;
}

/* `file` 0 prints the line only: some simulators answer vpiFile of a
 * range with the range's name. */
static void located(const char* tag, vpiHandle h, int file_too) {
    if (!h) {
        vpi_printf("%s: none\n", tag);
        return;
    }
    PLI_INT32 line = vpi_get(vpiLineNo, h);
    if (vpi_chk_error(NULL)) {
        vpi_printf("%s: no line\n", tag);
        return;
    }
    if (!file_too) {
        vpi_printf("%s: line %d\n", tag, (int)line);
        return;
    }
    const char* file = vpi_get_str(vpiFile, h);
    if (vpi_chk_error(NULL) || !file) file = "(none)";
    vpi_printf("%s: line %d %s\n", tag, (int)line, base(file));
}

static void where(const char* tag, vpiHandle h) { located(tag, h, 1); }

static vpiHandle first(PLI_INT32 relation, vpiHandle h) {
    vpiHandle it = h ? vpi_iterate(relation, h) : NULL;
    vpi_chk_error(NULL);
    vpiHandle item = it ? vpi_scan(it) : NULL;
    if (item) vpi_release_handle(it);
    return item;
}

static vpiHandle one(PLI_INT32 relation, vpiHandle h) {
    vpiHandle item = h ? vpi_handle(relation, h) : NULL;
    vpi_chk_error(NULL);
    return item;
}

static vpiHandle named(PLI_INT32 relation, vpiHandle h, const char* name) {
    vpiHandle it = h ? vpi_iterate(relation, h) : NULL, item;
    vpi_chk_error(NULL);
    while (it && (item = vpi_scan(it))) {
        const char* text = vpi_get_str(vpiName, item);
        if (text && !strcmp(text, name)) {
            vpi_release_handle(it);
            return item;
        }
    }
    return NULL;
}

static vpiHandle by(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_chk_error(NULL);
    return h;
}

static void same(const char* tag, vpiHandle x, vpiHandle y) {
    vpi_printf("%s: %d\n", tag, x && y ? (int)vpi_compare_objects(x, y) : -1);
}

static void properties(const char* who, vpiHandle h) {
    char tag[64];
    snprintf(tag, sizeof tag, "%s id", who);
    where(tag, named(vpiVariables, h, "id"));
    snprintf(tag, sizeof tag, "%s done", who);
    where(tag, first(vpiNamedEvent, h));
    snprintf(tag, sizeof tag, "%s steps", who);
    where(tag, first(vpiNamedEventArray, h));
    snprintf(tag, sizeof tag, "%s vif", who);
    where(tag, first(vpiInterfaceDecl, h));
}

static PLI_INT32 d86(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = by("tb");
    properties("defn", first(vpiClassDefn, tb));
    vpiHandle object = one(vpiClassObj, by("tb.p"));
    where("object", object);
    properties("object", object);
    vpiHandle da = by("tb.da"), mem = by("tb.mem"), a = by("tb.a"), b = by("tb.b");
    vpiHandle element = vpi_handle_by_index(da, 1);
    where("da[1]", element);
    where("da[1] index", first(vpiIndex, element));
    vpiHandle word = vpi_handle_by_index(mem, 1);
    where("mem[1]", word);
    where("mem[1] index", first(vpiIndex, word));
    located("mem range", first(vpiRange, mem), 0);
    located("a range", first(vpiRange, a), 0);
    where("a left", one(vpiLeftRange, a));
    where("a range left", one(vpiLeftRange, first(vpiRange, a)));
    located("su.st range", first(vpiRange, by("tb.su.st")), 0);
    where("su.st left", one(vpiLeftRange, by("tb.su.st")));
    where("su.smem[1] index", first(vpiIndex, vpi_handle_by_index(by("tb.su.smem"), 1)));
    where("su.blk.bv left", one(vpiLeftRange, by("tb.su.blk.bv")));
    where("a typespec", one(vpiTypespec, a));
    same("a range twice", first(vpiRange, a), first(vpiRange, a));
    same("mem range twice", first(vpiRange, mem), first(vpiRange, mem));
    same("a range vs b range", first(vpiRange, a), first(vpiRange, b));
    same("a left twice", one(vpiLeftRange, a), one(vpiLeftRange, a));
    same("a left vs range left", one(vpiLeftRange, a), one(vpiLeftRange, first(vpiRange, a)));
    same("a left vs b left", one(vpiLeftRange, a), one(vpiLeftRange, b));
    same("a left vs a right", one(vpiLeftRange, a), one(vpiRightRange, a));
    same("mem[1] index of two words", first(vpiIndex, word),
         first(vpiIndex, vpi_handle_by_index(mem, 1)));
    same("mem[1] index vs mem[2] index", first(vpiIndex, word),
         first(vpiIndex, vpi_handle_by_index(mem, 2)));
    same("da[1] index twice", first(vpiIndex, element),
         first(vpiIndex, vpi_handle_by_index(da, 1)));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d86";
    task.calltf = d86;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
