/* Companion of S43-D11_delayed_bits_one_change.sv: a VPI application. Each
 * value-change callback after time 0 records its time, object and what it
 * reads; the records print sorted at the end of the simulation, because the
 * callbacks of different nets in one slot run in any order. */
#include "vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    unsigned time;
    char text[64];
} record_t;

static record_t records[64];
static int n_records;

static void read_binary(const char* name, char* out, size_t size) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiBinStrVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &value);
    snprintf(out, size, "%s", value.value.str ? value.value.str : "?");
}

static PLI_INT32 changed(p_cb_data data) {
    s_vpi_time now;
    char q[16];
    char r[16];
    memset(&now, 0, sizeof(now));
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    if (now.low == 0 || n_records == 64) return 0;
    const char* label = (const char*)data->user_data;
    read_binary("tb.q", q, sizeof(q));
    read_binary("tb.r", r, sizeof(r));
    record_t* record = &records[n_records++];
    record->time = (unsigned)now.low;
    /* A callback of q reads q (and r only for r's own callback). */
    snprintf(record->text, sizeof(record->text), "%s reads %s", label,
             strcmp(label, "r") == 0 ? r : q);
    return 0;
}

static void watch(vpiHandle object, const char* label) {
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSuppressTime;
    value.format = vpiSuppressVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = object;
    cb.time = &time;
    cb.value = &value;
    cb.user_data = (PLI_BYTE8*)label;
    vpi_register_cb(&cb);
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    vpiHandle q = vpi_handle_by_name((PLI_BYTE8*)"tb.q", NULL);
    watch(q, "q");
    watch(vpi_handle_by_index(q, 5), "q[5]");
    watch(vpi_handle_by_name((PLI_BYTE8*)"tb.g[3].s.q", NULL), "g[3].s.q");
    watch(vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL), "r");
    return 0;
}

static int by_time_and_text(const void* left, const void* right) {
    const record_t* a = (const record_t*)left;
    const record_t* b = (const record_t*)right;
    if (a->time != b->time) return a->time < b->time ? -1 : 1;
    return strcmp(a->text, b->text);
}

static PLI_INT32 end(p_cb_data data) {
    (void)data;
    qsort(records, (size_t)n_records, sizeof(records[0]), by_time_and_text);
    for (int i = 0; i < n_records; i++)
        vpi_printf("%u callback %s\n", records[i].time, records[i].text);
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    vpi_register_cb(&cb);
    cb.reason = cbEndOfSimulation;
    cb.cb_rtn = end;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
