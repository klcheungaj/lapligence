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
#include <limits.h>
#include <ctype.h>

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

static void llg_fmt_args_destroy(llg_fmt_arg_t* args, int n);

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
    W_EVENT_TRIGGERED, // waiting on persistent same-time-slot event state
    W_EVENT_ORDER, // waiting for named events in a specified order
    W_MIXED, // atomic named-event + signal or-list (@(posedge a or ev))
    W_DEPS,  // typed packed/real dependency set
    W_EXPR,  // expressions and trigger-time qualifiers
    W_LEVEL,
    W_FORK,    // llg_join: waiting for a fork group
    W_FORK_ALL // llg_wait_fork: waiting for all of the current proc's groups
} llg_wait_kind_t;

typedef struct llg_nba {
    struct llg_nba* next;
    sv4_t* target;
    llg_event_object_t* event_target;
    sv4_t value;
    sv4_t mask;
    int has_mask;
    uint64_t time;
    uint64_t sequence;
    llg_region_t region;
    llg_proc_t* owner;
    int is_real;
    int is_event;
    double* real_target;
    double real_value;
    int is_string;
    llg_string_t* string_target;
    llg_string_t string_value;
} llg_nba_t;

struct llg_inertial {
    struct llg_inertial* next_all;
    struct llg_inertial* next_pending;
    llg_inertial_t** handle;
    sv4_t* target;
    llg_net_t* net;
    int slot;
    int pending;
    uint64_t time;
    llg_region_t region;
    sv4_t current;
    sv4_t value;
    sv4_t mask;
    int has_mask;
    uint64_t rise;
    uint64_t fall;
    uint64_t turn_off;
};

typedef struct llg_wait {
    struct llg_wait* next;         // all active waits (signal + timed + zero-delay)
    struct llg_wait* time_next;    // sorted timed list
    struct llg_wait* region_next;  // typed zero-delay region queue
    llg_proc_t* proc;
    llg_wait_kind_t kind;
    llg_region_t resume_region;
    uint64_t time;                // W_TIME
    llg_expr_event_spec_t* expressions;
    llg_event_spec_t* specs;     // W_EVENTS: copied array; W_MIXED: signal half
    llg_wait_dependency_t* dependencies; // W_DEPS: copied typed dependencies
    sv4_t* last;                  // W_EVENTS/W_MIXED: last-seen values
    double* real_last;            // W_EXPR: last-seen real expression values
    int n;                        // W_EVENTS/W_MIXED (signal entry count)
    llg_event_object_t** evs;    // W_EVENT/W_MIXED: resolved object list
    int n_evs;                    // W_EVENT/W_MIXED
    llg_event_object_t* triggered_ev; // W_EVENT_TRIGGERED registration
    llg_event_object_t** order_sequence; // W_EVENT_ORDER expected objects
    int n_order;
    int order_next;
    int order_result_value;
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
    int started;                  // join_none children became eligible to run
    llg_proc_t* parent;          // spawning proc
    llg_fork_child_t* children;  // for disable_fork
    struct llg_fork_group* next_g; // per-proc live-group list (or zombie list)
    uint32_t target_declaration;
    uint32_t target_instance;
    int has_target;
    llg_region_t child_region;    // region inherited by children at the fork site
    llg_activation_t* owner_activation;
};

typedef enum {
    LLG_FRAME_ALIAS_NONE = 0,
    LLG_FRAME_ALIAS_PACKED,
    LLG_FRAME_ALIAS_REAL,
    LLG_FRAME_ALIAS_SLOT,
} llg_frame_alias_kind_t;

typedef struct {
    llg_frame_slot_kind_t kind;
    llg_frame_alias_kind_t alias_kind;
    union {
        sv4_t packed;
        double real;
        void* opaque;
    } value;
    union {
        sv4_t* packed;
        double* real;
        struct {
            llg_frame_t* frame;
            size_t slot;
        } slot;
    } alias;
} llg_frame_slot_t;

struct llg_frame {
    size_t refs;
    size_t nslots;
    llg_frame_slot_t* slots;
};

struct llg_activation {
    uint32_t declaration;
    uint32_t instance;
    llg_proc_t* proc;
    struct llg_activation* parent;
    struct llg_activation* proc_next;
    struct llg_activation* all_next;
    size_t refs;
    int disabled;
    int detached;
};

struct llg_proc {
    aco_t* co;
    const char* name;
    void (*fn)(llg_proc_t*);
    llg_nba_t* nba_head;
    llg_nba_t* nba_tail;
    llg_wait_t wait;
    llg_proc_t* next_region;
    llg_region_t region;
    llg_region_t wait_resume_region;
    int has_wait_resume_region;
    int completed;
    int killed;
    llg_proc_t* next_retired;
    llg_fork_group_t* fork_groups; // live groups spawned by this proc
    llg_fork_group_t* grp;         // group this proc belongs to (NULL top-level)
    llg_frame_t* frame;            // retained activation storage, when captured
    llg_activation_t* activation_top; // innermost named block/task scope
    uint64_t budget_steps;         // loop back-edges at `budget_time`
    uint64_t budget_time;          // time step for the process budget
};

static int region_can_mutate(const char* action);
static llg_nba_t* new_nba(uint64_t ticks);
static void enqueue_nba(llg_nba_t* n);
static void deferred_trigger_source_change(sv4_t* sig, double* real);
static void deferred_trigger_event(llg_event_object_t* ev);
static void start_pending_fork_children(llg_proc_t* parent);
static void event_triggered_unlink(llg_wait_t* w);

static llg_frame_slot_t* frame_slot(llg_frame_t* frame, size_t slot) {
    if (!frame || slot >= frame->nslots) {
        fprintf(stderr, "llg: activation frame slot out of bounds\n");
        abort();
    }
    return &frame->slots[slot];
}

static const llg_frame_slot_t* frame_slot_const(const llg_frame_t* frame,
                                                size_t slot) {
    if (!frame || slot >= frame->nslots) {
        fprintf(stderr, "llg: activation frame slot out of bounds\n");
        abort();
    }
    return &frame->slots[slot];
}

static void frame_clear_alias(llg_frame_slot_t* entry) {
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        llg_frame_release(entry->alias.slot.frame);
    }
    entry->alias_kind = LLG_FRAME_ALIAS_NONE;
    entry->alias.packed = NULL;
}

static void frame_kind_error(llg_frame_slot_kind_t expected,
                             llg_frame_slot_kind_t actual) {
    fprintf(stderr,
            "llg: activation frame slot type mismatch (expected %d, got %d)\n",
            (int)expected, (int)actual);
    abort();
}

llg_frame_t* llg_frame_new(size_t slots) {
    llg_frame_t* frame = (llg_frame_t*)llg_checked_calloc(
        1, sizeof(*frame), "activation frame");
    frame->refs = 1;
    frame->nslots = slots;
    frame->slots = (llg_frame_slot_t*)llg_checked_calloc(
        slots, sizeof(*frame->slots), "activation frame slots");
    for (size_t i = 0; i < slots; i++) {
        frame->slots[i].kind = LLG_FRAME_PACKED;
        frame->slots[i].alias_kind = LLG_FRAME_ALIAS_NONE;
        frame->slots[i].value.packed = sv4_x(1, 0);
    }
    return frame;
}

void llg_frame_retain(llg_frame_t* frame) {
    if (!frame) return;
    if (frame->refs == SIZE_MAX) {
        fprintf(stderr, "llg: activation frame reference count overflow\n");
        abort();
    }
    frame->refs++;
}

void llg_frame_release(llg_frame_t* frame) {
    if (!frame) return;
    if (frame->refs == 0) {
        fprintf(stderr, "llg: activation frame reference count underflow\n");
        abort();
    }
    frame->refs--;
    if (frame->refs != 0) return;
    for (size_t i = 0; i < frame->nslots; i++) frame_clear_alias(&frame->slots[i]);
    free(frame->slots);
    free(frame);
}

void llg_frame_capture_value(llg_frame_t* frame, size_t slot, sv4_t value) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_PACKED;
    entry->value.packed = value;
}

void llg_frame_capture_real(llg_frame_t* frame, size_t slot, double value) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_REAL;
    entry->value.real = value;
}

void llg_frame_alias_value(llg_frame_t* frame, size_t slot, sv4_t* target) {
    if (!target) {
        fprintf(stderr, "llg: activation frame alias target is null\n");
        abort();
    }
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_PACKED;
    entry->alias_kind = LLG_FRAME_ALIAS_PACKED;
    entry->alias.packed = target;
}

void llg_frame_alias_real(llg_frame_t* frame, size_t slot, double* target) {
    if (!target) {
        fprintf(stderr, "llg: activation real alias target is null\n");
        abort();
    }
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_REAL;
    entry->alias_kind = LLG_FRAME_ALIAS_REAL;
    entry->alias.real = target;
}

void llg_frame_alias_slot(llg_frame_t* frame, size_t slot,
                          llg_frame_t* target, size_t target_slot) {
    if (!target || frame == target) {
        fprintf(stderr, "llg: invalid activation frame alias source\n");
        abort();
    }
    (void)frame_slot_const(target, target_slot);
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = llg_frame_slot_kind(target, target_slot);
    entry->alias_kind = LLG_FRAME_ALIAS_SLOT;
    entry->alias.slot.frame = target;
    entry->alias.slot.slot = target_slot;
    llg_frame_retain(target);
}

llg_frame_slot_kind_t llg_frame_slot_kind(const llg_frame_t* frame,
                                          size_t slot) {
    const llg_frame_slot_t* entry = frame_slot_const(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_slot_kind(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    return entry->kind;
}

sv4_t llg_frame_read_value(const llg_frame_t* frame, size_t slot) {
    const llg_frame_slot_t* entry = frame_slot_const(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_read_value(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    if (llg_frame_slot_kind(frame, slot) != LLG_FRAME_PACKED) {
        frame_kind_error(LLG_FRAME_PACKED, llg_frame_slot_kind(frame, slot));
    }
    return entry->alias_kind == LLG_FRAME_ALIAS_PACKED
               ? *entry->alias.packed
               : entry->value.packed;
}

void llg_frame_write_value(llg_frame_t* frame, size_t slot, sv4_t value) {
    if (!region_can_mutate("activation frame write")) return;
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        llg_frame_write_value(entry->alias.slot.frame, entry->alias.slot.slot, value);
    } else if (entry->alias_kind == LLG_FRAME_ALIAS_PACKED) {
        llg_ba(entry->alias.packed, value);
    } else {
        if (entry->kind != LLG_FRAME_PACKED) {
            frame_kind_error(LLG_FRAME_PACKED, entry->kind);
        }
        entry->value.packed = value;
    }
}

double llg_frame_read_real(const llg_frame_t* frame, size_t slot) {
    const llg_frame_slot_t* entry = frame_slot_const(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_read_real(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    if (llg_frame_slot_kind(frame, slot) != LLG_FRAME_REAL) {
        frame_kind_error(LLG_FRAME_REAL, llg_frame_slot_kind(frame, slot));
    }
    return entry->alias_kind == LLG_FRAME_ALIAS_REAL
               ? *entry->alias.real
               : entry->value.real;
}

void llg_frame_write_real(llg_frame_t* frame, size_t slot, double value) {
    if (!region_can_mutate("activation real frame write")) return;
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        llg_frame_write_real(entry->alias.slot.frame, entry->alias.slot.slot, value);
    } else if (entry->alias_kind == LLG_FRAME_ALIAS_REAL) {
        *entry->alias.real = value;
    } else {
        if (entry->kind != LLG_FRAME_REAL) {
            frame_kind_error(LLG_FRAME_REAL, entry->kind);
        }
        entry->value.real = value;
    }
}

typedef struct {
    sv4_t* target;
    sv4_t* enable;
    uint64_t site;
    sv4_t value;
    int active;
} llg_pca_binding_t;

typedef struct {
    double* target;
    sv4_t* enable;
    uint64_t site;
    double value;
    int active;
} llg_pca_real_binding_t;

typedef struct {
    int active;
    int is_real;
    llg_force_part_t* parts;
    sv4_t* masks; // live coverage in target coordinates
    int n_parts;
    uint32_t stream_slice;
    int stream_right_to_left;
    double* real_target;
    llg_force_eval_fn eval;
    llg_force_real_eval_fn real_eval;
    llg_force_read_t* reads;
    int n_reads;
    sv4_t value;
    double real_value;
    int evaluating;
} llg_force_entry_t;

// ── $monitor / $strobe state ──────────────────────────────────────────────────

typedef struct {
    int active;          // a monitor is registered
    int enabled;         // $monitoron / $monitoroff
    int dirty;           // a trigger signal changed since the last check
    int force_report;    // registration or enable requires one report
    char* fmt;           // strdup'd format string
    int n;               // number of displayed arguments
    llg_mon_eval_fn eval;
    sv4_t* last;         // last-printed argument values (n)
    sv4_t* work;         // scratch buffer the eval fn fills (n)
    sv4_t** reads;       // trigger signal pointers (not display-only values)
    int n_reads;
    llg_display_eval_fn typed_eval;
    llg_fmt_arg_t* typed_last;
    llg_fmt_arg_t* typed_work;
    llg_display_read_t* typed_reads;
    int n_typed_reads;
    char* scope;
    int typed;
    llg_region_t region;
} llg_monitor_state_t;

typedef struct llg_strobe {
    struct llg_strobe* next;
    char* fmt;           // strdup'd format string
    int n;
    llg_mon_eval_fn eval;
    sv4_t* work;         // scratch buffer the eval fn fills (n)
    llg_display_eval_fn typed_eval;
    llg_fmt_arg_t* typed_work;
    char* scope;
    int typed;
    llg_region_t region;
} llg_strobe_t;

typedef struct {
    llg_proc_t* head;
    llg_proc_t* tail;
} llg_proc_queue_t;

typedef struct {
    llg_wait_t* head;
    llg_wait_t* tail;
} llg_wait_queue_t;

typedef struct llg_region_callback {
    struct llg_region_callback* next;
    llg_region_t region;
    uint64_t time;
    uint64_t sequence;
    llg_region_callback_fn callback;
    void* data;
} llg_region_callback_t;

typedef struct llg_sampled_value {
    struct llg_sampled_value* next;
    sv4_t* signal;
    sv4_t value;
} llg_sampled_value_t;

// An event-controlled `->>` is not a suspended process.  Its source
// descriptors and snapshots live here until one source matches, then the
// target is submitted to the ordinary NBA queue.
typedef struct llg_deferred_trigger {
    struct llg_deferred_trigger* next;
    llg_event_object_t* target;
    llg_event_assignment_fn action;
    llg_frame_t* action_frame;
    llg_expr_event_spec_t* specs;
    sv4_t* last;
    double* real_last;
    int n;
    uint64_t remaining;
} llg_deferred_trigger_t;

typedef struct {
    aco_t* main_co;
    aco_share_stack_t* share_stack;
    llg_proc_queue_t process_queues[LLG_REGION_COUNT];
    llg_nba_t* delayed_nbas;
    llg_inertial_t* inertial_drivers;
    llg_inertial_t* inertial_pending;
    uint64_t nba_sequence;
    llg_wait_t* timed_head;   // sorted ascending by time
    llg_wait_queue_t zero_waits[LLG_REGION_COUNT];
    llg_wait_t* waiters;      // all active waits
    int wait_count;
    uint64_t now;
    llg_region_t current_region;
    uint64_t callback_sequence;
    llg_region_callback_t* callbacks; // sorted by time, region, issue order
    llg_sampled_value_t* sampled;
    llg_deferred_trigger_t* deferred_triggers;
    llg_deferred_trigger_t* deferred_trigger_tail;
    int in_deferred_action;
    int running;
    int finish;
    int config_error;
    uint64_t zero_loop_limit;
    uint64_t process_step_limit;
    uint64_t region_passes;    // zero-delay guard units in the current time step (region passes + coroutine resumes)
    const char* last_process_name; // survives completed-process reclamation
    llg_proc_t* all_procs[LLG_MAX_PROCS];
    int n_procs;
    llg_proc_t* retired_procs; // cancelled coroutines awaiting a safe destroy point
    llg_fork_group_t* zombie_groups; // completed/killed groups awaiting teardown
    llg_activation_t* activations; // active named block/task invocations
    llg_monitor_state_t mon;   // the active $monitor (at most one)
    llg_strobe_t* strobes;     // pending $strobe lines for this time step
    llg_strobe_t* strobe_tail; // preserves source issue order
    // Active procedural forces. Entries own copied target/source descriptors;
    // no pre-force value is retained because release is object-specific.
    llg_force_entry_t force_table[LLG_MAX_FORCE];
    int force_count;
    llg_pca_binding_t pca_table[LLG_MAX_PCA];
    int pca_count;
    llg_pca_real_binding_t pca_real_table[LLG_MAX_PCA];
    int pca_real_count;
    int argc;
    char** argv;
} llg_rt_ctx_t;

static llg_rt_ctx_t g;

typedef struct llg_dependency_binding {
    struct llg_dependency_binding* next;
    sv4_t* target;
    double* real_target;
    sv4_t* dependency;
} llg_dependency_binding_t;

static llg_dependency_binding_t* llg_dependency_bindings;

// Kept outside `g`: every completed run cleans the context, but callers need
// to inspect whether that run stopped with a controlled runtime failure.
static int llg_last_failure;
static int llg_last_config_error;
// Event objects are generated as file-scope storage and therefore survive
// `llg_rt_cleanup`. Bump this generation at each teardown so their persistent
// same-slot state cannot leak into a later runtime initialization without
// dereferencing an object whose owner may be outside the scheduler.
static uint64_t llg_event_generation;
static uint64_t llg_configured_zero_loop_limit;
static uint64_t llg_configured_process_step_limit;

static const char* const llg_region_names[LLG_REGION_COUNT] = {
    "Preponed",
    "Preponed PLI",
    "Pre-Active PLI",
    "Active",
    "Inactive",
    "Pre-NBA PLI",
    "Pre-NBA",
    "NBA",
    "Post-NBA",
    "Post-NBA PLI",
    "Pre-Observed PLI",
    "Pre-Observed",
    "Observed",
    "Post-Observed",
    "Post-Observed PLI",
    "Reactive",
    "Re-Inactive",
    "Pre-Re-NBA PLI",
    "Pre-Re-NBA",
    "Re-NBA",
    "Post-Re-NBA",
    "Post-Re-NBA PLI",
    "Pre-Postponed PLI",
    "Pre-Postponed",
    "Postponed",
    "Postponed PLI",
};

const char* llg_region_name(llg_region_t region) {
    if (region < 0 || region >= LLG_REGION_COUNT) return "invalid";
    return llg_region_names[region];
}

static int region_valid(llg_region_t region) {
    return region >= 0 && region < LLG_REGION_COUNT;
}

static int region_is_reactive(llg_region_t region) {
    return region >= LLG_REGION_REACTIVE && region <= LLG_REGION_POST_RE_NBA_PLI;
}

static int region_is_design(llg_region_t region) {
    return (region >= LLG_REGION_ACTIVE && region <= LLG_REGION_POST_NBA_PLI) ||
           region_is_reactive(region);
}

static int region_is_read_only_now(llg_region_t region) {
    if (!g.running) return 0;
    return region == LLG_REGION_PREPONED || region == LLG_REGION_PREPONED_PLI ||
           (region >= LLG_REGION_PRE_OBSERVED_PLI &&
            region <= LLG_REGION_POST_OBSERVED_PLI) ||
           region == LLG_REGION_POSTPONED || region == LLG_REGION_POSTPONED_PLI;
}

int llg_region_is_read_only(void) {
    return region_is_read_only_now(g.current_region);
}

llg_region_t llg_current_region(void) { return g.current_region; }

static void region_violation(const char* action, llg_region_t region) {
    fprintf(stderr, "llg: illegal %s in read-only or completed %s region\n",
            action, llg_region_name(region));
    llg_last_failure = 1;
    g.finish = 1;
}

static int region_can_mutate(const char* action) {
    if (region_is_read_only_now(g.current_region)) {
        region_violation(action, g.current_region);
        return 0;
    }
    return 1;
}

static int callback_region_allowed(llg_region_t region, uint64_t ticks) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for callback\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    if (!g.running || ticks != 0) return 1;
    if (region_is_read_only_now(g.current_region)) {
        if (g.current_region == LLG_REGION_OBSERVED &&
            region == LLG_REGION_REACTIVE)
            return 1;
        region_violation("callback scheduling", g.current_region);
        return 0;
    }
    if (region >= g.current_region) return 1;
    // A writable iterative region can return work to the design/reactive set;
    // the scheduler will drain that set again before reaching Postponed.
    if (region_is_design(region)) return 1;
    fprintf(stderr, "llg: illegal callback scheduling from %s to %s at time %llu\n",
            llg_region_name(g.current_region), llg_region_name(region),
            (unsigned long long)g.now);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

static int parse_positive_u64(const char* text, uint64_t* value) {
    if (!text || text[0] == '\0') return 0;
    uint64_t parsed = 0;
    for (const unsigned char* p = (const unsigned char*)text; *p; ++p) {
        if (*p < '0' || *p > '9') return 0;
        uint64_t digit = (uint64_t)(*p - '0');
        if (parsed > (UINT64_MAX - digit) / 10u) return 0;
        parsed = parsed * 10u + digit;
    }
    if (parsed == 0) return 0;
    *value = parsed;
    return 1;
}

static int load_limit(const char* name, uint64_t fallback, uint64_t* value,
                      int* present) {
    const char* text = getenv(name);
    *present = text != NULL;
    if (!text) {
        if (fallback == 0) {
            fprintf(stderr,
                    "llg: invalid %s default (must be a positive decimal uint64)\n",
                    name);
            return 0;
        }
        *value = fallback;
        return 1;
    }
    if (!parse_positive_u64(text, value)) {
        fprintf(stderr,
                "llg: invalid %s (must be a positive decimal uint64)\n", name);
        return 0;
    }
    return 1;
}

static int configure_limits(void) {
    int zero_present = 0;
    if (!load_limit("LLG_ZERO_LOOP_LIMIT", (uint64_t)LLG_ZERO_LOOP_LIMIT,
                    &g.zero_loop_limit, &zero_present)) {
        return 0;
    }

    const char* process_name = "LLG_PROCESS_STEP_LIMIT";
    const char* process_text = getenv(process_name);
    if (!process_text) {
        process_name = "LLG_NONCONVERGENCE_LIMIT";
        process_text = getenv(process_name);
    }
    if (process_text) {
        if (!parse_positive_u64(process_text, &g.process_step_limit)) {
            fprintf(stderr,
                    "llg: invalid %s (must be a positive decimal uint64)\n",
                    process_name);
            return 0;
        }
    } else if (zero_present) {
        // A single zero-time limit is convenient for callers that only need
        // to tighten the guard; the dedicated process setting still wins.
        g.process_step_limit = g.zero_loop_limit;
    } else {
        int process_present = 0;
        if (!load_limit("LLG_PROCESS_STEP_LIMIT",
                        (uint64_t)LLG_PROCESS_STEP_LIMIT,
                        &g.process_step_limit, &process_present)) {
            return 0;
        }
    }
    return 1;
}

static int consume_limit(uint64_t* counter, uint64_t limit) {
    if (*counter >= limit) return 0;
    *counter += 1;
    return 1;
}

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
    return aco_gtls_co && aco_gtls_co != g.main_co
               ? (llg_proc_t*)aco_get_arg() : NULL;
}

static void activation_retain(llg_activation_t* activation) {
    if (!activation) return;
    if (activation->refs == SIZE_MAX) {
        fprintf(stderr, "llg: named activation reference count overflow\n");
        abort();
    }
    activation->refs++;
}

static void activation_release(llg_activation_t* activation) {
    if (!activation) return;
    if (activation->refs == 0) {
        fprintf(stderr, "llg: named activation reference count underflow\n");
        abort();
    }
    activation->refs--;
    if (activation->refs == 0) {
        // A detached activation can remain the owner of a join_none group.
        // Keep its lexical ancestry alive until that last owner reference is
        // released so a later disable of an active outer scope still finds
        // the retained descendant through the parent chain.
        llg_activation_t* parent = activation->parent;
        activation->parent = NULL;
        free(activation);
        activation_release(parent);
    }
}

static void activation_unlink_all(llg_activation_t* activation) {
    llg_activation_t** pp = &g.activations;
    while (*pp) {
        if (*pp == activation) {
            *pp = activation->all_next;
            activation->all_next = NULL;
            return;
        }
        pp = &(*pp)->all_next;
    }
}

static void activation_unlink_process(llg_activation_t* activation) {
    llg_proc_t* proc = activation->proc;
    if (!proc) return;
    llg_activation_t** pp = &proc->activation_top;
    while (*pp) {
        if (*pp == activation) {
            *pp = activation->proc_next;
            activation->proc_next = NULL;
            return;
        }
        pp = &(*pp)->proc_next;
    }
}

static void activation_detach(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_unlink_process(activation);
    activation_unlink_all(activation);
    activation->detached = 1;
}

static void activation_unwind_proc(llg_proc_t* proc) {
    while (proc && proc->activation_top) {
        llg_activation_t* activation = proc->activation_top;
        activation_detach(activation);
        activation_release(activation);
    }
}

llg_activation_t* llg_activation_enter(uint32_t declaration,
                                        uint32_t instance) {
    llg_proc_t* proc = llg_current();
    if (!proc || !region_can_mutate("named activation scheduling")) return NULL;
    llg_activation_t* activation = (llg_activation_t*)llg_checked_calloc(
        1, sizeof(*activation), "named activation");
    activation->declaration = declaration;
    activation->instance = instance;
    activation->proc = proc;
    activation->parent = proc->activation_top;
    activation->refs = 1; // process activation-stack ownership
    if (activation->parent) activation_retain(activation->parent);
    activation->proc_next = proc->activation_top;
    proc->activation_top = activation;
    activation->all_next = g.activations;
    g.activations = activation;
    return activation;
}

void llg_activation_exit(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_detach(activation);
    activation_release(activation);
}

int llg_activation_cancelled(void) {
    llg_proc_t* proc = llg_current();
    for (llg_activation_t* activation = proc ? proc->activation_top : NULL;
         activation; activation = activation->proc_next) {
        if (activation->disabled) return 1;
    }
    return 0;
}

int llg_rt_failed(void) { return llg_last_failure != 0; }

static void budget_abort(llg_proc_t* p, const char* location) {
    const char* where = location && location[0] != '\0'
                            ? location
                            : (p && p->name ? p->name : "<unknown process>");
    fprintf(stderr,
            "llg: nonconvergent zero-time execution in process `%s` at time "
            "%llu (process step limit %llu)\n",
            where, (unsigned long long)g.now,
            (unsigned long long)g.process_step_limit);
    llg_last_failure = 1;
    g.finish = 1;
    aco_exit();
    abort();
}

void llg_budget_point(const char* location) {
    llg_proc_t* p = llg_current();
    if (!p || g.config_error) return;
    if (p->budget_time != g.now) {
        p->budget_time = g.now;
        p->budget_steps = 0;
    }
    if (!consume_limit(&p->budget_steps, g.process_step_limit)) {
        budget_abort(p, location);
    }
}

void llg_unique_priority_check(int check, int matched, int has_default,
                               const char* location) {
    if (check < 1 || check > 3) return;
    const char* where = location && location[0] != '\0' ? location : "<unknown>";
    const char* qualifier = check == 1 ? "unique" : check == 2 ? "unique0" : "priority";
    if (matched == 0 && !has_default && check != 2) {
        fprintf(stderr,
                "llg: warning: %s violation at %s: no matching item\n",
                qualifier, where);
    }
    if (matched > 1 && check != 3) {
        fprintf(stderr,
                "llg: warning: %s violation at %s: multiple matching items\n",
                qualifier, where);
    }
}

static void enqueue_region(llg_proc_t* p, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for process\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    p->region = region;
    p->next_region = NULL;
    llg_proc_queue_t* queue = &g.process_queues[region];
    if (queue->tail) {
        queue->tail->next_region = p;
        queue->tail = p;
    } else {
        queue->head = queue->tail = p;
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

static void insert_zero_wait(llg_wait_t* w, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for zero-delay wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    w->resume_region = region;
    w->region_next = NULL;
    llg_wait_queue_t* queue = &g.zero_waits[region];
    if (queue->tail) {
        queue->tail->region_next = w;
    } else {
        queue->head = w;
    }
    queue->tail = w;
}

static void remove_zero_wait_entry(llg_wait_t* w) {
    for (int i = 0; i < LLG_REGION_COUNT; i++) {
        llg_wait_queue_t* queue = &g.zero_waits[i];
        llg_wait_t** pp = &queue->head;
        while (*pp) {
            if (*pp == w) {
                *pp = w->region_next;
                if (queue->tail == w) {
                    queue->tail = NULL;
                    for (llg_wait_t* q = queue->head; q; q = q->region_next)
                        queue->tail = q;
                }
                w->region_next = NULL;
                return;
            }
            pp = &(*pp)->region_next;
        }
    }
}

static llg_proc_t* dequeue_region(llg_region_t region) {
    llg_proc_queue_t* queue = &g.process_queues[region];
    llg_proc_t* p = queue->head;
    if (!p) return NULL;
    queue->head = p->next_region;
    if (!queue->head) queue->tail = NULL;
    p->next_region = NULL;
    return p;
}

static void remove_region_entry(llg_proc_t* p) {
    for (int i = 0; i < LLG_REGION_COUNT; i++) {
        llg_proc_queue_t* queue = &g.process_queues[i];
        llg_proc_t** pp = &queue->head;
        while (*pp) {
            if (*pp == p) {
                *pp = p->next_region;
                if (queue->tail == p) {
                    queue->tail = NULL;
                    for (llg_proc_t* q = queue->head; q; q = q->next_region)
                        queue->tail = q;
                }
                p->next_region = NULL;
                return;
            }
            pp = &(*pp)->next_region;
        }
    }
}

static void remove_inactive_entry(llg_wait_t* w) {
    remove_zero_wait_entry(w);
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

static void free_expression_wait(llg_wait_t* w) {
    if (!w->expressions) return;
    for (int i = 0; i < w->n; i++) {
        llg_frame_release((llg_frame_t*)w->expressions[i].eval_context);
        llg_frame_release((llg_frame_t*)w->expressions[i].condition_context);
        free(w->expressions[i].reads);
        free(w->expressions[i].dependencies);
    }
    free(w->expressions);
    w->expressions = NULL;
}

static void release_expression_contexts(const llg_expr_event_spec_t* specs, int n) {
    if (!specs) return;
    for (int i = 0; i < n; i++) {
        llg_frame_release((llg_frame_t*)specs[i].eval_context);
        llg_frame_release((llg_frame_t*)specs[i].condition_context);
    }
}

static void free_deferred_trigger(llg_deferred_trigger_t* trigger) {
    if (!trigger) return;
    for (int i = 0; i < trigger->n; i++) {
        llg_frame_release((llg_frame_t*)trigger->specs[i].eval_context);
        llg_frame_release((llg_frame_t*)trigger->specs[i].condition_context);
        free(trigger->specs[i].reads);
        free(trigger->specs[i].dependencies);
    }
    free(trigger->specs);
    free(trigger->last);
    free(trigger->real_last);
    if (trigger->action_frame) llg_frame_release(trigger->action_frame);
    free(trigger);
}

static void free_deferred_triggers(void) {
    while (g.deferred_triggers) {
        llg_deferred_trigger_t* next = g.deferred_triggers->next;
        free_deferred_trigger(g.deferred_triggers);
        g.deferred_triggers = next;
    }
    g.deferred_trigger_tail = NULL;
}

static int expression_qualifies(const llg_expr_event_spec_t* spec) {
    if (!spec->condition) return 1;
    sv4_t result;
    spec->condition(&result, spec->condition_context);
    return sv4_to_bool(result);
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
    if (w->kind == W_EVENT || w->kind == W_EVENT_ORDER ||
        w->kind == W_MIXED || w->kind == W_EXPR) {
        event_unlink(w);
    }
    if (w->kind == W_EVENT_TRIGGERED) event_triggered_unlink(w);
    free_expression_wait(w);
    free(w->specs);
    free(w->dependencies);
    free(w->last);
    free(w->real_last);
    free(w->evs);
    free(w->order_sequence);
    w->specs = NULL;
    w->dependencies = NULL;
    w->last = NULL;
    w->real_last = NULL;
    w->evs = NULL;
    w->order_sequence = NULL;
    w->n = 0;
    w->n_evs = 0;
    w->triggered_ev = NULL;
    w->n_order = 0;
    w->order_next = 0;
    w->kind = W_NONE;
    g.wait_count--;
    enqueue_region(p, w->resume_region);
}

static void register_wait(void) {
    llg_proc_t* p = llg_current();
    if (!p) {
        fprintf(stderr, "llg: wait requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    // A join_none group is created immediately but its children are not
    // eligible until the spawning process reaches its first blocking control.
    // Registering any real wait is that suspension boundary.  Starting all
    // pending groups here also covers nested join_none groups and wait fork.
    start_pending_fork_children(p);
    llg_wait_t* w = &p->wait;
    w->proc = p;
    w->next = g.waiters;
    g.waiters = w;
    g.wait_count++;
}

// ── Named events ──────────────────────────────────────────────────────────────

// Register `p` on `ev`'s waiter table (fixed capacity, like the other
// runtime resource limits).
static void event_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->waiters[ev->n_waiters++] = p;
}

// Register a process on the persistent same-time-slot state of `ev`. This
// list is deliberately separate from ordinary event waiters: `@(ev)` remains
// edge-triggered and never observes a trigger that happened before it parked.
static void event_triggered_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_triggered_waiters >= LLG_MAX_EVENT_WAITERS) {
        fprintf(stderr,
                "llg: too many triggered waiters on one named event (limit %d)\n",
                LLG_MAX_EVENT_WAITERS);
        abort();
    }
    ev->triggered_waiters[ev->n_triggered_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    for (int i = 0; i < w->n_evs; i++) {
        llg_event_object_t* ev = w->evs[i];
        if (!ev) continue;
        for (int k = 0; k < ev->n_waiters; k++) {
            if (ev->waiters[k] == w->proc) {
                ev->waiters[k] = ev->waiters[ev->n_waiters - 1];
                ev->n_waiters--;
                break;
            }
        }
    }
}

static void event_triggered_unlink(llg_wait_t* w) {
    llg_event_object_t* ev = w->triggered_ev;
    if (!ev) return;
    for (int i = 0; i < ev->n_triggered_waiters; i++) {
        if (ev->triggered_waiters[i] == w->proc) {
            ev->triggered_waiters[i] =
                ev->triggered_waiters[ev->n_triggered_waiters - 1];
            ev->n_triggered_waiters--;
            break;
        }
    }
}

// ── fork/join (coroutine children) ────────────────────────────────────────────

static void llg_kill_proc_tree(llg_proc_t* p); // mutual recursion below
static void llg_proc_entry(void);               // defined in the public API section

// Unlink a suspended or queued proc from every scheduler queue, free its
// pending NBA list, and retire its storage. A named disable may cancel the
// executing coroutine; its destruction is deferred until the scheduler resumes.
static void llg_kill_proc(llg_proc_t* p) {
    if (!p || p->killed) return;
    p->killed = 1;
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
        if (w->kind == W_EVENT || w->kind == W_EVENT_ORDER ||
            w->kind == W_MIXED || w->kind == W_EXPR) {
            event_unlink(w);
        }
        if (w->kind == W_EVENT_TRIGGERED) event_triggered_unlink(w);
        free_expression_wait(w);
        free(w->specs);
        free(w->dependencies);
        free(w->last);
        free(w->real_last);
        free(w->evs);
        free(w->order_sequence);
        w->specs = NULL;
        w->dependencies = NULL;
        w->last = NULL;
        w->real_last = NULL;
        w->evs = NULL;
        w->order_sequence = NULL;
        w->n = 0;
        w->n_evs = 0;
        w->triggered_ev = NULL;
        w->n_order = 0;
        w->order_next = 0;
        w->order_result_value = 0;
        w->kind = W_NONE;
        g.wait_count--;
    }
    remove_region_entry(p);

    activation_unwind_proc(p);
    llg_frame_release(p->frame);
    p->frame = NULL;
    unregister_proc(p);
    p->next_retired = g.retired_procs;
    g.retired_procs = p;
}

// The active coroutine's stack/register state must survive until aco_exit
// returns control to the scheduler. Other cancelled processes can be reclaimed
// after cancellation traversal, including within a long-running caller.
static void reap_retired_procs(void) {
    llg_proc_t* current = llg_current();
    llg_proc_t** slot = &g.retired_procs;
    while (*slot) {
        llg_proc_t* proc = *slot;
        if (proc == current) {
            slot = &proc->next_retired;
            continue;
        }
        *slot = proc->next_retired;
        aco_destroy(proc->co);
        free(proc);
    }
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

static llg_fork_group_t* llg_fork_group_new_impl(int join_kind,
                                                  int has_target,
                                                  uint32_t declaration,
                                                  uint32_t instance) {
    llg_proc_t* parent = llg_current();
    if (!parent || !region_can_mutate("fork scheduling")) return NULL;
    llg_fork_group_t* grp = (llg_fork_group_t*)llg_checked_calloc(
        1, sizeof(llg_fork_group_t), "fork group");
    grp->join_kind = join_kind;
    grp->started = join_kind != LLG_JOIN_NONE;
    grp->parent = parent;
    grp->has_target = has_target;
    grp->target_declaration = declaration;
    grp->target_instance = instance;
    grp->child_region = region_is_reactive(g.current_region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    grp->owner_activation = parent->activation_top;
    if (grp->owner_activation) activation_retain(grp->owner_activation);
    // Preserve source creation order when one suspension releases multiple
    // join_none groups.  Child order within each group is already the branch
    // list order, so this gives the scheduler one deterministic sequence.
    llg_fork_group_t** tail = &parent->fork_groups;
    while (*tail) tail = &(*tail)->next_g;
    *tail = grp;
    return grp;
}

static void start_pending_fork_children(llg_proc_t* parent) {
    if (!parent) return;
    for (llg_fork_group_t* grp = parent->fork_groups; grp; grp = grp->next_g) {
        if (grp->join_kind != LLG_JOIN_NONE || grp->started) continue;
        grp->started = 1;
        for (llg_fork_child_t* child = grp->children; child; child = child->next) {
            llg_proc_t* child_proc = child->proc;
            if (!child_proc || child_proc->killed || child_proc->completed) continue;
            enqueue_region(child_proc, grp->child_region);
        }
    }
}

llg_fork_group_t* llg_fork_group_new(int join_kind) {
    return llg_fork_group_new_impl(join_kind, 0, 0, 0);
}

llg_fork_group_t* llg_fork_group_new_target(int join_kind,
                                             uint32_t declaration,
                                             uint32_t instance) {
    return llg_fork_group_new_impl(join_kind, 1, declaration, instance);
}

static llg_proc_t* llg_fork_impl(void (*fn)(llg_proc_t*), const char* name,
                                 llg_fork_group_t* grp, llg_frame_t* frame) {
    if (!fn || !grp || !region_can_mutate("fork scheduling")) return NULL;
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(llg_proc_t), "forked process");
    p->name = name;
    p->fn = fn;
    p->grp = grp;
    p->frame = frame;
    llg_frame_retain(frame);
    p->budget_time = g.now;
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
    if (grp->join_kind != LLG_JOIN_NONE) enqueue_region(p, grp->child_region);
    return p;
}

llg_proc_t* llg_fork(void (*fn)(llg_proc_t*), const char* name, llg_fork_group_t* grp) {
    return llg_fork_impl(fn, name, grp, NULL);
}

llg_proc_t* llg_fork_with_frame(void (*fn)(llg_proc_t*), const char* name,
                                llg_fork_group_t* grp, llg_frame_t* frame) {
    return llg_fork_impl(fn, name, grp, frame);
}

void llg_join(llg_fork_group_t* grp) {
    if (!grp || !region_can_mutate("fork wait scheduling")) return;
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
    if (!p || !region_can_mutate("fork wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK;
    w->grp = grp;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    aco_yield();
}

void llg_wait_fork(void) {
    llg_proc_t* p = llg_current();
    if (!p || p->fork_groups == NULL || !region_can_mutate("fork wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_FORK_ALL;
    w->parent = p;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    aco_yield();
}

void llg_disable_fork(void) {
    if (!region_can_mutate("fork scheduling")) return;
    llg_proc_t* current = llg_current();
    if (!current) return;
    llg_kill_proc_groups(current);
    reap_retired_procs();
}

static int activation_has_disabled_ancestor(llg_activation_t* activation) {
    for (; activation; activation = activation->parent) {
        if (activation->disabled) return 1;
    }
    return 0;
}

static void llg_kill_named_group(llg_fork_group_t* grp) {
    if (!grp || !grp->parent) return;
    llg_proc_t* parent = grp->parent;
    llg_fork_child_t* child = grp->children;
    while (child) {
        if (child->proc) {
            llg_kill_proc_tree(child->proc);
            child->proc = NULL;
        }
        child = child->next;
    }
    grp->remaining = 0;
    llg_fork_group_t** pp = &parent->fork_groups;
    while (*pp && *pp != grp) pp = &(*pp)->next_g;
    if (*pp == grp) *pp = grp->next_g;
    grp->next_g = g.zombie_groups;
    g.zombie_groups = grp;

    if (parent->wait.kind == W_FORK && parent->wait.grp == grp) {
        wake_proc(parent);
    }
    if (parent->wait.kind == W_FORK_ALL && parent->fork_groups == NULL) {
        wake_proc(parent);
    }
}

void llg_disable_target(uint32_t declaration, uint32_t instance) {
    if (!region_can_mutate("named activation scheduling")) return;
    llg_proc_t* current = llg_current();
    int matched = 0;
    for (llg_activation_t* activation = g.activations; activation;
         activation = activation->all_next) {
        if (activation->declaration == declaration &&
            activation->instance == instance) {
            activation->disabled = 1;
            matched = 1;
        }
    }

    // Restart after each removal: cancelling a tree can remove other groups
    // and processes from the registry, including the caller's ancestors.
    for (;;) {
        llg_fork_group_t* target = NULL;
        for (int i = 0; i < g.n_procs && !target; i++) {
            llg_proc_t* proc = g.all_procs[i];
            if (!proc) continue;
            for (llg_fork_group_t* group = proc->fork_groups; group;
                 group = group->next_g) {
                if ((group->has_target &&
                     group->target_declaration == declaration &&
                     group->target_instance == instance) ||
                    (group->owner_activation &&
                     activation_has_disabled_ancestor(group->owner_activation))) {
                    target = group;
                    break;
                }
            }
        }
        if (!target) break;
        matched = 1;
        llg_kill_named_group(target);
    }

    // A blocked activation must be detached from every wait list before its
    // coroutine is scheduled again. Immediate NBAs remain attached to the
    // process and are therefore committed normally after cancellation.
    if (matched) {
        for (llg_activation_t* activation = g.activations; activation;
             activation = activation->all_next) {
            if (activation->disabled && activation->proc &&
                activation->proc->wait.kind != W_NONE) {
                wake_proc(activation->proc);
            }
        }
    }
    reap_retired_procs();
    if (current && current->killed) {
        // Group accounting was already completed by cancellation. Do not call
        // llg_proc_done, which would decrement the group a second time.
        aco_exit();
        abort();
    }
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
            if (c->proc->fork_groups || c->proc->nba_head) {
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
            activation_release(grp->owner_activation);
            grp->owner_activation = NULL;
            free(grp);
        }
        grp = next_g;
    }
}

// ── Signal writes and waiter scanning ─────────────────────────────────────────

static int sv4_is_zero(sv4_t v) { return !sv4_is_unknown(v) && !sv4_to_bool(v); }
static int sv4_is_one(sv4_t v) { return !sv4_is_unknown(v) && sv4_to_bool(v); }

static void force_dependency_changed(sv4_t* sig, double* real, int is_real);
static void sig_write(sv4_t* target, sv4_t value);
static int pca_real_active(double* target);

void llg_dependency_bind(sv4_t* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid dependency binding\n");
        abort();
    }
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->target == target && binding->real_target == NULL && binding->dependency == dependency) return;
    }
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), "dependency binding");
    binding->target = target;
    binding->real_target = NULL;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
}

void llg_dependency_bind_real(double* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid real dependency binding\n");
        abort();
    }
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->real_target == target && binding->dependency == dependency) return;
    }
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), "real dependency binding");
    binding->target = NULL;
    binding->real_target = target;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
}

void llg_dependency_changed(sv4_t* dependency) {
    if (!dependency) return;
    sv4_t value = *dependency;
    value.width = 1;
    value.is_signed = 0;
    value.bits[0] ^= 1u;
    value.x[0] = 0;
    value.z[0] = 0;
    sig_write(dependency, value);
}

void llg_dependency_notify(sv4_t* contents, sv4_t* shape, int change) {
    if (change & 1)
        llg_dependency_changed(contents);
    if (change & 2)
        llg_dependency_changed(shape);
}

static int ev_matches(sv4_t old, sv4_t new, int kind) {
    if (kind == LLG_EV_ANY) return !sv4_same(old, new);
    // IEEE 1800-2009 9.4.2: vector edge controls observe only the LSB.
    old = sv4_bit_select(old, 0);
    new = sv4_bit_select(new, 0);
    if (kind == LLG_EV_POSEDGE) {
        return (sv4_is_zero(old) && (!sv4_is_zero(new))) ||
               (sv4_is_unknown(old) && sv4_is_one(new));
    }
    // negedge
    return (sv4_is_one(old) && (!sv4_is_one(new))) ||
           (sv4_is_unknown(old) && sv4_is_zero(new));
}

static int real_same(double old, double new) {
    uint64_t old_bits;
    uint64_t new_bits;
    memcpy(&old_bits, &old, sizeof(old_bits));
    memcpy(&new_bits, &new, sizeof(new_bits));
    return old_bits == new_bits;
}

static int real_ev_matches(double old, double new, int kind) {
    // Edge descriptors are rejected by lowering for real values. Keep the
    // runtime defensive: real event controls are any-change only.
    return kind == LLG_EV_ANY && !real_same(old, new);
}

static int dependency_matches(const llg_wait_dependency_t* dependency,
                              sv4_t* sig, double* real) {
    return (dependency->sig && dependency->sig == sig) ||
           (dependency->real && dependency->real == real);
}

static int expression_dependency_changed(const llg_expr_event_spec_t* spec,
                                          sv4_t* sig, double* real) {
    if ((spec->sig && spec->sig == sig) ||
        (spec->real_sig && spec->real_sig == real))
        return 1;
    if (spec->n_dependencies > 0) {
        for (int i = 0; i < spec->n_dependencies; i++) {
            if (dependency_matches(&spec->dependencies[i], sig, real)) return 1;
        }
    } else {
        for (int i = 0; i < spec->n_reads; i++) {
            if (spec->reads[i] == sig) return 1;
        }
    }
    return 0;
}

static int expression_update(llg_wait_t* wait, int index, sv4_t* sig,
                             double* real) {
    llg_expr_event_spec_t* spec = &wait->expressions[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) spec->real_eval(&value, spec->eval_context);
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(wait->real_last[index], value, spec->kind);
        wait->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    sv4_t value;
    if (spec->eval) spec->eval(&value, spec->eval_context);
    else if (spec->sig) value = *spec->sig;
    else return 0;
    int matched = ev_matches(wait->last[index], value, spec->kind);
    wait->last[index] = value;
    return matched && expression_qualifies(spec);
}

static int deferred_expression_update(llg_deferred_trigger_t* trigger,
                                      int index, sv4_t* sig, double* real) {
    llg_expr_event_spec_t* spec = &trigger->specs[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) spec->real_eval(&value, spec->eval_context);
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(trigger->real_last[index], value, spec->kind);
        trigger->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    sv4_t value;
    if (spec->eval) spec->eval(&value, spec->eval_context);
    else if (spec->sig) value = *spec->sig;
    else return 0;
    int matched = ev_matches(trigger->last[index], value, spec->kind);
    trigger->last[index] = value;
    return matched && expression_qualifies(spec);
}

static void invoke_deferred_action(llg_event_assignment_fn action,
                                   llg_frame_t* frame) {
    if (!action) {
        if (frame) llg_frame_release(frame);
        return;
    }
    int was_in_deferred_action = g.in_deferred_action;
    g.in_deferred_action = 1;
    action(frame);
    g.in_deferred_action = was_in_deferred_action;
    if (frame) llg_frame_release(frame);
}

static void deferred_trigger_fire(llg_deferred_trigger_t* trigger) {
    if (trigger->action) {
        llg_frame_t* frame = trigger->action_frame;
        trigger->action_frame = NULL;
        invoke_deferred_action(trigger->action, frame);
        return;
    }
    llg_nba_t* n = new_nba(0);
    if (!n) return;
    // The retained request is independent of both the issuer and the process
    // that happened to produce the matching source change.
    n->owner = NULL;
    n->event_target = trigger->target;
    n->is_event = 1;
    enqueue_nba(n);
}

static void deferred_trigger_source_change(sv4_t* sig, double* real) {
    llg_deferred_trigger_t** slot = &g.deferred_triggers;
    while (*slot) {
        llg_deferred_trigger_t* trigger = *slot;
        int matched = 0;
        for (int i = 0; i < trigger->n; i++) {
            if (deferred_expression_update(trigger, i, sig, real)) {
                matched = 1;
                break;
            }
        }
        if (!matched) {
            slot = &trigger->next;
            continue;
        }
        if (trigger->remaining > 1) {
            trigger->remaining--;
            slot = &trigger->next;
            continue;
        }
        *slot = trigger->next;
        if (g.deferred_trigger_tail == trigger) {
            g.deferred_trigger_tail = NULL;
            for (llg_deferred_trigger_t* tail = g.deferred_triggers; tail;
                 tail = tail->next)
                g.deferred_trigger_tail = tail;
        }
        deferred_trigger_fire(trigger);
        free_deferred_trigger(trigger);
    }
}

static void deferred_trigger_event(llg_event_object_t* ev) {
    if (!ev) return;
    llg_deferred_trigger_t** slot = &g.deferred_triggers;
    while (*slot) {
        llg_deferred_trigger_t* trigger = *slot;
        int matched = 0;
        for (int i = 0; i < trigger->n; i++) {
            llg_expr_event_spec_t* spec = &trigger->specs[i];
            if (spec->event_object == ev && expression_qualifies(spec)) {
                matched = 1;
                break;
            }
        }
        if (!matched) {
            slot = &trigger->next;
            continue;
        }
        if (trigger->remaining > 1) {
            trigger->remaining--;
            slot = &trigger->next;
            continue;
        }
        *slot = trigger->next;
        if (g.deferred_trigger_tail == trigger) {
            g.deferred_trigger_tail = NULL;
            for (llg_deferred_trigger_t* tail = g.deferred_triggers; tail;
                 tail = tail->next)
                g.deferred_trigger_tail = tail;
        }
        deferred_trigger_fire(trigger);
        free_deferred_trigger(trigger);
    }
}

static void sig_write(sv4_t* target, sv4_t value) {
    if (!region_can_mutate("signal write")) return;
    // Mask the written limbs to the vector's width before comparing/storing.
    for (int i = 0; i < (int)LLG_LIMBS; i++) {
        uint64_t m = llg_sv4_limb_mask(value.width, i);
        value.bits[i] &= m;
        value.x[i] &= m;
        value.z[i] &= m;
    }
    if (target->width == value.width && sv4_same(*target, value)) return;
    *target = value;
    if (g.mon.active) {
        for (int i = 0; i < g.mon.n_reads; i++) {
            if (g.mon.reads[i] == target) {
                g.mon.dirty = 1;
                break;
            }
        }
        for (int i = 0; i < g.mon.n_typed_reads; i++) {
            if (g.mon.typed_reads[i].kind == LLG_FMT_PACKED &&
                g.mon.typed_reads[i].ptr == target) {
                g.mon.dirty = 1;
                break;
            }
        }
    }
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
        } else if (w->kind == W_DEPS) {
            for (int i = 0; i < w->n; i++) {
                if (w->dependencies[i].sig == target) {
                    wake = 1;
                    break;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->n; i++) {
                if (expression_update(w, i, target, NULL)) wake = 1;
            }
        } else if (w->kind == W_LEVEL) {
            if (w->sig == target && sv4_same(*target, w->level_val)) wake = 1;
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
    deferred_trigger_source_change(target, NULL);
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(target, NULL, 0);
}

// Real equality is bitwise: repeated NaNs with the same payload are
// suppressed, while changes in NaN payload and signed zero are observable.
static void real_write(double* target, double value) {
    if (!region_can_mutate("real write")) return;
    double old = *target;
    if (real_same(old, value)) return;
    *target = value;
    if (g.mon.active) {
        for (int i = 0; i < g.mon.n_typed_reads; i++) {
            if (g.mon.typed_reads[i].kind == LLG_FMT_REAL &&
                g.mon.typed_reads[i].ptr == target) {
                g.mon.dirty = 1;
                break;
            }
        }
    }
#ifdef LLG_WAVEFORM
    llg_wave_changed_real(target, value, g.now);
#endif
    llg_wait_t* w = g.waiters;
    while (w) {
        llg_wait_t* next = w->next;
        int wake = 0;
        if (w->kind == W_DEPS) {
            for (int i = 0; i < w->n; i++) {
                if (w->dependencies[i].real == target) {
                    wake = 1;
                    break;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->n; i++) {
                if (expression_update(w, i, NULL, target)) wake = 1;
            }
        }
        if (wake) wake_proc(w->proc);
        w = next;
    }
    deferred_trigger_source_change(NULL, target);
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        if (binding->real_target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(NULL, target, 1);
}

// ── Procedural force / release ───────────────────────────────────────────────

static sv4_t llg_net_compute(const llg_net_t* net);
static void force_recompute_target(sv4_t* target, llg_net_t* net);
static void inertial_unlink_pending(llg_inertial_t* driver);

// Is `sig` currently covered by a packed force part? Procedural writes are
// dropped while a signal is forced; net driver slots remain writable so their
// current resolved value can be exposed on release.
static int llg_is_forced(sv4_t* sig) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int j = 0; j < entry->n_parts; j++)
            if (entry->parts[j].target == sig && sv4_to_bool(entry->masks[j])) return 1;
    }
    return 0;
}

static int llg_is_real_forced(double* target) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) return 1;
    }
    return 0;
}

static llg_pca_binding_t* pca_binding(sv4_t* target) {
    for (int i = 0; i < g.pca_count; i++) {
        if (g.pca_table[i].target == target) return &g.pca_table[i];
    }
    return NULL;
}

static int pca_active(sv4_t* target) {
    llg_pca_binding_t* binding = pca_binding(target);
    return binding && binding->active;
}

static llg_pca_real_binding_t* pca_real_binding(double* target) {
    for (int i = 0; i < g.pca_real_count; i++) {
        if (g.pca_real_table[i].target == target) return &g.pca_real_table[i];
    }
    return NULL;
}

static int pca_real_active(double* target) {
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    return binding && binding->active;
}

static void pca_set_enable(sv4_t* enable, int active) {
    sig_write(enable, sv4_from_u64(active ? 1 : 0, enable->width, enable->is_signed));
}

void llg_pca_assign(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding) {
        if (g.pca_count >= LLG_MAX_PCA) {
            fprintf(stderr, "llg: too many procedural continuous assignments (limit %d)\n",
                    LLG_MAX_PCA);
            abort();
        }
        binding = &g.pca_table[g.pca_count++];
        memset(binding, 0, sizeof(*binding));
        binding->target = target;
    }
    if (binding->active &&
        (binding->enable != enable || binding->site != site)) {
        pca_set_enable(binding->enable, 0);
    }
    binding->enable = enable;
    binding->site = site;
    binding->value = sv4_resize(value, target->width, target->is_signed);
    binding->active = 1;
    pca_set_enable(enable, 1);
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_drive(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active || binding->enable != enable || binding->site != site)
        return;
    binding->value = sv4_resize(value, target->width, target->is_signed);
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_deassign(sv4_t* target) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active) return;
    binding->active = 0;
    pca_set_enable(binding->enable, 0);
}

void llg_pca_assign_d(double* target, sv4_t* enable, uint64_t site, double value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding) {
        if (g.pca_real_count >= LLG_MAX_PCA) {
            fprintf(stderr, "llg: too many procedural continuous real assignments (limit %d)\n",
                    LLG_MAX_PCA);
            abort();
        }
        binding = &g.pca_real_table[g.pca_real_count++];
        memset(binding, 0, sizeof(*binding));
        binding->target = target;
    }
    if (binding->active &&
        (binding->enable != enable || binding->site != site)) {
        pca_set_enable(binding->enable, 0);
    }
    binding->enable = enable;
    binding->site = site;
    binding->value = value;
    binding->active = 1;
    pca_set_enable(enable, 1);
    if (!llg_is_real_forced(target)) real_write(target, value);
}

void llg_pca_drive_d(double* target, sv4_t* enable, uint64_t site, double value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding || !binding->active || binding->enable != enable || binding->site != site)
        return;
    binding->value = value;
    if (!llg_is_real_forced(target)) real_write(target, value);
}

void llg_pca_deassign_d(double* target) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding || !binding->active) return;
    binding->active = 0;
    pca_set_enable(binding->enable, 0);
}

static void force_free_entry(llg_force_entry_t* entry) {
    free(entry->parts);
    free(entry->masks);
    free(entry->reads);
    memset(entry, 0, sizeof(*entry));
}

static sv4_t force_part_mask(const llg_force_part_t* part) {
    if (!part->target || part->target->width > sizeof(part->target->bits) * 8u ||
        part->width > sizeof(part->target->bits) * 8u) {
        fprintf(stderr, "llg: invalid force target or width\n");
        abort();
    }
    sv4_t mask = sv4_from_u64(0, part->target->width, 0);
    sv4_t ones = sv4_from_u64(0, part->width, 0);
    for (uint32_t bit = 0; bit < part->width; bit++)
        ones.bits[bit / 64u] |= UINT64_C(1) << (bit % 64u);
    sv4_part_select_set(&mask, part->left, part->right, ones);
    return mask;
}

// Force replacement and release affect target bits, not the shape of the
// original LHS or its RHS offsets. Overwritten forces never become active again.
static void force_remove_coverage(const llg_force_part_t* parts, int n_parts) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        int remains = 0;
        for (int j = 0; j < entry->n_parts; j++) {
            sv4_t* mask = &entry->masks[j];
            for (int k = 0; k < n_parts; k++) {
                if (entry->parts[j].target != parts[k].target) continue;
                sv4_t removed = force_part_mask(&parts[k]);
                for (int limb = 0; limb < ((mask->width + 63u) / 64u); limb++)
                    mask->bits[limb] &= ~removed.bits[limb];
            }
            remains |= sv4_to_bool(*mask);
        }
        if (!remains) force_free_entry(entry);
    }
}

static int force_find_free_slot(void) {
    for (int i = 0; i < g.force_count; i++)
        if (!g.force_table[i].active) return i;
    if (g.force_count >= LLG_MAX_FORCE) return -1;
    return g.force_count++;
}

static llg_force_entry_t* force_prepare_packed(
    const llg_force_part_t* parts, int n_parts, uint32_t stream_slice,
    int stream_right_to_left, llg_force_eval_fn eval,
    const llg_force_read_t* reads, int n_reads) {
    if (!parts || n_parts <= 0 || n_reads < 0) {
        fprintf(stderr, "llg: invalid packed force descriptor\n");
        abort();
    }
    force_remove_coverage(parts, n_parts);
    int slot = force_find_free_slot();
    if (slot < 0) {
        fprintf(stderr, "llg: too many forced targets (limit %d)\n", LLG_MAX_FORCE);
        abort();
    }
    llg_force_entry_t* entry = &g.force_table[slot];
    memset(entry, 0, sizeof(*entry));
    entry->parts = (llg_force_part_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->parts), "force parts");
    memcpy(entry->parts, parts, (size_t)n_parts * sizeof(*parts));
    entry->masks = (sv4_t*)llg_checked_malloc(
        (size_t)n_parts, sizeof(*entry->masks), "force coverage masks");
    for (int i = 0; i < n_parts; i++) entry->masks[i] = force_part_mask(&parts[i]);
    if (n_reads > 0) {
        if (!reads) {
            fprintf(stderr, "llg: force dependency count has no descriptor array\n");
            abort();
        }
        entry->reads = (llg_force_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(*entry->reads), "force dependencies");
        memcpy(entry->reads, reads, (size_t)n_reads * sizeof(*reads));
    }
    entry->active = 1;
    entry->is_real = 0;
    entry->n_parts = n_parts;
    entry->stream_slice = stream_slice;
    entry->stream_right_to_left = stream_right_to_left;
    entry->eval = eval;
    entry->real_eval = NULL;
    entry->n_reads = n_reads;
    return entry;
}

static int force_read_matches(const llg_force_entry_t* entry, sv4_t* sig,
                              double* real, int is_real) {
    for (int i = 0; i < entry->n_reads; i++) {
        const llg_force_read_t* read = &entry->reads[i];
        if (read->is_real == is_real &&
            (is_real ? read->real == real : read->sig == sig))
            return 1;
    }
    return 0;
}

static void force_apply_part(sv4_t* target, const llg_force_part_t* part,
                             const sv4_t* mask, const sv4_t* value) {
    if (part->width == 0 || !sv4_to_bool(*mask)) return;
    uint64_t high = (uint64_t)part->value_lsb + part->width - 1;
    sv4_t selected = sv4_part_select(*value, (int64_t)high,
                                     (int64_t)part->value_lsb);
    if (part->two_state) selected = sv4_to_two_state(selected);
    sv4_t updated = *target;
    sv4_part_select_set(&updated, part->left, part->right, selected);
    for (uint32_t bit = 0; bit < target->width; bit++) {
        if ((mask->bits[bit / 64u] >> (bit % 64u)) & UINT64_C(1))
            sv4_part_select_set(target, bit, bit, sv4_part_select(updated, bit, bit));
    }
}

static llg_net_t* force_net_for_target(sv4_t* target, llg_net_t* fallback) {
    if (fallback) return fallback;
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int j = 0; j < entry->n_parts; j++) {
            if (entry->parts[j].target == target && entry->parts[j].net)
                return entry->parts[j].net;
        }
    }
    return NULL;
}

static void force_recompute_target(sv4_t* target, llg_net_t* net) {
    net = force_net_for_target(target, net);
    if (net && net->propagation) inertial_unlink_pending(net->propagation);
    sv4_t value;
    if (net) {
        value = llg_net_compute(net);
    } else {
        llg_pca_binding_t* pca = pca_binding(target);
        value = pca && pca->active ? pca->value : *target;
    }
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        sv4_t streamed = entry->value;
        if (entry->stream_slice)
            streamed = sv4_unstream(streamed, entry->stream_slice,
                                    entry->stream_right_to_left);
        for (int j = 0; j < entry->n_parts; j++) {
            llg_force_part_t* part = &entry->parts[j];
            if (part->target == target)
                force_apply_part(&value, part, &entry->masks[j], &streamed);
        }
    }
    sig_write(target, value);
}

static void force_entry_targets(const llg_force_entry_t* entry) {
    for (int i = 0; i < entry->n_parts; i++) {
        sv4_t* target = entry->parts[i].target;
        int seen = 0;
        for (int j = 0; j < i; j++)
            if (entry->parts[j].target == target) seen = 1;
        if (!seen) force_recompute_target(target, entry->parts[i].net);
    }
}

static void force_evaluate_entry(llg_force_entry_t* entry) {
    if (!entry->active || entry->evaluating) return;
    entry->evaluating = 1;
    if (entry->is_real) {
        if (entry->real_eval) entry->real_eval(&entry->real_value);
        real_write(entry->real_target, entry->real_value);
    } else {
        if (entry->eval) entry->eval(&entry->value);
        force_entry_targets(entry);
    }
    entry->evaluating = 0;
}

static void force_dependency_changed(sv4_t* sig, double* real, int is_real) {
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && force_read_matches(entry, sig, real, is_real))
            force_evaluate_entry(entry);
    }
}

void llg_force_expr_parts(const llg_force_part_t* parts, int n_parts,
                          uint32_t stream_slice, int stream_right_to_left,
                          llg_force_eval_fn eval,
                          const llg_force_read_t* reads, int n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    llg_force_entry_t* entry = force_prepare_packed(
        parts, n_parts, stream_slice, stream_right_to_left, eval, reads, n_reads);
    force_evaluate_entry(entry);
}

void llg_force_real(double* target, llg_force_real_eval_fn eval,
                    const llg_force_read_t* reads, int n_reads) {
    if (!region_can_mutate("force scheduling")) return;
    if (!target || !eval || n_reads < 0 || (n_reads > 0 && !reads)) {
        fprintf(stderr, "llg: invalid real force descriptor\n");
        abort();
    }
    llg_force_entry_t* entry = NULL;
    for (int i = 0; i < g.force_count; i++) {
        if (g.force_table[i].active && g.force_table[i].is_real &&
            g.force_table[i].real_target == target) {
            entry = &g.force_table[i];
            free(entry->reads);
            entry->reads = NULL;
            break;
        }
    }
    if (!entry) {
        int slot = force_find_free_slot();
        if (slot < 0) {
            fprintf(stderr, "llg: too many forced targets (limit %d)\n", LLG_MAX_FORCE);
            abort();
        }
        entry = &g.force_table[slot];
        memset(entry, 0, sizeof(*entry));
    }
    if (n_reads > 0) {
        entry->reads = (llg_force_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(*entry->reads), "real force dependencies");
        memcpy(entry->reads, reads, (size_t)n_reads * sizeof(*reads));
    }
    entry->active = 1;
    entry->is_real = 1;
    entry->real_target = target;
    entry->real_eval = eval;
    entry->eval = NULL;
    entry->n_reads = n_reads;
    force_evaluate_entry(entry);
}

void llg_release_parts(const llg_force_part_t* parts, int n_parts,
                       uint32_t stream_slice, int stream_right_to_left) {
    if (!region_can_mutate("force scheduling")) return;
    if (!parts || n_parts <= 0) return;
    (void)stream_slice;
    (void)stream_right_to_left;
    // Preserve net metadata before removing the final force on a target.
    llg_net_t** nets = llg_checked_malloc(
        (size_t)n_parts, sizeof(*nets), "released force nets");
    for (int i = 0; i < n_parts; i++)
        nets[i] = force_net_for_target(parts[i].target, parts[i].net);
    force_remove_coverage(parts, n_parts);
    for (int i = 0; i < n_parts; i++) {
        sv4_t* target = parts[i].target;
        int seen = 0;
        for (int j = 0; j < i; j++)
            if (parts[j].target == target) seen = 1;
        if (seen) continue;
        force_recompute_target(target, nets[i]);
        llg_pca_binding_t* pca = pca_binding(target);
        if (pca && pca->active) {
            pca_set_enable(pca->enable, 0);
            pca_set_enable(pca->enable, 1);
        }
    }
    free(nets);
}

void llg_release_real(double* target) {
    if (!region_can_mutate("force scheduling")) return;
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active && entry->is_real && entry->real_target == target) {
            // A procedural real variable retains the forced value; no saved
            // value exists to restore.
            force_free_entry(entry);
            llg_pca_real_binding_t* pca = pca_real_binding(target);
            if (pca && pca->active) {
                pca_set_enable(pca->enable, 0);
                pca_set_enable(pca->enable, 1);
            }
            return;
        }
    }
}

void llg_force(sv4_t* sig, sv4_t value) {
    if (!region_can_mutate("force scheduling")) return;
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)sig->width - 1, 0, sig->width, 0, 0
    };
    llg_force_entry_t* entry = force_prepare_packed(&part, 1, 0, 0, NULL, NULL, 0);
    entry->value = value;
    force_entry_targets(entry);
}

void llg_release(sv4_t* sig) {
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)sig->width - 1, 0, sig->width, 0, 0
    };
    llg_release_parts(&part, 1, 0, 0);
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
        activation_release(grp->owner_activation);
        grp->owner_activation = NULL;
        free(grp);
        grp = next_g;
    }
}

static void free_proc_storage(llg_proc_t* p) {
    llg_nba_t* n = p->nba_head;
    while (n) {
        llg_nba_t* next = n->next;
        if (n->is_string) llg_string_destroy(&n->string_value);
        free(n);
        n = next;
    }
    event_unlink(&p->wait);
    event_triggered_unlink(&p->wait);
    free_expression_wait(&p->wait);
    free(p->wait.specs);
    free(p->wait.dependencies);
    free(p->wait.last);
    free(p->wait.real_last);
    free(p->wait.evs);
    free(p->wait.order_sequence);
    activation_unwind_proc(p);
    llg_frame_release(p->frame);
    p->frame = NULL;
    if (p->co) aco_destroy(p->co);
    free(p);
}

static void free_region_callbacks(void) {
    while (g.callbacks) {
        llg_region_callback_t* next = g.callbacks->next;
        free(g.callbacks);
        g.callbacks = next;
    }
}

static void free_sampled_values(void) {
    while (g.sampled) {
        llg_sampled_value_t* next = g.sampled->next;
        free(g.sampled);
        g.sampled = next;
    }
}

void llg_rt_cleanup(void) {
    for (int i = 0; i < g.force_count; i++) force_free_entry(&g.force_table[i]);
    while (g.inertial_drivers) {
        llg_inertial_t* driver = g.inertial_drivers;
        g.inertial_drivers = driver->next_all;
        *driver->handle = NULL;
        free(driver);
    }
    while (g.delayed_nbas) {
        llg_nba_t* next = g.delayed_nbas->next;
        if (g.delayed_nbas->is_string)
            llg_string_destroy(&g.delayed_nbas->string_value);
        free(g.delayed_nbas);
        g.delayed_nbas = next;
    }
    free_deferred_triggers();
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
        if (g.strobes->typed) {
            llg_fmt_args_destroy(g.strobes->typed_work, g.strobes->n);
            free(g.strobes->typed_work);
            free(g.strobes->scope);
        } else {
            free(g.strobes->work);
        }
        free(g.strobes);
        g.strobes = next;
    }
    g.strobe_tail = NULL;
    free(g.mon.fmt);
    free(g.mon.last);
    free(g.mon.work);
    free(g.mon.reads);
    llg_fmt_args_destroy(g.mon.typed_last, g.mon.n);
    llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
    free(g.mon.typed_last);
    free(g.mon.typed_work);
    free(g.mon.typed_reads);
    free(g.mon.scope);
    free_region_callbacks();
    free_sampled_values();

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    reap_retired_procs();
    if (g.share_stack) aco_share_stack_destroy(g.share_stack);
    if (g.main_co) aco_destroy(g.main_co);
    aco_gtls_co = NULL;
    if (llg_event_generation == UINT64_MAX) {
        // A process cannot execute enough complete runtime lifetimes to wrap
        // this counter in practice. Keep the fallback deterministic if a
        // hostile embedding nevertheless reaches the boundary.
        llg_event_generation = 1;
    } else {
        llg_event_generation++;
    }
    memset(&g, 0, sizeof(g));
    while (llg_dependency_bindings) {
        llg_dependency_binding_t* next = llg_dependency_bindings->next;
        free(llg_dependency_bindings);
        llg_dependency_bindings = next;
    }
}

void llg_rt_init_with_args(int argc, char** argv) {
    llg_rt_cleanup();
    llg_last_failure = 0;
    llg_last_config_error = 0;
    llg_n_finals = 0; // a fresh run never inherits final registrations
    if (!configure_limits()) {
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_configured_zero_loop_limit = g.zero_loop_limit;
    llg_configured_process_step_limit = g.process_step_limit;
    g.current_region = LLG_REGION_PREPONED;
    g.argc = argc > 0 ? argc : 0;
    g.argv = g.argc > 0 ? argv : NULL;
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
}

void llg_rt_init(void) {
    llg_rt_init_with_args(0, NULL);
}

// ── Command-line plusargs ───────────────────────────────────────────────────

typedef struct {
    char conversion;
    char* prefix;
    size_t prefix_len;
    char* suffix;
    size_t suffix_len;
} llg_plusarg_format_t;

static int llg_plusarg_conversion(int c) {
    c = tolower((unsigned char)c);
    return c == 'd' || c == 'h' || c == 'x' || c == 'o' || c == 'b' ||
           c == 'f' || c == 'e' || c == 'g' || c == 's';
}

static char llg_plusarg_normalize_conversion(char c) {
    c = (char)tolower((unsigned char)c);
    return c == 'x' ? 'h' : c;
}

static void llg_plusarg_format_free(llg_plusarg_format_t* format) {
    if (!format) return;
    free(format->prefix);
    free(format->suffix);
    memset(format, 0, sizeof(*format));
}

static int llg_plusarg_format_parse(const char* text,
                                    llg_plusarg_format_t* format) {
    if (!text || !format) return 0;
    memset(format, 0, sizeof(*format));
    size_t length = strlen(text);
    format->prefix = llg_checked_malloc(length + 1, 1, "plusarg format prefix");
    format->suffix = llg_checked_malloc(length + 1, 1, "plusarg format suffix");
    int after_conversion = 0;
    for (size_t i = 0; i < length;) {
        char c = text[i++];
        char* output = after_conversion ? format->suffix : format->prefix;
        size_t* output_len = after_conversion ? &format->suffix_len : &format->prefix_len;
        if (c != '%') {
            output[(*output_len)++] = c;
            continue;
        }
        if (i >= length) {
            llg_plusarg_format_free(format);
            return 0;
        }
        c = text[i++];
        if (c == '%') {
            output[(*output_len)++] = '%';
            continue;
        }
        if (c == '0') {
            if (i >= length) {
                llg_plusarg_format_free(format);
                return 0;
            }
            c = text[i++];
        }
        if (after_conversion || format->conversion || !llg_plusarg_conversion(c)) {
            llg_plusarg_format_free(format);
            return 0;
        }
        format->conversion = llg_plusarg_normalize_conversion(c);
        after_conversion = 1;
    }
    if (!format->conversion) {
        llg_plusarg_format_free(format);
        return 0;
    }
    format->prefix[format->prefix_len] = '\0';
    format->suffix[format->suffix_len] = '\0';
    return 1;
}

static int llg_plusarg_find(const llg_plusarg_format_t* format,
                            const char** value, size_t* value_len) {
    if (!format || !value || !value_len || !g.argv) return 0;
    for (int i = 1; i < g.argc; ++i) {
        const char* argument = g.argv[i];
        if (!argument || argument[0] != '+') continue;
        const char* body = argument + 1;
        size_t body_len = strlen(body);
        if (body_len < format->prefix_len ||
            memcmp(body, format->prefix, format->prefix_len) != 0) {
            continue;
        }
        const char* candidate = body + format->prefix_len;
        size_t candidate_len = body_len - format->prefix_len;
        if (candidate_len < format->suffix_len ||
            memcmp(candidate + candidate_len - format->suffix_len,
                   format->suffix, format->suffix_len) != 0) {
            continue;
        }
        *value = candidate;
        *value_len = candidate_len - format->suffix_len;
        return 1;
    }
    return 0;
}

static void llg_plusarg_set_bit(sv4_t* value, uint32_t index, int state) {
    if (!value || index >= value->width) return;
    int limb = (int)(index >> 6);
    uint64_t mask = 1ULL << (index & 63u);
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
}

static void llg_plusarg_mask_top(sv4_t* value) {
    if (!value || value->width == 0 || value->width % 64u == 0) return;
    uint64_t mask = (1ULL << (value->width % 64u)) - 1ULL;
    int limb = (int)(value->width / 64u);
    value->bits[limb] &= mask;
    value->x[limb] &= mask;
    value->z[limb] &= mask;
}

static void llg_plusarg_unknown(sv4_t* value) {
    if (!value) return;
    memset(value->bits, 0, sizeof(value->bits));
    memset(value->x, 0xff, sizeof(value->x));
    memset(value->z, 0, sizeof(value->z));
    llg_plusarg_mask_top(value);
}

static int llg_plusarg_digit(int c, int base) {
    int digit = -1;
    if (c >= '0' && c <= '9') digit = c - '0';
    else if (c >= 'a' && c <= 'f') digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'F') digit = c - 'A' + 10;
    return digit >= 0 && digit < base ? digit : -1;
}

static int llg_plusarg_unknown_digit(int c) {
    return c == 'x' || c == 'X' || c == 'z' || c == 'Z';
}

static int llg_plusarg_decimal(const char* text, size_t length, sv4_t* output) {
    size_t start = 0;
    int negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start] == '-';
        ++start;
    }
    uint64_t limbs[LLG_LIMBS] = {0};
    int digits = 0;
    int unknown = 0;
    for (size_t i = start; i < length; ++i) {
        char c = text[i];
        if (c == '_') continue;
        if (llg_plusarg_unknown_digit(c)) {
            unknown = 1;
            digits = 1;
            continue;
        }
        int digit = llg_plusarg_digit((unsigned char)c, 10);
        if (digit < 0) return 0;
        digits = 1;
        uint64_t carry = (uint64_t)digit;
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
            uint64_t low = (limbs[limb] & UINT32_MAX) * 10ULL + carry;
            uint64_t high = (limbs[limb] >> 32) * 10ULL + (low >> 32);
            limbs[limb] = (high << 32) | (low & UINT32_MAX);
            carry = high >> 32;
        }
    }
    if (!digits) return 0;
    if (unknown) {
        llg_plusarg_unknown(output);
        return 1;
    }
    memcpy(output->bits, limbs, sizeof(limbs));
    memset(output->x, 0, sizeof(output->x));
    memset(output->z, 0, sizeof(output->z));
    if (negative) {
        uint64_t carry = 1;
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
            uint64_t inverted = ~output->bits[limb];
            uint64_t sum = inverted + carry;
            carry = sum < inverted;
            output->bits[limb] = sum;
        }
    }
    llg_plusarg_mask_top(output);
    return 1;
}

static int llg_plusarg_based(const char* text, size_t length, int base,
                             int bits_per_digit, sv4_t* output) {
    size_t start = 0;
    int negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start] == '-';
        ++start;
    }
    if (length - start >= 2 && text[start] == '0' &&
        ((base == 16 && (text[start + 1] == 'x' || text[start + 1] == 'X')) ||
         (base == 8 && (text[start + 1] == 'o' || text[start + 1] == 'O')) ||
         (base == 2 && (text[start + 1] == 'b' || text[start + 1] == 'B')))) {
        start += 2;
    }
    uint32_t bit = 0;
    int digits = 0;
    int unknown = 0;
    for (size_t i = length; i > start;) {
        char c = text[--i];
        if (c == '_') continue;
        int digit = llg_plusarg_digit((unsigned char)c, base);
        int state = 0;
        if (digit >= 0) state = 0;
        else if (llg_plusarg_unknown_digit((unsigned char)c)) {
            state = c == 'z' || c == 'Z' ? 3 : 2;
            unknown = 1;
        } else {
            return 0;
        }
        ++digits;
        for (int j = 0; j < bits_per_digit; ++j) {
            if (state != 0) {
                llg_plusarg_set_bit(output, bit + (uint32_t)j, state);
            } else {
                llg_plusarg_set_bit(output, bit + (uint32_t)j,
                                    (digit >> j) & 1);
            }
        }
        if (bit <= UINT32_MAX - (uint32_t)bits_per_digit) bit += (uint32_t)bits_per_digit;
    }
    if (!digits) return 0;
    if (negative && unknown) {
        llg_plusarg_unknown(output);
        return 1;
    }
    if (negative) {
        uint64_t carry = 1;
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
            uint64_t inverted = ~output->bits[limb];
            uint64_t sum = inverted + carry;
            carry = sum < inverted;
            output->bits[limb] = sum;
        }
        llg_plusarg_mask_top(output);
    }
    return 1;
}

static int llg_plusarg_real_value(const char* text, size_t length,
                                  double* output) {
    if (length == 0) {
        *output = 0.0;
        return 1;
    }
    char* copy = llg_checked_malloc(length + 1, 1, "plusarg real value");
    memcpy(copy, text, length);
    copy[length] = '\0';
    char* end = NULL;
    double parsed = strtod(copy, &end);
    int converted = end != copy && *end == '\0' && isfinite(parsed);
    if (converted) *output = parsed;
    free(copy);
    return converted;
}

static int llg_plusarg_packed_value(const char* text, size_t length,
                                     char conversion, sv4_t* output) {
    *output = sv4_from_u64(0, output->width, output->is_signed);
    if (length == 0) return 1;
    switch (conversion) {
        case 'd': return llg_plusarg_decimal(text, length, output);
        case 'h': return llg_plusarg_based(text, length, 16, 4, output);
        case 'o': return llg_plusarg_based(text, length, 8, 3, output);
        case 'b': return llg_plusarg_based(text, length, 2, 1, output);
        case 's': {
            // A packed string destination receives the rightmost bytes of
            // the argument, with zero extension on the left, just like an
            // integral assignment from a SystemVerilog string value.
            uint32_t bit = 0;
            for (size_t i = length; i > 0 && bit < output->width; --i) {
                unsigned char byte = (unsigned char)text[i - 1];
                for (int j = 0; j < 8 && bit + (uint32_t)j < output->width; ++j) {
                    llg_plusarg_set_bit(output, bit + (uint32_t)j,
                                        (byte >> j) & 1);
                }
                if (bit <= UINT32_MAX - 8u) bit += 8u;
            }
            llg_plusarg_mask_top(output);
            return 1;
        }
        case 'f':
        case 'e':
        case 'g': {
            double real = 0.0;
            if (!llg_plusarg_real_value(text, length, &real)) return 0;
            *output = sv4_from_real(real, output->width, output->is_signed);
            return 1;
        }
        default: return 0;
    }
}

static void llg_plusarg_to_two_state(sv4_t* value) {
    if (!value) return;
    for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
        value->bits[limb] &= ~(value->x[limb] | value->z[limb]);
        value->x[limb] = 0;
        value->z[limb] = 0;
    }
}

int llg_test_plusargs(const char* pattern) {
    if (!pattern || !g.argv) return 0;
    size_t length = strlen(pattern);
    for (int i = 1; i < g.argc; ++i) {
        const char* argument = g.argv[i];
        if (!argument || argument[0] != '+') continue;
        if (strncmp(argument + 1, pattern, length) == 0) return 1;
    }
    return 0;
}

int llg_value_plusargs_packed(const char* format_text, sv4_t* out,
                              uint32_t width, int is_signed, int two_state) {
    if (!out || width == 0 || width > LLG_MAX_WIDTH) return 0;
    llg_plusarg_format_t format;
    if (!llg_plusarg_format_parse(format_text, &format)) return 0;
    const char* value = NULL;
    size_t value_len = 0;
    int matched = llg_plusarg_find(&format, &value, &value_len);
    sv4_t parsed = sv4_from_u64(0, width, (int8_t)is_signed);
    int converted = 0;
    if (matched) {
        converted = llg_plusarg_packed_value(
            value, value_len, format.conversion, &parsed);
        if (!converted) {
            // A matching plusarg with an illegal value is still a successful
            // query; the LRM specifies an all-X packed result for this case.
            llg_plusarg_unknown(&parsed);
            converted = 1;
        }
        if (two_state) llg_plusarg_to_two_state(&parsed);
        *out = parsed;
    }
    llg_plusarg_format_free(&format);
    return converted;
}

int llg_value_plusargs_real(const char* format_text, double* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, &format) ||
        format.conversion == 's') {
        llg_plusarg_format_free(&format);
        return 0;
    }
    const char* value = NULL;
    size_t value_len = 0;
    int converted = 0;
    if (llg_plusarg_find(&format, &value, &value_len)) {
        if (format.conversion == 'f' || format.conversion == 'e' ||
            format.conversion == 'g') {
            converted = llg_plusarg_real_value(value, value_len, out);
            if (!converted) {
                // A matching malformed real conversion has no four-state
                // representation; keep the successful query and deterministic
                // zero result used by the real assignment path.
                *out = 0.0;
                converted = 1;
            }
        } else {
            sv4_t parsed = sv4_from_u64(
                0, LLG_MAX_WIDTH, value_len > 0 && value[0] == '-');
            converted = llg_plusarg_packed_value(
                value, value_len, format.conversion, &parsed);
            if (converted) *out = sv4_to_real(parsed);
            else *out = 0.0;
            // A matching malformed integral conversion is represented as X
            // before assignment to real storage, which yields zero here.
            converted = 1;
        }
    }
    llg_plusarg_format_free(&format);
    return converted;
}

int llg_value_plusargs_string(const char* format_text, llg_string_t* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, &format) ||
        format.conversion != 's') {
        llg_plusarg_format_free(&format);
        return 0;
    }
    const char* value = NULL;
    size_t value_len = 0;
    int converted = llg_plusarg_find(&format, &value, &value_len);
    if (converted) llg_string_move(out, llg_string_bytes(value, value_len));
    llg_plusarg_format_free(&format);
    return converted;
}

static void report_finish(int verbosity, const char* location) {
    if (verbosity >= 1) {
        fprintf(stderr, "llg: $finish at time %llu",
                (unsigned long long)g.now);
        if (location && location[0] != '\0') fprintf(stderr, " at %s", location);
        fputc('\n', stderr);
    }
    if (verbosity >= 2) {
        fprintf(stderr, "llg: simulation statistics: processes=%d\n", g.n_procs);
    }
}

_Noreturn void llg_rt_finish_with_level(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $finish verbosity %d\n", verbosity);
        abort();
    }
    report_finish(verbosity, location);
    g.finish = 1;
    llg_proc_done(llg_current());
}

_Noreturn void llg_rt_finish(void) {
    llg_rt_finish_with_level(0, NULL);
}

void llg_rt_request_finish(void) {
    g.finish = 1;
}

uint64_t llg_time(void) { return g.now; }

static void insert_region_callback(llg_region_callback_t* entry) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && ((*slot)->time < entry->time ||
                     ((*slot)->time == entry->time &&
                      ((*slot)->region < entry->region ||
                       ((*slot)->region == entry->region &&
                        (*slot)->sequence < entry->sequence))))) {
        slot = &(*slot)->next;
    }
    entry->next = *slot;
    *slot = entry;
}

int llg_schedule_region_callback_after(llg_region_t region,
                                       llg_region_callback_fn callback,
                                       void* data, uint64_t ticks) {
    if (!callback || !callback_region_allowed(region, ticks)) return 0;
    if (ticks > UINT64_MAX - g.now || g.callback_sequence == UINT64_MAX) {
        fprintf(stderr, "llg: fatal: region callback time or sequence overflow\n");
        abort();
    }
    llg_region_callback_t* entry = (llg_region_callback_t*)llg_checked_malloc(
        1, sizeof(*entry), "region callback");
    entry->region = region;
    entry->time = g.now + ticks;
    entry->sequence = g.callback_sequence++;
    entry->callback = callback;
    entry->data = data;
    entry->next = NULL;
    insert_region_callback(entry);
    return 1;
}

int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback_after(region, callback, data, 0);
}

int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback(region, callback, data);
}

void llg_sampled_register(sv4_t* signal) {
    if (!signal) {
        fprintf(stderr, "llg: cannot register a null sampled signal\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) return;
    }
    llg_sampled_value_t* item = (llg_sampled_value_t*)llg_checked_malloc(
        1, sizeof(*item), "sampled value");
    item->signal = signal;
    item->value = *signal;
    item->next = g.sampled;
    g.sampled = item;
}

const sv4_t* llg_sampled_value(const sv4_t* signal) {
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) return &item->value;
    }
    fprintf(stderr, "llg: sampled value requested for an unregistered signal\n");
    llg_last_failure = 1;
    g.finish = 1;
    return NULL;
}

int llg_sampled_copy(const sv4_t* signal, sv4_t* out) {
    if (!out) return 0;
    const sv4_t* value = llg_sampled_value(signal);
    if (!value) return 0;
    *out = *value;
    return 1;
}

static void sample_preponed_values(void) {
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next)
        item->value = *item->signal;
}

uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs) {
    if (unit_fs == 0) {
        fprintf(stderr, "llg runtime fatal: zero time unit\n");
        abort();
    }
#if defined(__SIZEOF_INT128__)
    __uint128_t physical = (__uint128_t)g.now * precision_fs;
    __uint128_t scaled = physical / unit_fs;
    __uint128_t remainder = physical % unit_fs;
    if (remainder >= (__uint128_t)unit_fs - remainder) {
        scaled++;
    }
    if (scaled > UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    return (uint64_t)scaled;
#else
    if (precision_fs != 0 && g.now > UINT64_MAX / precision_fs) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    uint64_t physical = g.now * precision_fs;
    uint64_t scaled = physical / unit_fs;
    uint64_t remainder = physical % unit_fs;
    if (remainder >= unit_fs - remainder) {
        if (scaled == UINT64_MAX) {
            fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
            abort();
        }
        scaled++;
    }
    return scaled;
#endif
}

int llg_rt_process_count(void) {
    int count = 0;
    for (int i = 0; i < g.n_procs; i++)
        if (g.all_procs[i]) count++;
    return count;
}

llg_proc_t* llg_spawn_in_region(void (*fn)(llg_proc_t*), const char* name,
                                llg_region_t region) {
    if (g.config_error || !fn || !region_valid(region)) return NULL;
    if (!callback_region_allowed(region, 0)) return NULL;
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(llg_proc_t), "process");
    p->name = name;
    p->fn = fn;
    p->budget_time = g.now;
    p->region = region;
    p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name) {
    return llg_spawn_in_region(fn, name, LLG_REGION_ACTIVE);
}

llg_frame_t* llg_proc_frame(llg_proc_t* self) {
    return self ? self->frame : NULL;
}

_Noreturn void llg_proc_done(llg_proc_t* self) {
    if (!self || self != llg_current()) {
        fprintf(stderr, "llg: process completion outside the current coroutine\n");
        abort();
    }
    // Natural process termination is the other join_none eligibility
    // boundary.  Release children before unwinding the creator's activation
    // and frame; their copied captures remain retained by the child process.
    start_pending_fork_children(self);
    self->completed = 1;
    activation_unwind_proc(self);
    llg_frame_release(self->frame);
    self->frame = NULL;
    if (self->grp) llg_fork_group_child_done(self->grp);
    aco_exit(); // never returns
}

void llg_wait_time(uint64_t ticks) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_TIME;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
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
        w->resume_region = region_is_reactive(p->region)
                               ? LLG_REGION_RE_INACTIVE
                               : LLG_REGION_INACTIVE;
        insert_zero_wait(w, w->resume_region);
    } else {
        insert_timed(w);
    }
    register_wait();
    aco_yield();
}

static llg_region_t take_wait_resume_region(llg_proc_t* p) {
    if (p->has_wait_resume_region) {
        p->has_wait_resume_region = 0;
        return p->wait_resume_region;
    }
    return region_is_reactive(p->region) ? LLG_REGION_REACTIVE : LLG_REGION_ACTIVE;
}

void llg_wait_resume_in_region(llg_region_t region) {
    llg_proc_t* p = llg_current();
    if (!p) {
        fprintf(stderr, "llg: wait region requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (!region_can_mutate("wait region scheduling")) return;
    p->wait_resume_region = region;
    p->has_wait_resume_region = 1;
}

void llg_wait_any(sv4_t** sigs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->resume_region = take_wait_resume_region(p);
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

void llg_wait_any_dependencies(const llg_wait_dependency_t* deps, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_DEPS;
    w->resume_region = take_wait_resume_region(p);
    w->n = n;
    w->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
    for (int i = 0; i < n; i++) {
        if ((deps[i].sig == NULL) == (deps[i].real == NULL)) {
            fprintf(stderr, "llg: typed wait dependency must name one storage kind\n");
            abort();
        }
        w->dependencies[i] = deps[i];
    }
    register_wait();
    aco_yield();
}

void llg_wait_any_events(llg_event_spec_t* specs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENTS;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
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
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_LEVEL;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->sig = sig;
    w->level_val = value;
    register_wait();
    aco_yield();
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────

// Return the outcome of one event observed by a wait_order waiter:
//  1 completes the sequence, 0 keeps waiting, and -1 takes the failure arm.
// Repeated occurrences of already-consumed events are ignored, while an
// event that is still ahead in the sequence is an ordering violation.
static int event_order_match(llg_wait_t* w, llg_event_object_t* ev) {
    if (!w->order_sequence || w->order_next < 0 ||
        w->order_next >= w->n_order)
        return -1;
    if (w->order_sequence[w->order_next] == ev) {
        w->order_next++;
        return w->order_next == w->n_order ? 1 : 0;
    }
    for (int i = 0; i < w->order_next; i++) {
        if (w->order_sequence[i] == ev) return 0;
    }
    return -1;
}

static void event_trigger_object(llg_event_object_t* ev) {
    if (!region_can_mutate("event scheduling")) return;
    if (!ev) return;

    // The state is tied to both the current simulation time and this runtime
    // generation. Comparing the generation avoids stale `.triggered` state
    // when a generated model is initialized again after cleanup; comparing
    // the time preserves all zero-delay deltas in the current slot.
    ev->triggered = 1;
    ev->triggered_time = g.now;
    ev->triggered_generation = llg_event_generation;

    int n_triggered = ev->n_triggered_waiters;
    llg_proc_t* triggered[LLG_MAX_EVENT_WAITERS];
    memcpy(triggered, ev->triggered_waiters,
           (size_t)n_triggered * sizeof(llg_proc_t*));
    ev->n_triggered_waiters = 0;
    for (int i = 0; i < n_triggered; i++) wake_proc(triggered[i]);

    int n = ev->n_waiters;
    // Snapshot and detach everyone first: wake_proc unlinks the waiter from
    // every event list it registered on, which must not fight the iteration
    // over this event's own table.  Wake order is the snapshot order, i.e.
    // the current table order: deterministic, and equal to registration
    // order unless earlier partial unlinks (swap-with-last) reordered it.
    llg_proc_t* wake[LLG_MAX_EVENT_WAITERS];
    memcpy(wake, ev->waiters, (size_t)n * sizeof(llg_proc_t*));
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        llg_wait_t* w = &wake[i]->wait;
        if (w->kind == W_EVENT_ORDER) {
            int result = event_order_match(w, ev);
            if (result != 0) {
                w->order_result_value = result;
                wake_proc(wake[i]);
            } else {
                event_list_add(ev, wake[i]);
            }
            continue;
        }
        int matched = w->kind != W_EXPR;
        if (!matched) {
            for (int j = 0; j < w->n; j++) {
                if (w->expressions[j].event_object == ev &&
                    expression_qualifies(&w->expressions[j]))
                    matched = 1;
            }
        }
        if (matched) wake_proc(wake[i]);
        else event_list_add(ev, wake[i]);
    }
    deferred_trigger_event(ev);
}

void llg_event_trigger(llg_event_t* ev) {
    event_trigger_object(ev ? ev->object : NULL);
}

int llg_event_triggered(const llg_event_t* ev) {
    if (!ev || !ev->object || !ev->object->triggered ||
        ev->object->triggered_generation != llg_event_generation)
        return 0;
    if (ev->object->triggered_time != g.now) {
        ev->object->triggered = 0;
        return 0;
    }
    return 1;
}

void llg_event_assign(llg_event_t* target, const llg_event_t* source) {
    if (!target || !region_can_mutate("event handle write")) return;
    target->object = source ? source->object : NULL;
}

void llg_event_assign_null(llg_event_t* target) {
    llg_event_assign(target, NULL);
}

llg_event_t* llg_event_array_select(llg_event_t* const* elements,
                                    uint64_t total,
                                    const int32_t* left,
                                    const int32_t* right,
                                    const sv4_t* indices,
                                    int n) {
    if (!elements || !left || !right || !indices || n <= 0 || !total)
        return NULL;
    uint64_t linear = 0;
    for (int i = 0; i < n; i++) {
        int64_t value;
        if (!sv4_to_index_i64(indices[i], &value)) return NULL;
        int64_t lo = left[i] < right[i] ? left[i] : right[i];
        int64_t hi = left[i] > right[i] ? left[i] : right[i];
        if (value < lo || value > hi) return NULL;
        uint64_t offset = left[i] >= right[i]
                              ? (uint64_t)((int64_t)left[i] - value)
                              : (uint64_t)(value - (int64_t)left[i]);
        uint64_t extent = (uint64_t)(llabs((int64_t)left[i] - right[i])) + 1;
        if (extent && linear > (UINT64_MAX - offset) / extent) return NULL;
        linear = linear * extent + offset;
    }
    return linear < total ? elements[linear] : NULL;
}

void llg_wait_event(llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    llg_wait_events(list, 1);
}

void llg_wait_events(const llg_event_t* const* evs, int n) {
    if (n <= 0) return;
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n_evs = n;
    w->evs = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "named-event wait list");
    for (int i = 0; i < n; i++) {
        w->evs[i] = evs[i] ? evs[i]->object : NULL;
        event_list_add(w->evs[i], p);
    }
    register_wait();
    aco_yield();
}

void llg_wait_event_triggered(const llg_event_t* ev) {
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    if (llg_event_triggered(ev)) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT_TRIGGERED;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->triggered_ev = ev ? ev->object : NULL;
    event_triggered_list_add(w->triggered_ev, p);
    register_wait();
    aco_yield();
}

void llg_wait_order(const llg_event_t* const* evs, int n, int* result) {
    if (n <= 0 || !result) return;
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    w->kind = W_EVENT_ORDER;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n_order = n;
    w->order_next = 0;
    w->order_result_value = 0;
    *result = 0;
    w->order_sequence = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order sequence");
    w->evs = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order event list");
    w->n_evs = 0;
    for (int i = 0; i < n; i++) {
        llg_event_object_t* object = evs[i] ? evs[i]->object : NULL;
        w->order_sequence[i] = object;
        if (!object) continue;
        int seen = 0;
        for (int j = 0; j < w->n_evs; j++) {
            if (w->evs[j] == object) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            w->evs[w->n_evs++] = object;
            event_list_add(object, p);
        }
    }
    register_wait();
    aco_yield();
    *result = w->order_result_value;
    w->order_result_value = 0;
}

void llg_wait_mixed(llg_wait_src_t* srcs, int n) {
    llg_proc_t* p = llg_current();
    if (!p || n < 0 || !region_can_mutate("wait scheduling")) return;
    llg_wait_t* w = &p->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n = nsig;
    w->specs = nsig ? (llg_event_spec_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(llg_event_spec_t), "mixed wait specifications") : NULL;
    w->last = nsig ? (sv4_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(sv4_t), "mixed wait snapshots") : NULL;
    w->n_evs = nev;
    w->evs = nev ? (llg_event_object_t**)llg_checked_malloc(
        (size_t)nev, sizeof(llg_event_object_t*), "mixed named-event wait list") : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            w->specs[si].sig = srcs[i].sig;
            w->specs[si].kind = srcs[i].kind;
            w->last[si] = *srcs[i].sig;
            si++;
        } else {
            w->evs[ei] = srcs[i].ev ? srcs[i].ev->object : NULL;
            event_list_add(w->evs[ei], p);
            ei++;
        }
    }
    register_wait();
    aco_yield();
}

void llg_wait_expressions(const llg_expr_event_spec_t* specs, int n) {
    if (n < 0) abort();
    llg_proc_t* p = llg_current();
    if (!p || !region_can_mutate("wait scheduling")) {
        release_expression_contexts(specs, n);
        return;
    }
    llg_wait_t* w = &p->wait;
    w->kind = W_EXPR;
    w->resume_region = region_is_reactive(p->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->n = n;
    w->n_evs = 0;
    w->expressions = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(llg_expr_event_spec_t), "expression event descriptors");
    w->last = (sv4_t*)llg_checked_malloc((size_t)n, sizeof(sv4_t), "expression event snapshots");
    w->real_last = (double*)llg_checked_malloc(
        (size_t)n, sizeof(double), "real expression event snapshots");
    w->evs = (llg_event_object_t**)llg_checked_malloc((size_t)n, sizeof(llg_event_object_t*), "expression named events");
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        w->expressions[i] = specs[i];
        w->expressions[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        w->expressions[i].reads = NULL;
        w->expressions[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            w->expressions[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "expression dependencies");
            memcpy(w->expressions[i].reads, specs[i].reads, (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            w->expressions[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "typed expression dependencies");
            for (int j = 0; j < specs[i].n_dependencies; j++) {
                if ((specs[i].dependencies[j].sig == NULL) ==
                    (specs[i].dependencies[j].real == NULL))
                    abort();
            }
            memcpy(w->expressions[i].dependencies, specs[i].dependencies,
                   (size_t)specs[i].n_dependencies * sizeof(llg_wait_dependency_t));
        }
        if (specs[i].event) {
            llg_event_object_t* object = specs[i].event->object;
            int seen = 0;
            for (int j = 0; j < w->n_evs; j++) if (w->evs[j] == object) seen = 1;
            if (!seen) {
                w->evs[w->n_evs++] = object;
                event_list_add(object, p);
            }
        } else if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&w->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig) w->real_last[i] = *specs[i].real_sig;
            else abort();
        } else if (specs[i].eval) {
            specs[i].eval(&w->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            w->last[i] = *specs[i].sig;
        } else {
            abort();
        }
    }
    register_wait();
    aco_yield();
}

uint64_t llg_repeat_count(sv4_t value) {
    sv4_t count = sv4_repeat_count(value);
    for (int i = 1; i < llg_sv4_nlimbs(count.width); i++) {
        if (count.bits[i]) {
            fprintf(stderr,
                    "llg runtime fatal: nonblocking repeat count exceeds 64 bits\n");
            abort();
        }
    }
    return sv4_to_u64(count);
}

static void register_deferred_trigger(const llg_expr_event_spec_t* specs,
                                      int n, uint64_t repeat,
                                      llg_event_object_t* target,
                                      llg_event_assignment_fn action,
                                      llg_frame_t* action_frame) {
    if (n < 0) abort();
    if ((!target && !action) ||
        !region_can_mutate("nonblocking event registration")) {
        release_expression_contexts(specs, n);
        if (action_frame) llg_frame_release(action_frame);
        return;
    }
    if (!repeat || n == 0) {
        release_expression_contexts(specs, n);
        if (action) {
            invoke_deferred_action(action, action_frame);
        } else if (target) {
            llg_nba_t* nba = new_nba(0);
            if (nba) {
                nba->event_target = target;
                nba->is_event = 1;
                enqueue_nba(nba);
            }
            if (action_frame) llg_frame_release(action_frame);
        } else if (action_frame) {
            llg_frame_release(action_frame);
        }
        return;
    }
    if (!specs) abort();
    llg_deferred_trigger_t* trigger = (llg_deferred_trigger_t*)llg_checked_calloc(
        1, sizeof(*trigger), "deferred nonblocking event trigger");
    trigger->target = target;
    trigger->action = action;
    trigger->action_frame = action_frame;
    trigger->n = n;
    trigger->remaining = repeat;
    trigger->specs = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->specs), "deferred event descriptors");
    trigger->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->last), "deferred event snapshots");
    trigger->real_last = (double*)llg_checked_calloc(
        (size_t)n, sizeof(*trigger->real_last), "deferred real event snapshots");
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        trigger->specs[i] = specs[i];
        trigger->specs[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        trigger->specs[i].reads = NULL;
        trigger->specs[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            trigger->specs[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "deferred event dependencies");
            memcpy(trigger->specs[i].reads, specs[i].reads,
                   (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            trigger->specs[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "deferred typed event dependencies");
            memcpy(trigger->specs[i].dependencies, specs[i].dependencies,
                   (size_t)specs[i].n_dependencies * sizeof(llg_wait_dependency_t));
        }
        if (specs[i].event) continue;
        if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&trigger->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig)
                trigger->real_last[i] = *specs[i].real_sig;
            else
                abort();
        } else if (specs[i].eval) {
            specs[i].eval(&trigger->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            trigger->last[i] = *specs[i].sig;
        } else {
            abort();
        }
    }
    if (g.deferred_trigger_tail)
        g.deferred_trigger_tail->next = trigger;
    else
        g.deferred_triggers = trigger;
    g.deferred_trigger_tail = trigger;
}

void llg_nba_event_when(const llg_expr_event_spec_t* specs, int n,
                        llg_event_t* target, uint64_t repeat) {
    register_deferred_trigger(specs, n, repeat,
                              target ? target->object : NULL, NULL, NULL);
}

void llg_nba_event_assign_when(const llg_expr_event_spec_t* specs, int n,
                               uint64_t repeat,
                               llg_event_assignment_fn action,
                               llg_frame_t* frame) {
    if (!action) {
        release_expression_contexts(specs, n);
        if (frame) llg_frame_release(frame);
        return;
    }
    register_deferred_trigger(specs, n, repeat, NULL, action, frame);
}

static llg_nba_t* new_nba(uint64_t ticks) {
    if (!region_can_mutate("nonblocking scheduling")) return NULL;
    llg_proc_t* owner = g.in_deferred_action ? NULL : llg_current();
    if (ticks > UINT64_MAX - g.now || g.nba_sequence == UINT64_MAX) {
        fprintf(stderr, "llg: fatal: nonblocking assignment time or sequence overflow\n");
        abort();
    }
    llg_nba_t* n = (llg_nba_t*)llg_checked_malloc(1, sizeof(llg_nba_t), "nonblocking assignment");
    n->target = NULL;
    n->event_target = NULL;
    n->is_real = 0;
    n->is_event = 0;
    n->has_mask = 0;
    n->real_target = NULL;
    n->real_value = 0.0;
    n->is_string = 0;
    n->string_target = NULL;
    n->string_value = (llg_string_t){0};
    n->time = g.now + ticks;
    n->sequence = g.nba_sequence++;
    n->region = region_is_reactive(g.current_region)
                    ? LLG_REGION_RE_NBA
                    : LLG_REGION_NBA;
    n->owner = owner;
    n->next = NULL;
    return n;
}

static void enqueue_nba(llg_nba_t* n) {
    if (!n) return;
    if (n->time == g.now && n->owner) {
        llg_proc_t* p = n->owner;
        if (p->nba_tail) p->nba_tail->next = n;
        else p->nba_head = n;
        p->nba_tail = n;
    } else {
        llg_nba_t** slot = &g.delayed_nbas;
        while (*slot && (*slot)->time <= n->time) slot = &(*slot)->next;
        n->next = *slot;
        *slot = n;
    }
}

void llg_nba_after(sv4_t* target, sv4_t value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->value = value;
    enqueue_nba(n);
}

void llg_nba_event_after(llg_event_t* ev, uint64_t ticks) {
    if (!ev || !ev->object) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->event_target = ev->object;
    n->is_event = 1;
    enqueue_nba(n);
}

void llg_nba_event(llg_event_t* ev) {
    llg_nba_event_after(ev, 0);
}

void llg_nba(sv4_t* target, sv4_t value) {
    llg_nba_after(target, value, 0);
}

void llg_nba_masked(sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->value = value;
    n->mask = mask;
    n->has_mask = 1;
    enqueue_nba(n);
}

void llg_nba_d_after(double* target, double value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->is_real = 1;
    n->real_target = target;
    n->real_value = value;
    enqueue_nba(n);
}

void llg_string_nba_after(llg_string_t* target, llg_string_t value,
                          uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) {
        llg_string_destroy(&value);
        return;
    }
    n->is_string = 1;
    n->string_target = target;
    n->string_value = value;
    enqueue_nba(n);
}

void llg_ba(sv4_t* target, sv4_t value) {
    // Procedural writes cannot override either a force or a procedural
    // continuous assignment (LRM 10.6.1/10.6.2).
    if (llg_is_forced(target) || pca_active(target)) return;
    sig_write(target, value);
}

void llg_ref_write(llg_ref_t* ref, sv4_t value) {
    if (!ref) return;
    sv4_t converted = sv4_cast(value, ref->width, ref->is_signed);
    if (ref->two_state) converted = sv4_to_two_state(converted);
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->queue_write)
            (void)ref->queue_write(ref->queue, ref->queue_identity, converted);
        return;
    }
    if (!ref->base) return;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_WHOLE) {
        llg_ba(ref->base, converted);
        return;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
        if (ref->index == UINT64_MAX || ref->index >= ref->array_size) return;
        llg_ba(&ref->base[ref->index], converted);
        return;
    }
    sv4_t updated = *ref->base;
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_BIT:
        sv4_bit_select_set(&updated, ref->index, converted);
        break;
    case LLG_REF_PART:
        sv4_part_select_set(&updated, ref->left, ref->right, converted);
        break;
    case LLG_REF_INDEXED:
        sv4_idx_part_select_set(&updated, ref->index, ref->indexed_width,
                                ref->indexed_negative, converted);
        break;
    default:
        return;
    }
    llg_ba(ref->base, updated);
}

void llg_nba_d(double* target, double value) {
    llg_nba_d_after(target, value, 0);
}

void llg_ba_d(double* target, double value) {
    if (llg_is_real_forced(target) || pca_real_active(target)) return;
    real_write(target, value);
}

// ── Collapsed inout nets ──────────────────────────────────────────────────────

static sv4_t llg_net_compute(const llg_net_t* net) {
    return sv4_resolve_strengths(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->n_drivers, net->width, net->is_signed, net->resolution);
}

static void llg_net_publish(llg_net_t* net, sv4_t resolved) {
    if (net->propagation_enabled) {
        llg_inertial_assign(&net->propagation, &net->resolved, resolved,
                            net->propagation_rise, net->propagation_fall,
                            net->propagation_turn_off);
    } else {
        sig_write(&net->resolved, resolved);
    }
}

void llg_net_resolve(llg_net_t* net) {
    if (!region_can_mutate("net resolution")) return;
    if (llg_is_forced(&net->resolved)) force_recompute_target(&net->resolved, net);
    else llg_net_publish(net, llg_net_compute(net));
}

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    if (!region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers) return;
    value = sv4_resize(value, net->width, net->is_signed);
    sv4_t* slot = net->drivers[idx];
    if (!slot) return;
    if (slot->width == value.width && sv4_same(*slot, value)) return;
    *slot = value;
    // Driver slots continue changing while a net is forced. Recompute the
    // visible value underneath the override so release observes all current
    // contributions.
    sv4_t resolved = llg_net_compute(net);
    if (llg_is_forced(&net->resolved)) force_recompute_target(&net->resolved, net);
    else llg_net_publish(net, resolved);
}

static int inertial_bit(const sv4_t* value, uint32_t bit) {
    uint64_t mask = 1ULL << (bit % 64u);
    uint32_t limb = bit / 64u;
    if (value->x[limb] & mask) return 2;
    if (value->z[limb] & mask) return 3;
    return (value->bits[limb] & mask) ? 1 : 0;
}

static void inertial_set_bit(sv4_t* value, uint32_t bit, int state) {
    uint64_t mask = 1ULL << (bit % 64u);
    uint32_t limb = bit / 64u;
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
}

static int inertial_masked_same(const sv4_t* a, const sv4_t* b,
                                const sv4_t* mask) {
    uint32_t width = a->width < b->width ? a->width : b->width;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (mask && inertial_bit(mask, bit) != 1) continue;
        if (inertial_bit(a, bit) != inertial_bit(b, bit)) return 0;
    }
    return 1;
}

static void inertial_merge(sv4_t* target, const sv4_t* value,
                           const sv4_t* mask) {
    uint32_t width = target->width < value->width ? target->width : value->width;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (inertial_bit(mask, bit) == 1)
            inertial_set_bit(target, bit, inertial_bit(value, bit));
    }
}

enum {
    INERTIAL_NO_TRANSITION = 0,
    INERTIAL_RISE = 1,
    INERTIAL_FALL = 2,
    INERTIAL_TURN_OFF = 3,
    INERTIAL_RISE_OR_FALL = 4,
};

static int inertial_transition(int old_state, int new_state) {
    if (old_state == new_state) return INERTIAL_NO_TRANSITION;
    if (new_state == 3) return INERTIAL_TURN_OFF;
    if (new_state == 1) {
        return old_state == 0 || old_state == 2 || old_state == 3
                   ? INERTIAL_RISE
                   : INERTIAL_NO_TRANSITION;
    }
    if (new_state == 0) {
        return old_state == 1 || old_state == 2 || old_state == 3
                   ? INERTIAL_FALL
                   : INERTIAL_NO_TRANSITION;
    }
    // A transition to X is ambiguous: its delay is selected from every
    // possible stable destination (0, 1, or Z). The known endpoint remains
    // directional when the transition is X -> 0/1.
    if (old_state == 0 || old_state == 1 || old_state == 3)
        return INERTIAL_RISE_OR_FALL;
    return INERTIAL_NO_TRANSITION;
}

static uint64_t inertial_transition_ticks(const sv4_t* old_value,
                                           const sv4_t* new_value,
                                           const sv4_t* mask, uint64_t rise,
                                           uint64_t fall, uint64_t turn_off) {
    uint64_t selected = UINT64_MAX;
    uint32_t width = old_value->width < new_value->width
                         ? old_value->width
                         : new_value->width;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (mask && inertial_bit(mask, bit) != 1) continue;
        int transition = inertial_transition(
            inertial_bit(old_value, bit), inertial_bit(new_value, bit));
        uint64_t ticks;
        switch (transition) {
        case INERTIAL_RISE: ticks = rise; break;
        case INERTIAL_FALL: ticks = fall; break;
        case INERTIAL_TURN_OFF: ticks = turn_off; break;
        case INERTIAL_RISE_OR_FALL:
            ticks = rise < fall ? rise : fall;
            if (turn_off < ticks) ticks = turn_off;
            break;
        default: continue;
        }
        if (ticks < selected) selected = ticks;
    }
    return selected == UINT64_MAX ? 0 : selected;
}

static void inertial_unlink_pending(llg_inertial_t* driver) {
    if (!driver->pending) return;
    llg_inertial_t** entry = &g.inertial_pending;
    while (*entry && *entry != driver) entry = &(*entry)->next_pending;
    if (*entry) *entry = driver->next_pending;
    driver->pending = 0;
    driver->next_pending = NULL;
}

static void inertial_update(llg_inertial_t** handle, sv4_t* target,
                            llg_net_t* net, int slot, sv4_t value,
                            const sv4_t* mask, uint64_t rise, uint64_t fall,
                            uint64_t turn_off) {
    if (!region_can_mutate("inertial scheduling")) return;
    llg_inertial_t* driver = *handle;
    if (!driver) {
        driver = llg_checked_calloc(1, sizeof(*driver), "inertial driver");
        driver->handle = handle;
        driver->target = target;
        driver->net = net;
        driver->slot = slot;
        driver->current = *target;
        driver->region = region_is_reactive(g.current_region)
                             ? LLG_REGION_REACTIVE
                             : LLG_REGION_ACTIVE;
        driver->next_all = g.inertial_drivers;
        g.inertial_drivers = driver;
        *handle = driver;
    } else if (driver->target != target || driver->net != net || driver->slot != slot) {
        inertial_unlink_pending(driver);
        driver->target = target;
        driver->net = net;
        driver->slot = slot;
        driver->current = *target;
    }
    driver->region = region_is_reactive(g.current_region)
                         ? LLG_REGION_REACTIVE
                         : LLG_REGION_ACTIVE;
    value = sv4_resize(value, target->width, target->is_signed);
    sv4_t selected_mask;
    if (mask) {
        selected_mask = sv4_resize(*mask, target->width, 0);
    }
    const sv4_t* effective_mask = mask ? &selected_mask : NULL;
    if (driver->pending) {
        // Unchanged expression values keep the original propagation time.
        if (driver->has_mask == (effective_mask != NULL) &&
            (!effective_mask || sv4_same(driver->mask, *effective_mask)) &&
            inertial_masked_same(&driver->value, &value, effective_mask))
            return;
        inertial_unlink_pending(driver);
    }
    if (effective_mask ? inertial_masked_same(&driver->current, &value, effective_mask)
                       : sv4_same(driver->current, value))
        return;
    driver->has_mask = effective_mask != NULL;
    if (effective_mask) driver->mask = *effective_mask;
    driver->rise = rise;
    driver->fall = fall;
    driver->turn_off = turn_off;
    uint64_t ticks = inertial_transition_ticks(
        &driver->current, &value, effective_mask, rise, fall, turn_off);
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling an inertial update\n");
        abort();
    }
    driver->value = value;
    driver->time = g.now + ticks;
    driver->pending = 1;
    llg_inertial_t** entry = &g.inertial_pending;
    while (*entry && (*entry)->time <= driver->time) entry = &(*entry)->next_pending;
    driver->next_pending = *entry;
    *entry = driver;
}

void llg_inertial_assign(llg_inertial_t** handle, sv4_t* target,
                         sv4_t value, uint64_t rise, uint64_t fall,
                         uint64_t turn_off) {
    inertial_update(handle, target, NULL, 0, value, NULL, rise, fall, turn_off);
}

void llg_inertial_net(llg_inertial_t** handle, llg_net_t* net, int slot,
                      sv4_t value, uint64_t rise, uint64_t fall,
                      uint64_t turn_off) {
    if (slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) {
        fprintf(stderr, "llg: fatal: invalid inertial net driver slot\n");
        abort();
    }
    inertial_update(handle, net->drivers[slot], net, slot, value, NULL, rise,
                    fall, turn_off);
}

void llg_inertial_selected_assign(llg_inertial_t** handle, sv4_t* target,
                                  sv4_t value, sv4_t mask, uint64_t rise,
                                  uint64_t fall, uint64_t turn_off) {
    inertial_update(handle, target, NULL, 0, value, &mask, rise, fall, turn_off);
}

void llg_inertial_selected_net(llg_inertial_t** handle, llg_net_t* net,
                               int slot, sv4_t value, sv4_t mask,
                               uint64_t rise, uint64_t fall,
                               uint64_t turn_off) {
    if (slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) {
        fprintf(stderr, "llg: fatal: invalid inertial net driver slot\n");
        abort();
    }
    inertial_update(handle, net->drivers[slot], net, slot, value, &mask, rise,
                    fall, turn_off);
}

static int inertial_ready(llg_region_t region) {
    for (llg_inertial_t* driver = g.inertial_pending; driver;
         driver = driver->next_pending) {
        if (driver->time == g.now && driver->region == region) return 1;
    }
    return 0;
}

static void commit_inertial(llg_region_t region) {
    llg_inertial_t** slot = &g.inertial_pending;
    while (*slot && ((*slot)->time != g.now || (*slot)->region != region))
        slot = &(*slot)->next_pending;
    llg_inertial_t* driver = *slot;
    if (!driver) return;
    *slot = driver->next_pending;
    driver->next_pending = NULL;
    driver->pending = 0;
    if (driver->has_mask) {
        sv4_t value = *driver->target;
        inertial_merge(&value, &driver->value, &driver->mask);
        driver->current = value;
        if (driver->net) llg_net_write(driver->net, driver->slot, value);
        else llg_ba(driver->target, value);
    } else {
        driver->current = driver->value;
        if (driver->net) llg_net_write(driver->net, driver->slot, driver->value);
        else llg_ba(driver->target, driver->value);
    }
}

static int nba_due(llg_region_t region) {
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        for (llg_nba_t* n = p ? p->nba_head : NULL; n; n = n->next)
            if (n->time == g.now && n->region == region) return 1;
    }
    for (llg_nba_t* n = g.delayed_nbas; n; n = n->next)
        if (n->time == g.now && n->region == region) return 1;
    return 0;
}

static void apply_nba(llg_nba_t* next) {
    if (next->is_event) event_trigger_object(next->event_target);
    else if (next->is_string) {
        if (next->string_target)
            llg_string_move(next->string_target, next->string_value);
        else
            llg_string_destroy(&next->string_value);
    }
    else if (next->is_real) {
        if (!llg_is_real_forced(next->real_target) && !pca_real_active(next->real_target))
            real_write(next->real_target, next->real_value);
    } else if (!llg_is_forced(next->target) && !pca_active(next->target)) {
        sv4_t value = next->value;
        if (next->has_mask) {
            value = *next->target;
            for (uint32_t i = 0; i < (value.width + 63u) / 64u; i++) {
                uint64_t mask = next->mask.bits[i];
                value.bits[i] = (value.bits[i] & ~mask) | (next->value.bits[i] & mask);
                value.x[i] = (value.x[i] & ~mask) | (next->value.x[i] & mask);
                value.z[i] = (value.z[i] & ~mask) | (next->value.z[i] & mask);
            }
        }
        sig_write(next->target, value);
    }
}

static void commit_nbas(llg_region_t region) {
    for (;;) {
        llg_nba_t* next = NULL;
        llg_nba_t** next_slot = NULL;
        llg_proc_t* owner = NULL;
        for (llg_nba_t** delayed_slot = &g.delayed_nbas; *delayed_slot;
             delayed_slot = &(*delayed_slot)->next) {
            llg_nba_t* n = *delayed_slot;
            if (n->time != g.now || n->region != region) continue;
            if (!next || n->sequence < next->sequence) {
                next = n;
                next_slot = delayed_slot;
                owner = NULL;
            }
        }
        for (int i = 0; i < g.n_procs; i++) {
            llg_proc_t* p = g.all_procs[i];
            if (!p) continue;
            for (llg_nba_t** slot = &p->nba_head; *slot;
                 slot = &(*slot)->next) {
                llg_nba_t* n = *slot;
                if (n->time != g.now || n->region != region) continue;
                if (!next || n->sequence < next->sequence) {
                    next = n;
                    next_slot = slot;
                    owner = p;
                }
            }
        }
        if (!next) break;
        *next_slot = next->next;
        if (owner && owner->nba_tail == next) {
            owner->nba_tail = NULL;
            for (llg_nba_t* n = owner->nba_head; n; n = n->next)
                owner->nba_tail = n;
        }
        apply_nba(next);
        free(next);
    }
}

// ── $monitor / $strobe ────────────────────────────────────────────────────────

// Format `fmt` with `n` sv4_t arguments from `args`: the legacy packed path
// handles the original %d/%h/%b/%o/%t family, while typed display calls below
// additionally preserve real/string ownership and the SystemVerilog format
// conversions. `%%` prints '%', and an unknown or missing specifier prints
// verbatim without consuming an argument.
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

static void llg_fmt_args_destroy(llg_fmt_arg_t* args, int n) {
    if (!args) return;
    for (int i = 0; i < n; i++) {
        if (args[i].kind == LLG_FMT_STRING) llg_string_destroy(&args[i].value.string);
    }
}

static llg_fmt_arg_t llg_fmt_arg_clone(const llg_fmt_arg_t* value) {
    llg_fmt_arg_t result = *value;
    if (value->kind == LLG_FMT_STRING)
        result.value.string = llg_string_clone(&value->value.string);
    return result;
}

static void llg_append_text(char* out, size_t cap, size_t* len,
                            const char* text, size_t n) {
    for (size_t i = 0; i < n && *len + 1 < cap; i++) out[(*len)++] = text[i];
}

typedef struct {
    int left;
    int plus;
    int space;
    int alternate;
    int zero;
    int width;
    int has_width;
    int precision;
    int has_precision;
} llg_fmt_spec_t;

static const char* llg_parse_typed_spec(const char* start, const char* p,
                                         llg_fmt_spec_t* spec) {
    memset(spec, 0, sizeof(*spec));
    for (;;) {
        if (*p == '-') spec->left = 1;
        else if (*p == '+') spec->plus = 1;
        else if (*p == ' ') spec->space = 1;
        else if (*p == '#') spec->alternate = 1;
        else if (*p == '0') spec->zero = 1;
        else break;
        p++;
    }
    while (*p >= '0' && *p <= '9') {
        spec->has_width = 1;
        if (spec->width <= (INT_MAX - (*p - '0')) / 10)
            spec->width = spec->width * 10 + (*p - '0');
        p++;
    }
    if (*p == '.') {
        spec->has_precision = 1;
        p++;
        while (*p >= '0' && *p <= '9') {
            if (spec->precision <= (INT_MAX - (*p - '0')) / 10)
                spec->precision = spec->precision * 10 + (*p - '0');
            p++;
        }
    }
    (void)start;
    return p;
}

static size_t llg_format_raw2(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (value.width + 63u) / 64u;
    uint32_t last_bits = value.width % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        // SFormat::formatRaw2 flattens X/Z to zero and emits the native
        // little-endian limb bytes, including the complete last 32-bit half
        // for values whose width is between 33 and 64 bits.
        uint64_t bits = value.bits[i] & ~(value.x[i] | value.z[i]);
        size_t bytes = (i == words - 1 && last_bits <= 32) ? sizeof(uint32_t)
                                                            : sizeof(uint64_t);
        for (size_t j = 0; j < bytes && len < cap; j++)
            raw[len++] = (char)(bits >> (j * 8));
    }
    return len;
}

static size_t llg_format_raw4(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (value.width + 63u) / 64u;
    uint32_t last_bits = value.width % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        uint64_t unknown = value.x[i] | value.z[i];
        uint64_t bits = value.bits[i];
        size_t halves = (i == words - 1 && last_bits <= 32) ? 1u : 2u;
        for (size_t half = 0; half < halves; half++) {
            // VPI's four-state encoding uses aval = known bits XOR unknown
            // and bval = unknown, matching Slang's formatRaw4 helper.
            uint32_t aval = (uint32_t)((bits ^ unknown) >> (half * 32));
            uint32_t bval = (uint32_t)(unknown >> (half * 32));
            if (len + sizeof(aval) + sizeof(bval) > cap) {
                size_t remaining = cap - len;
                if (remaining) {
                    size_t aval_bytes = remaining < sizeof(aval) ? remaining : sizeof(aval);
                    memcpy(raw + len, &aval, aval_bytes);
                    len += aval_bytes;
                    remaining -= aval_bytes;
                    if (remaining) {
                        size_t bval_bytes = remaining < sizeof(bval) ? remaining : sizeof(bval);
                        memcpy(raw + len, &bval, bval_bytes);
                        len += bval_bytes;
                    }
                }
                return len;
            }
            memcpy(raw + len, &aval, sizeof(aval));
            len += sizeof(aval);
            memcpy(raw + len, &bval, sizeof(bval));
            len += sizeof(bval);
        }
    }
    return len;
}

static size_t llg_format_strength(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    for (uint32_t bit = value.width; bit > 0; bit--) {
        uint32_t index = bit - 1;
        uint64_t mask = UINT64_C(1) << (index % 64u);
        uint32_t limb = index / 64u;
        const char* text;
        if (value.x[limb] & mask)
            text = "StX";
        else if (value.z[limb] & mask)
            text = "HiZ";
        else
            text = value.bits[limb] & mask ? "St1" : "St0";
        llg_append_text(raw, cap, &len, text, strlen(text));
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

static size_t llg_format_char(sv4_t value, char* raw, size_t cap) {
    if (cap == 0 || value.width == 0) return 0;
    uint64_t unknown = value.x[0] | value.z[0];
    raw[0] = (char)(unknown & 0xffu ? 0xffu : value.bits[0] & 0xffu);
    return 1;
}

static sv4_t llg_string_to_display_packed(const llg_string_t* value) {
    size_t max_bytes = (size_t)LLG_MAX_WIDTH / 8u;
    if (value->len > max_bytes || (value->len == 0 && LLG_MAX_WIDTH < 8u)) {
        fprintf(stderr,
                "llg runtime fatal: string display conversion exceeds packed width\n");
        abort();
    }
    uint32_t width = value->len ? (uint32_t)(value->len * 8u) : 8u;
    return llg_string_to_packed(llg_string_clone(value), width, 0);
}

static size_t llg_format_pattern_packed(sv4_t value, char* raw, size_t cap) {
    char digits[LLG_MAX_WIDTH * 2u + 256u];
    int has_unknown = sv4_is_unknown(value);
    int all_x = has_unknown;
    int all_z = has_unknown;
    for (int i = 0; i < llg_sv4_nlimbs(value.width); i++) {
        uint64_t mask = llg_sv4_limb_mask(value.width, i);
        all_x &= (value.x[i] & mask) == mask;
        all_z &= (value.z[i] & mask) == mask;
    }
    int base;
    if ((value.width < 8u && !value.is_signed) ||
        (has_unknown && value.width <= 64u && !all_x && !all_z)) {
        base = 'b';
    } else if (value.width <= 32u || value.is_signed || all_x || all_z) {
        base = 'd';
    } else {
        base = 'h';
    }
    sv4_format((char)base, value, digits, sizeof(digits));
    size_t digits_len = strlen(digits);
    size_t len = 0;
    const char* digit_text = digits;
    int include_base = !(base == 'd' && value.width == 32u && value.is_signed && !has_unknown);
    if (digits_len && digits[0] == '-') {
        llg_append(raw, cap, &len, '-');
        digit_text++;
        digits_len--;
    }
    if (include_base) {
        char prefix[64];
        int written = snprintf(prefix, sizeof(prefix), "%u'%s%c", value.width,
                               value.is_signed ? "s" : "", base);
        if (written > 0) llg_append_text(raw, cap, &len, prefix, (size_t)written);
    }
    llg_append_text(raw, cap, &len, digit_text, digits_len);
    return len;
}

static void llg_emit_field(char* out, size_t cap, size_t* len,
                            const char* value, size_t value_len,
                            llg_fmt_spec_t spec, char conversion) {
    char field[LLG_MAX_WIDTH * 2u + 256u];
    size_t n = value_len;
    if (n > sizeof(field) - 1) n = sizeof(field) - 1;
    memcpy(field, value, n);
    field[n] = 0;
    if (spec.has_precision && conversion == 's' && n > (size_t)spec.precision)
        n = (size_t)spec.precision;
    if (spec.has_precision && strchr("dhbox", conversion)) {
        size_t sign = n && field[0] == '-' ? 1u : 0u;
        size_t digits = n - sign;
        while (digits < (size_t)spec.precision && n + 1 < sizeof(field)) {
            memmove(field + sign + 1, field + sign, digits + 1);
            field[sign] = '0';
            n++;
            digits++;
        }
    }
    if (spec.alternate && strchr("hbo", conversion) && n > 0 &&
        !(n == 1 && (field[0] == 'x' || field[0] == 'z'))) {
        const char* prefix = conversion == 'h' ? "0x" : conversion == 'o' ? "0" : "0b";
        size_t prefix_len = strlen(prefix);
        if (n + prefix_len < sizeof(field)) {
            memmove(field + prefix_len, field, n + 1);
            memcpy(field, prefix, prefix_len);
            n += prefix_len;
        }
    }
    int numeric = strchr("dhbotxfeg", conversion) != NULL;
    if (numeric && n > 0 && field[0] != '-' && spec.plus) {
        if (n + 1 < sizeof(field)) {
            memmove(field + 1, field, n + 1);
            field[0] = '+';
            n++;
        }
    } else if (numeric && n > 0 && field[0] != '-' && spec.space) {
        if (n + 1 < sizeof(field)) {
            memmove(field + 1, field, n + 1);
            field[0] = ' ';
            n++;
        }
    }
    size_t pad = spec.width > 0 && (size_t)spec.width > n
                   ? (size_t)spec.width - n : 0;
    // Slang's integral formatter pads non-decimal bases with zeroes whenever
    // a width is present; decimal and textual values use spaces.  `%0d`
    // without a width still gets the natural decimal width and no padding.
    char pad_char = ' ';
    if (strchr("hbox", conversion) != NULL) pad_char = '0';
    // The zero flag is meaningful for host floating-point formatting.  The
    // SystemVerilog integer parser consumes it as syntax but formatInt still
    // uses decimal spaces (and non-decimal bases already use zeroes by
    // virtue of their width rule).
    if (!spec.left && spec.zero && strchr("feg", conversion) != NULL) {
        size_t prefix = (n && (field[0] == '-' || field[0] == '+' || field[0] == ' ')) ? 1u : 0u;
        if (prefix && pad) {
            llg_append_text(out, cap, len, field, prefix);
            for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, '0');
            llg_append_text(out, cap, len, field + prefix, n - prefix);
            return;
        }
    }
    if (!spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
    llg_append_text(out, cap, len, field, n);
    if (spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
}

static size_t llg_format_typed(char* out, size_t cap, const char* fmt,
                               const llg_fmt_arg_t* args, int n,
                               const char* scope) {
    size_t len = 0;
    int argi = 0;
    const char* p = fmt;
    while (*p && len + 1 < cap) {
        if (*p != '%') {
            llg_append(out, cap, &len, *p++);
            continue;
        }
        const char* start = p++;
        llg_fmt_spec_t spec;
        p = llg_parse_typed_spec(start, p, &spec);
        char source_conversion = *p ? *p++ : 0;
        char conversion = source_conversion;
        if (conversion >= 'A' && conversion <= 'Z') conversion = (char)(conversion - 'A' + 'a');
        if (conversion == '%') {
            llg_append(out, cap, &len, '%');
            continue;
        }
        if (conversion == 'm') {
            const char* text = scope ? scope : "";
            llg_emit_field(out, cap, &len, text, strlen(text), spec, 's');
            continue;
        }
        if (conversion == 'l') {
            // `%l` has no width-bearing form in the Slang grammar, so append
            // the library-qualified HDL scope directly and avoid allocating a
            // model-width temporary on the coroutine stack.
            llg_append_text(out, cap, &len, "work.", 5);
            if (scope && scope[0])
                llg_append_text(out, cap, &len, scope, strlen(scope));
            else
                llg_append_text(out, cap, &len, "$unit", 5);
            continue;
        }
        if (!conversion || argi >= n) {
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        const llg_fmt_arg_t* arg = &args[argi++];
        char raw[LLG_MAX_WIDTH * 2u + 256u];
        size_t raw_len = 0;
        if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_PACKED) {
            sv4_format(conversion == 'x' ? 'h' : conversion, arg->value.packed,
                       raw, sizeof(raw));
            raw_len = strlen(raw);
        } else if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            sv4_format(conversion == 'x' ? 'h' : conversion, packed, raw, sizeof(raw));
            raw_len = strlen(raw);
        } else if (conversion == 't' && arg->kind == LLG_FMT_PACKED) {
            sv4_format('d', arg->value.packed, raw, sizeof(raw));
            raw_len = strlen(raw);
            if (!spec.has_width && !spec.zero) {
                spec.width = 20;
                spec.has_width = 1;
            }
        } else if (conversion == 'c' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_char(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'c' && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            raw_len = llg_format_char(packed, raw, sizeof(raw));
        } else if (conversion == 'u' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw2(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'z' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw4(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'v' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_strength(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'p' && arg->kind == LLG_FMT_PACKED) {
            // Aggregate pattern formatting is rejected by lowering until the
            // owned aggregate representation is available.  A packed scalar
            // follows ConstantValue::toString's base-selection and literal
            // prefix rules, which is the scalar case of Slang's pattern
            // visitor.
            raw_len = llg_format_pattern_packed(arg->value.packed, raw, sizeof(raw));
        } else if (strchr("feg", conversion) && arg->kind == LLG_FMT_REAL) {
            char real_fmt[128];
            size_t spec_len = (size_t)(p - start);
            if (spec_len >= sizeof(real_fmt) - 1) spec_len = sizeof(real_fmt) - 2;
            memcpy(real_fmt, start, spec_len);
            real_fmt[spec_len] = 0;
            int written = snprintf(raw, sizeof(raw), real_fmt, arg->value.real);
            raw_len = written < 0 ? 0 : (size_t)written < sizeof(raw)
                                           ? (size_t)written
                                           : sizeof(raw) - 1;
        } else if (conversion == 's' && arg->kind == LLG_FMT_PACKED) {
            llg_string_t value = llg_string_from_packed(arg->value.packed);
            raw_len = value.len;
            if (raw_len > sizeof(raw)) raw_len = sizeof(raw);
            if (raw_len) memcpy(raw, value.data, raw_len);
            llg_string_destroy(&value);
        } else if (conversion == 's' && arg->kind == LLG_FMT_STRING) {
            raw_len = arg->value.string.len;
            if (raw_len > sizeof(raw)) raw_len = sizeof(raw);
            if (raw_len) memcpy(raw, arg->value.string.data, raw_len);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_STRING) {
            // Keep a string pattern visibly distinct from `%s`, matching the
            // quote-delimited form produced by Slang's pattern formatter.
            size_t value_len = arg->value.string.len;
            if (value_len + 2u <= sizeof(raw)) {
                raw[0] = '"';
                if (value_len) memcpy(raw + 1, arg->value.string.data, value_len);
                raw[value_len + 1] = '"';
                raw_len = value_len + 2u;
            } else {
                raw[0] = '"';
                raw_len = sizeof(raw);
                if (raw_len > 1) {
                    size_t copy = raw_len - 2u;
                    memcpy(raw + 1, arg->value.string.data, copy);
                    raw[raw_len - 1] = '"';
                }
            }
        } else {
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        llg_emit_field(out, cap, &len, raw, raw_len, spec, conversion);
    }
    out[len] = 0;
    return len;
}

static void llg_print_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                            const char* scope, int newline) {
    size_t cap = strlen(fmt) + (scope ? strlen(scope) : 0) + 64u;
    for (int i = 0; i < n; i++) {
        size_t extra = 64u;
        if (args[i].kind == LLG_FMT_PACKED) {
            if (args[i].value.packed.width > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            extra += (size_t)args[i].value.packed.width * 8u;
        }
        if (args[i].kind == LLG_FMT_STRING) extra += args[i].value.string.len;
        if (extra > SIZE_MAX - cap) llg_fatal_allocation("typed formatted line", cap, extra);
        cap += extra;
    }
    char* out = llg_checked_malloc(cap, 1, "typed formatted line");
    size_t len = llg_format_typed(out, cap, fmt, args, n, scope);
    fwrite(out, 1, len, stdout);
    if (newline) fputc('\n', stdout);
    fflush(stdout);
    free(out);
}

static int llg_fmt_arg_same(const llg_fmt_arg_t* a, const llg_fmt_arg_t* b) {
    if (a->kind != b->kind) return 0;
    if (a->kind == LLG_FMT_PACKED) return sv4_same(a->value.packed, b->value.packed);
    if (a->kind == LLG_FMT_REAL) return real_same(a->value.real, b->value.real);
    return a->value.string.len == b->value.string.len &&
           (!a->value.string.len ||
            memcmp(a->value.string.data, b->value.string.data,
                   a->value.string.len) == 0);
}

static void reset_monitor_state(void) {
    if (g.mon.active) {
        free(g.mon.fmt);
        free(g.mon.last);
        free(g.mon.work);
        free(g.mon.reads);
        llg_fmt_args_destroy(g.mon.typed_last, g.mon.n);
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        free(g.mon.typed_last);
        free(g.mon.typed_work);
        free(g.mon.typed_reads);
        free(g.mon.scope);
    }
    memset(&g.mon, 0, sizeof(g.mon));
}

void llg_monitor_with_reads(const char* fmt, int n, llg_mon_eval_fn eval,
                            sv4_t* const* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    reset_monitor_state();
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.dirty = 1;
    g.mon.force_report = 1;
    g.mon.n = n;
    g.mon.eval = eval;
    g.mon.n_reads = n_reads;
    g.mon.region = LLG_REGION_POSTPONED;
    g.mon.fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(g.mon.fmt, fmt);
    int alloc = n > 0 ? n : 1;
    g.mon.last = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor previous values");
    g.mon.work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "monitor working values");
    if (n_reads > 0) {
        g.mon.reads = (sv4_t**)llg_checked_malloc(
            (size_t)n_reads, sizeof(sv4_t*), "monitor trigger set");
        memcpy(g.mon.reads, reads, (size_t)n_reads * sizeof(sv4_t*));
    }
}

void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval) {
    llg_monitor_with_reads(fmt, n, eval, NULL, 0);
}

void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval) {
    if (!region_can_mutate("strobe scheduling")) return;
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = eval;
    e->typed = 0;
    e->typed_eval = NULL;
    e->typed_work = NULL;
    e->scope = NULL;
    e->region = LLG_REGION_POSTPONED;
    int alloc = n > 0 ? n : 1;
    e->work = (sv4_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(sv4_t), "strobe working values");
    e->next = NULL;
    if (g.strobe_tail) {
        g.strobe_tail->next = e;
    } else {
        g.strobes = e;
    }
    g.strobe_tail = e;
}

void llg_monitor_with_typed_reads(const char* fmt, int n,
                                  llg_display_eval_fn eval, const char* scope,
                                  const llg_display_read_t* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    reset_monitor_state();
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.dirty = 1;
    g.mon.force_report = 1;
    g.mon.n = n;
    g.mon.typed = 1;
    g.mon.typed_eval = eval;
    g.mon.n_typed_reads = n_reads;
    g.mon.region = LLG_REGION_POSTPONED;
    g.mon.fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "monitor format");
    strcpy(g.mon.fmt, fmt);
    g.mon.scope = (char*)llg_checked_malloc(strlen(scope ? scope : "") + 1, 1,
                                            "monitor scope");
    strcpy(g.mon.scope, scope ? scope : "");
    int alloc = n > 0 ? n : 1;
    g.mon.typed_last = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor previous values");
    g.mon.typed_work = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "monitor working values");
    if (n_reads > 0) {
        g.mon.typed_reads = (llg_display_read_t*)llg_checked_malloc(
            (size_t)n_reads, sizeof(llg_display_read_t), "monitor trigger set");
        memcpy(g.mon.typed_reads, reads,
               (size_t)n_reads * sizeof(llg_display_read_t));
    }
}

void llg_strobe_typed(const char* fmt, int n, llg_display_eval_fn eval,
                      const char* scope) {
    if (!region_can_mutate("strobe scheduling")) return;
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "typed strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = NULL;
    e->typed = 1;
    e->typed_eval = eval;
    e->scope = (char*)llg_checked_malloc(strlen(scope ? scope : "") + 1, 1,
                                         "strobe scope");
    strcpy(e->scope, scope ? scope : "");
    int alloc = n > 0 ? n : 1;
    e->work = NULL;
    e->typed_work = (llg_fmt_arg_t*)llg_checked_calloc(
        (size_t)alloc, sizeof(llg_fmt_arg_t), "strobe working values");
    e->region = LLG_REGION_POSTPONED;
    e->next = NULL;
    if (g.strobe_tail) {
        g.strobe_tail->next = e;
    } else {
        g.strobes = e;
    }
    g.strobe_tail = e;
}

// Re-print a dirty monitor line at the settled observation point. The
// registration and enable paths set force_report so equal values still print.
static void check_monitor(void) {
    if (!g.mon.active || !g.mon.enabled) return;
    if (!g.mon.dirty && !g.mon.force_report) return;
    if (g.mon.typed) {
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        g.mon.typed_eval(g.mon.typed_work, NULL);
        int changed = g.mon.force_report;
        if (!changed) {
            for (int i = 0; i < g.mon.n; i++) {
                if (!llg_fmt_arg_same(&g.mon.typed_work[i], &g.mon.typed_last[i])) {
                    changed = 1;
                    break;
                }
            }
        }
        g.mon.dirty = 0;
        g.mon.force_report = 0;
        if (changed) {
            llg_fmt_args_destroy(g.mon.typed_last, g.mon.n);
            for (int i = 0; i < g.mon.n; i++)
                g.mon.typed_last[i] = llg_fmt_arg_clone(&g.mon.typed_work[i]);
            llg_print_typed(g.mon.fmt, g.mon.typed_work, g.mon.n,
                            g.mon.scope, 1);
        }
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        return;
    }
    g.mon.eval(g.mon.work, NULL);
    int changed = g.mon.force_report;
    if (!changed) {
        for (int i = 0; i < g.mon.n; i++) {
            if (!sv4_same(g.mon.work[i], g.mon.last[i])) {
                changed = 1;
                break;
            }
        }
    }
    g.mon.dirty = 0;
    g.mon.force_report = 0;
    if (!changed) return;
    for (int i = 0; i < g.mon.n; i++) g.mon.last[i] = g.mon.work[i];
    llg_print_array(g.mon.fmt, g.mon.work, g.mon.n);
}

// Print queued $strobe lines after the current time step has settled.
static void flush_strobes(void) {
    while (g.strobes && !g.finish) {
        llg_strobe_t* e = g.strobes;
        g.strobes = e->next;
        if (!g.strobes) g.strobe_tail = NULL;
        if (e->typed) {
            e->typed_eval(e->typed_work, NULL);
            llg_print_typed(e->fmt, e->typed_work, e->n, e->scope, 1);
            llg_fmt_args_destroy(e->typed_work, e->n);
            free(e->typed_work);
            free(e->scope);
        } else {
            e->eval(e->work, NULL);
            llg_print_array(e->fmt, e->work, e->n);
            free(e->work);
        }
        free(e->fmt);
        free(e);
    }
}

void llg_monitor_set(int on) {
    if (!region_can_mutate("monitor scheduling")) return;
    if (!g.mon.active) return;
    if (on) {
        g.mon.enabled = 1;
        g.mon.dirty = 1;
        g.mon.force_report = 1;
    } else {
        g.mon.enabled = 0;
    }
}

static void report_zero_delay_loop(void) {
    const char* where = g.last_process_name ? g.last_process_name : "<scheduler>";
    fprintf(stderr,
            "llg: zero-delay loop detected at time %llu in process `%s` "
            "(scheduler pass limit %llu)\n",
            (unsigned long long)g.now, where,
            (unsigned long long)g.zero_loop_limit);
    llg_last_failure = 1;
    g.finish = 1;
}

static int callback_pending(llg_region_t region) {
    for (llg_region_callback_t* entry = g.callbacks; entry; entry = entry->next) {
        if (entry->time > g.now) break;
        if (entry->time == g.now && entry->region == region) return 1;
    }
    return 0;
}

static llg_region_callback_t* take_region_callback(llg_region_t region) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && (*slot)->time <= g.now) {
        llg_region_callback_t* entry = *slot;
        if (entry->time == g.now && entry->region == region) {
            *slot = entry->next;
            entry->next = NULL;
            return entry;
        }
        slot = &entry->next;
    }
    return NULL;
}

static int region_pending(llg_region_t region) {
    return g.process_queues[region].head != NULL || callback_pending(region);
}

static int run_region_queue(llg_region_t region) {
    g.current_region = region;
    while (!g.finish) {
        llg_region_callback_t* callback = take_region_callback(region);
        llg_proc_t* process = callback ? NULL : dequeue_region(region);
        if (!callback && !process) break;
        if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
            free(callback);
            report_zero_delay_loop();
            return 0;
        }
        if (callback) {
            llg_region_callback_fn fn = callback->callback;
            void* data = callback->data;
            free(callback);
            fn(data);
        } else {
            g.last_process_name = process->name;
            process->region = region;
            aco_resume(process->co);
            reap_retired_procs();
        }
    }
    return !g.finish;
}

static void wake_zero_waits(llg_region_t region) {
    llg_wait_queue_t* queue = &g.zero_waits[region];
    llg_wait_t* wait = queue->head;
    queue->head = NULL;
    queue->tail = NULL;
    while (wait) {
        llg_wait_t* next = wait->region_next;
        wait->region_next = NULL;
        wake_proc(wait->proc);
        wait = next;
    }
}

static int design_pending(void) {
    for (llg_region_t region = LLG_REGION_ACTIVE;
         region <= LLG_REGION_POST_NBA_PLI; region++) {
        if (region_pending(region)) return 1;
    }
    int pending = g.zero_waits[LLG_REGION_INACTIVE].head != NULL ||
                  inertial_ready(LLG_REGION_ACTIVE) || nba_due(LLG_REGION_NBA);
    return pending;
}

static int reactive_pending(void) {
    for (llg_region_t region = LLG_REGION_REACTIVE;
         region <= LLG_REGION_POST_RE_NBA_PLI; region++) {
        if (region_pending(region)) return 1;
    }
    return g.zero_waits[LLG_REGION_RE_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_REACTIVE) || nba_due(LLG_REGION_RE_NBA);
}

static int drain_design_set(void) {
    for (;;) {
        while (!g.finish &&
               (region_pending(LLG_REGION_ACTIVE) ||
                inertial_ready(LLG_REGION_ACTIVE))) {
            if (inertial_ready(LLG_REGION_ACTIVE)) {
                g.current_region = LLG_REGION_ACTIVE;
                if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                    report_zero_delay_loop();
                    return 0;
                }
                commit_inertial(LLG_REGION_ACTIVE);
            } else if (!run_region_queue(LLG_REGION_ACTIVE)) {
                return 0;
            }
        }
        if (g.finish) return 0;
        if (g.zero_waits[LLG_REGION_INACTIVE].head ||
            region_pending(LLG_REGION_INACTIVE)) {
            if (g.zero_waits[LLG_REGION_INACTIVE].head)
                wake_zero_waits(LLG_REGION_INACTIVE);
            if (!run_region_queue(LLG_REGION_INACTIVE)) return 0;
            continue;
        }
        break;
    }
    return 1;
}

// Earlier-phase work enabled by a callback precedes the next phase. NBA
// batches themselves retain issue order before another Active iteration.
static int design_pending_before(llg_region_t stop) {
    for (llg_region_t region = LLG_REGION_ACTIVE; region < stop; region++)
        if (region_pending(region)) return 1;
    return g.zero_waits[LLG_REGION_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_ACTIVE) ||
           (stop > LLG_REGION_NBA && nba_due(LLG_REGION_NBA));
}

static int run_design_set(void) {
    for (;;) {
        if (!drain_design_set()) return 0;
        if (!run_region_queue(LLG_REGION_PRE_NBA_PLI)) return 0;
        if (design_pending_before(LLG_REGION_PRE_NBA_PLI)) continue;
        if (!run_region_queue(LLG_REGION_PRE_NBA)) return 0;
        if (design_pending_before(LLG_REGION_PRE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_NBA)) return 0;
        if (nba_due(LLG_REGION_NBA)) {
            g.current_region = LLG_REGION_NBA;
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                return 0;
            }
            commit_nbas(LLG_REGION_NBA);
        }
        if (g.finish) return 0;
        if (design_pending_before(LLG_REGION_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_NBA)) return 0;
        if (design_pending_before(LLG_REGION_POST_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_NBA_PLI)) return 0;
        process_zombie_groups();
        if (!design_pending()) return 1;
    }
}

static int run_observed_set(void) {
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED_PLI)) return 0;
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED)) return 0;
    if (!run_region_queue(LLG_REGION_OBSERVED)) return 0;
    if (!run_region_queue(LLG_REGION_POST_OBSERVED)) return 0;
    return run_region_queue(LLG_REGION_POST_OBSERVED_PLI);
}

static int drain_reactive_set(void) {
    for (;;) {
        while (!g.finish &&
               (region_pending(LLG_REGION_REACTIVE) ||
                inertial_ready(LLG_REGION_REACTIVE))) {
            if (inertial_ready(LLG_REGION_REACTIVE)) {
                g.current_region = LLG_REGION_REACTIVE;
                if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                    report_zero_delay_loop();
                    return 0;
                }
                commit_inertial(LLG_REGION_REACTIVE);
            } else if (!run_region_queue(LLG_REGION_REACTIVE)) {
                return 0;
            }
        }
        if (g.finish) return 0;
        if (g.zero_waits[LLG_REGION_RE_INACTIVE].head ||
            region_pending(LLG_REGION_RE_INACTIVE)) {
            if (g.zero_waits[LLG_REGION_RE_INACTIVE].head)
                wake_zero_waits(LLG_REGION_RE_INACTIVE);
            if (!run_region_queue(LLG_REGION_RE_INACTIVE)) return 0;
            continue;
        }
        break;
    }
    return 1;
}

static int reactive_pending_before(llg_region_t stop) {
    for (llg_region_t region = LLG_REGION_REACTIVE; region < stop; region++)
        if (region_pending(region)) return 1;
    return g.zero_waits[LLG_REGION_RE_INACTIVE].head != NULL ||
           inertial_ready(LLG_REGION_REACTIVE) ||
           (stop > LLG_REGION_RE_NBA && nba_due(LLG_REGION_RE_NBA));
}

static int run_reactive_set(void) {
    // Exhaust the reactive set before returning to newly enabled design work
    // (IEEE 1800-2009 4.5).
    for (;;) {
        if (!drain_reactive_set()) return 0;
        if (!run_region_queue(LLG_REGION_PRE_RE_NBA_PLI)) return 0;
        if (reactive_pending_before(LLG_REGION_PRE_RE_NBA_PLI)) continue;
        if (!run_region_queue(LLG_REGION_PRE_RE_NBA)) return 0;
        if (reactive_pending_before(LLG_REGION_PRE_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_RE_NBA)) return 0;
        if (nba_due(LLG_REGION_RE_NBA)) {
            g.current_region = LLG_REGION_RE_NBA;
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                return 0;
            }
            commit_nbas(LLG_REGION_RE_NBA);
        }
        if (g.finish) return 0;
        if (reactive_pending_before(LLG_REGION_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_RE_NBA)) return 0;
        if (reactive_pending_before(LLG_REGION_POST_RE_NBA)) continue;
        if (!run_region_queue(LLG_REGION_POST_RE_NBA_PLI)) return 0;
        process_zombie_groups();
        if (!reactive_pending()) return 1;
    }
}

static int run_pre_postponed_set(void) {
    if (!run_region_queue(LLG_REGION_PRE_POSTPONED_PLI)) return 0;
    return run_region_queue(LLG_REGION_PRE_POSTPONED);
}

static int run_postponed_set(void) {
    if (!run_region_queue(LLG_REGION_POSTPONED)) return 0;
    g.current_region = LLG_REGION_POSTPONED;
    flush_strobes();
    if (g.finish) return 0;
    check_monitor();
    if (g.finish) return 0;
    return run_region_queue(LLG_REGION_POSTPONED_PLI);
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
    if (llg_last_config_error) {
        llg_n_finals = 0;
        return;
    }
    // Explicit reset, decoupled from the cleanup-memset invariant: a stale
    // $finish flag left by the scheduler exit must never read as
    // "$finish inside a final" after the first final completes.
    g.finish = 0;
    g.running = 0;
    g.current_region = LLG_REGION_POSTPONED;
    // Rebuild a minimal coroutine context: the scheduler-exit teardown in
    // llg_rt_run released the previous one.
    aco_thread_init(llg_last_word);
    g.main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    g.share_stack = aco_share_stack_new(llg_coroutine_stack_size());
    g.now = llg_final_time;
    g.zero_loop_limit = llg_configured_zero_loop_limit;
    g.process_step_limit = llg_configured_process_step_limit;
    llg_in_finals = 1;
    for (int i = 0; i < llg_n_finals; i++) {
        llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
            1, sizeof(llg_proc_t), "final process");
        p->name = llg_finals[i].name;
        p->fn = llg_finals[i].fn;
        p->budget_time = g.now;
        p->region = LLG_REGION_POSTPONED;
        p->co = aco_create(g.main_co, g.share_stack, 1u << 20, llg_proc_entry, p);
        register_proc(p);
        enqueue_region(p, LLG_REGION_POSTPONED);
        run_region_queue(LLG_REGION_POSTPONED);
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
    if (g.config_error) {
        llg_rt_cleanup();
        return;
    }
    g.running = 1;
    g.current_region = LLG_REGION_PREPONED;
    for (;;) {
        if (g.finish) break;
        sample_preponed_values();
        if (!run_region_queue(LLG_REGION_PREPONED)) break;
        if (!run_region_queue(LLG_REGION_PREPONED_PLI)) break;
        if (!run_region_queue(LLG_REGION_PRE_ACTIVE_PLI)) break;
        for (;;) {
            if (!run_design_set()) break;
            if (!run_observed_set()) break;
            if (!run_reactive_set()) break;
            if (design_pending() || reactive_pending()) continue;
            if (!run_pre_postponed_set()) break;
            if (design_pending() || reactive_pending()) continue;
            break;
        }
        if (g.finish) break;
        if (!run_postponed_set()) break;
        if (g.finish) break;

        int have_future_event = g.timed_head || g.delayed_nbas ||
                                g.inertial_pending || g.callbacks;
        uint64_t t = g.timed_head ? g.timed_head->time : UINT64_MAX;
        if (g.delayed_nbas && g.delayed_nbas->time < t) t = g.delayed_nbas->time;
        if (g.inertial_pending && g.inertial_pending->time < t) t = g.inertial_pending->time;
        if (g.callbacks && g.callbacks->time < t) t = g.callbacks->time;
        if (!have_future_event) {
            if (g.wait_count == 0) {
                fprintf(stderr, "llg: simulation ended without $finish "
                                "(no processes remain) at time %llu\n",
                        (unsigned long long)g.now);
            } else {
                fprintf(stderr, "llg: simulation deadlock at time %llu "
                                "(waiters never woken, no future events)\n",
                        (unsigned long long)g.now);
            }
            break;
        }
        if (t <= g.now) {
            if (!consume_limit(&g.region_passes, g.zero_loop_limit)) {
                report_zero_delay_loop();
                break;
            }
        } else {
            g.now = t;
            g.region_passes = 0;
        }
        llg_wait_t* wait = g.timed_head;
        while (wait && wait->time == g.now) {
            llg_wait_t* next = wait->time_next;
            wake_proc(wait->proc);
            wait = next;
        }
        g.current_region = LLG_REGION_PREPONED;
    }
    // Finals ($time inside them) report when the scheduler loop ended.
    llg_final_time = g.now;
    // No pending update or deferred evaluator executes between $finish and
    // final procedures, regardless of whether its issuing process completed.
    g.running = 0;
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

void llg_display_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                       const char* scope) {
    llg_print_typed(fmt, args, n, scope, 1);
    llg_fmt_args_destroy(args, n);
}

void llg_write_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                     const char* scope) {
    llg_print_typed(fmt, args, n, scope, 0);
    llg_fmt_args_destroy(args, n);
}
