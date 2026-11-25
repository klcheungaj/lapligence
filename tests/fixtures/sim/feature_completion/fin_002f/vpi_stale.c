/* Handles of objects that end: taken, used after the end, used again when the
 * design has a new object of that name, and a pending event handle kept past
 * the end of the run. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "vpi_user.h"
#include "sv_vpi_user.h"

#define COUNT(array) ((int32_t)(sizeof array / sizeof array[0]))

static const char* const names[] = {"tb.v", "tb.dyn[1]", "tb.dyn[3]", "tb.que[2]", "tb.words[\"k\"]", "tb.n.x", "tb.n.next.x"};
static vpiHandle taken[COUNT(names)], frame_handle, local_handle, thread_handle, pending_handle, released;

static void value(const char* label, vpiHandle handle) {
    s_vpi_value v;
    v.format = vpiDecStrVal;
    vpi_chk_error(NULL);
    vpi_get_value(handle, &v);
    if (vpi_chk_error(NULL))
        vpi_printf("  %s: error\n", label);
    else
        vpi_printf("  %s: %s\n", label, v.value.str);
}

static void relations(vpiHandle handle) {
    /* Every one-to-one and one-to-many relation and property of a handle
     * whose object may be gone: no more than an error. */
    for (PLI_INT32 type = 1; type < 760; type++) {
        vpiHandle other = vpi_handle(type, handle);
        if (other) vpi_release_handle(other);
        vpiHandle iterator = vpi_iterate(type, handle);
        if (iterator) vpi_release_handle(iterator);
        vpi_get(type, handle);
        vpi_get_str(type, handle);
    }
    vpi_chk_error(NULL);
}

static void write(vpiHandle handle) {
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = 1;
    vpi_chk_error(NULL);
    vpi_put_value(handle, &v, NULL, vpiNoDelay);
}

static void use_all(void) {
    for (int32_t i = 0; i < COUNT(names); i++) {
        value(names[i], taken[i]);
        relations(taken[i]);
    }
}

static vpiHandle kept[1500];

/* Runs inside vpi_put_value, which goes on with the handle it was given. */
static PLI_INT32 take_many(p_cb_data data) {
    (void)data;
    for (int32_t i = 0; i < COUNT(kept) / 2; i++) kept[i] = vpi_handle_by_name("tb.dyn[0]", NULL);
    return 0;
}

static void grow_in_a_write(void) {
    s_cb_data data;
    memset(&data, 0, sizeof data);
    data.reason = cbValueChange;
    data.cb_rtn = take_many;
    data.obj = vpi_handle_by_name("tb.dyn[1]", NULL);
    vpiHandle callback = vpi_register_cb(&data);
    write(data.obj);
    vpi_printf("  written %s\n", vpi_chk_error(NULL) ? "error" : "ok");
    vpi_remove_cb(callback);
    for (int32_t i = 0; i < COUNT(kept) / 2; i++) vpi_release_handle(kept[i]);
}

/* Handles are added one by one past every size of the table that holds them,
 * with one use of each kind after every addition. */
static void grow(void) {
    vpiHandle record = vpi_handle_by_name("tb.recs[0]", NULL);
    vpiHandle array = vpi_handle_by_name("tb.dyn", NULL);
    vpiHandle element = vpi_handle_by_name("tb.dyn[1]", NULL);
    int32_t members = 0;
    for (int32_t i = 0; i < COUNT(kept); i++) {
        vpiHandle iterators[] = {vpi_iterate(vpiMember, record), vpi_iterate(vpiIndex, element),
                                 vpi_iterate(vpiIntVar, array)};
        for (int32_t k = 0; k < COUNT(iterators); k++) {
            vpiHandle member;
            while (iterators[k] && (member = vpi_scan(iterators[k]))) {
                members++;
                vpi_release_handle(member);
            }
        }
        write(element);
        kept[i] = vpi_handle_by_name("tb.dyn[0]", NULL);
    }
    vpi_printf("  members %d\n", members);
    for (int32_t i = 0; i < COUNT(kept); i++) vpi_release_handle(kept[i]);
}

static PLI_INT32 step(PLI_BYTE8* user) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value v;
    char name[32];
    (void)user;
    v.format = vpiStringVal;
    vpi_get_value(vpi_scan(arguments), &v);
    snprintf(name, sizeof name, "%s", v.value.str);
    vpi_release_handle(arguments);
    vpi_printf("%s\n", name);
    vpi_chk_error(NULL);
    if (!strcmp(name, "take")) {
        for (int32_t i = 0; i < COUNT(names); i++) {
            taken[i] = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
            value(names[i], taken[i]);
        }
        released = vpi_handle_by_name("tb.v", NULL);
        vpi_release_handle(released);
        vpi_release_handle(released);
        relations(released);
        value("null", NULL);
    } else if (!strcmp(name, "gone") || !strcmp(name, "back")) {
        use_all();
        for (int32_t i = 1; i < COUNT(names); i++)
            if (!strcmp(name, "gone")) write(taken[i]);
        vpi_chk_error(NULL);
    } else if (!strcmp(name, "frame")) {
        frame_handle = vpi_handle(vpiScope, call);
        local_handle = vpi_handle_by_name("tb.framed.a_long_block_name.deep", NULL);
        value("local", local_handle);
        /* A name shorter than the block's. */
        vpiHandle scope = vpi_handle_by_name("tb.framed", NULL);
        vpi_printf("  short name %s\n", vpi_handle_by_name("a", scope) ? "found" : "none");
        vpi_chk_error(NULL);
    } else if (!strcmp(name, "left")) {
        value("local", local_handle);
        relations(local_handle);
        relations(frame_handle);
    } else if (!strcmp(name, "thread")) {
        vpiHandle threads = vpi_iterate(vpiThread, NULL);
        if (threads) {
            thread_handle = vpi_scan(threads);
            vpi_release_handle(threads);
        }
        vpi_chk_error(NULL);
    } else if (!strcmp(name, "ended")) {
        relations(thread_handle);
    } else if (!strcmp(name, "grow")) {
        grow_in_a_write();
        grow();
    } else if (!strcmp(name, "pending")) {
        s_vpi_time t;
        memset(&t, 0, sizeof t);
        t.type = vpiSimTime;
        t.low = 1000;
        v.format = vpiIntVal;
        v.value.integer = 3;
        pending_handle = vpi_put_value(vpi_handle_by_name("tb.delayed", NULL), &v, &t,
                                       vpiTransportDelay | vpiReturnEvent);
        vpi_printf("  event %d scheduled %d\n", pending_handle != NULL,
                   pending_handle ? (int)vpi_get(vpiScheduled, pending_handle) : 0);
    } else if (!strcmp(name, "final")) {
        use_all();
        relations(pending_handle);
    }
    return 0;
}

static PLI_INT32 ended(p_cb_data data) {
    (void)data;
    vpi_printf("end of simulation\n");
    value("tb.v", taken[0]);
    relations(taken[0]);
    relations(pending_handle);
    /* Registering is too late; the pending event handle is kept on purpose. */
    s_cb_data again;
    memset(&again, 0, sizeof again);
    s_vpi_time t;
    memset(&t, 0, sizeof t);
    t.type = vpiSimTime;
    again.reason = cbAfterDelay;
    again.cb_rtn = ended;
    again.time = &t;
    vpi_register_cb(&again);
    vpi_chk_error(NULL);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    s_cb_data data;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$step";
    task.calltf = step;
    vpi_register_systf(&task);
    memset(&data, 0, sizeof data);
    data.reason = cbEndOfSimulation;
    data.cb_rtn = ended;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
