// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with vendor/libaco (aco.c + acosw.S) and the generated
// model.c by the host C compiler; never linked into the Rust binaries.

#define _GNU_SOURCE

#include "llg_rt.h"
#include "aco.h"
#ifdef LLG_WAVEFORM
#include "llg_wave.h"
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>

// Generated budgets include function frames through the recursion guard.
#ifndef LLG_MODEL_STACK_VALUES
#define LLG_MODEL_STACK_VALUES 256u
#endif

// ── Fatal boundary checks ────────────────────────────────────────────────────

static void llg_fatal_allocation(const char* what, size_t count, size_t size) {
    fprintf(stderr,
            "llg: fatal: cannot allocate %zu element(s) of %zu byte(s) for %s\n",
            count, size, what);
    abort();
}

static void* llg_checked_malloc(size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size)
        llg_fatal_allocation(what, count, size);
    size_t bytes = count * size;
    void* ptr = malloc(bytes == 0 ? 1 : bytes);
    if (!ptr) llg_fatal_allocation(what, count, size);
    return ptr;
}

static void* llg_checked_calloc(size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size)
        llg_fatal_allocation(what, count, size);
    // Keep zero-sized requests non-null so callers never depend on a
    // platform-specific malloc(0)/calloc(0) result.
    if (count == 0 || size == 0) count = size = 1;
    void* ptr = calloc(count, size);
    if (!ptr) llg_fatal_allocation(what, count, size);
    return ptr;
}

static size_t llg_coroutine_stack_size(void) {
    const size_t base = 4u << 20;
    if (LLG_MODEL_STACK_VALUES > (SIZE_MAX - base) / sizeof(sv4_t))
        llg_fatal_allocation("coroutine stack", LLG_MODEL_STACK_VALUES, sizeof(sv4_t));
    return base + (size_t)LLG_MODEL_STACK_VALUES * sizeof(sv4_t);
}

static int llg_sv4_nlimbs(uint32_t width) {
    return width == 0 ? 0 : (int)((width + 63u) / 64u);
}

static uint64_t llg_sv4_limb_mask(uint32_t width, int index) {
    int limbs = llg_sv4_nlimbs(width);
    if (index < 0 || index >= limbs) return 0;
    if (index == limbs - 1 && (width % 64) != 0)
        return LLG_MASK((uint32_t)(width % 64));
    return ~0ULL;
}

static void llg_append(char* buf, size_t cap, size_t* len, char c) {
    if (*len + 1 < cap) buf[(*len)++] = c;
}

// ── Scheduler state ───────────────────────────────────────────────────────────

typedef enum {
    W_NONE,
    W_TIME,
    W_EVENTS,
    W_EVENT, // waiting on one or more named events
    W_MIXED, // atomic named-event + signal or-list (@(posedge a or ev))
    W_LEVEL,
    W_FORK,    // llg_join: waiting for a fork group
    W_FORK_ALL // llg_wait_fork: waiting for all of the current proc's groups
} llg_wait_kind_t;

typedef struct llg_nba {
    struct llg_nba* next;
    sv4_t* target;
    sv4_t value;
    int is_real;
    double* real_target;
    double real_value;
} llg_nba_t;

typedef struct llg_wait {
    struct llg_wait* next;         // all active waits (signal + timed + inactive)
    struct llg_wait* time_next;    // sorted timed list
    struct llg_wait* inactive_next; // inactive list (#0 waiters)
    llg_proc_t* proc;
    llg_wait_kind_t kind;
    uint64_t time;                // W_TIME
    llg_event_spec_t* specs;     // W_EVENTS: copied array; W_MIXED: signal half
    sv4_t* last;                  // W_EVENTS/W_MIXED: last-seen values
    int n;                        // W_EVENTS/W_MIXED (signal entry count)
    const llg_event_t** evs;     // W_EVENT/W_MIXED: copied event list
    int n_evs;                    // W_EVENT/W_MIXED
    sv4_t* sig;                   // W_LEVEL
    sv4_t level_val;              // W_LEVEL
    llg_fork_group_t* grp;       // W_FORK: group being joined
    llg_proc_t* parent;          // W_FORK_ALL: the waiting proc itself
} llg_wait_t;

typedef struct llg_fork_child {
    struct llg_fork_child* next;
    llg_proc_t* proc;            // NULL once freed by disable_fork
} llg_fork_child_t;

struct llg_fork_group {
    int join_kind;                // LLG_JOIN / LLG_JOIN_NONE / LLG_JOIN_ANY
    int remaining;                // live children; decremented on done AND on kill
    int resumed;                  // join_any: parent already woken
    llg_proc_t* parent;          // spawning proc
    llg_fork_child_t* children;  // for disable_fork
    struct llg_fork_group* next_g; // per-proc live-group list (or zombie list)
};

struct llg_proc {
    aco_t* co;
    const char* name;
    void (*fn)(llg_proc_t*);
    llg_nba_t* nba_head;
    llg_nba_t* nba_tail;
    llg_wait_t wait;
    llg_proc_t* next_ready;
    llg_fork_group_t* fork_groups; // live groups spawned by this proc
    llg_fork_group_t* grp;         // group this proc belongs to (NULL top-level)
};

#define LLG_ZERO_LOOP_LIMIT 10000000ULL

// ── $monitor / $strobe state ──────────────────────────────────────────────────

typedef struct {
    int active;          // a monitor is registered
    int enabled;         // $monitoron / $monitoroff
    char* fmt;           // strdup'd format string
    int n;               // number of displayed arguments
    llg_mon_eval_fn eval;
    sv4_t* last;         // last-printed argument values (n)
    sv4_t* work;         // scratch buffer the eval fn fills (n)
} llg_monitor_state_t;

typedef struct llg_strobe {
    struct llg_strobe* next;
    char* fmt;           // strdup'd format string
    int n;
    llg_mon_eval_fn eval;
    sv4_t* work;         // scratch buffer the eval fn fills (n)
} llg_strobe_t;

typedef struct {
    aco_t* main_co;
    aco_share_stack_t* share_stack;
    llg_proc_t* ready_head;
    llg_proc_t* ready_tail;
    llg_wait_t* timed_head;   // sorted ascending by time
    llg_wait_t* inactive_head; // #0 waiters at the current time (FIFO)
    llg_wait_t* inactive_tail;
    llg_wait_t* waiters;      // all active waits
    int wait_count;
    uint64_t now;
    int finish;
    uint64_t region_passes;    // zero-delay guard units in the current time step (region passes + coroutine resumes)
    llg_proc_t* all_procs[LLG_MAX_PROCS];
    int n_procs;
    llg_fork_group_t* zombie_groups; // completed/killed groups awaiting teardown
    llg_monitor_state_t mon;   // the active $monitor (at most one)
    llg_strobe_t* strobes;     // pending $strobe lines for this time step
    // Active procedural forces: signal pointer -> pre-force saved value.  A
    // second `force` on an already forced signal updates the forced value in
    // place but keeps `saved`; `release` removes the entry and writes `saved`
    // back through `sig_write`.
    struct { sv4_t* sig; sv4_t saved; } force_table[LLG_MAX_FORCE];
    int force_count;
} llg_rt_ctx_t;

static llg_rt_ctx_t g;

// Registered final-block processes (see llg_rt.h).  Kept OUTSIDE the runtime
// context: `llg_rt_cleanup` memsets the context, and registration happens
// around the `llg_rt_run()` call in generated `main()`.
static struct {
    void (*fn)(llg_proc_t*);
    const char* name;
} llg_finals[LLG_MAX_FINALS];
static int llg_n_finals;
static int llg_in_finals;
// Scheduler time when `llg_rt_run` exited; `$time` inside finals reports it.
static uint64_t llg_final_time;

static void register_proc(llg_proc_t* p) {
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == NULL) {
            g.all_procs[i] = p;
            return;
        }
    }
    if (g.n_procs >= LLG_MAX_PROCS) {
        fprintf(stderr, "llg: too many processes (limit %d)\n", LLG_MAX_PROCS);
        abort();
    }
    g.all_procs[g.n_procs++] = p;
}

static void unregister_proc(llg_proc_t* p) {
    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i] == p) {
            g.all_procs[i] = NULL;
            while (g.n_procs > 0 && g.all_procs[g.n_procs - 1] == NULL)
                g.n_procs--;
            return;
        }
    }
}

static llg_proc_t* llg_current(void) {
    return (llg_proc_t*)aco_get_arg();
}

static void enqueue_ready(llg_proc_t* p) {
    p->next_ready = NULL;
    if (g.ready_tail) {
        g.ready_tail->next_ready = p;
        g.ready_tail = p;
    } else {
        g.ready_head = g.ready_tail = p;
    }
}

static void remove_waiters_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.waiters;
    while (*pp) {
        if (*pp == w) {
            *pp = w->next;
            return;
        }
        pp = &(*pp)->next;
    }
}

static void remove_timed_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->time_next;
            return;
        }
        pp = &(*pp)->time_next;
    }
}

static void insert_inactive(llg_wait_t* w) {
    w->inactive_next = NULL;
    if (g.inactive_tail) {
        g.inactive_tail->inactive_next = w;
    } else {
        g.inactive_head = w;
    }
    g.inactive_tail = w;
}

static void remove_inactive_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.inactive_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->inactive_next;
            if (g.inactive_tail == w) {
                g.inactive_tail = NULL;
                for (llg_wait_t* q = g.inactive_head; q; q = q->inactive_next)
                    g.inactive_tail = q;
            }
            w->inactive_next = NULL;
            return;
        }
        pp = &(*pp)->inactive_next;
    }
}

static void remove_ready_entry(llg_proc_t* p) {
    llg_proc_t** pp = &g.ready_head;
    while (*pp) {
        if (*pp == p) {
            *pp = p->next_ready;
            if (g.ready_tail == p) {
                g.ready_tail = NULL;
                for (llg_proc_t* q = g.ready_head; q; q = q->next_ready)
                    g.ready_tail = q;
            }
            p->next_ready = NULL;
            return;
        }
        pp = &(*pp)->next_ready;
    }
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on; defined below with the other named-event helpers.
static void event_unlink(llg_wait_t* w);

static void insert_timed(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp && (*pp)->time <= w->time) pp = &(*pp)->time_next;
    w->time_next = *pp;
    *pp = w;
}

// Wake a suspended process: clear its wait node and schedule it.
static void wake_proc(llg_proc_t* p) {
    llg_wait_t* w = &p->wait;
    if (w->kind == W_NONE) return;
    remove_waiters_entry(w);
    if (w->kind == W_TIME) {
        remove_timed_entry(w);
        remove_inactive_entry(w);
    }
    if (w->kind == W_EVENT || w->kind == W_MIXED) {
        event_unlink(w);
    }
    free(w->specs);
    free(w->last);
    free(w->evs);
    w->specs = NULL;
    w->last = NULL;
    w->evs = NULL;
    w->n = 0;
    w->n_evs = 0;
    w->kind = W_NONE;
    g.wait_count--;
    enqueue_ready(p);
}

static void register_wait(void) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->proc = p;
    w->next = g.waiters;
    g.waiters = w;
    g.wait_count++;
}

// ── Named events ──────────────────────────────────────────────────────────────

// Register `p` on `ev`'s waiter table (fixed capacity, like the other
// runtime resource limits).
static void event_list_add(llg_event_t* ev, llg_proc_t* p) {
    if (ev->n_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->waiters[ev->n_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    for (int i = 0; i < w->n_evs; i++) {
        llg_event_t* ev = (llg_event_t*)w->evs[i];
        for (int k = 0; k < ev->n_waiters; k++) {
            if (ev->waiters[k] == w->proc) {
                ev->waiters[k] = ev->waiters[ev->n_waiters - 1];
                ev->n_waiters--;
                break;
            }
        }
    }
}

// ── fork/join (coroutine children) ────────────────────────────────────────────

static void llg_kill_proc_tree(llg_proc_t* p); // mutual recursion below
static void llg_proc_entry(void);               // defined in the public API section

// Unlink a suspended or ready proc from every scheduler queue, free its
// pending NBA list, destroy its coroutine and free the proc struct.  The proc
// must not be running (children are suspended or ready while the parent
// executes disable_fork).
static void llg_kill_proc(llg_proc_t* p) {
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* nx = n->next;
        free(n);
        n = nx;
    }
    p->nba_head = p->nba_tail = NULL;

    llg_wait_t* w = &p->wait;
    if (w->kind != W_NONE) {
        remove_waiters_entry(w);
        if (w->kind == W_TIME) {
            remove_timed_entry(w);
            remove_inactive_entry(w);
        }
        if (w->kind == W_EVENT || w->kind == W_MIXED) {
            event_unlink(w);
        }
        free(w->specs);
        free(w->last);
        free(w->evs);
        w->specs = NULL;
        w->last = NULL;
        w->evs = NULL;
        w->n = 0;
        w->n_evs = 0;
        w->kind = W_NONE;
        g.wait_count--;
    }
    remove_ready_entry(p);

    aco_destroy(p->co);
    unregister_proc(p);
    free(p);
}

// Kill every group spawned by `p`: each child (and its descendants) is freed
// recursively, the group structs go onto the zombie list for teardown.  `p`
// itself is untouched — used by disable_fork, which kills only descendants.
static void llg_kill_proc_groups(llg_proc_t* p) {
    llg_fork_group_t* grp = p->fork_groups;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            if (c->proc) {
                llg_kill_proc_tree(c->proc);
                c->proc = NULL; // freed inline; teardown skips it
            }
            c = next_c;
        }
        grp->remaining = 0;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        grp = next_g;
    }
    p->fork_groups = NULL;
}

// Kill `p` and all of its descendants.
static void llg_kill_proc_tree(llg_proc_t* p) {
    llg_kill_proc_groups(p);
    llg_kill_proc(p);
}

// One child of `grp` finished (llg_proc_done).  Decrement the live count,
// wake a join/wait_fork waiter whose condition is now met, and move the group
// to the zombie list once the last child is done.
static void llg_fork_group_child_done(llg_fork_group_t* grp) {
    llg_proc_t* parent = grp->parent;
    grp->remaining--;
    int wake = 0;
    if (grp->join_kind == LLG_JOIN) {
        if (grp->remaining == 0) wake = 1;
    } else if (grp->join_kind == LLG_JOIN_ANY) {
        if (!grp->resumed) {
            grp->resumed = 1;
            wake = 1;
        }
    }
    if (wake && parent->wait.kind == W_FORK && parent->wait.grp == grp) {
        wake_proc(parent);
    }
    if (grp->remaining == 0) {
        // Unlink from the parent's live-group list; the group and its child
        // list are freed by process_zombie_groups at the next safe point.
        // join_any / join_none groups stay live until the last child finishes
        // so wait_fork still works.
        llg_fork_group_t** pp = &parent->fork_groups;
        while (*pp && *pp != grp) pp = &(*pp)->next_g;
        if (*pp) *pp = grp->next_g;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        // Wake any wait_fork waiter whose own groups are now all done.
        llg_wait_t* w = g.waiters;
        while (w) {
            llg_wait_t* next = w->next;
            if (w->kind == W_FORK_ALL && w->parent->fork_groups == NULL) {
                wake_proc(w->proc);
            }
            w = next;
        }
    }
}

llg_fork_group_t* llg_fork_group_new(int join_kind) {
    llg_proc_t* parent = llg_current();
    llg_fork_group_t* grp = (llg_fork_group_t*)llg_checked_calloc(
        1, sizeof(llg_fork_group_t), "fork group");
    grp->join_kind = join_kind;
    grp->parent = parent;
    grp->next_g = parent->fork_groups;
    parent->fork_groups = grp;
    return grp;
}

llg_proc_t* llg_fork(void (*fn)(llg_proc_t*), const char* name, llg_fork_group_t* grp) {
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(llg_proc_t), "forked process");
    p->name = name;
    p->fn = fn;
    p->grp = grp;
    // aco_create from inside a coroutine is safe (mallocs/zeroes an aco_t and
    // sets registers only; no global state).  Children yield to g.main_co, the
    // scheduler, so aco_resume in llg_rt_run regains control.
    p->co = aco_create(g.main_co, g.share_stack, 256u << 10, llg_proc_entry, p);
    grp->remaining++;
    llg_fork_child_t** pp = &grp->children;
    while (*pp) pp = &(*pp)->next;
    llg_fork_child_t* c = (llg_fork_child_t*)llg_checked_malloc(
        1, sizeof(llg_fork_child_t), "fork child");
    c->proc = p;
    c->next = NULL;
    *pp = c;
    register_proc(p);
    enqueue_ready(p);
    return p;
}

void llg_join(llg_fork_group_t* grp) {
    if (grp->remaining == 0) {
        // Empty fork groups never receive a child-done callback, so finalize
        // them here before join or wait_fork can observe a permanently live
        // group. The parent is the currently running process.
        llg_fork_group_t** pp = &grp->parent->fork_groups;
        while (*pp && *pp != grp) pp = &(*pp)->next_g;
        if (*pp) *pp = grp->next_g;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        return;
    }
    if (grp->join_kind == LLG_JOIN_NONE) return;
    if (grp->join_kind == LLG_JOIN_ANY && grp->resumed) return; // already met
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK;
    w->grp = grp;
    register_wait();
    aco_yield();
}

void llg_wait_fork(void) {
    llg_proc_t* p = llg_current();
    if (p->fork_groups == NULL) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK_ALL;
    w->parent = p;
    register_wait();
    aco_yield();
}

void llg_disable_fork(void) {
    llg_kill_proc_groups(llg_current());
}

// Free completed/killed fork groups: their child procs that were not already
// freed by disable_fork, the child list nodes and the group struct.  Called
// from llg_rt_run after commit_nbas with no coroutine running, so done
// children's NBAs have been committed and their all_procs slots can be NULLed
// safely (the next commit_nbas then skips them).
static void process_zombie_groups(void) {
    llg_fork_group_t* grp = g.zombie_groups;
    g.zombie_groups = NULL;
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        int deferred = 0;
        for (llg_fork_child_t* c = grp->children; c; c = c->next) {
            if (!c->proc) continue;
            // A completed child can still own live join_none descendants.
            // Keep its process object until those groups unlink themselves;
            // their completion path dereferences the parent pointer.
            if (c->proc->fork_groups) {
                deferred = 1;
                continue;
            }
            aco_destroy(c->proc->co);
            unregister_proc(c->proc);
            free(c->proc);
            c->proc = NULL;
        }
        if (deferred) {
            grp->next_g = g.zombie_groups;
            g.zombie_groups = grp;
        } else {
            llg_fork_child_t* c = grp->children;
            while (c) {
                llg_fork_child_t* next_c = c->next;
                free(c);
                c = next_c;
            }
            free(grp);
        }
        grp = next_g;
    }
}

// ── Signal writes and waiter scanning ─────────────────────────────────────────

static int sv4_is_zero(sv4_t v) { return !sv4_is_unknown(v) && !sv4_to_bool(v); }
static int sv4_is_one(sv4_t v) { return !sv4_is_unknown(v) && sv4_to_bool(v); }

static int ev_matches(sv4_t old, sv4_t new, int kind) {
    if (kind == LLG_EV_ANY) return !sv4_same(old, new);
    if (kind == LLG_EV_POSEDGE) {
        return (sv4_is_zero(old) && (!sv4_is_zero(new))) ||
               (sv4_is_unknown(old) && sv4_is_one(new));
    }
    // negedge
    return (sv4_is_one(old) && (!sv4_is_one(new))) ||
           (sv4_is_unknown(old) && sv4_is_zero(new));
}

static void sig_write(sv4_t* target, sv4_t value) {
    // Mask the written limbs to the vector's width before comparing/storing.
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
        uint64_t m = llg_sv4_limb_mask(value.width, i);
        value.bits[i] &= m;
        value.x[i] &= m;
        value.z[i] &= m;
    }
    if (target->width == value.width && sv4_same(*target, value)) return;
    *target = value;
#ifdef LLG_WAVEFORM
    llg_wave_changed_sv4(target, &value, g.now);
#endif
    llg_wait_t* w = g.waiters;
    while (w) {
        llg_wait_t* next = w->next;
        int wake = 0;
        if (w->kind == W_EVENTS || w->kind == W_MIXED) {
            for (int i = 0; i < w->n; i++) {
                if (w->specs[i].sig == target) {
                    sv4_t old = w->last[i];
                    w->last[i] = *target;
                    if (ev_matches(old, *target, w->specs[i].kind)) wake = 1;
                }
            }
        } else if (w->kind == W_LEVEL) {
            if (w->sig == target && sv4_same(*target, w->level_val)) wake = 1;
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
}

// Real equality is bitwise: repeated NaNs with the same payload are
// suppressed, while changes in NaN payload and signed zero are observable.
static void real_write(double* target, double value) {
    uint64_t old_bits;
    uint64_t new_bits;
    memcpy(&old_bits, target, sizeof(old_bits));
    memcpy(&new_bits, &value, sizeof(new_bits));
    if (old_bits == new_bits) return;
    *target = value;
#ifdef LLG_WAVEFORM
    llg_wave_changed_real(target, value, g.now);
#endif
}

// ── Procedural force / release ───────────────────────────────────────────────

// Is `sig` currently forced?  Procedural writes (llg_ba and NBA commits) are
// dropped while a signal is forced; net resolution and monitor reads are not
// affected.
static int llg_is_forced(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++)
        if (g.force_table[i].sig == sig) return 1;
    return 0;
}

void llg_force(sv4_t* sig, sv4_t value) {
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].sig == sig) {
            // Re-force: update the forced value, keep the ORIGINAL saved value.
            sig_write(sig, value);
            return;
        }
    }
    if (g.force_count >= LLG_MAX_FORCE) {
        fprintf(stderr, "llg: too many forced signals (limit %d)\n", LLG_MAX_FORCE);
        abort();
    }
    g.force_table[g.force_count].sig = sig;
    g.force_table[g.force_count].saved = *sig;
    g.force_count++;
    sig_write(sig, value);
}

void llg_release(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].sig == sig) {
            sv4_t saved = g.force_table[i].saved;
            // Swap-with-last keeps the table compact; the restored value is
            // written through sig_write so waiters wake on the change.
            g.force_count--;
            g.force_table[i] = g.force_table[g.force_count];
            sig_write(sig, saved);
            return;
        }
    }
    // Releasing an unforced signal is a no-op (LRM 10.6.2).
}

// ── Public scheduler API ──────────────────────────────────────────────────────

static void llg_last_word(void) {
    fprintf(stderr, "llg: fatal: coroutine returned without aco_exit "
                    "(codegen bug)\n");
    abort();
}

static void llg_proc_entry(void) {
    llg_proc_t* self = (llg_proc_t*)aco_get_arg();
    self->fn(self);
    llg_last_word(); // never reached when the body called llg_proc_done
}

static void free_group_storage(llg_fork_group_t* grp) {
    while (grp) {
        llg_fork_group_t* next_g = grp->next_g;
        llg_fork_child_t* c = grp->children;
        while (c) {
            llg_fork_child_t* next_c = c->next;
            free(c);
            c = next_c;
        }
        free(grp);
        grp = next_g;
    }
}

static void free_proc_storage(llg_proc_t* p) {
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* next = n->next;
        free(n);
        n = next;
    }
    free(p->wait.specs);
    free(p->wait.last);
    free(p->wait.evs);
    if (p->co) aco_destroy(p->co);
    free(p);
}

void llg_rt_cleanup(void) {
    // Groups own only child-list nodes; process objects are owned once by
    // all_procs and are released separately below.
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (p && p->fork_groups) {
            free_group_storage(p->fork_groups);
            p->fork_groups = NULL;
        }
    }
    free_group_storage(g.zombie_groups);
    g.zombie_groups = NULL;

    while (g.strobes) {
        llg_strobe_t* next = g.strobes->next;
        free(g.strobes->fmt);
        free(g.strobes->work);
        free(g.strobes);
        g.strobes = next;
    }
    free(g.mon.fmt);
    free(g.mon.last);
    free(g.mon.work);

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    if (g.share_stack) aco_share_stack_destroy(g.share_stack);
    if (g.main_co) aco_destroy(g.main_co);
    memset(&g, 0, sizeof(g));
}

void llg_rt_init(void) {
    llg_rt_cleanup();
    llg_n_finals = 0; // a fresh run never inherits final registrations
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
}

void llg_rt_finish(void) {
    g.finish = 1;
    if (llg_in_finals) llg_proc_done(llg_current());
}

uint64_t llg_time(void) { return g.now; }

uint64_t llg_time_scaled(uint64_t precision_ps, uint64_t unit_ps) {
    if (unit_ps == 0) {
        fprintf(stderr, "llg runtime fatal: zero time unit\n");
        abort();
    }
#if defined(__SIZEOF_INT128__)
    __uint128_t scaled = (__uint128_t)g.now * precision_ps / unit_ps;
    if (scaled > UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    return (uint64_t)scaled;
#else
    if (precision_ps != 0 && g.now > UINT64_MAX / precision_ps) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    return g.now * precision_ps / unit_ps;
#endif
}

int llg_rt_process_count(void) {
    int count = 0;
    for (int i = 0; i < g.n_procs; i++)
        if (g.all_procs[i]) count++;
    return count;
}

llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name) {
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(llg_proc_t), "process");
    p->name = name;
    p->fn = fn;
    p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
    register_proc(p);
    enqueue_ready(p);
    return p;
}

void llg_proc_done(llg_proc_t* self) {
    if (self->grp) llg_fork_group_child_done(self->grp);
    aco_exit(); // never returns
}

void llg_wait_time(uint64_t ticks) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_TIME;
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr,
                "llg: fatal: simulation time overflow at %llu while scheduling a delay of %llu tick(s)\n",
                (unsigned long long)g.now, (unsigned long long)ticks);
        abort();
    }
    w->time = g.now + ticks;
    if (ticks == 0) {
        // `#0` yields into the INACTIVE region of the current time step
        // (LRM §4.4.2): it runs after the active region drains and before
        // the NBA region commits.
        insert_inactive(w);
    } else {
        insert_timed(w);
    }
    register_wait();
    aco_yield();
}

void llg_wait_any(sv4_t** sigs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->n = n;
    w->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "event wait specifications");
    w->last = (sv4_t*)llg_checked_malloc(
        (size_t)n, sizeof(sv4_t), "event wait snapshots");
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = sigs[i];
        w->specs[i].kind = LLG_EV_ANY;
        w->last[i] = *sigs[i];
    }
    register_wait();
    aco_yield();
}

void llg_wait_any_events(llg_event_spec_t* specs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->n = n;
    w->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "edge wait specifications");
    w->last = (sv4_t*)llg_checked_malloc(
        (size_t)n, sizeof(sv4_t), "edge wait snapshots");
    for (int i = 0; i < n; i++) {
        w->specs[i].sig = specs[i].sig;
        w->specs[i].kind = specs[i].kind;
        w->last[i] = *specs[i].sig;
    }
    register_wait();
    aco_yield();
}

void llg_wait_edge(sv4_t* sig, int posedge) {
    llg_event_spec_t spec;
    spec.sig = sig;
    spec.kind = posedge ? LLG_EV_POSEDGE : LLG_EV_NEGEDGE;
    llg_wait_any_events(&spec, 1);
}

void llg_wait_level(sv4_t* sig, sv4_t value) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_LEVEL;
    w->sig = sig;
    w->level_val = value;
    register_wait();
    aco_yield();
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────

void llg_event_trigger(llg_event_t* ev) {
    int n = ev->n_waiters;
    if (n == 0) return;
    // Snapshot and detach everyone first: wake_proc unlinks the waiter from
    // every event list it registered on, which must not fight the iteration
    // over this event's own table.  Wake order is the snapshot order, i.e.
    // the current table order: deterministic, and equal to registration
    // order unless earlier partial unlinks (swap-with-last) reordered it.
    llg_proc_t* wake[LLG_MAX_EVENT_WAITERS];
    memcpy(wake, ev->waiters, (size_t)n * sizeof(llg_proc_t*));
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        wake_proc(wake[i]);
    }
}

void llg_wait_event(llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    llg_wait_events(list, 1);
}

void llg_wait_events(const llg_event_t* const* evs, int n) {
    if (n <= 0) return;
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT;
    w->n_evs = n;
    w->evs = (const llg_event_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_t*), "named-event wait list");
    memcpy(w->evs, evs, (size_t)n * sizeof(llg_event_t*));
    for (int i = 0; i < n; i++) {
        // The lists are owned by the generated model's non-const globals.
        event_list_add((llg_event_t*)w->evs[i], p);
    }
    register_wait();
    aco_yield();
}

void llg_wait_mixed(llg_wait_src_t* srcs, int n) {
    llg_proc_t* p = llg_current();
    llg_wait_t* w = &p->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->n = nsig;
    w->specs = nsig ? (llg_event_spec_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(llg_event_spec_t), "mixed wait specifications") : NULL;
    w->last = nsig ? (sv4_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(sv4_t), "mixed wait snapshots") : NULL;
    w->n_evs = nev;
    w->evs = nev ? (const llg_event_t**)llg_checked_malloc(
        (size_t)nev, sizeof(llg_event_t*), "mixed named-event wait list") : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            w->specs[si].sig = srcs[i].sig;
            w->specs[si].kind = srcs[i].kind;
            w->last[si] = *srcs[i].sig;
            si++;
        } else {
            w->evs[ei++] = srcs[i].ev;
            event_list_add((llg_event_t*)srcs[i].ev, p);
        }
    }
    register_wait();
    aco_yield();
}

void llg_nba(sv4_t* target, sv4_t value) {
    llg_proc_t* p = llg_current();
    llg_nba_t* n = (llg_nba_t*)llg_checked_malloc(
        1, sizeof(llg_nba_t), "nonblocking assignment");
    n->target = target;
    n->value = value;
    n->is_real = 0;
    n->real_target = NULL;
    n->real_value = 0.0;
    n->next = NULL;
    if (p->nba_tail) p->nba_tail->next = n;
    else p->nba_head = n;
    p->nba_tail = n;
}

void llg_ba(sv4_t* target, sv4_t value) {
    // Procedural blocking writes to a forced signal are ignored (LRM 10.6.2).
    if (llg_is_forced(target)) return;
    sig_write(target, value);
}

void llg_nba_d(double* target, double value) {
    llg_proc_t* p = llg_current();
    llg_nba_t* n = (llg_nba_t*)llg_checked_malloc(
        1, sizeof(llg_nba_t), "real nonblocking assignment");
    n->target = NULL;
    n->value = sv4_x(0, 0);
    n->is_real = 1;
    n->real_target = target;
    n->real_value = value;
    n->next = NULL;
    if (p->nba_tail) p->nba_tail->next = n;
    else p->nba_head = n;
    p->nba_tail = n;
}

void llg_ba_d(double* target, double value) {
    real_write(target, value);
}

// ── Collapsed inout nets ──────────────────────────────────────────────────────

static sv4_t llg_net_compute(const llg_net_t* net) {
    return sv4_resolve_strengths(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->n_drivers, net->width, net->is_signed, net->resolution);
}

void llg_net_resolve(llg_net_t* net) {
    net->resolved = llg_net_compute(net);
}

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    if (idx < 0 || idx >= net->n_drivers) return;
    value = sv4_resize(value, net->width, net->is_signed);
    sv4_t* slot = net->drivers[idx];
    if (!slot) return;
    if (slot->width == value.width && sv4_same(*slot, value)) return;
    *slot = value;
    // `sig_write` stores and wakes waiters only when the resolved value
    // actually changed, so equal drivers never re-fire the net's readers.
    sig_write(&net->resolved, llg_net_compute(net));
}

static void commit_nbas(void) {
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (!p) continue; // slot freed by fork/join teardown or disable_fork
        while (p->nba_head) {
            llg_nba_t* n = p->nba_head;
            p->nba_head = n->next;
            if (!p->nba_head) p->nba_tail = NULL;
            if (n->is_real) {
                real_write(n->real_target, n->real_value);
            } else if (!llg_is_forced(n->target)) {
                sig_write(n->target, n->value);
            }
            free(n);
        }
    }
}

// ── $monitor / $strobe ────────────────────────────────────────────────────────

// Format `fmt` with `n` sv4_t arguments from `args`: %d/%h/%b/%o/%t consume
// arguments in order, %% prints '%', and an unknown or missing specifier
// prints verbatim without consuming an argument (mirrors `llg_display`).
static void llg_format_array(char* out, size_t cap, const char* fmt,
                              const sv4_t* args, int n) {
    size_t len = 0;
    const char* p = fmt;
    int argi = 0;
    while (*p && len + 1 < cap) {
        char c = *p++;
        if (c == '%') {
            // Skip flags and width/precision digits.
            while (*p == '-' || *p == '+' || *p == ' ' || *p == '#' ||
                   *p == '0' || *p == '.' || (*p >= '0' && *p <= '9')) {
                p++;
            }
            c = *p;
            if (c) ++p;
            if (c == '%') {
                llg_append(out, cap, &len, '%');
            } else if ((c == 'd' || c == 'h' || c == 'b' || c == 'o' || c == 't') &&
                       argi < n) {
                // %t prints its argument's value in ticks, like $display
                // (sv4_format has no 't' case, so format as decimal).
                char tmp[LLG_MAX_WIDTH + 2u];
                sv4_format(c == 't' ? 'd' : c, args[argi++], tmp, sizeof(tmp));
                for (char* q = tmp; *q && len + 1 < cap; q++) out[len++] = *q;
            } else {
                out[len++] = '%';
                if (c && len + 1 < cap) out[len++] = c;
            }
        } else {
            out[len++] = c;
        }
    }
    out[len] = 0;
}

// Print one formatted line to stdout (shared by display/monitor/strobe).
static void llg_print_line(const char* line) {
    fputs(line, stdout);
    fputc('\n', stdout);
    fflush(stdout);
}

static void llg_print_array(const char* fmt, const sv4_t* args, int n) {
    size_t cap = strlen(fmt) + 1;
    for (int i = 0; i < n; ++i) {
        size_t extra = (size_t)args[i].width + 2u;
        if (extra > SIZE_MAX - cap) llg_fatal_allocation("formatted line", cap, extra);
        cap += extra;
    }
    char* out = llg_checked_malloc(cap, 1, "formatted line");
    llg_format_array(out, cap, fmt, args, n);
    llg_print_line(out);
    free(out);
}

void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval) {
    if (g.mon.active) {
        free(g.mon.fmt);
        free(g.mon.last);
        free(g.mon.work);
    }
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.n = n;
    g.mon.eval = eval;
    g.mon.fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(g.mon.fmt, fmt);
    int alloc = n > 0 ? n : 1;
    g.mon.last = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor previous values");
    g.mon.work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor working values");
    // Initial print: the current values at registration time.
    eval(g.mon.work);
    for (int i = 0; i < n; i++) g.mon.last[i] = g.mon.work[i];
    llg_print_array(fmt, g.mon.work, n);
}

void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval) {
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = eval;
    int alloc = n > 0 ? n : 1;
    e->work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "strobe working values");
    e->next = g.strobes;
    g.strobes = e;
}

// Re-print the monitor line when any argument differs from the last printed
// snapshot.  Called after every NBA commit (and on $monitoron resume).
static void check_monitor(void) {
    if (!g.mon.active || !g.mon.enabled) return;
    g.mon.eval(g.mon.work);
    int changed = 0;
    for (int i = 0; i < g.mon.n; i++)
        if (!sv4_same(g.mon.work[i], g.mon.last[i])) {
            changed = 1;
            break;
        }
    if (!changed) return;
    for (int i = 0; i < g.mon.n; i++) g.mon.last[i] = g.mon.work[i];
    llg_print_array(g.mon.fmt, g.mon.work, g.mon.n);
}

// Print every queued $strobe line with the values committed by the NBA region
// of the current time step, then clear the queue.
static void flush_strobes(void) {
    while (g.strobes) {
        llg_strobe_t* e = g.strobes;
        g.strobes = e->next;
        e->eval(e->work);
        llg_print_array(e->fmt, e->work, e->n);
        free(e->fmt);
        free(e->work);
        free(e);
    }
}

void llg_monitor_set(int on) {
    if (!g.mon.active) return;
    if (on) {
        if (!g.mon.enabled) {
            g.mon.enabled = 1;
            // Resume: print now if the values changed while suspended.
            check_monitor();
        }
    } else {
        g.mon.enabled = 0;
    }
}

static void report_zero_delay_loop(void) {
    fprintf(stderr, "llg: zero-delay loop detected at time %llu\n",
            (unsigned long long)g.now);
}

// ── final blocks (see llg_rt.h) ─────────────────────────────────────────────

void llg_spawn_final(void (*fn)(llg_proc_t*), const char* name) {
    if (llg_n_finals >= LLG_MAX_FINALS) {
        fprintf(stderr, "llg: too many final blocks (limit %d)\n", LLG_MAX_FINALS);
        abort();
    }
    llg_finals[llg_n_finals].fn = fn;
    llg_finals[llg_n_finals].name = name;
    llg_n_finals++;
}

void llg_rt_run_finals(void) {
    if (llg_n_finals == 0) return;
    // Explicit reset, decoupled from the cleanup-memset invariant: a stale
    // $finish flag left by the scheduler exit must never read as
    // "$finish inside a final" after the first final completes.
    g.finish = 0;
    // Rebuild a minimal coroutine context: the scheduler-exit teardown in
    // llg_rt_run released the previous one.
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
    g.now = llg_final_time;
    uint64_t guard = 0;
    llg_in_finals = 1;
    for (int i = 0; i < llg_n_finals; i++) {
        llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
            1, sizeof(llg_proc_t), "final process");
        p->name = llg_finals[i].name;
        p->fn = llg_finals[i].fn;
        p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
        register_proc(p);
        if (++guard > LLG_ZERO_LOOP_LIMIT) {
            report_zero_delay_loop();
            break;
        }
        aco_resume(p->co);
        // A final never suspends on timing controls and fork/join is
        // rejected by codegen, so nothing else can be left on the ready
        // queue; this drain is dead-defensive only (if a future lowering
        // ever lets a final spawn children, joined descendants land here
        // and would run to completion before the next final starts).
        while (g.ready_head) {
            if (++guard > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                break;
            }
            llg_proc_t* q = g.ready_head;
            g.ready_head = q->next_ready;
            if (!g.ready_head) g.ready_tail = NULL;
            q->next_ready = NULL;
            aco_resume(q->co);
        }
        if (p->wait.kind != W_NONE) {
            fprintf(stderr,
                    "llg: fatal: final block `%s` suspended on a wait "
                    "(timing controls are rejected by codegen)\n",
                    p->name ? p->name : "final");
            abort();
        }
        // Finals permit function statements only. Codegen rejects NBAs,
        // deferred output tasks, waits, and forks, so no scheduler region is
        // run between these sequential zero-time calls.
        if (g.finish) break;
    }
    llg_in_finals = 0;
    llg_rt_cleanup();
    llg_n_finals = 0;
}

void llg_rt_run(void) {
    int zero_loop = 0;
    for (;;) {
        // Zero-delay guard: one counter per time step, tripped when the
        // design never reaches quiescence at `now` (e.g. `always #0;` or an
        // NBA-oscillation loop).  Reset whenever time advances below.
        if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
            report_zero_delay_loop();
            break;
        }
        // Active region: run every ready coroutine once.  Each resume counts
        // toward the zero-delay guard so a trigger cascade that never
        // quiesces (e.g. two processes ping-ponging named-event triggers
        // with no suspension point) trips the guard instead of spinning
        // forever inside one region pass.
        while (g.ready_head) {
            if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                zero_loop = 1;
                break;
            }
            llg_proc_t* p = g.ready_head;
            g.ready_head = p->next_ready;
            if (!g.ready_head) g.ready_tail = NULL;
            p->next_ready = NULL;
            aco_resume(p->co);
            // p either ended (aco_exit) or suspended in a fresh wait.
        }
        if (zero_loop) break;
        // Inactive region (#0): runs between the active region and the NBA
        // region.  Drain it in a loop so a `#0` executed from an inactive
        // continuation schedules a re-inactive pass; the woken continuations
        // (and anything they schedule) run in a fresh active pass.
        while (g.inactive_head) {
            if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                report_zero_delay_loop();
                zero_loop = 1;
                break;
            }
            // Wake every #0 waiter at the current time (FIFO).
            llg_wait_t* w = g.inactive_head;
            g.inactive_head = NULL;
            g.inactive_tail = NULL;
            while (w) {
                llg_wait_t* next = w->inactive_next;
                wake_proc(w->proc);
                w = next;
            }
            while (g.ready_head) {
                if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                    report_zero_delay_loop();
                    zero_loop = 1;
                    break;
                }
                llg_proc_t* p = g.ready_head;
                g.ready_head = p->next_ready;
                if (!g.ready_head) g.ready_tail = NULL;
                p->next_ready = NULL;
                aco_resume(p->co);
            }
            if (zero_loop) break;
        }
        if (zero_loop) break;
        // NBA region: commit recorded non-blocking assignments.
        commit_nbas();
        // Free fork groups whose children all finished (or were killed);
        // done children's NBAs were just committed, killed ones were
        // discarded by disable_fork, so freeing is safe here.
        process_zombie_groups();
        // $strobe lines print with the values committed above; the monitor
        // re-prints when any of its arguments changed.
        flush_strobes();
        check_monitor();
        if (g.ready_head) continue; // new events this time step
        if (g.finish) break;
        if (g.timed_head) {
            uint64_t t = g.timed_head->time;
            if (t == g.now) {
                // #0 waiters now live on the inactive list, so a timed
                // wakeup at `now` cannot come from them; keep the guard for
                // safety against a corrupted time list.
                if (++g.region_passes > LLG_ZERO_LOOP_LIMIT) {
                    report_zero_delay_loop();
                    break;
                }
            } else {
                g.now = t;
                g.region_passes = 0;
            }
            llg_wait_t* w = g.timed_head;
            while (w && w->time == t) {
                llg_wait_t* next = w->time_next;
                wake_proc(w->proc);
                w = next;
            }
            continue;
        }
        if (g.wait_count == 0) {
            fprintf(stderr, "llg: simulation ended without $finish "
                            "(no processes remain) at time %llu\n",
                    (unsigned long long)g.now);
            break;
        }
        fprintf(stderr, "llg: simulation deadlock at time %llu "
                        "(waiters never woken, no future events)\n",
                (unsigned long long)g.now);
        break;
    }
    // Finals ($time inside them) report when the scheduler loop ended.
    llg_final_time = g.now;
    llg_rt_cleanup();
}

// ── $display / $write ─────────────────────────────────────────────────────────

static void llg_vprint(const char* fmt, va_list ap, int newline) {
    const char* p = fmt;
    while (*p) {
        char c = *p++;
        if (c == '%') {
            const char* spec_start = p - 1;
            // Skip flags and width/precision digits.
            while (*p == '-' || *p == '+' || *p == ' ' || *p == '#' ||
                   *p == '0' || *p == '.' || (*p >= '0' && *p <= '9')) {
                p++;
            }
            c = *p;
            if (c) ++p;
            if (c == '%') {
                fputc('%', stdout);
            } else if (c == 't') {
                // %t prints the value of its argument (typically $time) in
                // ticks, matching the generated code which passes the arg.
                sv4_t v = va_arg(ap, sv4_t);
                char tmp[LLG_MAX_WIDTH + 2u];
                sv4_format('d', v, tmp, sizeof(tmp));
                fputs(tmp, stdout);
            } else if (c == 's') {
                const char* s = va_arg(ap, const char*);
                if (s) {
                    fputs(s, stdout);
                }
            } else if (c == 'd' || c == 'h' || c == 'b' || c == 'o') {
                sv4_t v = va_arg(ap, sv4_t);
                // One complete packed value, including a possible minus sign.
                char tmp[LLG_MAX_WIDTH + 2u];
                sv4_format(c, v, tmp, sizeof(tmp));
                fputs(tmp, stdout);
            } else if (c == 'f' || c == 'e' || c == 'g') {
                double v = va_arg(ap, double);
                char real_fmt[128];
                size_t spec_len = (size_t)(p - spec_start);
                if (spec_len >= sizeof(real_fmt)) spec_len = sizeof(real_fmt) - 1;
                memcpy(real_fmt, spec_start, spec_len);
                real_fmt[spec_len] = 0;
                fprintf(stdout, real_fmt, v);
            } else {
                // Unknown specifier: print it verbatim.
                fputc('%', stdout);
                if (c) fputc(c, stdout);
            }
        } else {
            fputc(c, stdout);
        }
    }
    if (newline) fputc('\n', stdout);
    fflush(stdout);
}

void llg_display(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 1);
    va_end(ap);
}

void llg_write(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 0);
    va_end(ap);
}
