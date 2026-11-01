// llg_vpi_services.c — VPI simulator action callbacks (IEEE 1800-2009
// 38.36.3): cbError for run-time errors, cbPLIError for errors of VPI
// routine calls and cbSignal for operating-system signals; and the
// strengths of resolved nets (vpiStrengthVal, 38.15, 38.36.1). Embedded right
// after llg_vpi_runtime.c in the same translation unit: it uses the private
// callback, error-status and dispatch helpers of llg_vpi.c. Nothing here
// runs unless an application registers one of these reasons.

#include <signal.h>

/* Signal numbers cbSignal accepts: the C library decides which of them
 * can be caught (signal() refuses the others). */
#ifdef NSIG
#define LLG_VPI_SIGNALS NSIG
#else
#define LLG_VPI_SIGNALS 65
#endif

/* Routine errors kept for cbPLIError between two safe points; more are
 * counted and dropped (an application failing in a loop without returning
 * to the simulator). */
#define LLG_VPI_MAX_PLI_ERRORS 1024

/* One error status as vpi_chk_error() reports it. */
typedef struct {
    int pending;
    int state;
    int level;
    int line;
    char message[sizeof(g_vpi.error_message)];
    char code[sizeof(g_vpi.error_code)];
} svc_error_t;

static struct {
    int error_cbs;     /* live cbError records */
    int pli_cbs;       /* live cbPLIError records */
    int signal_cbs;    /* live cbSignal records */
    int dispatching;   /* a cbPLIError dispatch is running */
    svc_error_t* queue;  /* errors recorded since the last dispatch point */
    size_t queued;
    size_t capacity;
    uint64_t dropped;  /* errors beyond LLG_VPI_MAX_PLI_ERRORS */
    int handlers[LLG_VPI_SIGNALS]; /* live cbSignal records per signal */
    void (*previous[LLG_VPI_SIGNALS])(int);
    /* Strength views attached to nets that had none (svc_strength_bind),
     * kept until shutdown. */
    llg_net_t** view_nets;
    sv4_t** views;
    size_t view_count;
    size_t view_capacity;
} g_svc;

/* Set by the signal handler, read at the next safe point. */
static volatile sig_atomic_t g_svc_signal_pending[LLG_VPI_SIGNALS];
static volatile sig_atomic_t g_svc_any_signal;

static void svc_save(svc_error_t* out) {
    out->pending = g_vpi.error_pending;
    out->state = g_vpi.error_state;
    out->level = g_vpi.error_level;
    out->line = g_vpi.error_line;
    memcpy(out->message, g_vpi.error_message, sizeof(out->message));
    memcpy(out->code, g_vpi.error_code, sizeof(out->code));
}

static void svc_restore(const svc_error_t* in) {
    g_vpi.error_pending = in->pending;
    g_vpi.error_state = in->state;
    g_vpi.error_level = in->level;
    g_vpi.error_line = in->line;
    memcpy(g_vpi.error_message, in->message, sizeof(in->message));
    memcpy(g_vpi.error_code, in->code, sizeof(in->code));
}

static int svc_reason(int reason) {
    return reason == cbError || reason == cbPLIError || reason == cbSignal;
}

static int svc_matches(const llg_vpi_cb_t* cb, const void* context) {
    const int* want = (const int*)context;
    return cb->kind == CB_SERVICE && cb->data.reason == want[0] &&
           (want[0] != cbSignal || cb->data.index == want[1]);
}

/* Run every live record of `reason` (and signal `index`), each with the
 * error status `status` (NULL: leave it) visible to vpi_chk_error(). The
 * status current before the dispatch is restored afterwards. */
static void svc_dispatch(int reason, int index, const svc_error_t* status) {
    int want[2] = {reason, index};
    svc_error_t saved;
    svc_save(&saved);
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, svc_matches, want, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.obj = NULL;
        data.index = reason == cbSignal ? index : 0;
        data.time = callback_time(cb->time.type, NULL, llg_time(), &time);
        if (status) svc_restore(status);
        invoke_cb(cb, &data);
    }
    if (items != buffer) free(items);
    dispatch_end();
    svc_restore(&saved);
}

/* ── cbPLIError ─────────────────────────────────────────────────────────── */

/* Called whenever an error status is recorded. An error of a routine the
 * application called is queued, not reported at once: the routine that
 * failed may still hold internal state, so the records run at the next
 * safe point (the application's next VPI routine call, or its return to
 * the simulator). Errors raised by a cbPLIError routine itself are not
 * reported again. */
static void svc_error_recorded(void) {
    if (!g_svc.pli_cbs || g_svc.dispatching || g_vpi.error_level < vpiError) return;
    if (g_svc.queued == g_svc.capacity) {
        if (g_svc.capacity >= LLG_VPI_MAX_PLI_ERRORS) {
            ++g_svc.dropped;
            return;
        }
        size_t grown = g_svc.capacity ? g_svc.capacity * 2u : 4u;
        if (grown > LLG_VPI_MAX_PLI_ERRORS) grown = LLG_VPI_MAX_PLI_ERRORS;
        svc_error_t* queue = (svc_error_t*)realloc(g_svc.queue, grown * sizeof(*queue));
        if (!queue) {
            ++g_svc.dropped;
            return;
        }
        g_svc.queue = queue;
        g_svc.capacity = grown;
    }
    svc_save(&g_svc.queue[g_svc.queued++]);
}

/* Report every queued routine error and every caught signal. Safe points
 * call this; it does nothing without pending work. */
static void svc_safe_point(void) {
    if (g_svc.dispatching) return;
    if (!g_svc.queued && !g_svc_any_signal) return;
    g_svc.dispatching = 1;
    /* Records registered or removed by the routines take effect for the
     * next error; an error a routine raises is not queued (dispatching). */
    for (size_t i = 0; i < g_svc.queued; ++i) {
        svc_error_t error = g_svc.queue[i];
        svc_dispatch(cbPLIError, 0, &error);
    }
    g_svc.queued = 0;
    while (g_svc_any_signal) {
        g_svc_any_signal = 0;
        for (int sig = 1; sig < LLG_VPI_SIGNALS; ++sig) {
            if (!g_svc_signal_pending[sig]) continue;
            g_svc_signal_pending[sig] = 0;
            svc_dispatch(cbSignal, sig, NULL);
        }
    }
    g_svc.dispatching = 0;
}

/* ── cbError ────────────────────────────────────────────────────────────── */

/* Runtime hook: a run-time error or fatal error was reported. The records
 * run at once, inside the process or service that reported it (as
 * cbTchkViolation does); vpi_chk_error() gives the report as a vpiRun
 * error (38.36.3: "On a cbError callback, the routine vpi_chk_error() can
 * be called to retrieve error information"). */
static void svc_runtime_error(int fatal, const char* message, const char* location) {
    if (!g_svc.error_cbs || !g_vpi.indexed) return;
    svc_error_t status;
    memset(&status, 0, sizeof(status));
    status.pending = 1;
    status.state = vpiRun;
    status.level = vpiError;
    snprintf(status.code, sizeof(status.code), "%s",
             fatal ? "LLG_RUNTIME_FATAL" : "LLG_RUNTIME_ERROR");
    if (location)
        snprintf(status.message, sizeof(status.message), "%s: %s", location, message);
    else
        snprintf(status.message, sizeof(status.message), "%s", message);
    svc_dispatch(cbError, 0, &status);
}

/* ── cbSignal ───────────────────────────────────────────────────────────── */

static void svc_signal_handler(int sig) {
    /* Some C libraries reset the disposition before calling the handler. */
    (void)signal(sig, svc_signal_handler);
    if (sig > 0 && sig < LLG_VPI_SIGNALS) g_svc_signal_pending[sig] = 1;
    g_svc_any_signal = 1;
}

/* ── registration ───────────────────────────────────────────────────────── */

/* 38.36.3: only reason, cb_rtn and user_data matter for these reasons;
 * cbSignal also takes the signal number in index. */
static int svc_bind_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    cb->target.object = LLG_VPI_NONE;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    if (data->reason != cbSignal) return 1;
    int sig = data->index;
    if (sig <= 0 || sig >= LLG_VPI_SIGNALS) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_CALLBACK",
                       "cbSignal needs a signal number in index, not %d", (int)sig);
        return 0;
    }
    if (!g_svc.handlers[sig]) {
        void (*previous)(int) = signal(sig, svc_signal_handler);
        if (previous == SIG_ERR) {
            vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_CALLBACK",
                           "signal %d cannot be caught", (int)sig);
            return 0;
        }
        g_svc.previous[sig] = previous;
    }
    return 1;
}

static void svc_cb_added(const llg_vpi_cb_t* cb) {
    switch (cb->data.reason) {
        case cbError:
            if (!g_svc.error_cbs++) llg_rt_error_hook(svc_runtime_error);
            break;
        case cbPLIError:
            ++g_svc.pli_cbs;
            llg_time_slot_hook(vpi_time_slot);
            break;
        case cbSignal:
            ++g_svc.signal_cbs;
            ++g_svc.handlers[cb->data.index];
            llg_time_slot_hook(vpi_time_slot);
            break;
        default: break;
    }
}

static void svc_cb_removed(const llg_vpi_cb_t* cb) {
    switch (cb->data.reason) {
        case cbError:
            if (!--g_svc.error_cbs) llg_rt_error_hook(NULL);
            break;
        case cbPLIError:
            if (!--g_svc.pli_cbs) g_svc.queued = 0;
            break;
        case cbSignal: {
            int sig = cb->data.index;
            --g_svc.signal_cbs;
            if (!--g_svc.handlers[sig]) {
                /* The last record restores the handler it replaced; a
                 * signal caught before is still reported to records
                 * registered later for it. */
                (void)signal(sig, g_svc.previous[sig] ? g_svc.previous[sig] : SIG_DFL);
                g_svc.previous[sig] = NULL;
            }
            break;
        }
        default: break;
    }
}

/* ── strengths (vpiStrengthVal) ───────────────────────────────────────── */

/* The resolved net whose bits a target reads, or NULL: variables, array
 * elements, members and nets without a resolution record report strong
 * strengths (38.15: "If the object is a reg or variable, the strength will
 * always be returned as strong."). */
static llg_net_t* svc_target_net(const llg_vpi_target_t* target) {
    if (target->element != LLG_VPI_NO_ELEMENT) return NULL;
    const llg_vpi_model_object_t* object = row_object(target->object);
    if (!object || !object->net || object->packed != &object->net->resolved ||
        object->net->width != object->width)
        return NULL;
    return object->net;
}

static uint32_t svc_target_width(const llg_vpi_target_t* target) {
    if (target->span) return target->span;
    const llg_vpi_model_object_t* object = row_object(target->object);
    return object->width;
}

/* The strength code of net bit `bit` in a view (llg_net_t.strength: high
 * nibble hi + 7, low nibble lo + 7 on the scale -7 = Su0 .. 7 = Su1). */
static unsigned svc_code(sv4_t view, uint32_t bit) {
    uint64_t position = (uint64_t)bit * 8u;
    return (unsigned)(llg_sv4_word(view, (size_t)(position / 64u), LLG_SV4_BITS) >>
                      (position % 64u)) &
           0xffu;
}

/* One bit's code as s_vpi_strengthval. A known value reports its strength
 * in both s0 and s1; an unknown one (x, L, H) reports vpiX with the
 * strength of its 1 part in s0 and of its 0 part in s1, vpiHiZ for a
 * missing part, as industry-standard tools do (the text gives the fields no
 * meaning beyond "refer to strength coding in Annex K"; decision A20-D21). */
static void svc_strength_of(unsigned code, s_vpi_strengthval* out) {
    static const PLI_INT32 drive[8] = {vpiHiZ,        vpiSmallCharge, vpiMediumCharge,
                                       vpiWeakDrive,  vpiLargeCharge, vpiPullDrive,
                                       vpiStrongDrive, vpiSupplyDrive};
    int hi = (int)(code >> 4) - 7, lo = (int)(code & 15u) - 7;
    if (hi < -7 || hi > 7 || lo < -7 || lo > 7 || lo > hi) {
        out->logic = vpiX;
        out->s0 = out->s1 = vpiStrongDrive;
    } else if (lo == 0 && hi == 0) {
        out->logic = vpiZ;
        out->s0 = out->s1 = vpiHiZ;
    } else if (lo > 0) {
        out->logic = vpi1;
        out->s0 = out->s1 = drive[hi];
    } else if (hi < 0) {
        out->logic = vpi0;
        out->s0 = out->s1 = drive[-lo];
    } else {
        out->logic = vpiX;
        out->s0 = hi > 0 ? drive[hi] : vpiHiZ;
        out->s1 = lo < 0 ? drive[-lo] : vpiHiZ;
    }
}

/* Replace the strong strengths fill_value() gave a resolved net's bits by
 * the net's strengths. */
static void svc_strength_fill(const llg_vpi_target_t* target, p_vpi_value value) {
    if (value->format != vpiStrengthVal || !value->value.strength) return;
    llg_net_t* net = svc_target_net(target);
    if (!net) return;
    sv4_t view = VPI_EMPTY;
    llg_net_strength_snapshot(net, &view);
    uint32_t width = svc_target_width(target);
    for (uint32_t i = 0; i < width; ++i)
        svc_strength_of(svc_code(view, target->lsb + i), &value->value.strength[i]);
    sv4_destroy(&view);
}

/* Whether the strength codes of a strength record's bits differ from those
 * it last reported; records the new codes. */
static int svc_strength_changed(llg_vpi_cb_t* cb) {
    uint32_t width = svc_target_width(&cb->target);
    sv4_t view = VPI_EMPTY;
    llg_net_strength_snapshot(cb->strength_net, &view);
    int changed = 0;
    for (uint32_t i = 0; i < width; ++i) {
        uint8_t code = (uint8_t)svc_code(view, cb->target.lsb + i);
        if (cb->strength_last[i] != code) {
            cb->strength_last[i] = code;
            changed = 1;
        }
    }
    sv4_destroy(&view);
    return changed;
}

/* 38.36.1: "If a cbValueChange callback is registered and the format is set
 * to vpiStrengthVal, then the callback shall occur whenever the object
 * changes strength, including changes that do not result in a value
 * change." A record on a resolved net (or its bits) also watches the net's
 * strength view, attaching one when the net has none; the net then
 * publishes its strengths after every resolution, a cost only nets with
 * such a record pay. */
static int svc_strength_bind(llg_vpi_cb_t* cb) {
    llg_net_t* net = svc_target_net(&cb->target);
    if (!net) return 1;
    if (!net->strength) {
        if (g_svc.view_count == g_svc.view_capacity) {
            size_t grown = g_svc.view_capacity ? g_svc.view_capacity * 2u : 4u;
            llg_net_t** nets = (llg_net_t**)realloc(g_svc.view_nets, grown * sizeof(*nets));
            if (nets) g_svc.view_nets = nets;
            sv4_t** views = nets ? (sv4_t**)realloc(g_svc.views, grown * sizeof(*views)) : NULL;
            if (views) g_svc.views = views;
            if (!nets || !views) {
                vpi_error("LLG_VPI_NOMEM", "VPI strength view allocation failed");
                return 0;
            }
            g_svc.view_capacity = grown;
        }
        sv4_t* view = (sv4_t*)malloc(sizeof(*view));
        if (!view) {
            vpi_error("LLG_VPI_NOMEM", "VPI strength view allocation failed");
            return 0;
        }
        *view = VPI_EMPTY;
        if (!llg_net_strength_attach(net, view)) {
            free(view);
        } else {
            g_svc.view_nets[g_svc.view_count] = net;
            g_svc.views[g_svc.view_count++] = view;
        }
    }
    uint32_t width = svc_target_width(&cb->target);
    uint8_t* last = (uint8_t*)calloc(width ? width : 1u, 1);
    llg_vpi_watch_t* watches = last ? (llg_vpi_watch_t*)realloc(
                                          cb->watches, (size_t)(cb->watch_total + 1u) *
                                                           sizeof(*watches))
                                    : NULL;
    if (!watches) {
        free(last);
        vpi_error("LLG_VPI_NOMEM", "VPI value-change watch allocation failed");
        return 0;
    }
    cb->watches = watches;
    llg_vpi_watch_t* watch = &watches[cb->watch_total++];
    memset(watch, 0, sizeof(*watch));
    watch->key = net->strength;
    watch->cb = cb;
    watch->element = LLG_VPI_NO_ELEMENT;
    watch->event = LLG_VPI_WATCH_VALUE;
    cb->strength_net = net;
    cb->strength_last = last;
    (void)svc_strength_changed(cb);
    return 1;
}

/* ── delayed puts to string variables ─────────────────────────────────── */

/* A static string variable of the catalog (37.17): its storage is the
 * row's own llg_string_t. */
static int svc_string_row(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_OBJECT) return 0;
    const llg_vpi_model_object_t* object = row_object(slot->object);
    return object && (object->flags & LLG_VPI_OBJ_DYNAMIC) && object->type == vpiStringVar &&
           object->storage;
}

/* 38.34: a delayed vpi_put_value to a string variable schedules the write
 * like one to a packed variable (an Active-region event of its time slot,
 * inertial and transport delays cancelling pending puts as for vectors, an
 * optional vpiSchedEvent handle); the value is converted now. Class
 * properties and container elements still take vpiNoDelay only, and force
 * and release stay refused (decision A19-D23). */
static vpiHandle svc_put_string_delayed(llg_vpi_slot_t* slot, p_vpi_value input, p_vpi_time time,
                                        int mode, int return_event) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    if (!input) {
        vpi_error("LLG_VPI_VALUE", "null vpi_put_value value");
        return NULL;
    }
    if (!time) {
        vpi_error("LLG_VPI_TIME", "a delayed vpi_put_value needs a time");
        return NULL;
    }
    int ok;
    uint64_t ticks = ticks_from_time(time, object, &ok);
    if (!ok) {
        vpi_error("LLG_VPI_TIME", "unsupported or negative vpi_put_value delay");
        return NULL;
    }
    llg_string_t text;
    if (!dyn_string_input(input, &text)) return NULL;
    llg_vpi_target_t target;
    memset(&target, 0, sizeof(target));
    target.object = slot->object;
    target.element = LLG_VPI_NO_ELEMENT;
    llg_vpi_datum_t datum;
    memset(&datum, 0, sizeof(datum));
    datum.packed = VPI_EMPTY;
    vpiHandle event = schedule_put(&target, &datum, &text, ticks, mode, return_event);
    llg_string_destroy(&text);
    return event;
}

static void svc_shutdown(void) {
    llg_rt_error_hook(NULL);
    for (size_t i = 0; i < g_svc.view_count; ++i) {
        llg_net_strength_detach(g_svc.view_nets[i], g_svc.views[i]);
        sv4_destroy(g_svc.views[i]);
        free(g_svc.views[i]);
    }
    free(g_svc.view_nets);
    free(g_svc.views);
    free(g_svc.queue);
    memset(&g_svc, 0, sizeof(g_svc));
}
