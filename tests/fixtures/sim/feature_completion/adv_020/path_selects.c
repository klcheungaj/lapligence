/* ADV-020: the terminal expressions of the paths of path_selects.sv and the
 * path terminals of its nets (IEEE 1800-2009 37.16, 37.35). */
#include "sv_vpi_user.h"
#include <string.h>

static void show(vpiHandle path, PLI_INT32 relation) {
    vpiHandle terms = vpi_iterate(relation, path), term;
    while (terms && (term = vpi_scan(terms))) {
        vpiHandle expr = vpi_handle(vpiExpr, term);
        vpi_printf(" %s", vpi_get_str(vpiType, expr));
    }
}

static PLI_INT32 selects_call(PLI_BYTE8* user) {
    (void)user;
    vpiHandle paths = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL)), path;
    while (paths && (path = vpi_scan(paths))) {
        show(path, vpiModPathIn);
        show(path, vpiModPathOut);
        vpi_printf("\n");
    }
    const char* nets[] = {"tb.u.a", "tb.u.b", "tb.u.y"};
    for (int i = 0; i < 3; ++i) {
        vpiHandle terms = vpi_iterate(vpiPathTerm, vpi_handle_by_name((PLI_BYTE8*)nets[i], NULL));
        int count = 0;
        while (terms && vpi_scan(terms)) ++count;
        vpi_printf("%s path terms %d\n", nets[i], count);
    }
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$selects";
    task.calltf = selects_call;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
