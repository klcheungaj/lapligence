// llg_vpi_services.c — VPI simulator action callbacks (IEEE 1800-2009
// 38.36.3): cbError for run-time errors, cbPLIError for errors of VPI
// routine calls and cbSignal for operating-system signals. Embedded right
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

static void svc_shutdown(void) {
    llg_rt_error_hook(NULL);
    free(g_svc.queue);
    memset(&g_svc, 0, sizeof(g_svc));
}
