/* Companion of A20-D16_delay_expressions.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static vpiHandle by_name(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static vpiHandle nth(PLI_INT32 type, vpiHandle scope, int index) {
    vpiHandle it = vpi_iterate(type, scope), h = NULL;
    for (int i = 0; it && i <= index; ++i) h = vpi_scan(it);
    if (h && it) vpi_release_handle(it);
    return h;
}

/* One delay expression: its type, and an operation's vpiOpType and
 * operands, an expression's integer value. */
static void expression(vpiHandle expr) {
    PLI_INT32 type = vpi_get(vpiType, expr);
    if (type == vpiOperation) {
        vpi_printf(" op %d (", (int)vpi_get(vpiOpType, expr));
        vpiHandle operands = vpi_iterate(vpiOperand, expr), operand;
        while (operands && (operand = vpi_scan(operands)) != NULL) expression(operand);
        vpi_printf(" )");
        return;
    }
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(expr, &value);
    vpi_printf(" %s=%d", vpi_get_str(vpiType, expr), (int)value.value.integer);
}

static void delay(const char* label, vpiHandle object) {
    vpiHandle expr = vpi_handle(vpiDelay, object);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s:", label);
    if (!expr) vpi_printf(" NULL level=%d", (int)level);
    else expression(expr);
    vpi_printf("\n");
}

static void get(const char* label, vpiHandle h) {
    s_vpi_time da[3];
    s_vpi_delay d;
    memset(da, 0, sizeof(da));
    memset(&d, 0, sizeof(d));
    d.da = da;
    d.no_of_delays = 3;
    d.time_type = vpiScaledRealTime;
    vpi_get_delays(h, &d);
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) vpi_printf("%s: error level=%d\n", label, (int)level);
    else vpi_printf("%s: %g %g %g\n", label, da[0].real, da[1].real, da[2].real);
}

static PLI_INT32 d16(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = by_name("tb"), u = by_name("tb.u");
    delay("undelayed buf", by_name("tb.b0"));
    delay("buf #3", by_name("tb.b1"));
    delay("bufif1 #(1, 2, 3)", by_name("tb.b2"));
    delay("buf #(1:2:3)", by_name("tb.b3"));
    delay("buf #(d)", by_name("tb.b4"));
    delay("gate array #2", nth(vpiPrimitiveArray, tb, 0));
    delay("assign #(1 + 1)", nth(vpiContAssign, tb, 0));
    delay("assign #(1, 2)", nth(vpiContAssign, tb, 1));
    delay("net #5", by_name("tb.nd"));
    delay("undelayed net", by_name("tb.nn"));
    delay("path", nth(vpiModPath, u, 0));
    delay("$width", nth(vpiTchk, u, 0));
    delay("$setuphold", nth(vpiTchk, u, 1));
    delay("port", nth(vpiPort, u, 0));
    vpiHandle initial = nth(vpiProcess, tb, 0);
    vpiHandle control = nth(vpiStmt, vpi_handle(vpiStmt, initial), 0);
    delay("delay control #4", control);
    get("buf #(1:2:3) delays", by_name("tb.b3"));
    get("buf #(d) delays", by_name("tb.b4"));
    get("net #5 delays", by_name("tb.nd"));
    get("undelayed net delays", by_name("tb.nn"));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d16";
    task.calltf = d16;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
