/* Chosen VPI routines on chosen handles: live ones, released ones, ones whose
 * object has ended, null and wild pointers. `+seed=<n>` selects the calls,
 * `+sweep` reads every relation and property of the reachable objects once. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "vpi_user.h"
#include "sv_vpi_user.h"

#define COUNT(array) ((uint32_t)(sizeof array / sizeof array[0]))
#define POOL 160
#define TYPES 760

static vpiHandle pool[POOL];
static uint64_t state = 1;
static uint32_t depth, registrations, sweeping, budget = 12000;
static volatile uint64_t sink;

static const char* const names[] = {
    "tb", "tb.d", "tb.u", "tb.u.q", "tb.mem", "tb.mem[1][0]", "tb.mem[9][9]", "tb.dyn", "tb.dyn[1]",
    "tb.dyn[7]", "tb.que[0]", "tb.que[$]", "tb.asoc", "tb.asoc[\"a\"]", "tb.iasoc[4]", "tb.b", "tb.b.x",
    "tb.b.next.x", "tb.dd.y", "tb.dd.y[1]", "tb.rec.s", "tb.rec.v[1]", "tb.pr.c", "tb.str", "tb.r", "tb.ev",
    "tb.vif.data", "tb.bi.data", "tb.bi.m", "tb.g[1].inner.z", "tb.g[0]", "tb.g", "tb.main", "tb.main.o",
    "tb.auto_t", "tb.auto_t.l1", "tb.auto_t.in_auto.deep", "tb.stat_t.s1", "tb.fact.n", "pk::shared", "pk::",
    "tb.wide[70:3]", "tb.wide[999]", "tb.two", "tb.nets[1]", "tb.st", "tb.mb", "tb.sem", "tb.p", "tb.cgi",
    "tb.ap", "tb.u.g1", "tb.u.u1", "$root", "$root.tb", "", "tb.", ".d", "tb..d", "tb.d[", "tb.d[]",
    "tb.d[x]", "tb.\\esc ", "nothing", "tb.un", "tb.col", "tb.i64", "tb.ch", "tb.sq", "tb.u.tp", "tb.u.W",
};

static const PLI_INT32 formats[] = {
    vpiBinStrVal, vpiOctStrVal, vpiDecStrVal,   vpiHexStrVal, vpiScalarVal, vpiIntVal,   vpiRealVal,
    vpiStringVal, vpiVectorVal, vpiStrengthVal, vpiTimeVal,   vpiObjTypeVal, vpiSuppressVal, 99, -1,
};

static const PLI_INT32 relations[] = {
    vpiModule, vpiNet, vpiReg, vpiVariables, vpiScope, vpiInternalScope, vpiPort, vpiParameter, vpiTypespec,
    vpiMember, vpiElement, vpiRange, vpiLeftRange, vpiRightRange, vpiIndex, vpiBit, vpiNetBit, vpiExpr,
    vpiLhs, vpiRhs, vpiStmt, vpiProcess, vpiTaskFunc, vpiIODecl, vpiArgument, vpiDriver, vpiLoad, vpiUse,
    vpiParent, vpiCallback, vpiThread, vpiFrame, vpiActive, vpiAutomatics, vpiClassDefn, vpiClassTypespec,
    vpiClassObj, vpiMethods, vpiConstraint, vpiAssertion, vpiActual, vpiInterface, vpiModport,
    vpiClockingBlock, vpiGenScope, vpiGenScopeArray, vpiPrimitive, vpiPrimTerm, vpiUdpDefn, vpiTableEntry,
    vpiModPath, vpiPathTerm, vpiTchk, vpiSysTfCall, vpiUserSystf, vpiTimeQueue, vpiContAssign, vpiDelay,
    vpiHighConn, vpiLowConn, vpiOperand, vpiCondition, vpiElemTypespec, vpiEnumConst, vpiExtends,
    vpiDerivedClasses, vpiCollection, vpiAttribute, vpiInstance, vpiPackage, vpiNamedEvent, vpiParamAssign,
    vpiRegArray, vpiNetArray, vpiInstanceArray, vpiConcurrentAssertions, vpiImport,
};

static const PLI_INT32 reasons[] = {
    cbValueChange, cbReadWriteSynch, cbReadOnlySynch, cbNextSimTime, cbAfterDelay, cbAtStartOfSimTime,
    cbNBASynch, cbAtEndOfSimTime, cbForce, cbRelease, cbDisable, cbStmt, cbStartOfThread, cbEndOfThread,
    cbEnterThread, cbCreateObj, cbReclaimObj, cbSizeChange, cbError, cbStartOfFrame, cbEndOfFrame,
    cbEndOfObject, 999, -3,
};

static const PLI_INT32 modes[] = {
    vpiNoDelay, vpiInertialDelay, vpiTransportDelay, vpiPureTransportDelay, vpiForceFlag, vpiReleaseFlag,
    vpiCancelEvent, 99,
};

static uint32_t below(uint32_t bound) {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    return bound ? (uint32_t)(state >> 16) % bound : 0;
}

static vpiHandle any(void) { return below(60) ? pool[below(POOL)] : NULL; }

static void keep(vpiHandle handle) {
    if (handle || !below(60)) pool[below(POOL)] = handle;
}

static PLI_INT32 relation(void) {
    return below(6) ? relations[below(COUNT(relations))] : (PLI_INT32)below(1000) - 20;
}

static void text(const char* string) {
    if (string) sink += strlen(string);
}

static vpiHandle by_name(const char* name, vpiHandle scope) {
    char copy[40];
    snprintf(copy, sizeof copy, "%s", name);
    return vpi_handle_by_name(copy, scope);
}

static void read_value(vpiHandle handle, PLI_INT32 format) {
    s_vpi_value value;
    s_vpi_time time;
    memset(&value, 0, sizeof value);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    value.format = format;
    if (format == vpiTimeVal) value.value.time = &time;
    vpi_chk_error(NULL);
    vpi_get_value(handle, &value);
    if (vpi_chk_error(NULL)) return;
    PLI_INT32 type = vpi_get(vpiType, handle), size = vpi_get(vpiSize, handle);
    if (vpi_chk_error(NULL) || size < 0 || size > 100000) return;
    /* The size of a primitive is the number of its inputs (37.31). */
    if (type == vpiGate || type == vpiSwitch || type == vpiUdp) size = 1;
    switch (value.format) {
        case vpiBinStrVal:
        case vpiOctStrVal:
        case vpiDecStrVal:
        case vpiHexStrVal:
        case vpiStringVal: text(value.value.str); break;
        case vpiVectorVal:
            for (PLI_INT32 i = 0; i < (size + 31) / 32; i++)
                sink += (uint32_t)value.value.vector[i].aval + (uint32_t)value.value.vector[i].bval;
            break;
        case vpiStrengthVal:
            for (PLI_INT32 i = 0; i < size; i++) sink += (uint32_t)value.value.strength[i].logic;
            break;
        default: break;
    }
}

static void write_value(vpiHandle handle) {
    static s_vpi_vecval vector[64];
    static s_vpi_strengthval strengths[2048];
    char string[41];
    s_vpi_value value;
    s_vpi_time time;
    memset(&value, 0, sizeof value);
    memset(&time, 0, sizeof time);
    for (uint32_t i = 0; i < 40; i++) string[i] = "01xzXZ0123456789abcdefgh _-?.%"[below(30)];
    string[below(41)] = 0;
    string[40] = 0;
    for (uint32_t i = 0; i < 4; i++) {
        vector[i].aval = (PLI_UINT32)below(UINT32_MAX);
        vector[i].bval = below(3) ? 0 : (PLI_UINT32)below(UINT32_MAX);
        strengths[i].logic = (PLI_INT32)below(5);
        strengths[i].s0 = (PLI_INT32)below(130);
        strengths[i].s1 = (PLI_INT32)below(130);
    }
    time.type = below(8) ? vpiSimTime : vpiScaledRealTime;
    time.low = below(6);
    time.real = below(5) * 0.5;
    value.format = formats[below(COUNT(formats))];
    switch (value.format) {
        case vpiScalarVal: value.value.scalar = (PLI_INT32)below(10); break;
        case vpiIntVal: value.value.integer = (PLI_INT32)below(UINT32_MAX); break;
        case vpiRealVal: value.value.real = below(4) ? below(1000) / 7.0 : -1e300 * 1e300; break;
        case vpiVectorVal: value.value.vector = vector; break;
        case vpiStrengthVal: value.value.strength = strengths; break;
        case vpiTimeVal: value.value.time = &time; break;
        default: value.value.str = string; break;
    }
    PLI_INT32 mode = modes[below(COUNT(modes))] | (below(5) ? 0 : vpiReturnEvent);
    keep(vpi_put_value(handle, &value, below(8) ? &time : NULL, mode));
}

static PLI_INT32 called_back(p_cb_data data);

static void register_callback(vpiHandle handle) {
    s_cb_data data;
    s_vpi_time time;
    s_vpi_value value;
    memset(&data, 0, sizeof data);
    memset(&time, 0, sizeof time);
    memset(&value, 0, sizeof value);
    data.reason = below(3) ? reasons[below(COUNT(reasons))] : cbValueChange;
    data.cb_rtn = below(30) ? called_back : NULL;
    data.obj = handle;
    time.type = below(4) ? vpiSimTime : vpiSuppressTime;
    time.low = below(5);
    value.format = (PLI_INT32)below(20);
    if (below(8)) data.time = &time;
    if (below(8)) data.value = &value;
    keep(vpi_register_cb(&data));
}

static void iterate(vpiHandle handle) {
    vpiHandle iterator = vpi_iterate(relation(), handle);
    if (!iterator) return;
    if (!below(8)) {
        keep(iterator);
        return;
    }
    for (uint32_t left = below(12);; left--) {
        vpiHandle member = vpi_scan(iterator);
        if (!member) return;
        keep(member);
        if (left == 0) break;
    }
    if (below(2))
        keep(iterator);
    else
        vpi_release_handle(iterator);
}

static void call(void) {
    vpiHandle handle = below(5) < 2 ? by_name(names[below(COUNT(names))], NULL) : any();
    s_cb_data data;
    s_vpi_systf_data systf;
    s_vpi_time time;
    if (!below(4)) keep(handle = vpi_handle(relation(), handle));
    vpi_chk_error(NULL);
    switch (below(20)) {
        case 0: keep(vpi_handle(relation(), handle)); break;
        case 1:
        case 2: iterate(handle); break;
        case 3: keep(vpi_scan(handle)); break;
        case 4: sink += (uint32_t)vpi_get((PLI_INT32)below(TYPES), handle); break;
        case 5: sink += (uint64_t)vpi_get64((PLI_INT32)below(TYPES), handle); break;
        case 6: text(vpi_get_str(below(2) ? vpiFullName : (PLI_INT32)below(TYPES), handle)); break;
        case 7:
        case 8: read_value(handle, formats[below(COUNT(formats))]); break;
        case 9:
        case 10: write_value(handle); break;
        case 11:
            if (registrations++ < 150) register_callback(handle);
            break;
        case 12: vpi_remove_cb(handle); break;
        case 13: {
            uint32_t slot = below(POOL);
            vpi_release_handle(pool[slot]);
            if (!below(4)) pool[slot] = NULL;
            break;
        }
        case 14: keep(by_name(names[below(COUNT(names))], below(3) ? NULL : handle)); break;
        case 15: keep(vpi_handle_by_index(handle, (PLI_INT32)below(12) - 3)); break;
        case 16: sink += (uint32_t)vpi_compare_objects(handle, any()); break;
        case 17:
            memset(&data, 0, sizeof data);
            vpi_get_cb_info(handle, &data);
            memset(&systf, 0, sizeof systf);
            vpi_get_systf_info(handle, &systf);
            break;
        case 18:
            memset(&time, 0, sizeof time);
            time.type = (PLI_INT32)below(5);
            vpi_get_time(handle, &time);
            break;
        case 19: keep(vpi_create(below(6) ? vpiCollection : relation(), any(), handle)); break;
    }
}

static void calls(uint32_t count) {
    if (sweeping || depth >= 2) return;
    depth++;
    for (; count && budget; count--, budget--) call();
    depth--;
}

static uint32_t known(const vpiHandle* list, uint32_t count, vpiHandle handle) {
    uint32_t same = 0;
    for (uint32_t i = 0; i < count; i++) same += vpi_get(vpiType, list[i]) == vpi_get(vpiType, handle);
    return same >= 4;
}

static void sweep(void) {
    static vpiHandle list[400];
    uint32_t count = 0;
    for (uint32_t i = 0; i < COUNT(names); i++) {
        vpiHandle handle = by_name(names[i], NULL);
        if (handle) list[count++] = handle;
    }
    for (uint32_t i = 0; i < count; i++) {
        for (PLI_INT32 type = 1; type < TYPES; type++) {
            vpiHandle handle = vpi_handle(type, list[i]), iterator = vpi_iterate(type, list[i]);
            uint32_t members = 0;
            for (; handle || iterator; handle = NULL) {
                if (!handle && !(handle = vpi_scan(iterator))) break;
                if (count < COUNT(list) && !known(list, count, handle))
                    list[count++] = handle;
                else
                    vpi_release_handle(handle);
                if (!iterator) break;
                if (++members == 40) {
                    vpi_release_handle(iterator);
                    break;
                }
            }
        }
    }
    for (uint32_t i = 0; i < count; i++) {
        for (PLI_INT32 type = 1; type < TYPES; type++) {
            sink += (uint32_t)vpi_get(type, list[i]);
            text(vpi_get_str(type, list[i]));
        }
        for (uint32_t f = 0; f < COUNT(formats); f++) read_value(list[i], formats[f]);
        for (PLI_INT32 index = -2; index < 10; index++) {
            vpiHandle handle = vpi_handle_by_index(list[i], index);
            if (handle) vpi_release_handle(handle);
        }
    }
    for (uint32_t i = 0; i < count; i++) vpi_release_handle(list[i]);
    vpi_printf("%s\n", count > 150 ? "swept" : "too few objects");
}

static PLI_INT32 called_back(p_cb_data data) {
    if (data->obj) keep(data->obj);
    if (data->value && (data->value->format == vpiBinStrVal || data->value->format == vpiHexStrVal))
        text(data->value->value.str);
    calls(below(5));
    return 0;
}

static PLI_INT32 step(PLI_BYTE8* user) {
    vpiHandle here = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, here), argument;
    s_vpi_value value;
    (void)user;
    keep(here);
    value.format = vpiIntVal;
    value.value.integer = 1;
    if (arguments && (argument = vpi_scan(arguments))) {
        vpi_get_value(argument, &value);
        keep(argument);
        keep(arguments);
    }
    for (uint32_t i = 0; i < 20; i++) keep(by_name(names[below(COUNT(names))], NULL));
    calls((uint32_t)value.value.integer * 150);
    if (sweeping == 1) {
        sweeping = 2;
        sweep();
    }
    return 0;
}

static PLI_INT32 function(PLI_BYTE8* user) {
    vpiHandle here = vpi_handle(vpiSysTfCall, NULL);
    s_vpi_value value;
    (void)user;
    keep(here);
    calls(50);
    value.format = vpiIntVal;
    value.value.integer = 2;
    vpi_put_value(here, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 compiled(PLI_BYTE8* user) {
    (void)user;
    keep(vpi_handle(vpiSysTfCall, NULL));
    calls(30);
    return 0;
}

static PLI_INT32 phase(p_cb_data data) {
    (void)data;
    calls(120);
    return 0;
}

static void startup(void) {
    static const PLI_INT32 phases[] = {cbEndOfCompile, cbStartOfSimulation, cbEndOfSimulation};
    s_vpi_systf_data task;
    s_cb_data data;
    s_vpi_vlog_info info;
    uint64_t seed = 1;
    if (vpi_get_vlog_info(&info)) {
        for (PLI_INT32 i = 0; i < info.argc; i++) {
            if (!strncmp(info.argv[i], "+seed=", 6)) seed = strtoull(info.argv[i] + 6, NULL, 10);
            if (!strncmp(info.argv[i], "+budget=", 8)) budget = (uint32_t)strtoul(info.argv[i] + 8, NULL, 10);
            if (!strcmp(info.argv[i], "+sweep")) sweeping = 1;
        }
    }
    state = seed * 0x9E3779B97F4A7C15u + 88172645463325252u;
    for (uint32_t i = 0; i < 20; i++) below(2);
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$step";
    task.calltf = step;
    task.compiletf = compiled;
    keep(vpi_register_systf(&task));
    memset(&task, 0, sizeof task);
    task.type = vpiSysFunc;
    task.sysfunctype = vpiIntFunc;
    task.tfname = "$value";
    task.calltf = function;
    keep(vpi_register_systf(&task));
    for (uint32_t i = 0; i < COUNT(phases); i++) {
        memset(&data, 0, sizeof data);
        data.cb_rtn = phase;
        data.reason = phases[i];
        keep(vpi_register_cb(&data));
    }
    pool[0] = (vpiHandle)(uintptr_t)0x1234;
    pool[1] = (vpiHandle)(uintptr_t)UINTPTR_MAX;
    pool[2] = (vpiHandle)(uintptr_t)1;
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
