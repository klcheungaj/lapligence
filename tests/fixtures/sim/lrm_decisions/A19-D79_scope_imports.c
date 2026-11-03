/* Companion of A19-D79_scope_imports.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_ITEMS 32

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static int by_text(const void* a, const void* b) {
    return strcmp((const char*)a, (const char*)b);
}

/* One line per imported item: the name of its scope (its package), its
 * name and its type, sorted, since the iteration order is unspecified. */
static void imports(const char* path) {
    vpiHandle scope = vpi_handle_by_name((PLI_BYTE8*)path, NULL), it, item, owner, one;
    char lines[MAX_ITEMS][96];
    int count = 0, i, error;
    const char* name;
    memset(lines, 0, sizeof lines);
    if (!scope) {
        vpi_printf("%s: no scope\n", path);
        return;
    }
    it = vpi_iterate(vpiImport, scope);
    error = level() != 0;
    vpi_printf("%s imports=%s error=%d\n", path, it ? "list" : "NULL", error);
    while (it && (item = vpi_scan(it)) != NULL) {
        char package[48] = {0};
        owner = vpi_handle(vpiScope, item);
        name = owner ? vpi_get_str(vpiName, owner) : NULL;
        snprintf(package, sizeof package, "%s", name ? name : "NULL");
        name = vpi_get_str(vpiName, item);
        if (count < MAX_ITEMS)
            snprintf(lines[count++], sizeof lines[0], "%s %s type=%d", package, name ? name : "NULL",
                     (int)vpi_get(vpiType, item));
    }
    qsort(lines, (size_t)count, sizeof lines[0], by_text);
    for (i = 0; i < count; ++i) vpi_printf("  %s\n", lines[i]);
    one = vpi_handle(vpiImport, scope);
    error = level() != 0;
    vpi_printf("  handle=%s error=%d\n", one ? "obj" : "NULL", error);
}

static PLI_INT32 d79(PLI_BYTE8* user) {
    (void)user;
    imports("tb");
    imports("tb.u");
    imports("tb.blk");
    imports("tb.f2");
    imports("pk");
    imports("pk3");
    return 0;
}

static void register_d79(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d79";
    task.calltf = d79;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d79, NULL};
