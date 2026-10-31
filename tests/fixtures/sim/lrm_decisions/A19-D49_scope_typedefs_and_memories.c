/* Decision A19-D49: typedefs and memories of a scope.
 * Companion of A19-D49_scope_typedefs_and_memories.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

/* The error level of the last call, clearing it. */
static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

/* The typedefs of `scope`: name, typespec type, line and alias. */
static void typedefs(const char* scope) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)scope, NULL);
    vpiHandle it = vpi_iterate(vpiTypedef, handle), typespec;
    int iterate_level = level();
    vpi_printf("%s typedefs level=%d:", scope, iterate_level);
    if (!it) vpi_printf(" NULL");
    while (it && (typespec = vpi_scan(it))) {
        char name[64], alias_name[64];
        const char* text = (const char*)vpi_get_str(vpiName, typespec);
        snprintf(name, sizeof name, "%s", text ? text : "-");
        vpiHandle alias = vpi_handle(vpiTypedefAlias, typespec);
        level();
        text = alias ? (const char*)vpi_get_str(vpiName, alias) : NULL;
        snprintf(alias_name, sizeof alias_name, "%s", text ? text : "-");
        vpi_printf(" [%s type=%d line=%d alias=%s:%d]", name, (int)vpi_get(vpiType, typespec),
                   (int)vpi_get(vpiLineNo, typespec), alias_name,
                   alias ? (int)vpi_get(vpiType, alias) : 0);
        level();
    }
    vpi_printf("\n");
}

/* The memories of `scope`: name, type and vpiIsMemory. */
static void memories(const char* scope) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)scope, NULL);
    vpiHandle it = vpi_iterate(vpiMemory, handle), memory;
    int iterate_level = level();
    vpi_printf("%s memories level=%d:", scope, iterate_level);
    if (!it) vpi_printf(" NULL");
    while (it && (memory = vpi_scan(it)))
        vpi_printf(" [%s type=%d mem=%d]", (const char*)vpi_get_str(vpiName, memory),
                   (int)vpi_get(vpiType, memory), (int)vpi_get(vpiIsMemory, memory));
    vpi_printf("\n");
}

static PLI_INT32 d49(PLI_BYTE8* data) {
    (void)data;
    typedefs("tb");
    typedefs("tb.u");
    typedefs("tb.blk");
    typedefs("pk");
    memories("tb");
    memories("tb.u");
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d49";
    task.calltf = d49;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
