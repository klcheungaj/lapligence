/* ADV-019 callback kinds (FIN-001 V3 family (e), IEEE 1800-2009 38.36.1):
 * $cbk(0) places cbValueChange records on the chandle variables of
 * callback_kinds.sv in several value formats and tries the callbacks llg
 * refuses (decision A19-D91); $cbk(1) reads h, writes it
 * through vpi_put_value and removes one record; $cbk(2) only marks the
 * end. Every callback prints its time, label, obj type, index and value. */
#include "sv_vpi_user.h"
#include <stdint.h>
#include <string.h>

/* The DPI import of callback_kinds.sv: a distinct non-null pointer per n. */
void* cbk_make(int n);
void* cbk_make(int n) { return (void*)(uintptr_t)(0x1000u + (unsigned)n); }

static PLI_UINT32 now(void) {
    s_vpi_time time;
    time.type = vpiSimTime;
    vpi_get_time(NULL, &time);
    return time.low;
}

static void print_value(p_vpi_value value) {
    if (!value) {
        vpi_printf(" value=NULL");
        return;
    }
    switch (value->format) {
        case vpiHexStrVal: vpi_printf(" hex=%s", value->value.str); break;
        case vpiDecStrVal: vpi_printf(" dec=%s", value->value.str); break;
        case vpiIntVal: vpi_printf(" int=%d", (int)value->value.integer); break;
        case vpiVectorVal:
            vpi_printf(" vector=%08x%08x/%08x%08x", (unsigned)value->value.vector[1].aval,
                       (unsigned)value->value.vector[0].aval, (unsigned)value->value.vector[1].bval,
                       (unsigned)value->value.vector[0].bval);
            break;
        default: vpi_printf(" format=%d", (int)value->format); break;
    }
}

static PLI_INT32 on_change(p_cb_data data) {
    vpi_printf("%u: %s obj=%s index=%d", now(), (const char*)data->user_data,
               vpi_get_str(vpiType, data->obj), (int)data->index);
    print_value(data->value);
    vpi_printf("\n");
    return 0;
}

static vpiHandle place(int reason, vpiHandle obj, PLI_INT32 format, const char* label) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = on_change;
    data.obj = obj;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)label;
    vpiHandle cb = vpi_register_cb(&data);
    s_vpi_error_info error;
    int level = vpi_chk_error(&error);
    vpi_printf("register %s: %s level=%d\n", label, cb ? "handle" : "NULL", level);
    return cb;
}

static vpiHandle named(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static vpiHandle dec_cb;

static PLI_INT32 cbk(PLI_BYTE8* user) {
    (void)user;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value step;
    step.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &step);
    vpi_free_object(args);
    vpiHandle h = named("tb.h");
    if (step.value.integer == 0) {
        place(cbValueChange, h, vpiObjTypeVal, "h objtype");
        place(cbValueChange, h, vpiHexStrVal, "h hex");
        place(cbValueChange, h, vpiIntVal, "h int");
        dec_cb = place(cbValueChange, h, vpiDecStrVal, "h dec");
        place(cbValueChange, h, vpiVectorVal, "h vector");
        place(cbValueChange, h, vpiSuppressVal, "h suppress");
        place(cbValueChange, named("tb.h2"), vpiHexStrVal, "h2 hex");
        /* Table 38-5: vpiStrengthVal is for scalar objects only. */
        place(cbValueChange, h, vpiStrengthVal, "h strength");
        /* Decision A19-D91: only vpiSuppressVal on a container is served. */
        place(cbSizeChange, named("tb.da"), vpiIntVal, "da size");
        place(cbSizeChange, named("tb.q"), vpiSuppressVal, "q size");
        place(cbSizeChange, named("tb.aa"), vpiIntVal, "aa size");
        place(cbSizeChange, named("tb.s"), vpiIntVal, "s size");
        /* 38.36.1 names no continuous assignment or primitive. */
        vpiHandle tb = named("tb");
        vpiHandle it = vpi_iterate(vpiContAssign, tb), object;
        while ((object = vpi_scan(it))) place(cbValueChange, object, vpiHexStrVal, "cont assign");
        it = vpi_iterate(vpiPrimitive, tb);
        while ((object = vpi_scan(it))) place(cbValueChange, object, vpiScalarVal, "gate");
    } else if (step.value.integer == 1) {
        s_vpi_value value;
        value.format = vpiHexStrVal;
        vpi_get_value(h, &value);
        vpi_printf("%u: get h hex=%s\n", now(), value.value.str);
        value.format = vpiIntVal;
        value.value.integer = 0x2222;
        vpi_put_value(h, &value, NULL, vpiNoDelay);
        vpi_printf("%u: put h returned\n", now());
        vpi_remove_cb(dec_cb);
    } else {
        vpi_printf("%u: end\n", now());
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$cbk";
    task.calltf = cbk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
