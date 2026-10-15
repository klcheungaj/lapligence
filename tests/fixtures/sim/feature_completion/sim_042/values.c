/* SIM-042 value access: every Table 38-3 format with exact X/Z, and
 * vpi_put_value with each delay mode, scheduled events, cancellation, force
 * and release. Companion of values.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle event_handle;

static vpiHandle named(const char* name) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    if (!handle) vpi_printf("FAIL lookup %s\n", name);
    return handle;
}

/* Copy each result: the simulator reuses its value buffer per call (38.15). */
static const char* shown(vpiHandle handle, PLI_INT32 format) {
    static char ring[16][160];
    static unsigned next;
    char* buffer = ring[next++ % 16];
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    vpi_get_value(handle, &value);
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) {
        snprintf(buffer, sizeof(ring[0]), "<error %d>", (int)level);
        return buffer;
    }
    switch (value.format) {
        case vpiIntVal:
            snprintf(buffer, sizeof(ring[0]), "%d", (int)value.value.integer);
            break;
        case vpiRealVal:
            snprintf(buffer, sizeof(ring[0]), "%.2f", value.value.real);
            break;
        case vpiScalarVal:
            snprintf(buffer, sizeof(ring[0]), "%c", "01zx"[value.value.scalar & 3]);
            break;
        case vpiTimeVal:
            snprintf(buffer, sizeof(ring[0]), "%u:%u", (unsigned)value.value.time->high,
                     (unsigned)value.value.time->low);
            break;
        case vpiVectorVal: {
            PLI_INT32 size = vpi_get(vpiSize, handle);
            size_t at = 0;
            for (PLI_INT32 word = (size + 31) / 32 - 1; word >= 0 && at < sizeof(ring[0]); --word) {
                int written = snprintf(buffer + at, sizeof(ring[0]) - at, "%s%08x/%08x",
                                       at ? " " : "", (unsigned)value.value.vector[word].aval,
                                       (unsigned)value.value.vector[word].bval);
                if (written < 0) break;
                at += (size_t)written;
            }
            break;
        }
        case vpiStrengthVal: {
            PLI_INT32 size = vpi_get(vpiSize, handle);
            size_t at = 0;
            for (PLI_INT32 bit = size - 1; bit >= 0 && at < sizeof(ring[0]); --bit) {
                int written = snprintf(buffer + at, sizeof(ring[0]) - at, "%s%c:%02x:%02x",
                                       at ? " " : "", "01zx"[value.value.strength[bit].logic & 3],
                                       (unsigned)value.value.strength[bit].s0,
                                       (unsigned)value.value.strength[bit].s1);
                if (written < 0) break;
                at += (size_t)written;
            }
            break;
        }
        default:
            snprintf(buffer, sizeof(ring[0]), "%s",
                     value.value.str ? (const char*)value.value.str : "<null>");
            break;
    }
    return buffer;
}

static void all_formats(const char* name) {
    vpiHandle handle = named(name);
    s_vpi_value natural;
    memset(&natural, 0, sizeof(natural));
    natural.format = vpiObjTypeVal;
    vpi_get_value(handle, &natural);
    vpi_printf("%s natural=%d bin=%s oct=%s hex=%s dec=%s int=%s\n", name, (int)natural.format,
               shown(handle, vpiBinStrVal), shown(handle, vpiOctStrVal),
               shown(handle, vpiHexStrVal), shown(handle, vpiDecStrVal),
               shown(handle, vpiIntVal));
}

static void put_text(const char* name, PLI_INT32 format, const char* text) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    value.value.str = (PLI_BYTE8*)text;
    vpiHandle result = vpi_put_value(named(name), &value, NULL, vpiNoDelay);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("put %s fmt=%d \"%s\" -> %s level=%d\n", name, (int)format, text,
               result ? "handle" : "null", (int)level);
}

static PLI_INT32 formats(PLI_BYTE8* data) {
    (void)data;
    all_formats("tb.v");
    all_formats("tb.xs");
    all_formats("tb.zs");
    all_formats("tb.pz");
    all_formats("tb.s");
    vpiHandle s = named("tb.s");
    vpi_printf("tb.s real=%s vector=%s\n", shown(s, vpiRealVal), shown(s, vpiVectorVal));
    vpiHandle wide = named("tb.wide");
    vpi_printf("tb.wide hex=%s vector=%s\n", shown(wide, vpiHexStrVal), shown(wide, vpiVectorVal));
    vpiHandle text = named("tb.str");
    vpi_printf("tb.str string=\"%s\" hex=%s\n", shown(text, vpiStringVal), shown(text, vpiHexStrVal));
    vpiHandle r = named("tb.r");
    vpi_printf("tb.r real=%s int=%s natural=%s bin=%s\n", shown(r, vpiRealVal), shown(r, vpiIntVal),
               shown(r, vpiObjTypeVal), shown(r, vpiBinStrVal));
    vpi_printf("tb.pz strength=%s\n", shown(named("tb.pz"), vpiStrengthVal));
    vpi_printf("tb.d time=%s\n", shown(named("tb.d"), vpiTimeVal));

    put_text("tb.str", vpiStringVal, "OK");
    vpi_printf("tb.str now \"%s\" hex=%s\n", shown(text, vpiStringVal), shown(text, vpiHexStrVal));
    put_text("tb.xs", vpiOctStrVal, "7_7");
    put_text("tb.zs", vpiHexStrVal, "z");
    put_text("tb.s", vpiDecStrVal, "-1");
    put_text("tb.v", vpiBinStrVal, "10xz");
    put_text("tb.r", vpiStringVal, "1.0");
    put_text("tb.wide", vpiHexStrVal, "fe_dcba_9876_5432_10ff");
    /* vpiStrengthVal is illegal for a vector (38.34); a bit takes it. */
    s_vpi_strengthval bits[4];
    memset(bits, 0, sizeof(bits));
    bits[0].logic = vpiX;
    bits[0].s0 = bits[0].s1 = vpiStrongDrive;
    s_vpi_value strength;
    memset(&strength, 0, sizeof(strength));
    strength.format = vpiStrengthVal;
    strength.value.strength = bits;
    vpiHandle vector_strength = vpi_put_value(named("tb.pz"), &strength, NULL, vpiNoDelay);
    vpi_printf("strength to vector -> %s level=%d\n", vector_strength ? "handle" : "null",
               (int)vpi_chk_error(NULL));
    (void)vpi_put_value(vpi_handle_by_index(named("tb.pz"), 0), &strength, NULL, vpiNoDelay);
    s_vpi_value scalar;
    memset(&scalar, 0, sizeof(scalar));
    scalar.format = vpiScalarVal;
    scalar.value.scalar = vpi1;
    vpiHandle refused = vpi_put_value(named("tb.v"), &scalar, NULL, vpiNoDelay);
    vpi_printf("scalar to vector -> %s level=%d\n", refused ? "handle" : "null",
               (int)vpi_chk_error(NULL));
    s_vpi_value real;
    memset(&real, 0, sizeof(real));
    real.format = vpiRealVal;
    real.value.real = 6.5;
    (void)vpi_put_value(named("tb.s"), &real, NULL, vpiNoDelay);
    vpi_printf("after: xs=%s zs=%s s=%s v=%s pz=%s wide=%s\n", shown(named("tb.xs"), vpiBinStrVal),
               shown(named("tb.zs"), vpiBinStrVal), shown(named("tb.s"), vpiDecStrVal),
               shown(named("tb.v"), vpiBinStrVal), shown(named("tb.pz"), vpiBinStrVal),
               shown(wide, vpiHexStrVal));
    return 0;
}

static vpiHandle delayed(PLI_INT32 integer, PLI_INT32 time_type, unsigned ticks, double real,
                         PLI_INT32 flags) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    value.value.integer = integer;
    s_vpi_time time;
    memset(&time, 0, sizeof(time));
    time.type = time_type;
    time.low = ticks;
    time.real = real;
    vpiHandle result = vpi_put_value(named("tb.d"), &value, &time, flags);
    if (vpi_chk_error(NULL)) vpi_printf("FAIL delayed put %d\n", (int)integer);
    return result;
}

static PLI_INT32 schedule(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("inertial=%s\n", delayed(1, vpiSimTime, 2, 0.0, vpiInertialDelay) ? "handle" : "null");
    (void)delayed(2, vpiSimTime, 3, 0.0, vpiInertialDelay);
    (void)delayed(4, vpiSimTime, 6, 0.0, vpiTransportDelay);
    (void)delayed(5, vpiSimTime, 5, 0.0, vpiTransportDelay);
    event_handle = delayed(6, vpiSimTime, 4, 0.0, vpiPureTransportDelay | vpiReturnEvent);
    vpi_printf("event type=%s scheduled=%d\n",
               event_handle ? (const char*)vpi_get_str(vpiType, event_handle) : "<null>",
               (int)vpi_get(vpiScheduled, event_handle));
    vpiHandle cancelled = delayed(7, vpiSimTime, 7, 0.0, vpiPureTransportDelay | vpiReturnEvent);
    vpiHandle cancel = vpi_put_value(cancelled, NULL, NULL, vpiCancelEvent);
    PLI_INT32 level = vpi_chk_error(NULL);
    /* Cancelling also frees the event handle (38.34). */
    int stale = vpi_get(vpiScheduled, cancelled) == vpiUndefined && vpi_chk_error(NULL) == vpiError;
    vpi_printf("cancel -> %s level=%d stale=%d\n", cancel ? "handle" : "null", (int)level, stale);
    (void)delayed(8, vpiScaledRealTime, 0, 8.0, vpiPureTransportDelay);
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    value.value.integer = 3;
    vpiHandle missing = vpi_put_value(named("tb.d"), &value, NULL, vpiTransportDelay);
    vpi_printf("missing time -> %s level=%d\n", missing ? "handle" : "null", (int)vpi_chk_error(NULL));
    return 0;
}

static PLI_INT32 late(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("late scheduled=%d\n", (int)vpi_get(vpiScheduled, event_handle));
    vpi_printf("release=%d\n", (int)vpi_release_handle(event_handle));
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiHexStrVal;
    value.value.str = (PLI_BYTE8*)"c";
    (void)vpi_put_value(named("tb.d"), &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 force_some(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiHexStrVal;
    value.value.str = (PLI_BYTE8*)"aa";
    vpiHandle result = vpi_put_value(named("tb.f"), &value, NULL, vpiForceFlag);
    vpi_printf("force f -> %s level=%d\n", result ? "handle" : "null", (int)vpi_chk_error(NULL));
    value.value.str = (PLI_BYTE8*)"3";
    (void)vpi_put_value(named("tb.n"), &value, NULL, vpiForceFlag);
    vpi_printf("forced: f=%s n=%s\n", shown(named("tb.f"), vpiHexStrVal),
               shown(named("tb.n"), vpiHexStrVal));
    return 0;
}

static PLI_INT32 release_some(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiHexStrVal;
    (void)vpi_put_value(named("tb.f"), &value, NULL, vpiReleaseFlag);
    vpi_printf("release f value=%s level=%d\n", value.value.str ? (const char*)value.value.str : "<null>",
               (int)vpi_chk_error(NULL));
    memset(&value, 0, sizeof(value));
    value.format = vpiHexStrVal;
    (void)vpi_put_value(named("tb.n"), &value, NULL, vpiReleaseFlag);
    vpi_printf("release n value=%s level=%d\n", value.value.str ? (const char*)value.value.str : "<null>",
               (int)vpi_chk_error(NULL));
    return 0;
}

static void register_task(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*)) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)name;
    task.calltf = calltf;
    if (!vpi_register_systf(&task)) vpi_printf("registration of %s failed\n", name);
}

static void register_all(void) {
    register_task("$formats", formats);
    register_task("$schedule", schedule);
    register_task("$late", late);
    register_task("$force_some", force_some);
    register_task("$release_some", release_some);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
