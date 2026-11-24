/* VPI application of string_literals.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static void quoted(const char* text) {
    vpi_printf(" [");
    for (const char* at = text ? text : "(null)"; *at; ++at) {
        unsigned char ch = (unsigned char)*at;
        if (ch < 32 || ch > 126) vpi_printf("<%02x>", ch);
        else vpi_printf("%c", ch);
    }
    vpi_printf("]");
}

static void show(vpiHandle object) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiStringVal;
    vpi_get_value(object, &value);
    quoted(value.value.str);
    value.format = vpiHexStrVal;
    vpi_get_value(object, &value);
    vpi_printf(" %s size=%d\n", value.value.str, vpi_get(vpiSize, object));
}

static void statements(vpiHandle block) {
    vpiHandle iterator = vpi_iterate(vpiStmt, block), statement;
    while ((statement = vpi_scan(iterator))) {
        vpiHandle rhs = vpi_handle(vpiRhs, statement);
        vpi_printf("line %d", vpi_get(vpiLineNo, statement));
        quoted(vpi_get_str(vpiDecompile, rhs));
        if (vpi_get(vpiType, rhs) == vpiConstant) show(rhs);
        else vpi_printf("\n");
    }
}

static void parameters(vpiHandle scope) {
    vpiHandle iterator = vpi_iterate(vpiParameter, scope), parameter;
    while (iterator && (parameter = vpi_scan(iterator))) {
        vpi_printf("%s", vpi_get_str(vpiName, parameter));
        show(parameter);
    }
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL), iterator, object;
    parameters(tb);
    iterator = vpi_iterate(vpiClassDefn, tb);
    while (iterator && (object = vpi_scan(iterator))) parameters(object);
    iterator = vpi_iterate(vpiAttribute, vpi_handle_by_name("tb.marked", NULL));
    while (iterator && (object = vpi_scan(iterator))) {
        s_vpi_value value;
        memset(&value, 0, sizeof(value));
        value.format = vpiStringVal;
        vpi_get_value(object, &value);
        vpi_printf("attribute %s", vpi_get_str(vpiName, object));
        quoted(value.value.str);
        vpi_printf("\n");
    }
    iterator = vpi_iterate(vpiProcess, tb);
    object = vpi_scan(iterator);
    vpi_free_object(iterator);
    statements(vpi_handle(vpiStmt, object));
    return 0;
}

/* The vector format carries a NUL character both ways. */
static PLI_INT32 lit(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    vpiHandle object = vpi_scan(arguments);
    s_vpi_value value;
    s_vpi_vecval words[2];
    vpi_free_object(arguments);
    memset(&value, 0, sizeof(value));
    value.format = vpiVectorVal;
    vpi_get_value(object, &value);
    words[0] = value.value.vector[0];
    words[1] = value.value.vector[1];
    words[1].aval = words[0].aval;
    value.value.vector = words;
    vpi_put_value(object, &value, NULL, vpiNoDelay);
    vpi_printf("put");
    show(object);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    s_cb_data callback;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$lit";
    task.calltf = lit;
    vpi_register_systf(&task);
    memset(&callback, 0, sizeof(callback));
    callback.reason = cbStartOfSimulation;
    callback.cb_rtn = start;
    vpi_register_cb(&callback);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
