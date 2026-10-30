/* ADV-019 runtime structures walker (IEEE 1800-2009 37.32, 37.38, 37.54,
 * 37.75, 37.76, 37.79). */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle placed_value, placed_delay;

static PLI_INT32 ignore(p_cb_data data) {
    (void)data;
    return 0;
}

static vpiHandle place(PLI_INT32 reason, vpiHandle obj, PLI_UINT32 delay, const char* tag) {
    s_cb_data data;
    s_vpi_time time;
    s_vpi_value value;
    memset(&data, 0, sizeof data);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = delay;
    value.format = vpiIntVal;
    data.reason = reason;
    data.cb_rtn = ignore;
    data.obj = obj;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)tag;
    return vpi_register_cb(&data);
}

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static void callbacks(const char* label, vpiHandle reference) {
    vpiHandle it = vpi_iterate(vpiCallback, reference), cb;
    int error = failed();
    vpi_printf("  callbacks of %s (error %d):", label, error);
    while (it && (cb = vpi_scan(it))) {
        s_cb_data info;
        vpi_get_cb_info(cb, &info);
        vpi_printf(" %s reason=%d type=%d", info.user_data ? (const char*)info.user_data : "-",
                   (int)info.reason, (int)vpi_get(vpiType, cb));
        if (info.obj) vpi_printf(" obj=%s", vpi_get_str(vpiFullName, info.obj));
        if (placed_value && vpi_compare_objects(cb, placed_value)) vpi_printf(" (registered handle)");
    }
    vpi_printf("\n");
}

static void time_queues(void) {
    vpiHandle it = vpi_iterate(vpiTimeQueue, NULL), queue;
    int error = failed();
    vpi_printf("  time queue (error %d):\n", error);
    while (it && (queue = vpi_scan(it))) {
        s_vpi_time time;
        time.type = vpiScaledRealTime;
        vpi_get_time(queue, &time);
        vpi_printf("    %s at %g\n", vpi_get_str(vpiType, queue), time.real);
        callbacks("the queue", queue);
    }
}

static void udps(void) {
    vpiHandle it = vpi_iterate(vpiUdpDefn, NULL), defn;
    while (it && (defn = vpi_scan(it))) {
        vpi_printf("  %s %s inputs=%d prim=%d protected=%d line=%d\n", vpi_get_str(vpiType, defn),
                   vpi_get_str(vpiDefName, defn), (int)vpi_get(vpiSize, defn),
                   (int)vpi_get(vpiPrimType, defn), (int)vpi_get(vpiProtected, defn),
                   (int)vpi_get(vpiLineNo, defn));
        (void)vpi_get_str(vpiName, defn);
        vpi_printf("    vpiName error %d\n", failed());
        vpiHandle ios = vpi_iterate(vpiIODecl, defn), io;
        while (ios && (io = vpi_scan(ios)))
            vpi_printf("    io %s dir=%d scalar=%d same defn=%d\n", vpi_get_str(vpiName, io),
                       (int)vpi_get(vpiDirection, io), (int)vpi_get(vpiScalar, io),
                       (int)vpi_compare_objects(vpi_handle(vpiUdpDefn, io), defn));
        vpiHandle entries = vpi_iterate(vpiTableEntry, defn), entry;
        while (entries && (entry = vpi_scan(entries))) {
            s_vpi_value value;
            value.format = vpiStringVal;
            vpi_get_value(entry, &value);
            vpi_printf("    entry %s symbols=%d line=%d\n", value.value.str,
                       (int)vpi_get(vpiSize, entry), (int)vpi_get(vpiLineNo, entry));
        }
        vpiHandle initial = vpi_handle(vpiInitial, defn);
        int error = failed();
        vpi_printf("    vpiInitial %s error %d\n", initial ? "object" : "NULL", error);
    }
    vpiHandle u1 = vpi_handle_by_name((PLI_BYTE8*)"tb.u1", NULL);
    vpi_printf("  tb.u1 definition %s\n", vpi_get_str(vpiDefName, vpi_handle(vpiUdpDefn, u1)));
}

static void uses(void) {
    vpiHandle top = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle r = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    vpiHandle it = vpi_iterate(vpiUse, r), use;
    vpi_printf("  uses of tb.r:");
    while (it && (use = vpi_scan(it)))
        vpi_printf(" %s@%d", vpi_get_str(vpiType, use), (int)vpi_get(vpiLineNo, use));
    vpi_printf("\n");
    it = vpi_iterate(vpiNet, top);
    vpi_printf("  net iterator vpiUse is tb: %d\n", (int)vpi_compare_objects(vpi_handle(vpiUse, it), top));
    vpi_free_object(it);
    it = vpi_iterate(vpiModule, NULL);
    vpiHandle reference = vpi_handle(vpiUse, it);
    int error = failed();
    vpi_printf("  module iterator vpiUse: %s error %d\n", reference ? "object" : "NULL", error);
    vpi_free_object(it);
    vpiHandle bit = vpi_handle_by_index(r, 0);
    it = vpi_iterate(vpiUse, bit);
    vpiHandle first = it ? vpi_scan(it) : NULL;
    vpi_printf("  first use of tb.r[0]: %s@%d\n", vpi_get_str(vpiType, first),
               (int)vpi_get(vpiLineNo, first));
    vpi_release_handle(bit);
    reference = vpi_handle(vpiUse, it);
    error = failed();
    vpi_printf("  use of the iterator after its reference was released: %s error %d\n",
               reference ? "object" : "NULL", error);
}

static PLI_INT32 runtime(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_free_object(args);
    vpi_printf("step %d\n", (int)value.value.integer);
    vpiHandle r = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    if (value.value.integer == 1) {
        vpiHandle it = vpi_iterate(vpiUserSystf, NULL), systf;
        while (it && (systf = vpi_scan(it))) {
            s_vpi_systf_data info;
            vpi_get_systf_info(systf, &info);
            vpi_printf("  %s %s\n", vpi_get_str(vpiType, systf), (const char*)info.tfname);
        }
        placed_value = place(cbValueChange, r, 0, "vc");
        placed_delay = place(cbAfterDelay, NULL, 3, "ad");
        vpi_remove_cb(place(cbAfterDelay, NULL, 4, "removed"));
        place(cbEndOfSimulation, NULL, 0, "eos");
        place(cbAtEndOfSimTime, NULL, 0, "eot");
        callbacks("NULL", NULL);
        callbacks("tb.r", r);
        callbacks("tb", vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
        callbacks("tb.u1", vpi_handle_by_name((PLI_BYTE8*)"tb.u1", NULL));
        time_queues();
        udps();
        uses();
    } else {
        time_queues();
        callbacks("tb.r", r);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$runtime";
    task.calltf = runtime;
    vpi_register_systf(&task);
    task.type = vpiSysFunc;
    task.sysfunctype = vpiIntFunc;
    task.tfname = (PLI_BYTE8*)"$runtime_unused";
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
