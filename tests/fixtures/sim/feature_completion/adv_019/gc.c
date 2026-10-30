/* ADV-019 family 5, collection: cbCreateObj, cbReclaimObj and cbEndOfObject
 * on the class objects of gc.sv (IEEE 1800-2009 37.3.7, 37.29, 38.36.1),
 * and the handles into an object after it is reclaimed. Objects that die in
 * one collection are reported in any order, so the reclaimed values are
 * printed sorted by $check. */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle held, held_value, held_item, held_constraint;
static int reclaimed[64];
static int reclaimed_count, changes;

static int first_int(vpiHandle obj) {
    vpiHandle it = vpi_iterate(vpiVariables, obj);
    vpiHandle property = vpi_scan(it);
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(property, &v);
    vpi_release_handle(it);
    return vpi_chk_error(NULL) ? -1 : (int)v.value.integer;
}

static PLI_INT32 on_create(p_cb_data data) {
    vpi_printf("create %s v=%d id=%lld\n", (const char*)data->user_data, first_int(data->obj),
               (long long)vpi_get64(vpiObjId, data->obj));
    return 0;
}

static PLI_INT32 on_reclaim(p_cb_data data) {
    int value = first_int(data->obj);
    if (reclaimed_count < 64) reclaimed[reclaimed_count++] = value;
    if (data->time) vpi_printf("reclaim with a time\n");
    return 0;
}

static PLI_INT32 on_reclaim_held(p_cb_data data) {
    vpi_printf("reclaim held v=%d", first_int(data->obj));
    vpi_get(vpiIsConstraintEnabled, held_constraint);
    vpi_printf(" constraint level=%d", vpi_chk_error(NULL));
    vpiHandle it = vpi_iterate(vpiVariables, data->obj), p;
    while ((p = vpi_scan(it))) {
        if (strcmp(vpi_get_str(vpiName, p), "link") == 0) {
            vpiHandle target = vpi_handle(vpiClassObj, p);
            vpi_printf(" navigate=%s level=%d", target ? "set" : "null", vpi_chk_error(NULL));
        }
        if (strcmp(vpi_get_str(vpiName, p), "v") == 0) {
            s_vpi_value v;
            v.format = vpiIntVal;
            v.value.integer = 7;
            vpi_put_value(p, &v, NULL, vpiNoDelay);
            vpi_printf(" put level=%d", vpi_chk_error(NULL));
        }
    }
    vpi_printf("\n");
    return 0;
}

static PLI_INT32 on_end(p_cb_data data) {
    vpi_printf("end of object id=%lld time=%s\n", (long long)vpi_get64(vpiObjId, data->obj),
               data->time ? "set" : "NULL");
    return 0;
}

static PLI_INT32 on_change(p_cb_data data) {
    (void)data;
    ++changes;
    return 0;
}

static void watch(int reason, vpiHandle obj, PLI_INT32 (*routine)(p_cb_data), const char* tag) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.obj = obj;
    cb.cb_rtn = routine;
    cb.user_data = (PLI_BYTE8*)tag;
    vpiHandle registered = vpi_register_cb(&cb);
    vpi_printf("register %d %s level=%d\n", reason, registered ? "ok" : "refused",
               vpi_chk_error(NULL));
}

static PLI_INT32 setup(PLI_BYTE8* data) {
    (void)data;
    vpiHandle node = vpi_handle(vpiClassTypespec, vpi_handle_by_name("tb.keep", NULL));
    vpiHandle derived = vpi_iterate(vpiDerivedClasses, vpi_handle(vpiClassDefn, node));
    vpiHandle leaf = vpi_handle(vpiClassTypespec, vpi_scan(derived));
    vpi_release_handle(derived);
    vpi_printf("typespecs %s %s\n", vpi_get_str(vpiName, node), vpi_get_str(vpiName, leaf));
    watch(cbCreateObj, node, on_create, "Node");
    watch(cbCreateObj, leaf, on_create, "Leaf");
    watch(cbReclaimObj, node, on_reclaim, "Node");
    watch(cbReclaimObj, leaf, on_reclaim, "Leaf");
    watch(cbValueChange, vpi_handle_by_name("tb.tmp", NULL), on_change, "tmp");
    watch(cbCreateObj, vpi_handle_by_name("tb.tmp", NULL), on_create, "var");
    return 0;
}

static PLI_INT32 grab(PLI_BYTE8* data) {
    (void)data;
    held = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.tmp", NULL));
    held_value = vpi_handle_by_name("tb.tmp.v", NULL);
    held_item = vpi_handle_by_name("tb.tmp.items[0]", NULL);
    /* A constraint obtained from the object follows the object (37.30). */
    vpiHandle constraints = vpi_iterate(vpiConstraint, held);
    held_constraint = vpi_scan(constraints);
    vpi_release_handle(constraints);
    vpi_printf("grab id=%lld v=%d\n", (long long)vpi_get64(vpiObjId, held), first_int(held));
    watch(cbReclaimObj, held, on_reclaim_held, "held");
    watch(cbEndOfObject, held, on_end, "held");
    return 0;
}

static int compare(const void* a, const void* b) { return *(const int*)a - *(const int*)b; }

static PLI_INT32 check(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &v);
    vpi_release_handle(args);
    int phase = (int)v.value.integer;
    for (int i = 1; i < reclaimed_count; ++i)
        for (int j = i; j > 0 && compare(&reclaimed[j - 1], &reclaimed[j]) > 0; --j) {
            int t = reclaimed[j];
            reclaimed[j] = reclaimed[j - 1];
            reclaimed[j - 1] = t;
        }
    vpi_printf("check %d reclaimed", phase);
    for (int i = 0; i < reclaimed_count; ++i) vpi_printf(" %d", reclaimed[i]);
    vpi_printf(" changes=%d\n", changes);
    reclaimed_count = 0;
    vpiHandle tmp = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.tmp", NULL));
    vpiHandle keep = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.keep", NULL));
    vpi_printf("  keep id=%lld tmp id=%lld\n", (long long)vpi_get64(vpiObjId, keep),
               tmp ? (long long)vpi_get64(vpiObjId, tmp) : 0LL);
    if (phase == 1) {
        vpi_get_value(held_value, &v);
        vpi_printf("  held property level=%d\n", vpi_chk_error(NULL));
        vpi_get_value(held_item, &v);
        vpi_printf("  held element level=%d\n", vpi_chk_error(NULL));
        vpi_iterate(vpiConstraintItem, held_constraint);
        vpi_printf("  held constraint level=%d\n", vpi_chk_error(NULL));
        PLI_INT64 id = vpi_get64(vpiObjId, held);
        vpi_printf("  held object id=%lld level=%d\n", (long long)id, vpi_chk_error(NULL));
        vpi_printf("  tb.tmp.v now=%s\n", vpi_handle_by_name("tb.tmp.v", NULL) ? "set" : "null");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$setup";
    task.calltf = setup;
    vpi_register_systf(&task);
    task.tfname = "$grab";
    task.calltf = grab;
    vpi_register_systf(&task);
    task.tfname = "$check";
    task.calltf = check;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
