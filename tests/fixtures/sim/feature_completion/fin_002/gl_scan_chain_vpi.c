/* FIN-002 VPI application of gl_scan_chain.sv: $inspect lists the
 * primitives, module paths and timing checks of the module instance its
 * string literal argument names. */
#include "vpi_user.h"
#include <string.h>

static int count(vpiHandle scope, PLI_INT32 type)
{
    vpiHandle iterator = vpi_iterate(type, scope);
    int number = 0;
    if (!iterator) return 0;
    while (vpi_scan(iterator)) number++;
    return number;
}

static void primitives(vpiHandle scope)
{
    vpiHandle iterator = vpi_iterate(vpiPrimitive, scope);
    vpiHandle primitive;
    if (!iterator) return;
    while ((primitive = vpi_scan(iterator))) {
        PLI_INT32 type = vpi_get(vpiType, primitive);
        vpiHandle terminals = vpi_iterate(vpiPrimTerm, primitive);
        vpiHandle terminal;
        vpi_printf("  primitive %s: %s inputs=%d terminals",
                   vpi_get_str(vpiName, primitive),
                   type == vpiUdp ? vpi_get_str(vpiDefName, primitive)
                                  : type == vpiGate ? "gate" : "switch",
                   (int)vpi_get(vpiSize, primitive));
        while (terminals && (terminal = vpi_scan(terminals))) {
            s_vpi_value value;
            value.format = vpiBinStrVal;
            vpi_get_value(vpi_handle(vpiExpr, terminal), &value);
            vpi_printf(" %s%s", vpi_get(vpiDirection, terminal) == vpiOutput ? "out=" : "",
                       value.value.str);
        }
        vpi_printf("\n");
    }
}

static void delays(vpiHandle scope, PLI_INT32 type, const char* label, int limits)
{
    vpiHandle iterator = vpi_iterate(type, scope);
    vpiHandle object;
    if (!iterator) return;
    while ((object = vpi_scan(iterator))) {
        s_vpi_time times[2];
        s_vpi_delay delay;
        memset(&delay, 0, sizeof(delay));
        delay.da = times;
        delay.no_of_delays = limits;
        delay.time_type = vpiScaledRealTime;
        vpi_get_delays(object, &delay);
        if (limits == 2)
            vpi_printf("  %s: %.2f %.2f\n", label, times[0].real, times[1].real);
        else vpi_printf("  %s: %.2f\n", label, times[0].real);
    }
}

static PLI_INT32 inspect(PLI_BYTE8* user)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle path = arguments ? vpi_scan(arguments) : NULL;
    vpiHandle scope = NULL;
    s_vpi_value value;
    (void)user;
    if (path) {
        value.format = vpiStringVal;
        vpi_get_value(path, &value);
        scope = vpi_handle_by_name(value.value.str, NULL);
        vpi_free_object(arguments);
    }
    if (!scope) {
        vpi_printf("vpi: scope missing\n");
        return 0;
    }
    vpi_printf("vpi %s: cell %s ports=%d primitives=%d paths=%d checks=%d\n",
               vpi_get_str(vpiFullName, scope), vpi_get_str(vpiDefName, scope),
               count(scope, vpiPort), count(scope, vpiPrimitive), count(scope, vpiModPath),
               count(scope, vpiTchk));
    primitives(scope);
    delays(scope, vpiModPath, "path", 2);
    delays(scope, vpiTchk, "check limit", 1);
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$inspect";
    data.calltf = inspect;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
