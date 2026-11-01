/* ADV-020: vpiStrengthVal values and strength-change callbacks of nets
 * (IEEE 1800-2009 38.15, 38.36.1). Companion of strengths.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* logic_name(PLI_INT32 logic) {
    switch (logic) {
        case vpi0: return "0";
        case vpi1: return "1";
        case vpiX: return "x";
        case vpiZ: return "z";
        default: return "?";
    }
}

static void print_bits(const s_vpi_strengthval* bits, int width) {
    for (int i = width; i-- > 0;)
        vpi_printf(" [%d]%s s0=%02x s1=%02x", i, logic_name(bits[i].logic), (unsigned)bits[i].s0,
                   (unsigned)bits[i].s1);
}

static PLI_INT32 st_read(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.w1", "tb.w2", "tb.w3", "tb.w4",  "tb.w5",  "tb.w6", "tb.w7",
                                  "tb.w8", "tb.w9", "tb.w10", "tb.w11", "tb.w12", "tb.a", "tb.v",
                                  "tb.v[1]"};
    for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); ++i) {
        vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        s_vpi_value value;
        value.format = vpiStrengthVal;
        vpi_get_value(handle, &value);
        PLI_INT32 level = vpi_chk_error(NULL);
        vpi_printf("%s level=%d", names[i], (int)level);
        if (!level) print_bits(value.value.strength, (int)vpi_get(vpiSize, handle));
        vpi_printf("\n");
    }
    return 0;
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("  %s at %d:", (const char*)data->user_data, (int)data->time->low);
    if (data->value->format == vpiStrengthVal)
        print_bits(data->value->value.strength, (int)vpi_get(vpiSize, data->obj));
    else
        vpi_printf(" %s", data->value->value.str);
    vpi_printf("\n");
    return 0;
}

static vpiHandle watch(const char* name, PLI_INT32 format, const char* label) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value values[8];
    static int used;
    s_vpi_value* value = &values[used++ % 8];
    value->format = format;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    cb.time = &time;
    cb.value = value;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("watch %s: %s level=%d\n", label, handle ? "handle" : "NULL",
               (int)vpi_chk_error(NULL));
    return handle;
}

static PLI_INT32 st_watch(PLI_BYTE8* data) {
    (void)data;
    watch("tb.w", vpiStrengthVal, "w strength");
    watch("tb.w", vpiBinStrVal, "w value");
    watch("tb.v", vpiStrengthVal, "v strength");
    watch("tb.v[1]", vpiStrengthVal, "v[1] strength");
    watch("tb.v[0]", vpiStrengthVal, "v[0] strength");
    vpiHandle removed = watch("tb.w", vpiStrengthVal, "removed");
    vpi_printf("remove: %d\n", (int)vpi_remove_cb(removed));
    return 0;
}

static void registration(void) {
    static const char* names[] = {"$st_read", "$st_watch"};
    PLI_INT32 (*routines[])(PLI_BYTE8*) = {st_read, st_watch};
    for (int i = 0; i < 2; ++i) {
        s_vpi_systf_data task;
        memset(&task, 0, sizeof(task));
        task.type = vpiSysTask;
        task.tfname = (PLI_BYTE8*)names[i];
        task.calltf = routines[i];
        vpi_register_systf(&task);
    }
}

void (*vlog_startup_routines[])(void) = {registration, 0};
