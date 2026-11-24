/* ADV-020: the edges and delays of the paths of path_edges.sv (IEEE 1800-2009
 * 30.4.3, 37.35). */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 edges_call(PLI_BYTE8* user) {
    (void)user;
    vpiHandle args = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    s_vpi_value value;
    value.format = vpiStringVal;
    vpi_get_value(vpi_scan(args), &value);
    char name[64];
    strncpy(name, value.value.str, sizeof(name) - 1);
    name[sizeof(name) - 1] = 0;
    vpi_release_handle(args);
    vpiHandle paths = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)name, NULL)), path;
    while (paths && (path = vpi_scan(paths))) {
        const PLI_INT32 relations[2] = {vpiModPathIn, vpiModPathOut};
        vpi_printf("%s", vpi_get_str(vpiFullName, path));
        for (int i = 0; i < 2; ++i) {
            vpiHandle terms = vpi_iterate(relations[i], path), term;
            while (terms && (term = vpi_scan(terms)))
                vpi_printf(" edge=%d", (int)vpi_get(vpiEdge, term));
        }
        s_vpi_time da[2];
        s_vpi_delay delay;
        memset(da, 0, sizeof(da));
        memset(&delay, 0, sizeof(delay));
        delay.da = da;
        delay.no_of_delays = 2;
        delay.time_type = vpiScaledRealTime;
        vpi_get_delays(path, &delay);
        vpi_printf(" delays=%g,%g\n", da[0].real, da[1].real);
    }
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$edges";
    task.calltf = edges_call;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
