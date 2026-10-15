/* SIM-042 A01: VPI object model traversal, reads and selected writes.
 * Companion of objects.sv; registers the task $walk. Output is printed with
 * vpi_printf and compared with objects.out. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failures;

static void fail(const char* what) {
    ++failures;
    vpi_printf("FAIL %s\n", what);
}

static const char* text(PLI_BYTE8* value) { return value ? (const char*)value : "<null>"; }

/* vpi_get_str results live in a temporary buffer until the next call
 * (38.11); copy them so several fit in one vpi_printf. */
static const char* str(PLI_INT32 property, vpiHandle handle) {
    static char ring[12][256];
    static unsigned next;
    char* buffer = ring[next++ % 12];
    snprintf(buffer, sizeof(ring[0]), "%s", text(vpi_get_str(property, handle)));
    return buffer;
}

static vpiHandle named(const char* name) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    if (!handle) fail(name);
    return handle;
}

/* Values are copied into a ring of buffers, so several may appear in one
 * vpi_printf call: the simulator's own result buffer is reused per call. */
static const char* format_value(vpiHandle handle, PLI_INT32 format) {
    static char ring[12][256];
    static unsigned next;
    char* buffer = ring[next++ % 12];
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    vpi_get_value(handle, &value);
    if (vpi_chk_error(NULL)) return "<error>";
    switch (format) {
        case vpiIntVal:
            snprintf(buffer, sizeof(ring[0]), "%d", (int)value.value.integer);
            return buffer;
        case vpiRealVal:
            snprintf(buffer, sizeof(ring[0]), "%.3f", value.value.real);
            return buffer;
        case vpiScalarVal:
            snprintf(buffer, sizeof(ring[0]), "%c", "01zx"[value.value.scalar & 3]);
            return buffer;
        default:
            snprintf(buffer, sizeof(ring[0]), "%s", text(value.value.str));
            return buffer;
    }
}

/* name:type:size of every object an iteration returns. */
static void list(const char* label, PLI_INT32 type, vpiHandle reference) {
    vpi_printf("%s:", label);
    vpiHandle iterator = vpi_iterate(type, reference);
    if (!iterator) {
        vpi_printf(" (none)%s\n", vpi_chk_error(NULL) ? " error" : "");
        return;
    }
    vpiHandle item;
    while ((item = vpi_scan(iterator)) != NULL) {
        PLI_INT32 size = vpi_get(vpiSize, item);
        if (vpi_chk_error(NULL))
            vpi_printf(" %s:%s", str(vpiName, item), str(vpiType, item));
        else
            vpi_printf(" %s:%s:%d", str(vpiName, item),
                       str(vpiType, item), (int)size);
    }
    vpi_printf("\n");
}

static void bounds(const char* label, vpiHandle handle) {
    vpiHandle left = vpi_handle(vpiLeftRange, handle);
    vpiHandle right = vpi_handle(vpiRightRange, handle);
    if (!left || !right) {
        fail(label);
        return;
    }
    vpi_printf("%s: [%s:%s]\n", label, format_value(left, vpiDecStrVal),
               format_value(right, vpiDecStrVal));
    vpi_release_handle(left);
    vpi_release_handle(right);
}

static void put(vpiHandle handle, PLI_INT32 format, const char* text_value, PLI_INT32 integer,
                double real) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    if (format == vpiIntVal) value.value.integer = integer;
    else if (format == vpiScalarVal) value.value.scalar = integer;
    else if (format == vpiRealVal) value.value.real = real;
    else value.value.str = (PLI_BYTE8*)text_value;
    if (vpi_put_value(handle, &value, NULL, vpiNoDelay) != NULL || vpi_chk_error(NULL))
        fail("put");
}

static void hierarchy(void) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle scope = vpi_handle(vpiScope, call);
    vpi_printf("scope=%s type=%s\n", str(vpiFullName, scope),
               str(vpiType, scope));
    vpiHandle module = vpi_handle(vpiModule, call);
    vpi_printf("module=%s\n", str(vpiFullName, module));
    vpiHandle tops = vpi_iterate(vpiModule, NULL);
    vpiHandle top;
    while ((top = vpi_scan(tops)) != NULL)
        vpi_printf("top=%s def=%s top_module=%d\n", str(vpiName, top),
                   str(vpiDefName, top), (int)vpi_get(vpiTopModule, top));
    vpiHandle tb = named("tb");
    list("modules", vpiModule, tb);
    list("internal", vpiInternalScope, tb);
    list("variables", vpiVariables, tb);
    list("nets", vpiNet, tb);
    list("reals", vpiRealVar, tb);
    vpiHandle u1 = named("tb.u1");
    vpi_printf("u1 def=%s parent=%s top_module=%d\n", str(vpiDefName, u1),
               str(vpiName, vpi_handle(vpiParent, u1)), (int)vpi_get(vpiTopModule, u1));
    list("u1.parameters", vpiParameter, u1);
    vpiHandle parameters = vpi_iterate(vpiParameter, u1);
    vpiHandle parameter;
    while ((parameter = vpi_scan(parameters)) != NULL)
        vpi_printf("param %s local=%d bin=%s dec=%s\n", str(vpiName, parameter),
                   (int)vpi_get(vpiLocalParam, parameter), format_value(parameter, vpiBinStrVal),
                   format_value(parameter, vpiDecStrVal));
    vpiHandle ports = vpi_iterate(vpiPort, u1);
    vpiHandle port;
    while ((port = vpi_scan(ports)) != NULL) {
        vpiHandle low = vpi_handle(vpiLowConn, port);
        vpi_printf("port %s index=%d direction=%d size=%d low=%s\n",
                   str(vpiName, port), (int)vpi_get(vpiPortIndex, port),
                   (int)vpi_get(vpiDirection, port), (int)vpi_get(vpiSize, port),
                   low ? str(vpiFullName, low) : "<none>");
    }
    vpiHandle gen1 = named("tb.gen[1]");
    vpi_printf("gen1 type=%s name=%s full=%s\n", str(vpiType, gen1),
               str(vpiName, gen1), str(vpiFullName, gen1));
    vpiHandle gv = vpi_handle_by_name((PLI_BYTE8*)"gv", gen1);
    vpi_printf("gen1.gv full=%s bin=%s\n", gv ? str(vpiFullName, gv) : "<none>",
               gv ? format_value(gv, vpiBinStrVal) : "-");
    vpiHandle local = vpi_handle_by_name((PLI_BYTE8*)"local_v", scope);
    vpi_printf("main.local_v full=%s bin=%s\n",
               local ? str(vpiFullName, local) : "<none>",
               local ? format_value(local, vpiBinStrVal) : "-");
    /* Relative lookup is confined to the scope (38.21). */
    vpi_printf("relative miss=%d\n", vpi_handle_by_name((PLI_BYTE8*)"v", u1) == NULL &&
                                         vpi_chk_error(NULL) == vpiError);
}

static void types_and_values(void) {
    static const char* const names[] = {"tb.v", "tb.up", "tb.w", "tb.r", "tb.i", "tb.bv",
                                        "tb.b", "tb.ii", "tb.t"};
    for (size_t k = 0; k < sizeof(names) / sizeof(names[0]); ++k) {
        vpiHandle handle = named(names[k]);
        PLI_INT32 size = vpi_get(vpiSize, handle);
        int sized = !vpi_chk_error(NULL);
        s_vpi_value natural;
        memset(&natural, 0, sizeof(natural));
        natural.format = vpiObjTypeVal;
        vpi_get_value(handle, &natural);
        vpi_printf("%s type=%s size=%d signed=%d natural=%d", names[k],
                   str(vpiType, handle), sized ? (int)size : -1,
                   (int)vpi_get(vpiSigned, handle), (int)natural.format);
        if (vpi_get(vpiType, handle) == vpiRealVar) {
            vpi_printf(" real=%s int=%s\n", format_value(handle, vpiRealVal),
                       format_value(handle, vpiIntVal));
            continue;
        }
        vpi_printf(" bin=%s hex=%s dec=%s int=%s\n", format_value(handle, vpiBinStrVal),
                   format_value(handle, vpiHexStrVal), format_value(handle, vpiDecStrVal),
                   format_value(handle, vpiIntVal));
    }
    vpiHandle t = named("tb.t");
    s_vpi_value time_value;
    memset(&time_value, 0, sizeof(time_value));
    time_value.format = vpiTimeVal;
    vpi_get_value(t, &time_value);
    vpi_printf("t time=%u:%u\n", (unsigned)time_value.value.time->high,
               (unsigned)time_value.value.time->low);
    bounds("v", named("tb.v"));
    bounds("up", named("tb.up"));
    bounds("bv", named("tb.bv"));
}

static void arrays(void) {
    vpiHandle mem = named("tb.mem");
    vpi_printf("mem type=%s size=%d memory=%d array=%d\n", str(vpiType, mem),
               (int)vpi_get(vpiSize, mem), (int)vpi_get(vpiIsMemory, mem),
               (int)vpi_get(vpiArray, mem));
    bounds("mem", mem);
    vpiHandle element = vpi_handle_by_index(mem, 2);
    vpiHandle index = vpi_handle(vpiIndex, element);
    vpi_printf("mem[2] type=%s member=%d name=%s full=%s index=%s size=%d bin=%s hex=%s dec=%s\n",
               str(vpiType, element), (int)vpi_get(vpiArrayMember, element),
               str(vpiName, element), str(vpiFullName, element),
               format_value(index, vpiDecStrVal), (int)vpi_get(vpiSize, element),
               format_value(element, vpiBinStrVal), format_value(element, vpiHexStrVal),
               format_value(element, vpiDecStrVal));
    vpi_printf("mem[2] parent=%s\n", str(vpiName, vpi_handle(vpiParent, element)));
    vpi_release_handle(index);
    vpi_release_handle(element);
    vpi_printf("mem[4]=%s\n", vpi_handle_by_index(mem, 4) == NULL &&
                                      vpi_chk_error(NULL) == vpiError ? "null" : "handle");
    vpi_printf("elements:");
    vpiHandle words = vpi_iterate(vpiReg, mem);
    vpiHandle word;
    while ((word = vpi_scan(words)) != NULL) {
        vpi_printf(" %s=%s", str(vpiName, word), format_value(word, vpiHexStrVal));
        vpi_release_handle(word);
    }
    vpi_printf("\n");
    vpiHandle grid = named("tb.grid");
    vpi_printf("grid size=%d memory=%d\n", (int)vpi_get(vpiSize, grid),
               (int)vpi_get(vpiIsMemory, grid));
    vpi_printf("ranges:");
    vpiHandle ranges = vpi_iterate(vpiRange, grid);
    vpiHandle range;
    while ((range = vpi_scan(ranges)) != NULL) {
        vpiHandle left = vpi_handle(vpiLeftRange, range);
        vpiHandle right = vpi_handle(vpiRightRange, range);
        vpi_printf(" [%s:%s]", format_value(left, vpiDecStrVal), format_value(right, vpiDecStrVal));
        vpi_release_handle(range);
    }
    vpi_printf("\n");
    PLI_INT32 indices[2] = {0, 1};
    vpiHandle cell = vpi_handle_by_multi_index(grid, 2, indices);
    vpi_printf("grid[0][1] name=%s type=%s dec=%s int=%s\n", str(vpiName, cell),
               str(vpiType, cell), format_value(cell, vpiDecStrVal),
               format_value(cell, vpiIntVal));
    vpiHandle by_name = named("tb.grid[1][2]");
    vpi_printf("tb.grid[1][2]=%s same=%d\n", format_value(by_name, vpiDecStrVal),
               (int)vpi_compare_objects(by_name, cell));
}

static void selects(void) {
    vpiHandle part = named("tb.v[5:2]");
    vpi_printf("v[5:2] type=%s size=%d bin=%s\n", str(vpiType, part),
               (int)vpi_get(vpiSize, part), format_value(part, vpiBinStrVal));
    bounds("v[5:2]", part);
    vpiHandle bit0 = vpi_handle_by_index(named("tb.v"), 0);
    vpi_printf("v[0] type=%s name=%s scalar=%s\n", str(vpiType, bit0),
               str(vpiName, bit0), format_value(bit0, vpiScalarVal));
    vpiHandle up2 = vpi_handle_by_index(named("tb.up"), 2);
    vpi_printf("up[2] scalar=%s\n", format_value(up2, vpiScalarVal));
    vpi_printf("v bits:");
    vpiHandle bits = vpi_iterate(vpiBit, named("tb.v"));
    vpiHandle bit;
    while ((bit = vpi_scan(bits)) != NULL) {
        vpi_printf(" %s", format_value(bit, vpiScalarVal));
        vpi_release_handle(bit);
    }
    vpi_printf("\n");
    vpiHandle wbit = vpi_handle_by_index(named("tb.w"), 1);
    vpi_printf("w[1] type=%s\n", str(vpiType, wbit));
    vpiHandle mem_bit = named("tb.mem[1][5]");
    vpi_printf("mem[1][5] type=%s scalar=%s\n", str(vpiType, mem_bit),
               format_value(mem_bit, vpiScalarVal));
}

static void writes(void) {
    vpiHandle mem = named("tb.mem");
    vpiHandle element = vpi_handle_by_index(mem, 3);
    put(element, vpiHexStrVal, "a5", 0, 0.0);
    put(named("tb.v[5:2]"), vpiBinStrVal, "1010", 0, 0.0);
    put(vpi_handle_by_index(named("tb.up"), 3), vpiScalarVal, NULL, vpi1, 0.0);
    PLI_INT32 indices[2] = {1, 0};
    put(vpi_handle_by_multi_index(named("tb.grid"), 2, indices), vpiIntVal, NULL, 77, 0.0);
    put(named("tb.r"), vpiRealVal, NULL, 0, 3.5);
    put(named("tb.i"), vpiDecStrVal, "-123", 0, 0.0);
    /* A two-state variable stores X/Z bits as 0 (6.11.1 conversion). */
    put(named("tb.bv"), vpiBinStrVal, "0x01", 0, 0.0);
    s_vpi_vecval word = {2u, 0u};
    s_vpi_value vector;
    memset(&vector, 0, sizeof(vector));
    vector.format = vpiVectorVal;
    vector.value.vector = &word;
    if (vpi_put_value(named("tb.main.local_v"), &vector, NULL, vpiNoDelay) || vpi_chk_error(NULL))
        fail("vector put");
    put(named("tb.gen[1].gv"), vpiIntVal, NULL, 2, 0.0);
    vpi_printf("after: v=%s mem[3]=%s up=%s bv=%s\n", format_value(named("tb.v"), vpiHexStrVal),
               format_value(element, vpiHexStrVal), format_value(named("tb.up"), vpiBinStrVal),
               format_value(named("tb.bv"), vpiBinStrVal));
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    hierarchy();
    types_and_values();
    arrays();
    selects();
    writes();
    vpi_printf("failures=%d\n", failures);
    return 0;
}

static void register_walk(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$walk";
    task.calltf = walk;
    if (!vpi_register_systf(&task)) vpi_printf("registration failed\n");
}

void (*vlog_startup_routines[])(void) = {register_walk, NULL};
