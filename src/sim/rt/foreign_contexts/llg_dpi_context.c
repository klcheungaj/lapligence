// llg_dpi_context.c — foreign call contexts for DPI-C chains whose exported
// tasks consume time (SV 35.5.1.5, 35.8, 35.9; ADV-018).
//
// Design and protocol: foreign_contexts/readme.md. A context is a pooled OS
// thread that runs one import thunk body on its own native stack. Exactly one
// participant (the scheduler thread or one context) holds the run token and
// runs; the others wait on their own condition variable. Every hand-off
// happens under `g_contexts.mutex`, which orders the token holders' accesses
// to the single-threaded runtime. Only models whose generated shims call
// llg_dpi_context_begin reference this unit, so only they need the platform
// thread library.
#include "llg_platform.h"
#include "llg_vpi.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "llg_platform_native.h"

// Reserved stack of each context. Zero-time exports run their HDL plain
// functions on it, so it matches a default main-thread stack rather than the
// smaller platform defaults for new threads (musl 128 KiB, macOS 512 KiB).
#ifndef LLG_DPI_CONTEXT_STACK_BYTES
#define LLG_DPI_CONTEXT_STACK_BYTES (8u * 1024u * 1024u)
#endif

enum {
    LLG_DPI_CONTEXT_IDLE = 0,
    LLG_DPI_CONTEXT_DONE = 1,   // the body returned
    LLG_DPI_CONTEXT_EXPORT = 2, // `request` holds an exported task frame to run
    LLG_DPI_CONTEXT_PARKED = 3  // blocked for good after a fatal violation
};

typedef struct llg_dpi_party {
    llg_cond_t wake;
    struct llg_dpi_party* resumer; // who resumed it, and gets the token back
    llg_dpi_guard_t* guards;       // its open-array guard stack while parked
    int unwinding;                 // its chain belongs to a killed process
} llg_dpi_party_t;

struct llg_dpi_context {
    llg_dpi_party_t party; // first: a running context is found from `running`
    llg_thread_t thread;
    llg_dpi_body_fn body;
    void* frame;
    llg_co_chain_t* chain;
    void* owner; // llg_proc_t* of `chain`
    llg_co_anchor_t* request;
    int event;
    int quit;
    llg_dpi_context_t* next;
};

static struct {
    int ready;
    llg_mutex_t mutex;
    llg_dpi_party_t scheduler;
    llg_dpi_party_t* token;   // the participant allowed to run
    llg_dpi_party_t* running; // equal to `token` outside a hand-off
    llg_dpi_context_t* active; // started and not ended, newest first
    llg_dpi_context_t* idle;
    // Contexts that never resume: their chain ended with the simulation, or
    // they broke the disable protocol while unwinding. Their threads stay
    // blocked until the process exits; the records stay reachable.
    llg_dpi_context_t* orphans;
} g_contexts;

static void contexts_process_end(llg_proc_t* proc, int unwind);
static void contexts_park_forever(void);
static void contexts_shutdown(void);

static void contexts_init(void) {
    if (g_contexts.ready) return;
    llg_mutex_init(&g_contexts.mutex);
    llg_cond_init(&g_contexts.scheduler.wake);
    g_contexts.token = &g_contexts.scheduler;
    g_contexts.running = &g_contexts.scheduler;
    g_contexts.ready = 1;
}

// With the mutex held by the token holder: make `to` the running
// participant. The guard stack and unwinding state follow the token.
static void contexts_switch(llg_dpi_party_t* to) {
    llg_dpi_party_t* from = g_contexts.running;
    from->guards = llg_dpi_context_switch(to->guards, to->unwinding);
    g_contexts.running = to;
    g_contexts.token = to;
    llg_cond_signal(&to->wake);
}

// With the mutex held: block `self` until the token comes back to it.
static void contexts_wait(llg_dpi_party_t* self) {
    while (g_contexts.token != self) llg_cond_wait(&self->wake, &g_contexts.mutex);
}

// Resume `context` and wait until it hands the token back.
static void contexts_resume(llg_dpi_context_t* context) {
    llg_mutex_lock(&g_contexts.mutex);
    llg_dpi_party_t* self = g_contexts.running;
    context->party.resumer = self;
    contexts_switch(&context->party);
    contexts_wait(self);
    llg_mutex_unlock(&g_contexts.mutex);
}

static LLG_THREAD_RETURN contexts_thread(void* argument) {
    llg_dpi_context_t* context = (llg_dpi_context_t*)argument;
    llg_mutex_lock(&g_contexts.mutex);
    for (;;) {
        contexts_wait(&context->party);
        if (context->quit) break;
        llg_mutex_unlock(&g_contexts.mutex);
        context->body(context->frame);
        llg_mutex_lock(&g_contexts.mutex);
        context->event = LLG_DPI_CONTEXT_DONE;
        contexts_switch(context->party.resumer);
    }
    contexts_switch(context->party.resumer);
    llg_mutex_unlock(&g_contexts.mutex);
    return LLG_THREAD_RESULT;
}

static void contexts_unlink(llg_dpi_context_t** list, llg_dpi_context_t* context) {
    for (; *list; list = &(*list)->next) {
        if (*list == context) {
            *list = context->next;
            context->next = NULL;
            return;
        }
    }
}

llg_dpi_context_t* llg_dpi_context_begin(llg_co_chain_t* chain, llg_dpi_body_fn body,
                                         void* frame) {
    if (!g_contexts.ready) {
        contexts_init();
        llg_dpi_context_hooks(contexts_park_forever, contexts_shutdown);
        llg_rt_set_process_end_hook(contexts_process_end);
    }
    llg_dpi_context_t* context = g_contexts.idle;
    if (context) {
        g_contexts.idle = context->next;
    } else {
        context = (llg_dpi_context_t*)calloc(1, sizeof(*context));
        if (!context) return NULL;
        llg_cond_init(&context->party.wake);
        char error[128];
        if (llg_thread_start_sized(&context->thread, contexts_thread, context,
                                   LLG_DPI_CONTEXT_STACK_BYTES, error, sizeof(error)) != 0) {
            fprintf(stderr, "llg: cannot start a DPI-C call context thread: %s\n", error);
            llg_cond_destroy(&context->party.wake);
            free(context);
            return NULL;
        }
    }
    context->body = body;
    context->frame = frame;
    context->chain = chain;
    context->owner = chain->owner;
    context->request = NULL;
    context->event = LLG_DPI_CONTEXT_IDLE;
    context->party.guards = NULL;
    context->party.unwinding = 0;
    context->next = g_contexts.active;
    g_contexts.active = context;
    return context;
}

llg_co_anchor_t* llg_dpi_context_run(llg_dpi_context_t* context) {
    contexts_resume(context);
    if (context->event != LLG_DPI_CONTEXT_EXPORT) return NULL;
    context->event = LLG_DPI_CONTEXT_IDLE;
    return context->request;
}

void llg_dpi_context_end(llg_dpi_context_t* context) {
    contexts_unlink(&g_contexts.active, context);
    if (context->event == LLG_DPI_CONTEXT_PARKED) {
        context->next = g_contexts.orphans;
        g_contexts.orphans = context;
        return;
    }
    context->event = LLG_DPI_CONTEXT_IDLE;
    context->next = g_contexts.idle;
    g_contexts.idle = context;
}

llg_co_anchor_t* llg_dpi_export_begin(const llg_co_desc_t* desc, const char* c_name) {
    if (!g_contexts.ready || g_contexts.running == &g_contexts.scheduler) {
        // No context below: the caller is an imported function, or an
        // import that runs without a context (35.8: an imported function
        // may never enable an exported task).
        if (!llg_rt_exiting()) {
            fprintf(stderr,
                    "llg: DPI-C exported task `%s` can consume time, but its foreign "
                    "caller cannot suspend: only a context imported task may call it "
                    "(SV 35.8)\n",
                    c_name);
            llg_rt_mark_failed();
            llg_rt_fatal_typed(0, "DPI-C protocol violation", NULL, 0, "", c_name);
        }
        return NULL;
    }
    llg_dpi_context_t* context = (llg_dpi_context_t*)g_contexts.running;
    llg_co_chain_t* ch = context->chain;
    llg_co_anchor_t* slot;
    LLG_CO_ARENA_ENTER(ch, desc, slot);
    slot->desc = desc;
    return slot;
}

int llg_dpi_export_wait(llg_co_anchor_t* slot) {
    llg_mutex_lock(&g_contexts.mutex);
    llg_dpi_context_t* context = (llg_dpi_context_t*)g_contexts.running;
    context->request = slot;
    context->event = LLG_DPI_CONTEXT_EXPORT;
    contexts_switch(context->party.resumer);
    contexts_wait(&context->party);
    llg_mutex_unlock(&g_contexts.mutex);
    return context->party.unwinding;
}

// Called through llg_dpi_park_hook by the running context: report nothing
// more, hand the token back and never run again.
static void contexts_park_forever(void) {
    llg_mutex_lock(&g_contexts.mutex);
    llg_dpi_context_t* context = (llg_dpi_context_t*)g_contexts.running;
    context->event = LLG_DPI_CONTEXT_PARKED;
    contexts_switch(context->party.resumer);
    for (;;) llg_cond_wait(&context->party.wake, &g_contexts.mutex);
}

// The scheduler ends `proc`. Its contexts are blocked in exported task calls;
// the newest is the innermost of the chain and goes first.
static void contexts_process_end(llg_proc_t* proc, int unwind) {
    for (;;) {
        llg_dpi_context_t* context = g_contexts.active;
        while (context && (context->owner != (void*)proc ||
                           &context->party == g_contexts.running))
            context = context->next;
        if (!context) return;
        contexts_unlink(&g_contexts.active, context);
        if (unwind) {
            // 35.9: the blocked export returns 1 and the foreign code must
            // return 1 without calling another export.
            llg_dpi_call_t saved;
            llg_dpi_chain_snapshot(&saved);
            context->party.unwinding = 1;
            contexts_resume(context);
            llg_dpi_import_abort(&saved);
            if (context->event == LLG_DPI_CONTEXT_DONE) {
                context->event = LLG_DPI_CONTEXT_IDLE;
                context->party.unwinding = 0;
                context->next = g_contexts.idle;
                g_contexts.idle = context;
                continue;
            }
        }
        context->next = g_contexts.orphans;
        g_contexts.orphans = context;
    }
}

// Model shutdown: chains still blocked never resume; idle threads end.
static void contexts_shutdown(void) {
    while (g_contexts.active) {
        llg_dpi_context_t* context = g_contexts.active;
        g_contexts.active = context->next;
        context->next = g_contexts.orphans;
        g_contexts.orphans = context;
    }
    while (g_contexts.idle) {
        llg_dpi_context_t* context = g_contexts.idle;
        g_contexts.idle = context->next;
        context->quit = 1;
        contexts_resume(context);
        llg_thread_join(context->thread);
        llg_cond_destroy(&context->party.wake);
        free(context);
    }
}
