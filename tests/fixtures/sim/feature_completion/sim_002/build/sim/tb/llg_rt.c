// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with llg_co.c and the generated model by the host C
// compiler; never linked into the Rust binaries.

#define _GNU_SOURCE

#include "llg_rt.h"
#include "llg_container.h"
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
#include <errno.h>

#if defined(__unix__) || defined(__APPLE__)
#include <sys/resource.h>
#endif

// Formatting precision is a request limit, not a storage capacity.
// Scratch allocations are sized for each value and requested conversion.
#define LLG_TIMEFORMAT_MAX_PRECISION (2u * LLG_SUPPORTED_WIDTH_LIMIT + 128u)

static void llg_warn_host_stack_limit(void) {
#if defined(__unix__) || defined(__APPLE__)
    struct rlimit limit;
    const rlim_t estimate = (rlim_t)LLG_HOST_STACK_ESTIMATE_BYTES;
    if (getrlimit(RLIMIT_STACK, &limit) == 0 &&
        limit.rlim_cur != RLIM_INFINITY && limit.rlim_cur < estimate) {
        fprintf(stderr,
                "llg: warning: host stack limit is %llu bytes; at least %llu "
                "bytes is estimated for the scheduler, one polled coroutine "
                "segment, the 256-call depth guard, runtime helpers, and "
                "DPI/libc headroom\n",
                (unsigned long long)limit.rlim_cur,
                (unsigned long long)estimate);
    }
#endif
}

// ── Fatal boundary checks ────────────────────────────────────────────────────

static _Noreturn void llg_fatal_allocation(const char* what, size_t count,
                                           size_t size) {
    fprintf(stderr,
            "llg: fatal: cannot allocate %zu element(s) of %zu byte(s) for %s\n",
            count, size, what);
    abort();
}

typedef struct llg_rt_co_cached_chunk {
    struct llg_rt_co_cached_chunk* next;
    size_t bytes;
} llg_rt_co_cached_chunk_t;

static struct {
    llg_rt_co_cached_chunk_t* heads[sizeof(size_t) * CHAR_BIT];
    llg_rt_co_cache_stats_t stats;
} llg_rt_co_chunk_cache;

static void llg_rt_co_count(size_t* counter) {
    if (*counter != SIZE_MAX) (*counter)++;
}

_Noreturn void llg_rt_co_oom(size_t bytes) {
    llg_fatal_allocation("coroutine frame", 1, bytes);
}

static llg_co_chain_t* llg_rt_current_chain(void);

static void llg_rt_co_trace_frame(void* user, const llg_co_desc_t* desc,
                                  const llg_co_frame_t* frame,
                                  unsigned depth) {
    FILE* output = (FILE*)user;
    const char* location = NULL;
    if (desc && desc->sites && frame && frame->state < desc->n_sites)
        location = desc->sites[frame->state].loc;
    fprintf(output, "  #%u %s", depth,
            desc && desc->name ? desc->name : "<unknown HDL frame>");
    if (location && location[0] != '\0') fprintf(output, " at %s", location);
    fputc('\n', output);
}

_Noreturn void llg_rt_co_bad_state(const llg_co_frame_t* co, const char* fn) {
    fprintf(stderr,
            "llg: fatal: invalid coroutine state in %s: frame=%p state=%lu\n",
            fn ? fn : "<unknown>", (const void*)co,
            co ? (unsigned long)co->state : 0ul);
    llg_co_chain_t* chain = llg_rt_current_chain();
    if (chain) {
        fputs("llg: coroutine HDL backtrace:\n", stderr);
        (void)llg_co_backtrace(chain, llg_rt_co_trace_frame, stderr);
    }
    abort();
}

static size_t llg_rt_co_size_class(size_t bytes) {
    size_t size_class = 0;
    while (bytes >>= 1) size_class++;
    return size_class;
}

void* llg_co_host_chunk_alloc(size_t bytes) {
    const size_t size_class = llg_rt_co_size_class(bytes);
    llg_rt_co_cached_chunk_t* previous = NULL;
    llg_rt_co_cached_chunk_t* chunk =
        llg_rt_co_chunk_cache.heads[size_class];
    while (chunk && chunk->bytes != bytes) {
        previous = chunk;
        chunk = chunk->next;
    }
    if (chunk) {
        if (previous)
            previous->next = chunk->next;
        else
            llg_rt_co_chunk_cache.heads[size_class] = chunk->next;
        llg_rt_co_chunk_cache.stats.cached_bytes -= bytes;
        llg_rt_co_count(&llg_rt_co_chunk_cache.stats.cache_hits);
        return chunk;
    }

    void* allocation = malloc(bytes);
    if (!allocation) llg_rt_co_oom(bytes);
    llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_allocations);
    return allocation;
}

void llg_co_host_chunk_free(void* allocation, size_t bytes) {
    const size_t cap = (size_t)LLG_CO_CHUNK_CACHE_MAX_BYTES;
    if (bytes >= sizeof(llg_rt_co_cached_chunk_t) &&
        llg_rt_co_chunk_cache.stats.cached_bytes <= cap &&
        bytes <= cap - llg_rt_co_chunk_cache.stats.cached_bytes) {
        const size_t size_class = llg_rt_co_size_class(bytes);
        llg_rt_co_cached_chunk_t* chunk = (llg_rt_co_cached_chunk_t*)allocation;
        chunk->bytes = bytes;
        chunk->next = llg_rt_co_chunk_cache.heads[size_class];
        llg_rt_co_chunk_cache.heads[size_class] = chunk;
        llg_rt_co_chunk_cache.stats.cached_bytes += bytes;
        if (llg_rt_co_chunk_cache.stats.peak_cached_bytes <
            llg_rt_co_chunk_cache.stats.cached_bytes)
            llg_rt_co_chunk_cache.stats.peak_cached_bytes =
                llg_rt_co_chunk_cache.stats.cached_bytes;
        return;
    }
    free(allocation);
    llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_frees);
}

void llg_rt_co_cache_get_stats(llg_rt_co_cache_stats_t* stats) {
    if (stats) *stats = llg_rt_co_chunk_cache.stats;
}

static void llg_rt_co_cache_release(void) {
    size_t size_class;
    for (size_class = 0;
         size_class < sizeof(llg_rt_co_chunk_cache.heads) /
                          sizeof(llg_rt_co_chunk_cache.heads[0]);
         size_class++) {
        llg_rt_co_cached_chunk_t* chunk =
            llg_rt_co_chunk_cache.heads[size_class];
        while (chunk) {
            llg_rt_co_cached_chunk_t* next = chunk->next;
            free(chunk);
            llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_frees);
            chunk = next;
        }
        llg_rt_co_chunk_cache.heads[size_class] = NULL;
    }
    llg_rt_co_chunk_cache.stats.cached_bytes = 0;
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
static size_t llg_format_time_integer(sv4_t value, uint64_t source_unit_fs,
                                      char* raw, size_t cap);

static const char* llg_parse_legacy_spec(const char* p, int* has_width,
                                         int* width, int* zero) {
    *has_width = 0;
    *width = 0;
    *zero = 0;
    while (*p == '-' || *p == '+' || *p == ' ' || *p == '#') p++;
    if (*p == '0') {
        *zero = 1;
        p++;
    }
    while (*p >= '0' && *p <= '9') {
        *has_width = 1;
        if (*width <= (INT_MAX - (*p - '0')) / 10)
            *width = *width * 10 + (*p - '0');
        p++;
    }
    if (*p == '.') {
        p++;
        while (*p >= '0' && *p <= '9') p++;
    }
    return p;
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
    W_EVENTS_INLINE, // one packed signal with an inline four-state snapshot
    W_EVENT, // waiting on one or more named events
    W_EVENT_TRIGGERED, // waiting on persistent same-time-slot event state
    W_EVENT_ORDER, // waiting for named events in a specified order
    W_MIXED, // atomic named-event + signal or-list (@(posedge a or ev))
    W_DEPS,  // typed packed/real dependency set
    W_EXPR,  // expressions and trigger-time qualifiers
    W_LEVEL,
    W_LEVEL_INLINE,
    W_FORK,    // llg_join: waiting for a fork group
    W_FORK_ALL, // llg_wait_fork: waiting for all of the current proc's groups
    W_PROCESS, // process::await: waiting for one stable process handle
    W_SEMAPHORE, // semaphore::get: waiting for one FIFO key request
    W_MAILBOX_GET,
    W_MAILBOX_PUT,
    W_ASSERTION, // procedural expect waiting for one assertion endpoint
} llg_wait_kind_t;

static void value_scope_release(llg_value_scope_t* scope);
static llg_value_scope_t* value_scope_retain_target(const void* target);

typedef struct llg_nba {
    struct llg_nba* queue_next;
    struct llg_nba* queue_prev;
    struct llg_nba* owner_next;
    struct llg_nba* owner_prev;
    sv4_t* target;
    llg_fixed_array_t *fixed_target, *fixed_value;
    llg_value_scope_t* target_scope;
    llg_net_t* net_target;
    int net_slot;
    llg_event_object_t* event_target;
    sv4_t value;
    sv4_t mask;
    int has_mask;
    uint32_t range_offset;
    uint32_t range_width;
    int has_range;
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
    // Owned copy of a tagged-member write's commit-time checks, or NULL.
    llg_ref_view_t* tag_view;
} llg_nba_t;

typedef struct {
    llg_nba_t* head;
    llg_nba_t* tail;
} llg_nba_queue_t;

typedef struct llg_nba_bucket {
    struct llg_nba_bucket* next;
    uint64_t time;
    llg_nba_queue_t queues[LLG_REGION_COUNT];
} llg_nba_bucket_t;

static void nba_destroy(llg_nba_t* nba) {
    if (!nba) return;
    if (nba->fixed_value) {
        llg_fixed_array_destroy(nba->fixed_value);
        free(nba->fixed_value);
    }
    sv4_destroy(&nba->value);
    sv4_destroy(&nba->mask);
    if (nba->is_string) llg_string_destroy(&nba->string_value);
    free(nba->tag_view);
    value_scope_release(nba->target_scope);
    free(nba);
}

struct llg_inertial {
    struct llg_inertial* next_all;
    struct llg_inertial* next_pending;
    llg_inertial_t** handle;
    sv4_t* target;
    llg_net_t* net;
    llg_net_t* publication_net; // net-delay commit, not a driver contribution
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

typedef struct llg_semaphore_wait llg_semaphore_wait_t;

typedef struct {
    llg_expr_event_spec_t* expressions;
    llg_event_spec_t* specs;
    llg_wait_dependency_t* dependencies;
    sv4_t* last;
    double* real_last;
    int n;
    llg_event_object_t** evs;
    int* event_slots; // positions in named-event waiter tables
    int n_evs;
} llg_wait_expression_payload_t;

typedef struct {
    llg_event_object_t** evs;
    int* event_slots; // positions in named-event waiter tables
    int n_evs;
    llg_event_object_t* triggered_ev;
    llg_event_object_t* inline_ev;
    int inline_slot;
} llg_wait_event_payload_t;

typedef struct {
    llg_event_spec_t* specs;
    sv4_t* last;
    int n;
    llg_event_object_t** evs;
    int* event_slots; // positions in named-event waiter tables
    int n_evs;
} llg_wait_mixed_payload_t;

typedef struct {
    llg_event_object_t** evs;
    int* event_slots; // positions in named-event waiter tables
    int n_evs;
    llg_event_object_t** sequence;
    int n_order;
    int next;
    int* result;
} llg_wait_order_payload_t;

typedef struct {
    sv4_t* sig;
    sv4_t value;
} llg_wait_level_payload_t;

typedef struct { llg_fork_group_t* group; } llg_wait_fork_payload_t;
typedef struct { llg_proc_t* parent; } llg_wait_fork_all_payload_t;
typedef struct { llg_process_handle_t* target; } llg_wait_process_payload_t;

typedef struct {
    llg_semaphore_t* semaphore;
    llg_semaphore_wait_t* waiter;
    uint64_t keys;
} llg_wait_semaphore_payload_t;

typedef struct {
    struct llg_wait* next;
    llg_mailbox_t* mailbox;
    llg_mailbox_target_t target;
    int peek;
} llg_wait_mailbox_get_payload_t;

typedef struct {
    struct llg_wait* next;
    llg_mailbox_t* mailbox;
    llg_mailbox_value_t value;
} llg_wait_mailbox_put_payload_t;

typedef struct { uint64_t identity; } llg_wait_assertion_payload_t;

typedef union {
    llg_wait_mixed_payload_t mixed;
    llg_wait_order_payload_t order;
    llg_wait_level_payload_t level;
    llg_wait_fork_payload_t fork;
    llg_wait_fork_all_payload_t fork_all;
    llg_wait_process_payload_t process;
    llg_wait_semaphore_payload_t semaphore;
    llg_wait_mailbox_get_payload_t mailbox_get;
    llg_wait_mailbox_put_payload_t mailbox_put;
    llg_wait_assertion_payload_t assertion;
} llg_wait_rare_t;

typedef struct llg_wait_source llg_wait_source_t;
typedef struct llg_wait_subscription {
    struct llg_wait_subscription* next;
    struct llg_wait_subscription** prev_link;
    llg_wait_source_t* source;
    struct llg_wait* wait; // NULL for a publication cursor
} llg_wait_subscription_t;

struct llg_wait_source {
    const void* key;
    llg_wait_subscription_t* head;
    llg_wait_source_t* next;
    llg_wait_source_t** prev_link;
};

// One limb covers scalar clocks and packed values through 64 bits. Larger
// capacities grow every process; keep the measured 128-byte wait budget.
#define LLG_WAIT_INLINE_LIMBS 1u
#define LLG_WAIT_INLINE_SPECS 1u

typedef struct {
    llg_wait_subscription_t subscription;
    llg_event_spec_t specs[LLG_WAIT_INLINE_SPECS];
    uint32_t width;
    int8_t is_signed;
    llg_sv4_vpi_word_t words[LLG_WAIT_INLINE_LIMBS];
    uint64_t reserved; // Preserve the process record layout across this bridge migration.
} llg_wait_inline_payload_t;

typedef struct llg_wait {
    struct llg_wait* next;         // all active waits (signal + timed + zero-delay)
    struct llg_wait** prev_link;  // O(1) global-list removal
    llg_wait_subscription_t* subscriptions;
    size_t n_subscriptions;
    llg_proc_t* proc;
    llg_wait_kind_t kind;
    llg_region_t resume_region;
    union {
        struct {
            uint64_t time;
            struct llg_wait* next; // sorted timed list
            struct llg_wait* region_next; // typed zero-delay region queue
        } timer;
        llg_wait_inline_payload_t single;
        llg_wait_event_payload_t event;
        llg_wait_expression_payload_t expression;
        llg_wait_rare_t* rare;
    } payload;
} llg_wait_t;

struct llg_semaphore_wait {
    llg_semaphore_wait_t* next;
    llg_semaphore_t* owner;
    llg_proc_t* proc;
    uint64_t keys;
};

struct llg_semaphore {
    uint64_t available;
    llg_semaphore_wait_t* wait_head;
    llg_semaphore_wait_t* wait_tail;
    int cancelled_waiter;        // FIFO service deferred until cancellation ends
    llg_semaphore_t* next_all;
};

typedef struct llg_mailbox_message {
    struct llg_mailbox_message* next;
    llg_mailbox_value_t value;
} llg_mailbox_message_t;

struct llg_mailbox {
    uint64_t bound;              // zero is unbounded
    int kind;                    // LLG_MAILBOX_* expected message kind
    uint32_t width;
    int8_t is_signed;
    int8_t two_state;
    int8_t shortreal;
    uint64_t length;
    llg_mailbox_message_t* head;
    llg_mailbox_message_t* tail;
    llg_wait_t* get_head;
    llg_wait_t* get_tail;
    llg_wait_t* put_head;
    llg_wait_t* put_tail;
    struct llg_mailbox* next;
};

typedef struct llg_fork_child {
    struct llg_fork_child* next;
    llg_proc_t* proc;            // NULL once freed by disable_fork
} llg_fork_child_t;

struct llg_fork_group {
    int join_kind;                // LLG_JOIN / LLG_JOIN_NONE / LLG_JOIN_ANY
    int remaining;                // live children; decremented on done AND on kill
    int resumed;                  // join_any: parent already woken
    int terminal;                 // group was moved to the zombie list
    int started;                  // join_none children became eligible to run
    llg_proc_t* parent;          // spawning proc
    llg_fork_child_t* children;  // for disable_fork
    llg_fork_child_t* children_tail; // source-order O(1) append
    struct llg_fork_group* next_g; // per-proc live-group list (or zombie list)
    struct llg_fork_group* prev_g; // live-list predecessor, NULL when detached
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
    struct llg_activation** all_prev_link;
    struct llg_activation** proc_prev_link;
    size_t refs;
    int disabled;
    int detached;
};

// Process handles intentionally outlive the coroutine they identify. The
// runtime owns one reference while `proc` is live; HDL variables and await
// registrations add their own references. `linked` is cleared during runtime
// teardown so a handle released by generated model cleanup never touches a
// zeroed scheduler context.
struct llg_process_handle {
    size_t refs;
    llg_proc_t* proc;
    int status;
    int linked;
    struct llg_process_handle* next;
    struct llg_process_handle** prev_link;
};

typedef struct llg_process_local_ref {
    llg_process_handle_t** slot;
    llg_process_handle_t* value;
    struct llg_process_local_ref* next;
} llg_process_local_ref_t;

typedef struct llg_program {
    uint64_t instance;
    size_t live_initials;
    int had_initial;
    int closed;
    struct llg_program* next;
} llg_program_t;

static llg_value_scope_t* root_value_scopes;
static llg_value_scope_t* all_value_scopes;

struct llg_proc {
    llg_nba_t* nba_head;
    llg_nba_t* nba_tail;
    llg_region_t wait_resume_region;
    int has_wait_resume_region;
    int completed;
    int killed;
    int suspended;
    int wake_pending;
    llg_process_handle_t* handle;
    llg_process_local_ref_t* process_locals;
    llg_proc_t* next_retired;
    llg_fork_group_t* fork_groups; // live groups spawned by this proc
    llg_fork_group_t* fork_groups_tail; // source-order O(1) append
    llg_fork_group_t* pending_fork_groups; // first group not yet checked for start
    llg_fork_group_t* grp;         // group this proc belongs to (NULL top-level)
    llg_fork_child_t* fork_child;   // owning group node for O(1) cancellation
    llg_frame_t* frame;            // retained activation storage, when captured
    llg_ref_scope_t* reference_top; // retained call-argument cells
    llg_rng_state_t rng;           // process-local random stream
    llg_program_t* program;         // runtime-owned originating program instance
    int registry_slot;             // stable all_procs position
    int program_live;              // counted initial, never a fork descendant
    uint64_t assertion_owner;      // stable per-run identity for deferred reports
    uint64_t action_assertion;     // assertion whose Reactive action spawned us
    int is_assertion_action;
    // Resume-hot fields are packed next to the chain and appended root frame.
    llg_value_scope_t* value_scopes;
    llg_activation_t* activation_top;
    const char* name;
    llg_proc_t* next_region;
    llg_region_t region;
    int queued;
    int status;
    uint64_t budget_steps;
    uint64_t budget_time;
    llg_wait_t wait;
    llg_co_chain_t chain; // last: LLG_CO_ROOT(&chain) follows the record
};

static void free_proc_record(llg_proc_t* proc) {
    if (!proc) return;
    llg_co_arena_release(&proc->chain.arena);
    free(proc);
}

#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(llg_wait_t) == 128,
               "64-bit wait record size changed; update the measured layout contract");
_Static_assert(offsetof(llg_proc_t, chain) + sizeof(llg_co_chain_t) -
                       offsetof(llg_proc_t, value_scopes) ==
                   240,
               "64-bit resume-hot process block size changed");
_Static_assert(offsetof(llg_proc_t, chain) + sizeof(llg_co_chain_t) ==
                   sizeof(llg_proc_t),
               "coroutine chain must remain the process record's last member");
_Static_assert(sizeof(llg_proc_t) == 424,
               "64-bit process record size changed; update the layout contract");
#endif

static void llg_runtime_service_enter(const llg_proc_t* self,
                                      const char* service) {
#ifdef LLG_CO_DEBUG
    if (self && self->chain.exiting) {
        fprintf(stderr, "llg: runtime service %s called on an exiting process\n",
                service ? service : "<unknown>");
        abort();
    }
#else
    (void)self;
    (void)service;
#endif
}

static int region_can_mutate(const char* action);
static llg_nba_t* new_nba(uint64_t ticks);
static llg_nba_t* new_clocking_nba(uint64_t ticks);
static void enqueue_nba(llg_nba_t* n);
static void cancel_proc_nbas(llg_proc_t* proc);
static void free_all_nbas(void);
static void promote_delayed_nbas(void);
static void deferred_trigger_source_change(sv4_t* sig, double* real);
static void deferred_trigger_event(llg_event_object_t* ev);
static void process_local_release_all(llg_proc_t* proc);
static void start_pending_fork_children(llg_proc_t* parent);
static void proc_complete(llg_proc_t* self);
static void event_triggered_unlink(llg_wait_t* w);
static void mailbox_unlink_wait(llg_wait_t* w);
static void mailbox_value_destroy(llg_mailbox_value_t* value);
static void assertion_disable_signal_changed(sv4_t* signal);
static void assertion_abort_condition_changed(void);
static void assertion_clock_signal_changed(sv4_t* signal, sv4_t old,
                                           sv4_t value);
struct llg_concurrent_assertion;
static void free_assertion_clock_events(struct llg_concurrent_assertion* assertion);
static void wake_proc(llg_proc_t* p);
static void wake_assertion_waiter(uint64_t identity);
static void semaphore_waiter_unlink(llg_wait_t* wait);
static void semaphore_wake_available(llg_semaphore_t* semaphore);
static void llg_kill_proc_tree(llg_proc_t* p);
static void llg_kill_proc_tree_internal(llg_proc_t* p, int notify_parent);
static void llg_fork_group_child_done(llg_fork_group_t* grp);
static void flush_deferred_assertions(void);
static void run_deferred_assertions_now(void);
llg_proc_t* llg_current(void);

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
    if (entry->alias_kind == LLG_FRAME_ALIAS_NONE && entry->kind == LLG_FRAME_PACKED)
        sv4_destroy(&entry->value.packed);
    memset(&entry->value, 0, sizeof(entry->value));
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
    sv4_t copy = sv4_clone(&value);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_PACKED;
    sv4_move(&entry->value.packed, &copy);
}

void llg_frame_capture_real(llg_frame_t* frame, size_t slot, double value) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_REAL;
    entry->value.real = value;
}

void llg_frame_capture_opaque(llg_frame_t* frame, size_t slot, void* value) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    frame_clear_alias(entry);
    entry->kind = LLG_FRAME_OPAQUE;
    entry->value.opaque = value;
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
    llg_frame_retain(target);
    frame_clear_alias(entry);
    entry->kind = llg_frame_slot_kind(target, target_slot);
    entry->alias_kind = LLG_FRAME_ALIAS_SLOT;
    entry->alias.slot.frame = target;
    entry->alias.slot.slot = target_slot;
}

llg_frame_slot_kind_t llg_frame_slot_kind(const llg_frame_t* frame,
                                          size_t slot) {
    const llg_frame_slot_t* entry = frame_slot_const(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_slot_kind(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    return entry->kind;
}

sv4_t* llg_frame_value_address(llg_frame_t* frame, size_t slot) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_value_address(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    if (entry->kind != LLG_FRAME_PACKED) {
        frame_kind_error(LLG_FRAME_PACKED, entry->kind);
    }
    return entry->alias_kind == LLG_FRAME_ALIAS_PACKED
               ? entry->alias.packed
               : &entry->value.packed;
}

double* llg_frame_real_address(llg_frame_t* frame, size_t slot) {
    llg_frame_slot_t* entry = frame_slot(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_real_address(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    if (entry->kind != LLG_FRAME_REAL) {
        frame_kind_error(LLG_FRAME_REAL, entry->kind);
    }
    return entry->alias_kind == LLG_FRAME_ALIAS_REAL
               ? entry->alias.real
               : &entry->value.real;
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
               ? sv4_clone(entry->alias.packed)
               : sv4_clone(&entry->value.packed);
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
        sv4_copy(&entry->value.packed, &value);
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

void* llg_frame_read_opaque(const llg_frame_t* frame, size_t slot) {
    const llg_frame_slot_t* entry = frame_slot_const(frame, slot);
    if (entry->alias_kind == LLG_FRAME_ALIAS_SLOT) {
        return llg_frame_read_opaque(entry->alias.slot.frame, entry->alias.slot.slot);
    }
    if (llg_frame_slot_kind(frame, slot) != LLG_FRAME_OPAQUE) {
        frame_kind_error(LLG_FRAME_OPAQUE, llg_frame_slot_kind(frame, slot));
    }
    return entry->value.opaque;
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
    uint32_t descriptor;
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
    uint32_t descriptor;
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

// One deferred immediate-assertion result remains pending until the
// Observed-to-Reactive handoff. Repeated evaluations of one assertion from
// one process in a single time slot replace this record, suppressing transient
// glitches while retaining the last sampled condition/action. The owner is a
// stable per-run process identity rather than a process pointer, because a
// completed process can be reclaimed before the Reactive callback runs.
typedef struct llg_deferred_assertion_report {
    struct llg_deferred_assertion_report* next;
    uint64_t owner;
    uint64_t time;
    int kind;
    int passed;
    uint64_t identity;
    const char* label;
    const char* location;
    const char* scope;
    llg_deferred_assertion_fn action;
    llg_frame_t* frame;
} llg_deferred_assertion_report_t;

// Deferred assertions can be controlled before their first execution. Retain
// selector rules, not only entries for reports that happen to exist already.
typedef struct llg_assertion_rule {
    struct llg_assertion_rule* next;
    char* scope; // NULL means the whole design
    uint64_t assertion_type, directive_type;
    int enabled;
} llg_assertion_rule_t;
static llg_assertion_rule_t* llg_assertion_rules;

typedef struct llg_sampled_value {
    struct llg_sampled_value* next;
    sv4_t* signal;
    sv4_t value;
    struct llg_sampled_history* history;
} llg_sampled_value_t;

typedef struct llg_sampled_history {
    struct llg_sampled_history* next;
    uint64_t time;
    sv4_t value;
} llg_sampled_history_t;

typedef struct llg_sampled_domain_history {
    struct llg_sampled_domain_history* next;
    uint64_t time;
    uint64_t sequence;
    sv4_t value;
} llg_sampled_domain_history_t;

typedef struct llg_sampled_domain {
    struct llg_sampled_domain* next;
    uint64_t identity;
    sv4_t* clock;
    int edge;
    llg_sampled_domain_eval_fn value;
    llg_sampled_domain_eval_fn gate;
    void* data;
    sv4_t initial;
    llg_sampled_domain_history_t* history;
} llg_sampled_domain_t;

static llg_sampled_value_t* find_sampled_value(const sv4_t* signal);
static void sampled_record_write(sv4_t* signal);
static void sampled_domain_clock_signal_changed(sv4_t* signal, sv4_t old,
                                                sv4_t value);

typedef struct llg_assertion_attempt {
    struct llg_assertion_attempt* next;
    /* 1 means the consequent is due on the next matching clock edge. */
    uint64_t due;
} llg_assertion_attempt_t;

typedef struct llg_sequence_scope {
    struct llg_sequence_scope* parent;
    size_t refs;
    uint32_t identity;
    int matched;
    uint64_t time, tick;
    sv4_t* clock;
    int edge;
} llg_sequence_scope_t;

typedef struct llg_sequence_endpoint {
    uint32_t local_count;
    struct llg_sequence_endpoint* next;
    sv4_t* locals;
    sv4_t* clock;
    int edge;
    uint64_t time, order, tick;
    int empty;
} llg_sequence_endpoint_t;

typedef struct llg_sequence_token {
    uint32_t local_count;
    struct llg_sequence_token* next;
    uint32_t state;
    uint32_t transition; /* UINT32_MAX expands this state; otherwise one pending edge. */
    llg_sequence_scope_t* scope;
    int checked;
    uint64_t last_order;
    uint64_t entered_time;
    uint64_t entered_order;
    uint64_t entered_tick;
    sv4_t* entered_clock;
    int entered_edge;
    /* A sequence thread carries its own local assertion state.  Keeping this
     * on the token prevents `or`/repetition joins from merging distinct
     * match-item histories merely because their automaton state is equal. */
    sv4_t* locals;
} llg_sequence_token_t;

typedef struct llg_sequence_attempt {
    struct llg_sequence_attempt* next;
    const llg_sequence_graph_t* graph;
    llg_sequence_token_t* tokens;
    /* Diagnostic creation ordinal; launch uses the endpoint's clock/time. */
    uint64_t due_cycle;
    int matched;
    int started;
    int launch_pending;
    int launch_strict;
    llg_sequence_endpoint_t launch;
    llg_sequence_endpoint_t* endpoints;
    uint8_t* inherited;
    sv4_t* locals;
} llg_sequence_attempt_t;

/* A sequence can advance on a clock other than its leading assertion clock.
 * Keep each observed edge until the Observed pass consumes it so separate
 * clocks toggling in one time slot cannot be collapsed into one root-clock
 * boolean. */
typedef struct llg_assertion_clock_event {
    struct llg_assertion_clock_event* next;
    sv4_t* signal;
    int edge;
    uint64_t time;
    uint64_t order;
    uint64_t tick;
} llg_assertion_clock_event_t;

typedef struct llg_concurrent_assertion {
    struct llg_concurrent_assertion* next;
    sv4_t* clock;
    int edge;
    sv4_t* disable;
    llg_concurrent_assertion_predicate_fn antecedent;
    llg_concurrent_assertion_predicate_fn consequent;
    llg_concurrent_assertion_predicate_fn abort_condition;
    const llg_co_desc_t* pass_desc;
    const llg_co_desc_t* fail_desc;
    void* data;
    int kind;
    int overlapped;
    int abort_reject;
    int abort_sync;
    int enabled;
    int expect_active;
    uint64_t identity;
    const char* label;
    const char* location;
    const char* scope;
    int edge_pending;
    llg_assertion_attempt_t* attempts;
    llg_assertion_attempt_t* attempts_tail;
    const llg_sequence_graph_t* antecedent_sequence;
    const llg_sequence_graph_t* consequent_sequence;
    llg_sequence_attempt_t* sequence_antecedents;
    llg_sequence_attempt_t* sequence_antecedents_tail;
    llg_sequence_attempt_t* sequence_consequents;
    llg_sequence_attempt_t* sequence_consequents_tail;
    llg_assertion_clock_event_t* clock_events;
    llg_assertion_clock_event_t* clock_events_tail;
    /* Retain this slot's edges, even when delivered before the source edge. */
    llg_assertion_clock_event_t* clock_history;
    llg_assertion_clock_event_t* clock_history_tail;
    uint64_t sequence_cycle;
} llg_concurrent_assertion_t;

typedef struct {
    uint64_t unit_fs;
    int precision;
    int minimum_field_width;
    llg_string_t suffix;
} llg_timeformat_state_t;

// Exact descriptor identities, not limb-buffer addresses. Slot-local flags
// answer late clocking queries; sequence clocks also retain cumulative ticks.
// See dependencies.c for lifetime, reuse and late-registration constraints.
typedef struct llg_clocking_edge {
    struct llg_clocking_edge* next;
    struct llg_clocking_edge* prev;
    sv4_t* signal;
    uint64_t time;
    uint64_t posedge_count;
    uint64_t negedge_count;
    unsigned occurred;
    int keep_ticks;
} llg_clocking_edge_t;

// A synchronous drive issued away from its clocking event retains its
// issue-time value and waits for the next matching event. The target storage,
// resolved net slot, and optional packed mask are all stable model objects;
// only the source descriptor array is copied here because it may be a
// generated process-local array.
typedef struct llg_clocking_drive {
    struct llg_clocking_drive* next;
    llg_wait_src_t* specs;
    int n_specs;
    sv4_t* target;
    llg_value_scope_t* target_scope;
    llg_net_t* net_target;
    int net_slot;
    double* real_target;
    sv4_t value;
    sv4_t mask;
    int has_mask;
    uint32_t range_offset;
    uint32_t range_width;
    int has_range;
    int is_real;
    double real_value;
    uint64_t ticks;
} llg_clocking_drive_t;

static void clocking_drive_signal_match(sv4_t* signal, sv4_t old, sv4_t value);
static void clocking_drive_event_match(llg_event_object_t* event);

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

// Stochastic analysis queues are a scheduler-owned registry, not container
// values. Entries retain the HDL job/information IDs and the simulation tick
// at which they were accepted so wait statistics never depend on host time.
typedef struct llg_q_entry {
    struct llg_q_entry* next;
    int64_t job_id;
    int64_t inform_id;
    uint64_t arrival;
} llg_q_entry_t;

typedef struct llg_q_queue {
    struct llg_q_queue* next;
    int64_t id;
    int type;                 // 1 = FIFO, 2 = LIFO
    uint64_t capacity;
    uint64_t length;
    llg_q_entry_t* head;
    llg_q_entry_t* tail;
    uint64_t arrivals;
    uint64_t last_arrival;
    uint64_t interarrival_sum;
    uint64_t maximum_length;
    uint64_t shortest_wait;
    int has_arrival;
    int has_shortest_wait;
} llg_q_queue_t;

typedef struct {
    int initialized;
    int process_turn_active;
    int deferred_stop;
    llg_proc_t* current;
    llg_proc_queue_t process_queues[LLG_REGION_COUNT];
    llg_nba_queue_t nba_queues[LLG_REGION_COUNT];
    llg_nba_bucket_t* delayed_nba_buckets;
    llg_inertial_t* inertial_drivers;
    llg_inertial_t* inertial_pending;
    uint64_t nba_sequence;
    llg_wait_t* timed_head;   // sorted ascending by time
    llg_wait_queue_t zero_waits[LLG_REGION_COUNT];
    llg_wait_t* waiters;      // all active waits
    int wait_count;
    llg_wait_source_t** wait_sources;
    size_t wait_sources_capacity;
    size_t n_wait_sources;
    uint64_t now;
    uint64_t design_precision_fs;
    llg_timeformat_state_t time_format;
    llg_region_t current_region;
    uint64_t callback_sequence;
    llg_region_callback_t* callbacks; // sorted by time, region, issue order
    llg_sampled_value_t* sampled;
    llg_sampled_domain_t* sampled_domains;
    uint64_t sampled_domain_sequence;
    llg_clocking_edge_t* clocking_edges;
    llg_clocking_edge_t** clocking_index;
    size_t clocking_capacity;
    size_t clocking_count;
    size_t clocking_used;
    llg_clocking_drive_t* clocking_drives;
    llg_clocking_drive_t* clocking_drives_tail;
    uint64_t sampled_time;
    int sampled_time_valid;
    llg_concurrent_assertion_t* assertions;
    // Set when a clock edge reaches a concurrent assertion and cleared when
    // the Observed region evaluates them. An edge enabled by the reactive
    // set sends the time slot back around through Observed (SV 24.3.1).
    int assertion_edges_pending;
    llg_concurrent_assertion_t* assertion_tail;
    llg_deferred_trigger_t* deferred_triggers;
    llg_deferred_trigger_t* deferred_trigger_tail;
    llg_deferred_assertion_report_t* deferred_assertions;
    llg_deferred_assertion_report_t* deferred_assertion_tail;
    uint64_t next_process_identity;
    int in_deferred_action;
    // Set while Postponed display evaluators run. Lowering admits only
    // helpers whose stores target their own static storage (SV 4.4.2.9
    // forbids visible writes there); those stores apply without publication.
    int private_evaluation;
    int running;
    int finish;
    int suspended;
    int stop_policy;
    llg_proc_t* stop_proc;
    llg_region_t stop_region;
    int config_error;
    uint64_t zero_loop_limit;
    uint64_t process_step_limit;
    uint64_t region_passes;    // zero-delay guard units in the current time step (region passes + coroutine resumes)
    const char* last_process_name; // survives completed-process reclamation
    llg_proc_t** all_procs;   // checked-growable slot table; NULL holes are free
    int n_procs;
    int all_procs_capacity;
    // Six radix-64 levels cover the entire int-indexed registry.
    uint64_t* proc_free_bits[6];
    int proc_free_levels;
    size_t program_processes;       // live program initial procedures only
    llg_program_t* programs;        // stable origins, owned until runtime cleanup
    int program_completion_pending; // service after the full cancellation batch
    llg_proc_t* retired_procs; // cancelled coroutines awaiting a safe destroy point
    llg_process_handle_t* process_handles; // stable identities for live/exited procs
    llg_semaphore_t* semaphores; // all semaphore objects owned by this run
    llg_mailbox_t* mailboxes;       // runtime-owned mailbox objects
    llg_fork_group_t* zombie_groups; // completed/killed groups awaiting teardown
    llg_activation_t* activations; // active named block/task invocations
    llg_monitor_state_t mon;   // the active $monitor (at most one)
    llg_strobe_t* strobes;     // pending $strobe lines for this time step
    llg_strobe_t* strobe_tail; // preserves source issue order
    // Active procedural forces. Entries own copied target/source descriptors;
    // no pre-force value is retained because release is object-specific.
    llg_force_entry_t* force_table;   // checked-growable live-entry table
    int force_count;
    int force_capacity;
    llg_pca_binding_t* pca_table;
    int pca_count;
    int pca_capacity;
    llg_pca_real_binding_t* pca_real_table;
    int pca_real_count;
    int pca_real_capacity;
    llg_rng_state_t rng_root;
    int argc;
    char** argv;
    llg_q_queue_t* q_queues;
} llg_rt_ctx_t;

static llg_rt_ctx_t g;

static void process_status_set(llg_proc_t* proc, int status) {
    if (!proc) return;
    proc->status = status;
    if (proc->handle) proc->handle->status = status;
}

static void process_handle_unlink(llg_process_handle_t* handle) {
    if (!handle || !handle->linked) return;
    *handle->prev_link = handle->next;
    if (handle->next) handle->next->prev_link = handle->prev_link;
    handle->linked = 0;
    handle->next = NULL;
    handle->prev_link = NULL;
}

static llg_process_handle_t* process_handle_new(llg_proc_t* proc) {
    llg_process_handle_t* handle = (llg_process_handle_t*)llg_checked_calloc(
        1, sizeof(*handle), "process handle");
    handle->refs = 1; // process ownership
    handle->proc = proc;
    handle->status = LLG_PROCESS_RUNNING;
    handle->linked = 1;
    handle->next = g.process_handles;
    handle->prev_link = &g.process_handles;
    if (handle->next) handle->next->prev_link = &handle->next;
    g.process_handles = handle;
    return handle;
}

// File descriptors live outside the scheduler so final procedures can use them.
// FD values have bit 31 set; MCD values are independent channel masks. Keep
// distinct banks so a bit mask can never select an ordinary FD by accident.
#define LLG_FILE_SLOTS 64u
#define LLG_FILE_STDIN 0u
#define LLG_FILE_STDOUT 1u
#define LLG_FILE_STDERR 2u
#define LLG_FILE_MCD_FIRST 3u
#define LLG_FILE_MCD_END 33u
#define LLG_FILE_FD_FIRST 33u
#define LLG_FILE_FD_TAG UINT32_C(0x80000000)
#define LLG_FILE_PUSHBACK 256u

typedef struct {
    FILE* stream;
    int open;
    int owned;
    int error;
    int eof;
    char message[160];
    unsigned char pushback[LLG_FILE_PUSHBACK];
    size_t pushback_len;
} llg_file_slot_t;

static llg_file_slot_t llg_file_slots[LLG_FILE_SLOTS];
static int llg_files_initialized;
static int llg_file_global_error;
static char llg_file_global_message[160];
// Keep ordinary streams alive between the scheduler and registered final
// blocks; the final phase's cleanup closes them after its last output.
static int llg_file_defer_cleanup;

static void llg_file_cleanup(void);

// `llg_rt_run` tears down the scheduler before generated final blocks run,
// but `$timeformat` is design-wide state that remains observable there. Keep
// a cloned snapshot across that teardown and move it back for the finals.
static llg_timeformat_state_t llg_final_time_format;
static uint64_t llg_final_design_precision_fs;
static int llg_final_timeformat_valid;

static void llg_clear_final_timeformat(void) {
    if (!llg_final_timeformat_valid) return;
    llg_string_destroy(&llg_final_time_format.suffix);
    memset(&llg_final_time_format, 0, sizeof(llg_final_time_format));
    llg_final_design_precision_fs = 0;
    llg_final_timeformat_valid = 0;
}

static void llg_save_final_timeformat(void) {
    llg_clear_final_timeformat();
    llg_final_design_precision_fs = g.design_precision_fs;
    llg_final_time_format.unit_fs = g.time_format.unit_fs;
    llg_final_time_format.precision = g.time_format.precision;
    llg_final_time_format.minimum_field_width = g.time_format.minimum_field_width;
    llg_final_time_format.suffix = llg_string_clone(&g.time_format.suffix);
    llg_final_timeformat_valid = 1;
}

static void llg_restore_final_timeformat(void) {
    if (!llg_final_timeformat_valid) return;
    g.design_precision_fs = llg_final_design_precision_fs;
    g.time_format.unit_fs = llg_final_time_format.unit_fs;
    g.time_format.precision = llg_final_time_format.precision;
    g.time_format.minimum_field_width = llg_final_time_format.minimum_field_width;
    g.time_format.suffix = llg_final_time_format.suffix;
    llg_final_time_format.suffix = (llg_string_t){0};
    llg_final_design_precision_fs = 0;
    llg_final_timeformat_valid = 0;
}

// `$timeformat` units are decimal powers of seconds.  The runtime stores all
// quantities as femtoseconds, so keep the finite standard range in one table
// instead of relying on floating-point conversions.
// Time units span 1fs (10^-15 s) to 100s (10^2 s), SV 3.14.2.1. `$timeformat`
// separately restricts its display unit to 1fs..1s.
static uint64_t llg_time_unit_from_exponent(int64_t exponent) {
    static const uint64_t units[] = {
        1ULL, 10ULL, 100ULL, 1000ULL, 10000ULL, 100000ULL,
        1000000ULL, 10000000ULL, 100000000ULL, 1000000000ULL,
        10000000000ULL, 100000000000ULL, 1000000000000ULL,
        10000000000000ULL, 100000000000000ULL, 1000000000000000ULL,
        10000000000000000ULL, 100000000000000000ULL,
    };
    if (exponent < -15 || exponent > 2) return 0;
    return units[(size_t)(exponent + 15)];
}

static int llg_time_unit_exponent(uint64_t unit_fs) {
    for (int exponent = -15; exponent <= 2; ++exponent) {
        if (llg_time_unit_from_exponent(exponent) == unit_fs) return exponent;
    }
    return INT_MIN;
}

static void llg_timeformat_defaults(uint64_t precision_fs) {
    g.design_precision_fs = precision_fs;
    g.time_format.unit_fs = precision_fs;
    g.time_format.precision = 0;
    g.time_format.minimum_field_width = 20;
    g.time_format.suffix = llg_string_bytes("", 0);
}

typedef struct llg_dependency_binding {
    struct llg_dependency_binding* next;
    /* Chain of bindings whose target hashes to the same bucket. */
    struct llg_dependency_binding* bucket_next;
    sv4_t* target;
    double* real_target;
    sv4_t* dependency;
} llg_dependency_binding_t;

/* The list owns every binding; the bucket index borrows them. Each write
 * consults only the written target's bucket, so dense arrays with one binding
 * per cell keep both startup binding and per-write publication independent
 * of the cell count. */
static llg_dependency_binding_t* llg_dependency_bindings;
static llg_dependency_binding_t** llg_dependency_buckets;
static size_t llg_dependency_bucket_count;
static size_t llg_dependency_binding_count;

// Kept outside `g`: every completed run cleans the context, but callers need
// to inspect whether that run stopped with a controlled runtime failure.
static int llg_last_failure;
static int llg_last_config_error;
static uint64_t llg_severity_counts[4];
static uint64_t llg_assertion_failure_counts[4];
static uint64_t llg_assertion_cover_count;
static uint64_t llg_assertion_vacuous_total;
static uint64_t llg_assertion_event_order;
// Event objects are generated as file-scope storage and therefore survive
// `llg_rt_cleanup`. Bump this generation at each teardown so their persistent
// same-slot state cannot leak into a later runtime initialization without
// dereferencing an object whose owner may be outside the scheduler.
static uint64_t llg_event_generation;
static uint64_t llg_configured_zero_loop_limit;
static uint64_t llg_configured_process_step_limit;
static int llg_configured_stop_policy = LLG_STOP_POLICY_RESUME;
static int llg_stop_policy_override;

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

// A packed or real store in a read-only region: applied silently inside a
// Postponed display evaluator's private evaluation (see `private_evaluation`),
// otherwise reported. Returns whether the caller should store unpublished.
static int region_private_store(const char* action) {
    if (g.private_evaluation && g.current_region == LLG_REGION_POSTPONED) return 1;
    region_violation(action, g.current_region);
    return 0;
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

static int configure_stop_policy(void) {
    if (llg_stop_policy_override) {
        g.stop_policy = llg_configured_stop_policy;
        return 1;
    }
    const char* text = getenv("LLG_STOP_POLICY");
    if (!text || strcmp(text, "resume") == 0) {
        g.stop_policy = LLG_STOP_POLICY_RESUME;
        return 1;
    }
    if (strcmp(text, "exit") == 0) {
        g.stop_policy = LLG_STOP_POLICY_EXIT;
        return 1;
    }
    fprintf(stderr,
            "llg: invalid LLG_STOP_POLICY `%s` (expected resume or exit)\n",
            text);
    return 0;
}

static int consume_limit(uint64_t* counter, uint64_t limit) {
    if (*counter >= limit) return 0;
    *counter += 1;
    return 1;
}
// ── Run-time output placement ───────────────────────────────────────────────
//
// Environment read by every runtime initialization, so one built model can run
// many times with different destinations:
//
// - LLG_SIM_OUT_DIR:   base for relative paths the simulation writes
//                      (waveform, $fopen write/append modes, $writemem*,
//                      LLG_SIM_LOG_FILE). Created when missing. Reads keep
//                      resolving from the current directory.
// - LLG_SIM_WAVE_FILE: waveform file; replaces $dumpfile and `dump.vcd`.
//                      Read by llg_wave.c itself, which stays independent of
//                      this runtime; the directory is created here.
// - LLG_SIM_LOG_FILE:  copy of stdout and stderr (POSIX only).

#if defined(_WIN32)
#include <direct.h>
#include <sys/stat.h>
#else
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

static char* llg_output_dir;  // NULL: relative writes use the CWD

static char* llg_output_strdup(const char* text) {
    size_t len = strlen(text);
    char* copy = (char*)llg_checked_malloc(len + 1u, 1, "output path");
    memcpy(copy, text, len + 1u);
    return copy;
}

static int llg_output_is_separator(char c) {
#if defined(_WIN32)
    return c == '/' || c == '\\';
#else
    return c == '/';
#endif
}

static int llg_output_is_absolute(const char* path) {
    if (llg_output_is_separator(path[0])) return 1;
#if defined(_WIN32)
    return isalpha((unsigned char)path[0]) && path[1] == ':';
#else
    return 0;
#endif
}

// Resolve a path the simulation writes against LLG_SIM_OUT_DIR (absolute paths
// and an unset directory leave it unchanged). The caller frees the result.
static char* llg_output_path(const char* path) {
    if (!path) path = "";
    if (!llg_output_dir || path[0] == '\0' || llg_output_is_absolute(path))
        return llg_output_strdup(path);
    size_t dir_len = strlen(llg_output_dir);
    size_t path_len = strlen(path);
    int separator = dir_len > 0 && !llg_output_is_separator(llg_output_dir[dir_len - 1]);
    char* joined = (char*)llg_checked_malloc(dir_len + (size_t)separator + path_len + 1u, 1,
                                             "output path");
    memcpy(joined, llg_output_dir, dir_len);
    if (separator) joined[dir_len] = '/';
    memcpy(joined + dir_len + (size_t)separator, path, path_len + 1u);
    return joined;
}

static int llg_output_mkdir_one(const char* dir) {
#if defined(_WIN32)
    if (_mkdir(dir) == 0 || errno == EEXIST) return 1;
#else
    if (mkdir(dir, 0777) == 0 || errno == EEXIST) return 1;
#endif
    return 0;
}

static int llg_output_is_dir(const char* dir) {
#if defined(_WIN32)
    struct _stat info;
    return _stat(dir, &info) == 0 && (info.st_mode & _S_IFDIR);
#else
    struct stat info;
    return stat(dir, &info) == 0 && S_ISDIR(info.st_mode);
#endif
}

// `mkdir -p`: create every missing component of `dir`.
static int llg_output_make_dirs(const char* dir) {
    char* path = llg_output_strdup(dir);
    int ok = 1;
    for (char* p = path + 1; *p && ok; ++p) {
        if (!llg_output_is_separator(*p) || llg_output_is_separator(p[-1])) continue;
#if defined(_WIN32)
        if (p[-1] == ':') continue;  // drive root such as `C:\`
#endif
        char saved = *p;
        *p = '\0';
        ok = llg_output_mkdir_one(path);
        *p = saved;
    }
    if (ok) ok = llg_output_mkdir_one(path);
    int saved_errno = errno;
    if (ok && !llg_output_is_dir(path)) {
        ok = 0;
        saved_errno = ENOTDIR;
    }
    free(path);
    errno = saved_errno;
    return ok;
}

// ── Console log (LLG_SIM_LOG_FILE) ──────────────────────────────────────────
//
// stdout and stderr are redirected into pipes read by a forked tee process
// that writes each chunk to the original stream and to the log. A separate
// process (not a thread) keeps output that precedes an abort or crash: the
// tee sees EOF when the simulator dies and still drains the pipes. When both
// streams reach the same file (a terminal, `2>&1`), one shared pipe keeps
// their exact order; separately redirected streams use one pipe each, so only
// the log's stdout/stderr interleaving can differ. Set up once per process;
// later initializations keep the active log.

static int llg_console_log_active;

#if !defined(_WIN32)
static pid_t llg_console_log_pid = -1;
static int llg_console_saved_out = -1;
static int llg_console_saved_err = -1;

static int llg_console_write_all(int fd, const char* data, size_t len) {
    while (len > 0) {
        ssize_t written = write(fd, data, len);
        if (written < 0) {
            if (errno == EINTR) continue;
            return 0;
        }
        data += written;
        len -= (size_t)written;
    }
    return 1;
}

// Child side: only async-signal-safe calls, since the parent may already be
// multithreaded when embedded.
static void llg_console_tee(int log_fd, int out_read, int err_read, int out, int err) {
    // Interrupts and a closed console must not cut the log short; the tee
    // exits on EOF once the simulator side has gone.
    signal(SIGINT, SIG_IGN);
    signal(SIGQUIT, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGHUP, SIG_IGN);
    signal(SIGPIPE, SIG_IGN);
    // err_read < 0: stderr shares the stdout pipe (poll ignores negative fds).
    struct pollfd fds[2] = {{out_read, POLLIN, 0}, {err_read, POLLIN, 0}};
    int targets[2] = {out, err};
    int open_count = err_read < 0 ? 1 : 2;
    static char buffer[65536];
    while (open_count > 0) {
        if (poll(fds, 2, -1) < 0) {
            if (errno == EINTR) continue;
            break;
        }
        for (int i = 0; i < 2; ++i) {
            if (fds[i].fd < 0 || !(fds[i].revents & (POLLIN | POLLHUP | POLLERR))) continue;
            ssize_t count = read(fds[i].fd, buffer, sizeof(buffer));
            if (count < 0 && errno == EINTR) continue;
            if (count <= 0) {
                close(fds[i].fd);
                fds[i].fd = -1;
                --open_count;
                continue;
            }
            if (targets[i] >= 0 && !llg_console_write_all(targets[i], buffer, (size_t)count))
                targets[i] = -1;
            (void)llg_console_write_all(log_fd, buffer, (size_t)count);
        }
    }
    _exit(0);
}

// Restore the original streams and wait until the tee has written everything.
static void llg_console_log_finish(void) {
    if (llg_console_log_pid < 0) return;
    fflush(stdout);
    fflush(stderr);
    (void)dup2(llg_console_saved_out, STDOUT_FILENO);
    (void)dup2(llg_console_saved_err, STDERR_FILENO);
    close(llg_console_saved_out);
    close(llg_console_saved_err);
    int status;
    while (waitpid(llg_console_log_pid, &status, 0) < 0 && errno == EINTR) {
    }
    llg_console_log_pid = -1;
}

static int llg_console_dup_cloexec(int fd) {
    return fcntl(fd, F_DUPFD_CLOEXEC, 3);
}

static int llg_console_log_start(const char* path) {
    if (llg_console_log_active) return 1;
    char* resolved = llg_output_path(path);
    int log_fd = open(resolved, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0666);
    if (log_fd < 0) {
        fprintf(stderr, "llg: cannot open LLG_SIM_LOG_FILE `%s`: %s\n", resolved,
                strerror(errno));
        free(resolved);
        return 0;
    }
    free(resolved);
    int out_pipe[2] = {-1, -1};
    int err_pipe[2] = {-1, -1};
    fflush(stdout);
    fflush(stderr);
    int saved_out = llg_console_dup_cloexec(STDOUT_FILENO);
    int saved_err = llg_console_dup_cloexec(STDERR_FILENO);
    struct stat out_info;
    struct stat err_info;
    int shared = saved_out >= 0 && saved_err >= 0 && fstat(saved_out, &out_info) == 0 &&
                 fstat(saved_err, &err_info) == 0 && out_info.st_dev == err_info.st_dev &&
                 out_info.st_ino == err_info.st_ino;
    if (saved_out < 0 || saved_err < 0 || pipe(out_pipe) != 0 ||
        (!shared && pipe(err_pipe) != 0)) {
        fprintf(stderr, "llg: cannot set up LLG_SIM_LOG_FILE: %s\n", strerror(errno));
        int fds[] = {log_fd, saved_out, saved_err, out_pipe[0], out_pipe[1],
                     err_pipe[0], err_pipe[1]};
        for (size_t i = 0; i < sizeof(fds) / sizeof(fds[0]); ++i)
            if (fds[i] >= 0) close(fds[i]);
        return 0;
    }
    pid_t pid = fork();
    if (pid < 0) {
        fprintf(stderr, "llg: cannot start LLG_SIM_LOG_FILE writer: %s\n", strerror(errno));
        int fds[] = {log_fd, saved_out, saved_err, out_pipe[0], out_pipe[1],
                     err_pipe[0], err_pipe[1]};
        for (size_t i = 0; i < sizeof(fds) / sizeof(fds[0]); ++i)
            if (fds[i] >= 0) close(fds[i]);
        return 0;
    }
    if (pid == 0) {
        close(out_pipe[1]);
        if (!shared) close(err_pipe[1]);
        llg_console_tee(log_fd, out_pipe[0], shared ? -1 : err_pipe[0], saved_out, saved_err);
    }
    close(log_fd);
    close(out_pipe[0]);
    (void)dup2(out_pipe[1], STDOUT_FILENO);
    if (shared) {
        (void)dup2(out_pipe[1], STDERR_FILENO);
    } else {
        close(err_pipe[0]);
        (void)dup2(err_pipe[1], STDERR_FILENO);
        close(err_pipe[1]);
    }
    close(out_pipe[1]);
    // A pipe makes stdout fully buffered; keep line-at-a-time output when the
    // user is watching a terminal.
    if (isatty(saved_out)) setvbuf(stdout, NULL, _IOLBF, 0);
    llg_console_saved_out = saved_out;
    llg_console_saved_err = saved_err;
    llg_console_log_pid = pid;
    llg_console_log_active = 1;
    atexit(llg_console_log_finish);
    return 1;
}
#else
static int llg_console_log_start(const char* path) {
    (void)path;
    fprintf(stderr, "llg: LLG_SIM_LOG_FILE is not supported on Windows\n");
    return 0;
}
#endif

static const char* llg_output_env(const char* name) {
    const char* value = getenv(name);
    return value && value[0] ? value : NULL;
}

static int configure_output_files(void) {
    free(llg_output_dir);
    llg_output_dir = NULL;
    const char* dir = llg_output_env("LLG_SIM_OUT_DIR");
    if (dir) {
        if (!llg_output_make_dirs(dir)) {
            fprintf(stderr, "llg: cannot create LLG_SIM_OUT_DIR `%s`: %s\n", dir,
                    strerror(errno));
            return 0;
        }
        llg_output_dir = llg_output_strdup(dir);
    }
    const char* log = llg_output_env("LLG_SIM_LOG_FILE");
    return !log || llg_console_log_start(log);
}

// Registered final-block processes (see llg_rt.h).  Kept OUTSIDE the runtime
// context: `llg_rt_cleanup` memsets the context, and registration happens
// around the `llg_rt_run()` call in generated `main()`.
typedef struct {
    void (*fn)(void);
    const char* name;
} llg_final_registration_t;
static llg_final_registration_t* llg_finals;
static int llg_n_finals;
static int llg_finals_capacity;
static int llg_in_finals;
// Scheduler time when `llg_rt_run` exited; `$time` inside finals reports it.
static uint64_t llg_final_time;

#define LLG_REGISTRY_INITIAL 16

// Double a registry capacity until it can hold `needed` entries. Growth is
// bounded by the `int` index type; overflow aborts explicitly. Every grown
// table is fully populated before it replaces the live one, so an allocation
// failure aborts without a partially rebound registry.
static int llg_registry_capacity(int current, int needed) {
    int capacity = current > 0 ? current : LLG_REGISTRY_INITIAL;
    while (capacity < needed) {
        if (capacity > INT_MAX / 2) {
            fprintf(stderr, "llg runtime fatal: registry capacity overflow\n");
            abort();
        }
        capacity *= 2;
    }
    return capacity;
}

// A set bit denotes a hole below n_procs; upper levels summarize nonempty
// words. Lowest-hole reuse preserves cancellation traversal order. At most
// six words are visited for any int-sized registry, independent of population.
static void proc_free_set(int slot, int available) {
    size_t index = (size_t)slot;
    for (int level = 0; level < g.proc_free_levels; level++) {
        size_t word = index / 64;
        uint64_t mask = UINT64_C(1) << (index % 64);
        uint64_t before = g.proc_free_bits[level][word];
        if (available) g.proc_free_bits[level][word] |= mask;
        else g.proc_free_bits[level][word] &= ~mask;
        uint64_t after = g.proc_free_bits[level][word];
        if ((before != 0) == (after != 0)) break;
        available = after != 0;
        index = word;
    }
}

static unsigned proc_first_bit(uint64_t bits) {
    unsigned index = 0;
    // Portable C11 bounded binary search; callers supply a nonzero word.
    if (!(bits & UINT64_C(0xffffffff))) { bits >>= 32; index += 32; }
    if (!(bits & UINT64_C(0xffff))) { bits >>= 16; index += 16; }
    if (!(bits & UINT64_C(0xff))) { bits >>= 8; index += 8; }
    if (!(bits & UINT64_C(0xf))) { bits >>= 4; index += 4; }
    if (!(bits & UINT64_C(0x3))) { bits >>= 2; index += 2; }
    if (!(bits & UINT64_C(0x1))) index++;
    return index;
}

static int proc_first_free(void) {
    if (!g.proc_free_levels ||
        !g.proc_free_bits[g.proc_free_levels - 1][0]) return g.n_procs;
    size_t index = 0;
    for (int level = g.proc_free_levels - 1; level >= 0; level--)
        index = index * 64 + proc_first_bit(g.proc_free_bits[level][index]);
    return (int)index;
}

static void all_procs_reserve(int needed) {
    if (needed <= g.all_procs_capacity) return;
    int capacity = llg_registry_capacity(g.all_procs_capacity, needed);
    llg_proc_t** grown = (llg_proc_t**)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "process registry");
    if (g.all_procs)
        memcpy(grown, g.all_procs, (size_t)g.n_procs * sizeof(*grown));
    uint64_t* bits[6] = {0};
    size_t count = (size_t)capacity;
    int levels = 0;
    do {
        count = (count + 63) / 64;
        bits[levels++] = (uint64_t*)llg_checked_calloc(
            count, sizeof(uint64_t), "process free slots");
    } while (count > 1);
    for (int level = 0; level < g.proc_free_levels; level++)
        free(g.proc_free_bits[level]);
    memcpy(g.proc_free_bits, bits, sizeof(bits));
    g.proc_free_levels = levels;
    free(g.all_procs);
    g.all_procs = grown;
    g.all_procs_capacity = capacity;
    for (int i = 0; i < g.n_procs; i++)
        if (!grown[i]) proc_free_set(i, 1);
}

static void finals_reserve(int needed) {
    if (needed <= llg_finals_capacity) return;
    int capacity = llg_registry_capacity(llg_finals_capacity, needed);
    llg_final_registration_t* grown = (llg_final_registration_t*)llg_checked_malloc(
        (size_t)capacity, sizeof(*grown), "final block registry");
    if (llg_finals)
        memcpy(grown, llg_finals, (size_t)llg_n_finals * sizeof(*grown));
    free(llg_finals);
    llg_finals = grown;
    llg_finals_capacity = capacity;
}

static void finals_release(void) {
    free(llg_finals);
    llg_finals = NULL;
    llg_finals_capacity = 0;
}

static void register_proc(llg_proc_t* p) {
    if (!p || g.next_process_identity == UINT64_MAX) {
        fprintf(stderr, "llg: process identity overflow\n");
        abort();
    }
    p->assertion_owner = ++g.next_process_identity;
    int slot = proc_first_free();
    if (slot == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: process registry size overflow\n");
        abort();
    }
    all_procs_reserve(slot + 1);
    p->registry_slot = slot;
    g.all_procs[slot] = p;
    if (slot == g.n_procs) g.n_procs++;
    else proc_free_set(slot, 0);
}

static void unregister_proc(llg_proc_t* p) {
    if (!p || p->registry_slot < 0 || p->registry_slot >= g.n_procs ||
        g.all_procs[p->registry_slot] != p) return;
    int slot = p->registry_slot;
    p->registry_slot = -1;
    g.all_procs[slot] = NULL;
    proc_free_set(slot, 1);
    // Each trailing slot is trimmed once per registration: amortized O(1).
    while (g.n_procs > 0 && !g.all_procs[g.n_procs - 1]) {
        proc_free_set(g.n_procs - 1, 0);
        g.n_procs--;
    }
}

llg_proc_t* llg_current(void) { return g.current; }

static llg_co_chain_t* llg_rt_current_chain(void) {
    return g.current ? &g.current->chain : NULL;
}

// Do not recurse into cancellation while a tree is being unlinked. The
// caller services completed programs after finishing the cancellation batch.
static void release_program_process(llg_proc_t* process) {
    if (!process || !process->program_live) return;
    process->program_live = 0;
    if (!process->program || process->program->live_initials == 0 ||
        g.program_processes == 0) {
        fprintf(stderr, "llg: program initial accounting underflow\n");
        abort();
    }
    process->program->live_initials--;
    g.program_processes--;
    g.program_completion_pending = 1;
}

static void service_program_completions(void) {
    if (!g.program_completion_pending || g.finish || g.config_error) return;
    g.program_completion_pending = 0;
    for (llg_program_t* program = g.programs; program; program = program->next) {
        if (!program->had_initial || program->live_initials || program->closed)
            continue;
        program->closed = 1;
        // Restart after cancellation: it changes the process and group lists.
        for (;;) {
            llg_proc_t* victim = NULL;
            for (int i = 0; i < g.n_procs; i++) {
                llg_proc_t* proc = g.all_procs[i];
                if (proc && proc->program == program && !proc->completed &&
                    !proc->killed) {
                    victim = proc;
                    break;
                }
            }
            if (!victim) break;
            llg_kill_proc_tree(victim);
        }
    }
    // This is an immediate finish boundary, not a request to drain Re-NBAs
    // or execute detached descendants before entering final procedures.
    if (g.program_processes == 0) g.finish = 1;
}

/* Calls made while a generated process is running use that process's stream.
 * The root stream is the safe fallback for initialization callbacks and
 * embedding code that invokes the service outside a coroutine. */
static llg_rng_state_t* llg_process_rng(void) {
    llg_proc_t* process = llg_current();
    return process ? &process->rng : &g.rng_root;
}

static int llg_rng_argument(sv4_t value, uint32_t* result) {
    if (sv4_is_unknown(value) || llg_sv4_width(value) == 0) return 0;
    *result = (uint32_t)sv4_to_u64(value);
    return 1;
}

sv4_t llg_urandom(void) {
    return sv4_from_u64((uint64_t)llg_rng_state_next(llg_process_rng()), 32, 0);
}

sv4_t llg_urandom_seed(sv4_t seed) {
    uint32_t value = 0;
    if (!llg_rng_argument(seed, &value)) return sv4_x(32, 0);
    llg_rng_state_seed(llg_process_rng(), value);
    return llg_urandom();
}

sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min) {
    uint32_t high;
    uint32_t low = 0;
    if (!llg_rng_argument(max, &high) ||
        (has_min && !llg_rng_argument(min, &low)))
        return sv4_x(32, 0);
    if (!has_min) low = 0;
    return sv4_from_u64(
        (uint64_t)llg_rng_state_uniform(llg_process_rng(), high, low), 32, 0);
}

void llg_process_srandom(sv4_t seed) {
    uint32_t value = 0;
    if (!llg_rng_argument(seed, &value)) {
        fprintf(stderr, "llg: random runtime: srandom seed is unknown or real\n");
        llg_last_failure = 1;
        return;
    }
    llg_rng_state_seed(llg_process_rng(), value);
}

llg_string_t llg_process_get_randstate(void) {
    return llg_rng_state_get(llg_process_rng());
}

int llg_process_set_randstate(llg_string_t state) {
    int ok = llg_rng_state_set(llg_process_rng(), &state);
    if (!ok) {
        fprintf(stderr, "llg: random runtime: invalid randstate string\n");
        llg_last_failure = 1;
    }
    llg_string_destroy(&state);
    return ok;
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
    // Iterative: the parent chain follows lexical and call nesting, so its
    // length depends on the design and must not consume native stack.
    while (activation) {
        if (activation->refs == 0) {
            fprintf(stderr, "llg: named activation reference count underflow\n");
            abort();
        }
        activation->refs--;
        if (activation->refs != 0) return;
        // A detached activation can remain the owner of a join_none group.
        // Keep its lexical ancestry alive until that last owner reference is
        // released so a later disable of an active outer scope still finds
        // the retained descendant through the parent chain.
        llg_activation_t* parent = activation->parent;
        activation->parent = NULL;
        free(activation);
        activation = parent;
    }
}

static void activation_unlink_all(llg_activation_t* activation) {
    if (!activation->all_prev_link) return;
    *activation->all_prev_link = activation->all_next;
    if (activation->all_next)
        activation->all_next->all_prev_link = activation->all_prev_link;
    activation->all_prev_link = NULL;
    activation->all_next = NULL;
}

static void activation_unlink_process(llg_activation_t* activation) {
    if (!activation->proc_prev_link) return;
    *activation->proc_prev_link = activation->proc_next;
    if (activation->proc_next)
        activation->proc_next->proc_prev_link = activation->proc_prev_link;
    activation->proc_prev_link = NULL;
    activation->proc_next = NULL;
}

static void activation_detach(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_unlink_process(activation);
    activation_unlink_all(activation);
    activation->detached = 1;
}

typedef struct llg_ref_binding {
    llg_ref_t descriptor;
    struct llg_ref_binding* next;
} llg_ref_binding_t;

struct llg_ref_scope {
    llg_proc_t* proc;
    llg_ref_scope_t* parent;
    llg_ref_binding_t* bindings;
};

static llg_ref_scope_t* root_reference_top;

llg_ref_scope_t* llg_ref_scope_begin(void) {
    llg_proc_t* proc = llg_current();
    llg_ref_scope_t** top = proc ? &proc->reference_top : &root_reference_top;
    llg_ref_scope_t* scope = llg_checked_calloc(1, sizeof(*scope), "reference scope");
    scope->proc = proc;
    scope->parent = *top;
    *top = scope;
    return scope;
}

void llg_ref_scope_end(llg_ref_scope_t* scope) {
    if (!scope) return;
    llg_ref_scope_t** top = scope->proc ? &scope->proc->reference_top : &root_reference_top;
    if (*top != scope) { fprintf(stderr, "llg: unbalanced reference scopes\n"); abort(); }
    *top = scope->parent;
    while (scope->bindings) {
        llg_ref_binding_t* binding = scope->bindings;
        scope->bindings = binding->next;
        llg_queue_ref_release(binding->descriptor.retained);
        free(binding);
    }
    free(scope);
}

static void reference_owner_destroy(void* payload) {
    llg_ref_scope_t* scope = *(llg_ref_scope_t**)payload;
    if (scope) llg_ref_scope_end(scope);
}

void llg_ref_scope_begin_owned(void) {
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_ref_scope_t*), reference_owner_destroy);
    *(llg_ref_scope_t**)llg_value_scope_object(owner) = llg_ref_scope_begin();
}

llg_ref_t* llg_ref_queue(llg_queue_t* queue, uint64_t index) {
    llg_proc_t* proc = llg_current();
    llg_ref_scope_t* scope = proc ? proc->reference_top : root_reference_top;
    if (!scope || !queue) { fprintf(stderr, "llg: queue reference without call scope\n"); abort(); }
    llg_ref_binding_t* binding = llg_checked_calloc(1, sizeof(*binding), "queue reference");
    binding->descriptor.width = queue->element_width;
    binding->descriptor.is_signed = queue->element_signed;
    binding->descriptor.two_state = queue->element_two_state;
    binding->descriptor.kind = LLG_REF_QUEUE;
    binding->descriptor.retained = llg_queue_ref_acquire(queue, index);
    binding->descriptor.retained_read = llg_queue_cell_read;
    binding->descriptor.retained_write = llg_queue_cell_write;
    binding->next = scope->bindings;
    scope->bindings = binding;
    return &binding->descriptor;
}

static void activation_unwind_proc(llg_proc_t* proc) {
    while (proc && proc->reference_top) llg_ref_scope_end(proc->reference_top);
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
    activation->proc_prev_link = &proc->activation_top;
    if (activation->proc_next)
        activation->proc_next->proc_prev_link = &activation->proc_next;
    proc->activation_top = activation;
    activation->all_next = g.activations;
    activation->all_prev_link = &g.activations;
    if (activation->all_next)
        activation->all_next->all_prev_link = &activation->all_next;
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
    if (p) p->chain.exiting = LLG_EXIT_ABANDON;
}

int llg_budget_point(const char* location) {
    llg_proc_t* p = llg_current();
    llg_runtime_service_enter(p, "budget point");
    if (!p || g.config_error) return 0;
    if (p->budget_time != g.now) {
        p->budget_time = g.now;
        p->budget_steps = 0;
    }
    if (!consume_limit(&p->budget_steps, g.process_step_limit)) {
        budget_abort(p, location);
        return 1;
    }
    return 0;
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
/* Exact descriptor identity lookup. Hash integer representations, but compare
 * pointers only for equality; never order/subtract unrelated C pointers.
 * Entries outlive lexical scope exit when an NBA retains the scope. */
typedef struct {
    const void* target;
    llg_value_scope_t* scope;
} llg_value_scope_entry_t;

static llg_value_scope_entry_t* value_scope_index;
static size_t value_scope_capacity;
static size_t value_scope_count;
static size_t value_scope_used;
/* A real, private object is a portable tombstone, not a fabricated pointer. */
static sv4_t value_scope_deleted_key = SV4_EMPTY;

static size_t value_scope_hash(const void* target) {
    uint64_t hash = (uint64_t)(uintptr_t)target;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void value_scope_index_rebuild(size_t capacity) {
    llg_value_scope_entry_t* entries = (llg_value_scope_entry_t*)llg_checked_calloc(
        capacity, sizeof(*entries), "value scope address index");
    for (size_t i = 0; i < value_scope_capacity; ++i) {
        llg_value_scope_entry_t entry = value_scope_index[i];
        if (!entry.scope) continue;
        size_t slot = value_scope_hash(entry.target) & (capacity - 1);
        while (entries[slot].target) slot = (slot + 1) & (capacity - 1);
        entries[slot] = entry;
    }
    free(value_scope_index);
    value_scope_index = entries;
    value_scope_capacity = capacity;
    value_scope_used = value_scope_count;
}

/* Packed scopes index each descriptor; native scopes index their payload base.
 * Interior native fields do not acquire lifetime guarantees from this index. */
static size_t value_scope_key_count(const llg_value_scope_t* scope) {
    return scope->count ? scope->count : (scope->object != NULL ? 1u : 0u);
}

static const void* value_scope_key(const llg_value_scope_t* scope, size_t index) {
    return scope->count ? (const void*)&scope->values[index] : scope->object;
}

static void value_scope_index_add(llg_value_scope_t* scope) {
    size_t count = value_scope_key_count(scope);
    if (!count) return;
    if (count > SIZE_MAX - value_scope_count) {
        fputs("llg runtime fatal: value scope index size overflow\n", stderr);
        abort();
    }
    size_t needed = value_scope_count + count;
    size_t capacity = value_scope_capacity ? value_scope_capacity : 16;
    while (needed > capacity - capacity / 4) {
        if (capacity > SIZE_MAX / 2) {
            fputs("llg runtime fatal: value scope index capacity overflow\n", stderr);
            abort();
        }
        capacity *= 2;
    }
    /* Reclaim tombstones before they can force a full-table probe. */
    if (capacity != value_scope_capacity ||
        count > capacity - capacity / 4 - value_scope_used)
        value_scope_index_rebuild(capacity);
    for (size_t i = 0; i < count; ++i) {
        const void* target = value_scope_key(scope, i);
        size_t slot = value_scope_hash(target) & (capacity - 1);
        while (value_scope_index[slot].scope)
            slot = (slot + 1) & (capacity - 1);
        if (!value_scope_index[slot].target) ++value_scope_used;
        value_scope_index[slot] = (llg_value_scope_entry_t){target, scope};
        ++value_scope_count;
    }
}

static llg_value_scope_t* value_scope_index_find(const void* target) {
    if (!value_scope_count) return NULL;
    size_t slot = value_scope_hash(target) & (value_scope_capacity - 1);
    while (value_scope_index[slot].target) {
        if (value_scope_index[slot].target == target)
            return value_scope_index[slot].scope;
        slot = (slot + 1) & (value_scope_capacity - 1);
    }
    return NULL;
}

static void value_scope_index_remove(llg_value_scope_t* scope) {
    size_t count = value_scope_key_count(scope);
    for (size_t i = 0; i < count; ++i) {
        const void* target = value_scope_key(scope, i);
        size_t slot = value_scope_hash(target) & (value_scope_capacity - 1);
        while (value_scope_index[slot].target != target) {
            if (!value_scope_index[slot].target) {
                fputs("llg runtime fatal: value scope index entry missing\n", stderr);
                abort();
            }
            slot = (slot + 1) & (value_scope_capacity - 1);
        }
        value_scope_index[slot] =
            (llg_value_scope_entry_t){&value_scope_deleted_key, NULL};
        --value_scope_count;
    }
    if (!value_scope_count) {
        free(value_scope_index);
        value_scope_index = NULL;
        value_scope_capacity = 0;
        value_scope_used = 0;
    }
}

/* Scope references protect descriptor addresses, not just their limb payloads.
 * A queued NBA may outlive both lexical scope exit and its issuing process. */
static void value_scope_release(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->references == 0) {
        fputs("llg runtime fatal: value scope reference underflow\n", stderr);
        abort();
    }
    if (--scope->references != 0) return;
    if (scope->all_prev) scope->all_prev->all_next = scope->all_next;
    else all_value_scopes = scope->all_next;
    if (scope->all_next) scope->all_next->all_prev = scope->all_prev;
    for (size_t i = 0; i < scope->count; ++i)
        llg_clocking_forget_signal(&scope->values[i]);
    value_scope_index_remove(scope);
    sv4_destroy_array(scope->values, scope->count);
    if (!scope->intrusive) free(scope->values);
    if (scope->destroy_object) scope->destroy_object(scope->object);
    if (!scope->intrusive) {
        free(scope->object);
        free(scope);
    } else {
        *scope = (llg_value_scope_t){0};
    }
}

static llg_value_scope_t* value_scope_retain_target(const void* target) {
    if (!target) return NULL;
    llg_value_scope_t* scope = value_scope_index_find(target);
    if (scope) {
        if (scope->references == SIZE_MAX) {
            fputs("llg runtime fatal: value scope reference overflow\n", stderr);
            abort();
        }
        ++scope->references;
    }
    return scope; /* Global storage has no entry; model teardown owns it. */
}

static llg_value_scope_t* value_scope_register(llg_value_scope_t* scope) {
    value_scope_index_add(scope);
    scope->references = 1;
    scope->active = 1;
    scope->owner = llg_current();
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    scope->next = *head;
    *head = scope;
    scope->all_next = all_value_scopes;
    if (all_value_scopes) all_value_scopes->all_prev = scope;
    all_value_scopes = scope;
    return scope;
}

llg_value_scope_t* llg_value_scope_begin(size_t count) {
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope), "value owner scope");
    scope->values = count ? (sv4_t*)llg_checked_calloc(
        count, sizeof(sv4_t), "scoped values") : NULL;
    scope->count = count;
    return value_scope_register(scope);
}

llg_value_scope_t* llg_value_scope_register(llg_value_scope_t* node,
                                          sv4_t* values, size_t count) {
    *node = (llg_value_scope_t){.intrusive = 1, .count = count, .values = values};
    return value_scope_register(node);
}

llg_value_scope_t* llg_value_scope_register_object(llg_value_scope_t* node,
    void* object, void (*destroy)(void*)) {
    *node = (llg_value_scope_t){.intrusive = 1, .object = object,
                                .destroy_object = destroy};
    return value_scope_register(node);
}

llg_value_scope_t* llg_value_scope_begin_object(size_t size, void (*destroy)(void*)) {
    llg_value_scope_t* scope = llg_value_scope_begin(0);
    scope->object = size ? llg_checked_calloc(1, size, "scoped native object") : NULL;
    scope->destroy_object = destroy;
    value_scope_index_add(scope);
    return scope;
}

void* llg_value_scope_object(llg_value_scope_t* scope) {
    return scope ? scope->object : NULL;
}

sv4_t* llg_value_scope_values(llg_value_scope_t* scope) {
    return scope ? scope->values : NULL;
}

void llg_value_scope_end(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->intrusive && scope->references != 1) {
        fputs("llg runtime fatal: frame value cell escaped its scope\n", stderr);
        abort();
    }
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    while (*head && *head != scope) head = &(*head)->next;
    if (!scope->active || !*head) {
        fputs("llg runtime fatal: value scope is not registered\n", stderr);
        abort();
    }
    *head = scope->next;
    scope->next = NULL;
    scope->owner = NULL;
    scope->active = 0;
    value_scope_release(scope);
}

llg_value_scope_t* llg_value_scope_mark(void) {
    llg_proc_t* owner = llg_current();
    return owner ? owner->value_scopes : root_value_scopes;
}

void llg_value_scopes_end_since(llg_value_scope_t* mark) {
    llg_proc_t* owner = llg_current();
    llg_value_scope_t** head = owner ? &owner->value_scopes : &root_value_scopes;
    /* Validate first so a stale/foreign mark cannot partially unwind a caller. */
    if (mark) {
        llg_value_scope_t* cursor = *head;
        while (cursor && cursor != mark) cursor = cursor->next;
        if (!cursor) {
            fputs("llg runtime fatal: value scope mark is not registered\n", stderr);
            abort();
        }
    }
    while (*head != mark) llg_value_scope_end(*head);
}

static void value_scopes_unwind(llg_proc_t* proc) {
    llg_value_scope_t** head = proc ? &proc->value_scopes : &root_value_scopes;
    while (*head) llg_value_scope_end(*head);
}

/* Pin another process's descriptor while publishing to reentrant callbacks.
 * The pin itself is registered on the writer, so nonlocal exit also releases it. */
static void value_target_pin_destroy(void* payload) {
    value_scope_release(*(llg_value_scope_t**)payload);
}

static llg_value_scope_t* value_target_pin(const void* target) {
    llg_value_scope_t* target_scope = value_scope_retain_target(target);
    if (!target_scope) return NULL;
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_value_scope_t*), value_target_pin_destroy);
    *(llg_value_scope_t**)llg_value_scope_object(owner) = target_scope;
    return owner;
}

static void enqueue_region(llg_proc_t* p, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for process\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    if (p->queued) return;
    p->region = region;
    p->next_region = NULL;
    p->queued = 1;
    llg_proc_queue_t* queue = &g.process_queues[region];
    if (queue->tail) {
        queue->tail->next_region = p;
        queue->tail = p;
    } else {
        queue->head = queue->tail = p;
    }
}

// Source rows are stable across rehash. Both hash-chain and subscriber-list
// links have back-links, so unlink never searches either list. Empty rows are
// reclaimed immediately; address churn cannot accumulate historical sources.
#define LLG_WAIT_SOURCES_INITIAL 16u

static size_t wait_source_hash(const void* key) {
    uint64_t hash = (uint64_t)(uintptr_t)key;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void wait_sources_reserve(void) {
    size_t capacity = g.wait_sources_capacity;
    if (capacity && g.n_wait_sources < capacity - capacity / 4) return;
    if (capacity > SIZE_MAX / 2) {
        fputs("llg runtime fatal: wait source index capacity overflow\n", stderr);
        abort();
    }
    capacity = capacity ? capacity * 2 : LLG_WAIT_SOURCES_INITIAL;
    llg_wait_source_t** table = (llg_wait_source_t**)llg_checked_calloc(
        capacity, sizeof(*table), "wait source index");
    for (size_t i = 0; i < g.wait_sources_capacity; i++) {
        llg_wait_source_t* source = g.wait_sources[i];
        while (source) {
            llg_wait_source_t* next = source->next;
            size_t slot = wait_source_hash(source->key) & (capacity - 1);
            source->next = table[slot];
            source->prev_link = &table[slot];
            if (source->next) source->next->prev_link = &source->next;
            table[slot] = source;
            source = next;
        }
    }
    free(g.wait_sources);
    g.wait_sources = table;
    g.wait_sources_capacity = capacity;
}

static llg_wait_source_t* wait_source_find(const void* key) {
    if (!key || !g.n_wait_sources) return NULL;
    size_t slot = wait_source_hash(key) & (g.wait_sources_capacity - 1);
    for (llg_wait_source_t* source = g.wait_sources[slot]; source;
         source = source->next)
        if (source->key == key) return source;
    return NULL;
}

static llg_wait_source_t* wait_source_get(const void* key) {
    llg_wait_source_t* source = wait_source_find(key);
    if (source) return source;
    wait_sources_reserve();
    size_t slot = wait_source_hash(key) & (g.wait_sources_capacity - 1);
    source = (llg_wait_source_t*)llg_checked_calloc(
        1, sizeof(*source), "wait source");
    source->key = key;
    source->next = g.wait_sources[slot];
    source->prev_link = &g.wait_sources[slot];
    if (source->next) source->next->prev_link = &source->next;
    g.wait_sources[slot] = source;
    g.n_wait_sources++;
    return source;
}

static void wait_subscription_insert(llg_wait_subscription_t* node,
                                      llg_wait_subscription_t** slot) {
    node->next = *slot;
    node->prev_link = slot;
    if (node->next) node->next->prev_link = &node->next;
    *slot = node;
}

static void wait_subscription_unlink(llg_wait_subscription_t* node) {
    *node->prev_link = node->next;
    if (node->next) node->next->prev_link = node->prev_link;
    node->prev_link = NULL;
    llg_wait_source_t* source = node->source;
    if (!source->head) {
        *source->prev_link = source->next;
        if (source->next) source->next->prev_link = source->prev_link;
        g.n_wait_sources--;
        free(source);
    }
}

// A stack-owned cursor is a subscriber with no waiter. Moving it past the
// current subscriber before callbacks makes arbitrary removal (including the
// next subscriber) safe, without snapshots or a scan of active iterators.
// Nested publications skip other cursors; rows stay alive until cursors leave.
static llg_wait_t* wait_source_next(llg_wait_subscription_t* cursor) {
    llg_wait_subscription_t* node = cursor->next;
    while (node && !node->wait) node = node->next;
    if (!node) return NULL;
    *cursor->prev_link = cursor->next;
    if (cursor->next) cursor->next->prev_link = cursor->prev_link;
    wait_subscription_insert(cursor, &node->next);
    return node->wait;
}

typedef struct {
    const void** keys;
    size_t count;
    size_t capacity;
} llg_wait_keys_t;

static void wait_key_add(llg_wait_keys_t* keys, const void* key) {
    if (!key) return;
    if (keys->count == keys->capacity) {
        if (keys->capacity > SIZE_MAX / 2) {
            fputs("llg runtime fatal: wait dependency count overflow\n", stderr);
            abort();
        }
        size_t capacity = keys->capacity ? keys->capacity * 2 : 8;
        const void** grown = (const void**)llg_checked_malloc(
            capacity, sizeof(*grown), "wait dependency keys");
        if (keys->count) memcpy(grown, keys->keys, keys->count * sizeof(*grown));
        free(keys->keys);
        keys->keys = grown;
        keys->capacity = capacity;
    }
    keys->keys[keys->count++] = key;
}

static void wait_dependency_key_add(llg_wait_keys_t* keys,
                                    const llg_wait_dependency_t* dependency) {
    wait_key_add(keys, dependency->sig ? (const void*)dependency->sig
                                       : (const void*)dependency->real);
}

static int wait_key_compare(const void* left, const void* right) {
    uintptr_t a = (uintptr_t)*(const void* const*)left;
    uintptr_t b = (uintptr_t)*(const void* const*)right;
    return (a > b) - (a < b);
}

static int wait_inline_fits(const sv4_t* value) {
    return llg_sv4_width(*value) <= 64u * LLG_WAIT_INLINE_LIMBS;
}

static void wait_inline_copy(llg_wait_t* wait, const sv4_t* value) {
    llg_wait_inline_payload_t* single = &wait->payload.single;
    single->width = llg_sv4_width(*value);
    single->is_signed = (int8_t)(llg_sv4_signed(*value) != 0);
    llg_sv4_export_vpi_words(*value, 0, single->words, LLG_WAIT_INLINE_LIMBS);
}

static int wait_inline_same(llg_wait_t* wait, sv4_t value) {
    return llg_sv4_same_vpi_words(value, wait->payload.single.words,
                                 LLG_WAIT_INLINE_LIMBS);
}

static void wait_subscriptions_register(llg_wait_t* wait) {
    if (wait->kind == W_EVENTS_INLINE || wait->kind == W_LEVEL_INLINE) {
        const void* key = wait->payload.single.specs[0].sig;
        if (!key) return;
        llg_wait_subscription_t* node = &wait->payload.single.subscription;
        wait->subscriptions = node;
        wait->n_subscriptions = 1;
        node->source = wait_source_get(key);
        node->wait = wait;
        wait_subscription_insert(node, &node->source->head);
        return;
    }
    llg_wait_keys_t keys = {0};
    if (wait->kind == W_EVENTS) {
        for (int i = 0; i < wait->payload.expression.n; i++)
            wait_key_add(&keys, wait->payload.expression.specs[i].sig);
    } else if (wait->kind == W_DEPS) {
        for (int i = 0; i < wait->payload.expression.n; i++)
            wait_dependency_key_add(&keys, &wait->payload.expression.dependencies[i]);
    } else if (wait->kind == W_MIXED) {
        for (int i = 0; i < wait->payload.rare->mixed.n; i++)
            wait_key_add(&keys, wait->payload.rare->mixed.specs[i].sig);
    } else if (wait->kind == W_PROCESS) {
        wait_key_add(&keys, wait->payload.rare->process.target);
    } else if (wait->kind == W_LEVEL) {
        wait_key_add(&keys, wait->payload.rare->level.sig);
    } else if (wait->kind == W_EXPR) {
        for (int i = 0; i < wait->payload.expression.n; i++) {
            const llg_expr_event_spec_t* spec = &wait->payload.expression.expressions[i];
            if (spec->event) continue;
            wait_key_add(&keys, spec->sig);
            wait_key_add(&keys, spec->real_sig);
            if (spec->n_dependencies > 0) {
                for (int j = 0; j < spec->n_dependencies; j++)
                    wait_dependency_key_add(&keys, &spec->dependencies[j]);
            } else {
                for (int j = 0; j < spec->n_reads; j++)
                    wait_key_add(&keys, spec->reads[j]);
            }
        }
    }
    if (!keys.count) return;
    qsort(keys.keys, keys.count, sizeof(*keys.keys), wait_key_compare);
    size_t unique = 0;
    for (size_t i = 0; i < keys.count; i++)
        if (!unique || keys.keys[i] != keys.keys[unique - 1])
            keys.keys[unique++] = keys.keys[i];
    wait->subscriptions = (llg_wait_subscription_t*)llg_checked_calloc(
        unique, sizeof(*wait->subscriptions), "wait subscriptions");
    wait->n_subscriptions = unique;
    for (size_t i = 0; i < unique; i++) {
        llg_wait_subscription_t* node = &wait->subscriptions[i];
        node->source = wait_source_get(keys.keys[i]);
        node->wait = wait;
        // Head insertion matches the global list's reverse registration order.
        wait_subscription_insert(node, &node->source->head);
    }
    free(keys.keys);
}

static void wait_inline_promote(llg_wait_t* wait, const sv4_t* value) {
    llg_event_spec_t spec = wait->payload.single.specs[0];
    llg_wait_subscription_t* old = &wait->payload.single.subscription;
    llg_wait_subscription_t* node = (llg_wait_subscription_t*)llg_checked_malloc(
        1, sizeof(*node), "wait subscriptions");
    *node = *old;
    *node->prev_link = node;
    if (node->next) node->next->prev_link = &node->next;
    wait->subscriptions = node;
    memset(&wait->payload, 0, sizeof(wait->payload));
    wait->kind = W_EVENTS;
    llg_wait_expression_payload_t* payload = &wait->payload.expression;
    payload->n = 1;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        1, sizeof(*payload->specs), "edge wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        1, sizeof(*payload->last), "edge wait snapshots");
    payload->specs[0] = spec;
    payload->last[0] = sv4_clone(value);
}

static void remove_waiters_entry(llg_wait_t* w) {
    if (w->prev_link) {
        *w->prev_link = w->next;
        if (w->next) w->next->prev_link = w->prev_link;
        w->prev_link = NULL;
    }
    for (size_t i = 0; i < w->n_subscriptions; i++)
        wait_subscription_unlink(&w->subscriptions[i]);
    if (w->subscriptions != &w->payload.single.subscription)
        free(w->subscriptions);
    w->subscriptions = NULL;
    w->n_subscriptions = 0;
}

static void remove_timed_entry(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp) {
        if (*pp == w) {
            *pp = w->payload.timer.next;
            return;
        }
        pp = &(*pp)->payload.timer.next;
    }
}

static llg_wait_rare_t* wait_rare_allocate(llg_wait_t* wait,
                                           const char* description) {
    llg_wait_rare_t* payload = (llg_wait_rare_t*)llg_checked_calloc(
        1, sizeof(*payload), description);
    wait->payload.rare = payload;
    return payload;
}

static void insert_zero_wait(llg_wait_t* w, llg_region_t region) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for zero-delay wait\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    w->resume_region = region;
    w->payload.timer.region_next = NULL;
    llg_wait_queue_t* queue = &g.zero_waits[region];
    if (queue->tail) {
        queue->tail->payload.timer.region_next = w;
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
                *pp = w->payload.timer.region_next;
                if (queue->tail == w) {
                    queue->tail = NULL;
                    for (llg_wait_t* q = queue->head; q; q = q->payload.timer.region_next)
                        queue->tail = q;
                }
                w->payload.timer.region_next = NULL;
                return;
            }
            pp = &(*pp)->payload.timer.region_next;
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
    p->queued = 0;
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
                p->queued = 0;
                return;
            }
            pp = &(*pp)->next_region;
        }
    }
}

static void remove_inactive_entry(llg_wait_t* w) {
    remove_zero_wait_entry(w);
}

static int semaphore_key_count(sv4_t value, uint64_t* result) {
    int64_t signed_value = 0;
    if (!result || !sv4_to_index_i64(value, &signed_value) || signed_value < 0) {
        fprintf(stderr,
                "llg: semaphore key count must be a known nonnegative integral value\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    *result = (uint64_t)signed_value;
    return 1;
}

static int semaphore_valid(llg_semaphore_t* semaphore, const char* action) {
    if (semaphore) return 1;
    fprintf(stderr, "llg: semaphore %s requires a live semaphore handle\n", action);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

// Remove a blocked get without changing the semaphore's available keys.  This
// is used by process cancellation and teardown before the process storage is
// reclaimed, so a killed waiter can never consume a later put.
static void semaphore_waiter_unlink(llg_wait_t* wait) {
    if (!wait || wait->kind != W_SEMAPHORE || !wait->payload.rare ||
        !wait->payload.rare->semaphore.waiter)
        return;
    llg_wait_semaphore_payload_t* payload = &wait->payload.rare->semaphore;
    llg_semaphore_wait_t* node = payload->waiter;
    llg_semaphore_t* semaphore = payload->semaphore ? payload->semaphore : node->owner;
    if (semaphore) {
        llg_semaphore_wait_t** slot = &semaphore->wait_head;
        while (*slot && *slot != node) slot = &(*slot)->next;
        if (*slot == node) {
            *slot = node->next;
            semaphore->cancelled_waiter = 1;
            if (semaphore->wait_tail == node) {
                semaphore->wait_tail = NULL;
                for (llg_semaphore_wait_t* item = semaphore->wait_head; item;
                     item = item->next)
                    semaphore->wait_tail = item;
            }
        }
    }
    free(node);
    payload->semaphore = NULL;
    payload->waiter = NULL;
    payload->keys = 0;
}

// Service only the head request.  A later smaller request cannot bypass a
// larger request at the front of the specified semaphore FIFO.
static void semaphore_wake_available(llg_semaphore_t* semaphore) {
    if (!semaphore) return;
    while (semaphore->wait_head) {
        llg_semaphore_wait_t* node = semaphore->wait_head;
        if (node->keys > semaphore->available) break;
        semaphore->wait_head = node->next;
        if (!semaphore->wait_head) semaphore->wait_tail = NULL;
        llg_proc_t* proc = node->proc;
        llg_wait_t* wait = proc ? &proc->wait : NULL;
        if (!proc || !wait || wait->kind != W_SEMAPHORE ||
            !wait->payload.rare || wait->payload.rare->semaphore.waiter != node) {
            free(node);
            continue;
        }
        semaphore->available -= node->keys;
        free(node);
        wait->payload.rare->semaphore.semaphore = NULL;
        wait->payload.rare->semaphore.waiter = NULL;
        wait->payload.rare->semaphore.keys = 0;
        wake_proc(proc);
    }
}

// Finish the entire cancellation batch before granting keys. Servicing from
// unlink would let a sibling that is about to be killed consume a grant.
// Teardown only unlinks requests; it must never schedule new work.
static void semaphore_service_cancelled_waiters(void) {
    if (g.finish) return;
    for (llg_semaphore_t* semaphore = g.semaphores; semaphore;
         semaphore = semaphore->next_all) {
        if (!semaphore->cancelled_waiter) continue;
        semaphore->cancelled_waiter = 0;
        semaphore_wake_available(semaphore);
    }
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on; defined below with the other named-event helpers.
static void event_unlink(llg_wait_t* w);

static void insert_timed(llg_wait_t* w) {
    llg_wait_t** pp = &g.timed_head;
    while (*pp && (*pp)->payload.timer.time <= w->payload.timer.time)
        pp = &(*pp)->payload.timer.next;
    w->payload.timer.next = *pp;
    *pp = w;
}

static void free_expression_wait(llg_wait_t* w) {
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    if (!payload->expressions) return;
    for (int i = 0; i < payload->n; i++) {
        llg_frame_release((llg_frame_t*)payload->expressions[i].eval_context);
        llg_frame_release((llg_frame_t*)payload->expressions[i].condition_context);
        free(payload->expressions[i].reads);
        free(payload->expressions[i].dependencies);
    }
    free(payload->expressions);
    payload->expressions = NULL;
}

static void wait_payload_release(llg_wait_t* wait) {
    if (!wait) return;
    llg_process_handle_t* process_target = NULL;
    switch (wait->kind) {
        case W_EVENTS_INLINE:
        case W_LEVEL_INLINE:
            break;
        case W_EVENTS:
            free(wait->payload.expression.specs);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            break;
        case W_DEPS:
            free(wait->payload.expression.dependencies);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            break;
        case W_EXPR:
            free_expression_wait(wait);
            sv4_destroy_array(wait->payload.expression.last,
                              wait->payload.expression.last
                                  ? (size_t)wait->payload.expression.n
                                  : 0);
            free(wait->payload.expression.last);
            free(wait->payload.expression.real_last);
            free(wait->payload.expression.evs);
            break;
        case W_EVENT:
            if (wait->payload.event.evs != &wait->payload.event.inline_ev)
                free(wait->payload.event.evs);
            break;
        case W_MIXED:
            if (wait->payload.rare) {
                llg_wait_mixed_payload_t* mixed = &wait->payload.rare->mixed;
                free(mixed->specs);
                sv4_destroy_array(mixed->last,
                                  mixed->last ? (size_t)mixed->n : 0);
                free(mixed->last);
                free(mixed->evs);
            }
            break;
        case W_EVENT_ORDER:
            if (wait->payload.rare) {
                free(wait->payload.rare->order.evs);
                free(wait->payload.rare->order.sequence);
            }
            break;
        case W_LEVEL:
            if (wait->payload.rare)
                sv4_destroy(&wait->payload.rare->level.value);
            break;
        case W_PROCESS:
            if (wait->payload.rare)
                process_target = wait->payload.rare->process.target;
            break;
        case W_MAILBOX_PUT:
            if (wait->payload.rare)
                mailbox_value_destroy(&wait->payload.rare->mailbox_put.value);
            break;
        default:
            break;
    }
    if (wait->kind != W_TIME && wait->kind != W_EVENTS &&
        wait->kind != W_DEPS && wait->kind != W_EXPR &&
        wait->kind != W_EVENT && wait->kind != W_EVENT_TRIGGERED &&
        wait->kind != W_EVENTS_INLINE && wait->kind != W_LEVEL_INLINE &&
        wait->kind != W_NONE)
        free(wait->payload.rare);
    memset(&wait->payload, 0, sizeof(wait->payload));
    wait->kind = W_NONE;
    if (process_target) llg_process_release(process_target);
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
    sv4_destroy_array(trigger->last, trigger->last ? (size_t)trigger->n : 0);
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

static void free_deferred_assertion_report(
    llg_deferred_assertion_report_t* report) {
    if (!report) return;
    llg_frame_release(report->frame);
    free(report);
}

static void free_assertion_rules(void) {
    while (llg_assertion_rules) {
        llg_assertion_rule_t* next = llg_assertion_rules->next;
        free(llg_assertion_rules->scope);
        free(llg_assertion_rules);
        llg_assertion_rules = next;
    }
}

static void free_deferred_assertions(void) {
    while (g.deferred_assertions) {
        llg_deferred_assertion_report_t* next = g.deferred_assertions->next;
        free_deferred_assertion_report(g.deferred_assertions);
        g.deferred_assertions = next;
    }
    g.deferred_assertion_tail = NULL;
}

static void deferred_assertion_callback(void* data) {
    llg_deferred_assertion_report_t* report =
        (llg_deferred_assertion_report_t*)data;
    if (!report) return;
    llg_frame_t* frame = report->frame;
    report->frame = NULL;
    if (report->passed) {
        if (report->kind == LLG_ASSERTION_COVER)
            llg_assertion_cover(report->identity, report->label, report->location);
        if (report->action) {
            int saved = g.in_deferred_action;
            g.in_deferred_action = 1;
            report->action(frame);
            g.in_deferred_action = saved;
        }
    } else if (report->kind != LLG_ASSERTION_COVER) {
        if (report->action) {
            int saved = g.in_deferred_action;
            g.in_deferred_action = 1;
            report->action(frame);
            g.in_deferred_action = saved;
        } else {
            llg_assertion_failure(report->kind, report->identity,
                                  report->label, report->location);
        }
    }
    llg_frame_release(frame);
    free(report);
}

// Transfer queued reports to the Reactive region while the scheduler is at
// the Observed-to-Reactive handoff. Region callback ordering preserves source
// issue order after same-assertion coalescing.
static void flush_deferred_assertions(void) {
    while (g.deferred_assertions) {
        llg_deferred_assertion_report_t* report = g.deferred_assertions;
        g.deferred_assertions = report->next;
        report->next = NULL;
        if (!llg_schedule_region_callback(LLG_REGION_REACTIVE,
                                          deferred_assertion_callback, report)) {
            free_deferred_assertion_report(report);
            break;
        }
    }
    if (!g.deferred_assertions) g.deferred_assertion_tail = NULL;
}

// A finish request from a later read-only callback can stop the normal
// scheduler before the Reactive queue gets a turn. Deferred assertion
// callbacks are still mature reports and must run before teardown; unrelated
// callbacks remain subject to the ordinary finish discard rule.
static llg_region_callback_t* take_deferred_assertion_callback_now(void) {
    llg_region_callback_t** slot = &g.callbacks;
    while (*slot && (*slot)->time <= g.now) {
        llg_region_callback_t* entry = *slot;
        if (entry->time == g.now && entry->callback == deferred_assertion_callback) {
            *slot = entry->next;
            entry->next = NULL;
            return entry;
        }
        slot = &entry->next;
    }
    return NULL;
}

static int deferred_assertion_callback_pending_now(void) {
    for (llg_region_callback_t* entry = g.callbacks;
         entry && entry->time <= g.now; entry = entry->next) {
        if (entry->time == g.now && entry->callback == deferred_assertion_callback)
            return 1;
    }
    return 0;
}

// Finish/deadlock teardown can occur before the normal Observed handoff (for
// example, a process executes `$finish` immediately after `assert #0`). Run
// those reports in the Reactive context before releasing the scheduler.
static void run_deferred_assertions_now(void) {
    if (!g.deferred_assertions && !deferred_assertion_callback_pending_now()) return;
    llg_region_t saved_region = g.current_region;
    int saved_action = g.in_deferred_action;
    g.current_region = LLG_REGION_REACTIVE;
    for (;;) {
        while (g.deferred_assertions) {
            llg_deferred_assertion_report_t* report = g.deferred_assertions;
            g.deferred_assertions = report->next;
            report->next = NULL;
            deferred_assertion_callback(report);
        }
        g.deferred_assertion_tail = NULL;
        llg_region_callback_t* callback = take_deferred_assertion_callback_now();
        if (!callback) break;
        llg_region_callback_fn fn = callback->callback;
        void* data = callback->data;
        free(callback);
        fn(data);
    }
    g.in_deferred_action = saved_action;
    g.current_region = saved_region;
}

static int expression_qualifies(const llg_expr_event_spec_t* spec) {
    if (!spec->condition) return 1;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* result = llg_value_scope_values(scope);
    spec->condition(result, spec->condition_context);
    if (llg_rt_exiting()) {
        llg_value_scope_end(scope);
        return 0;
    }
    int qualifies = sv4_to_bool(*result);
    llg_value_scope_end(scope);
    return qualifies;
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
    if (w->kind == W_SEMAPHORE) semaphore_waiter_unlink(w);
    if (w->kind == W_MAILBOX_GET || w->kind == W_MAILBOX_PUT)
        mailbox_unlink_wait(w);
    wait_payload_release(w);
    g.wait_count--;
    if (p->suspended) {
        // A suspended waiter keeps its condition registered until it fires;
        // once it fires, retain only a pending wake so resume cannot enqueue
        // the same continuation twice.
        p->wake_pending = 1;
        process_status_set(p, LLG_PROCESS_SUSPENDED);
    } else {
        process_status_set(p, LLG_PROCESS_RUNNING);
        enqueue_region(p, w->resume_region);
    }
}

static void wake_assertion_waiter(uint64_t identity) {
    llg_wait_t* wait = g.waiters;
    while (wait) {
        llg_wait_t* next = wait->next;
        if (wait->kind == W_ASSERTION && wait->payload.rare &&
            wait->payload.rare->assertion.identity == identity)
            wake_proc(wait->proc);
        wait = next;
    }
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
    w->prev_link = &g.waiters;
    if (w->next) w->next->prev_link = &w->next;
    g.waiters = w;
    wait_subscriptions_register(w);
    g.wait_count++;
    if (!p->suspended) process_status_set(p, LLG_PROCESS_WAITING);
}

// ── Mailboxes ────────────────────────────────────────────────────────────────

static void mailbox_value_destroy(llg_mailbox_value_t* value) {
    if (!value) return;
    if (value->kind == LLG_MAILBOX_PACKED) sv4_destroy(&value->value.packed);
    if (value->kind == LLG_MAILBOX_STRING)
        llg_string_destroy(&value->value.string);
    memset(value, 0, sizeof(*value));
}

llg_mailbox_value_t llg_mailbox_typed_value(llg_mailbox_value_t value, uint64_t type_id) {
    value.type_id = type_id;
    return value;
}

llg_mailbox_target_t llg_mailbox_typed_target(llg_mailbox_target_t target, uint64_t type_id) {
    target.type_id = type_id;
    return target;
}

llg_mailbox_target_t llg_mailbox_target_ref(llg_ref_t* ref) {
    llg_mailbox_target_t target = {0};
    target.kind = LLG_MAILBOX_PACKED;
    target.reference = ref;
    if (ref) {
        target.width = ref->width;
        target.is_signed = ref->is_signed;
        target.two_state = ref->two_state;
    }
    return target;
}

llg_mailbox_value_t llg_mailbox_value_packed(sv4_t value, uint32_t width,
                                              int is_signed, int two_state) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_PACKED;
    result.width = width;
    result.is_signed = (int8_t)is_signed;
    result.two_state = (int8_t)two_state;
    result.value.packed = sv4_clone(&value);
    return result;
}

llg_mailbox_value_t llg_mailbox_value_real(double value, int shortreal) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_REAL;
    result.shortreal = (int8_t)shortreal;
    result.value.real = shortreal ? (double)(float)value : value;
    return result;
}

llg_mailbox_value_t llg_mailbox_value_string(llg_string_t value) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_STRING;
    result.value.string = value;
    return result;
}

llg_mailbox_value_t llg_mailbox_value_handle(void* value) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_HANDLE;
    result.value.handle = value;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_packed(sv4_t* target, uint32_t width,
                                                int is_signed, int two_state) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_PACKED;
    result.width = width;
    result.is_signed = (int8_t)is_signed;
    result.two_state = (int8_t)two_state;
    result.target.packed = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_real(double* target, int shortreal) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_REAL;
    result.shortreal = (int8_t)shortreal;
    result.target.real = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_string(llg_string_t* target) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_STRING;
    result.target.string = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_handle(void** target) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_HANDLE;
    result.target.handle = target;
    return result;
}

static int mailbox_message_kind_matches(const llg_mailbox_t* mailbox,
                                        const llg_mailbox_value_t* value) {
    if (!mailbox || !value) return 0;
    if (mailbox->kind == LLG_MAILBOX_UNTYPED) return 1;
    if (mailbox->kind != value->kind) return 0;
    switch (mailbox->kind) {
    case LLG_MAILBOX_PACKED:
        return mailbox->width == value->width &&
               mailbox->is_signed == value->is_signed &&
               mailbox->two_state == value->two_state;
    case LLG_MAILBOX_REAL:
        return mailbox->shortreal == value->shortreal;
    case LLG_MAILBOX_STRING:
    case LLG_MAILBOX_HANDLE:
        return 1;
    default:
        return 0;
    }
}

static int mailbox_target_matches(const llg_mailbox_value_t* value,
                                  const llg_mailbox_target_t* target) {
    if (!value || !target || value->kind != target->kind ||
        value->type_id != target->type_id) return 0;
    switch (value->kind) {
    case LLG_MAILBOX_PACKED:
        return value->width == target->width &&
               value->is_signed == target->is_signed &&
               value->two_state == target->two_state;
    case LLG_MAILBOX_REAL:
        return value->shortreal == target->shortreal;
    case LLG_MAILBOX_STRING:
        return 1;
    case LLG_MAILBOX_HANDLE:
        // The declared nominal type was checked above, even for null values.
        return 1;
    default:
        return 0;
    }
}

static void mailbox_deliver(const llg_mailbox_value_t* value,
                            const llg_mailbox_target_t* target) {
    if (!mailbox_target_matches(value, target)) return;
    if (!target->target.packed && !target->reference && target->kind == LLG_MAILBOX_PACKED) return;
    if (!target->target.real && target->kind == LLG_MAILBOX_REAL) return;
    if (!target->target.string && target->kind == LLG_MAILBOX_STRING) return;
    if (!target->target.handle && target->kind == LLG_MAILBOX_HANDLE) return;
    switch (target->kind) {
    case LLG_MAILBOX_PACKED: {
        llg_value_scope_t* scope = llg_value_scope_begin(1);
        sv4_t* converted = llg_value_scope_values(scope);
        sv4_replace(converted, sv4_cast(value->value.packed, target->width, target->is_signed));
        if (target->two_state) sv4_replace(converted, sv4_to_two_state(*converted));
        if (target->reference) llg_ref_write(target->reference, *converted);
        else llg_ba(target->target.packed, *converted);
        llg_value_scope_end(scope);
        break;
    }
    case LLG_MAILBOX_REAL:
        llg_ba_d(target->target.real,
                 target->shortreal ? (double)(float)value->value.real
                                   : value->value.real);
        break;
    case LLG_MAILBOX_STRING:
        llg_string_move(target->target.string,
                        llg_string_clone(&value->value.string));
        break;
    case LLG_MAILBOX_HANDLE:
        *target->target.handle = value->value.handle;
        break;
    default:
        break;
    }
}

static void mailbox_message_append(llg_mailbox_t* mailbox,
                                   llg_mailbox_value_t value) {
    llg_mailbox_message_t* message = (llg_mailbox_message_t*)llg_checked_calloc(
        1, sizeof(*message), "mailbox message");
    message->value = value;
    if (mailbox->tail)
        mailbox->tail->next = message;
    else
        mailbox->head = message;
    mailbox->tail = message;
    mailbox->length++;
}

static llg_mailbox_message_t* mailbox_message_pop(llg_mailbox_t* mailbox) {
    llg_mailbox_message_t* message = mailbox->head;
    if (!message) return NULL;
    mailbox->head = message->next;
    if (!mailbox->head) mailbox->tail = NULL;
    message->next = NULL;
    mailbox->length--;
    return message;
}

static void mailbox_snapshot_destroy(void* payload) {
    mailbox_value_destroy((llg_mailbox_value_t*)payload);
}

/* Freeze a successful delivery before publishing to HDL. A peek needs an
 * independent copy because a reentrant get can destroy the queue's head. */
static llg_value_scope_t* mailbox_snapshot(llg_mailbox_t* mailbox, int peek) {
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_mailbox_value_t), mailbox_snapshot_destroy);
    llg_mailbox_value_t* value = llg_value_scope_object(owner);
    llg_mailbox_message_t* message = peek ? mailbox->head : mailbox_message_pop(mailbox);
    *value = message->value;
    if (peek) {
        if (value->kind == LLG_MAILBOX_PACKED)
            value->value.packed = sv4_clone(&message->value.value.packed);
        else if (value->kind == LLG_MAILBOX_STRING)
            value->value.string = llg_string_clone(&message->value.value.string);
    } else {
        memset(&message->value, 0, sizeof(message->value));
        free(message);
    }
    return owner;
}

static void mailbox_unlink_wait(llg_wait_t* wait) {
    if (!wait || !wait->payload.rare) return;
    llg_mailbox_t* mailbox = wait->kind == W_MAILBOX_PUT
                                 ? wait->payload.rare->mailbox_put.mailbox
                                 : wait->payload.rare->mailbox_get.mailbox;
    if (!mailbox) return;
    llg_wait_t** head = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_head
                            : &mailbox->get_head;
    llg_wait_t** tail = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_tail
                            : &mailbox->get_tail;
    llg_wait_t** cursor = head;
    while (*cursor) {
        if (*cursor == wait) {
            *cursor = wait->kind == W_MAILBOX_PUT
                          ? wait->payload.rare->mailbox_put.next
                          : wait->payload.rare->mailbox_get.next;
            if (*tail == wait) *tail = NULL;
            if (!*head) {
                *tail = NULL;
            } else if (!*tail) {
                llg_wait_t* last = *head;
                if (wait->kind == W_MAILBOX_PUT) {
                    while (last->payload.rare->mailbox_put.next)
                        last = last->payload.rare->mailbox_put.next;
                } else {
                    while (last->payload.rare->mailbox_get.next)
                        last = last->payload.rare->mailbox_get.next;
                }
                *tail = last;
            }
            if (wait->kind == W_MAILBOX_PUT)
                wait->payload.rare->mailbox_put.next = NULL;
            else
                wait->payload.rare->mailbox_get.next = NULL;
            return;
        }
        cursor = wait->kind == W_MAILBOX_PUT
                     ? &(*cursor)->payload.rare->mailbox_put.next
                     : &(*cursor)->payload.rare->mailbox_get.next;
    }
    if (wait->kind == W_MAILBOX_PUT)
        wait->payload.rare->mailbox_put.next = NULL;
    else
        wait->payload.rare->mailbox_get.next = NULL;
}

static void mailbox_append_wait(llg_mailbox_t* mailbox, llg_wait_t* wait,
                                 int put) {
    llg_wait_t** head = put ? &mailbox->put_head : &mailbox->get_head;
    llg_wait_t** tail = put ? &mailbox->put_tail : &mailbox->get_tail;
    llg_wait_t** next = put ? &wait->payload.rare->mailbox_put.next
                            : &wait->payload.rare->mailbox_get.next;
    *next = NULL;
    if (*tail) {
        if (put)
            (*tail)->payload.rare->mailbox_put.next = wait;
        else
            (*tail)->payload.rare->mailbox_get.next = wait;
    } else {
        *head = wait;
    }
    *tail = wait;
}

static void mailbox_type_error(void) {
    fprintf(stderr, "llg: mailbox retrieval type mismatch\n");
    llg_last_failure = 1;
    g.finish = 1;
    llg_proc_t* current = llg_current();
    if (current) current->chain.exiting = LLG_EXIT_COMPLETE;
}

static void mailbox_remove_and_wake(llg_wait_t* wait) {
    if (!wait) return;
    mailbox_unlink_wait(wait);
    if (wait->kind == W_MAILBOX_PUT)
        wait->payload.rare->mailbox_put.mailbox = NULL;
    else
        wait->payload.rare->mailbox_get.mailbox = NULL;
    wake_proc(wait->proc);
}

// Service only FIFO heads. Unlinking/granting cannot execute a resumed
// continuation inline, so list ownership stays with this service loop.
static void mailbox_service_waiters(llg_mailbox_t* mailbox) {
    while (mailbox && !g.finish) {
        if (mailbox->head && mailbox->get_head) {
            llg_wait_t* get = mailbox->get_head;
            llg_wait_mailbox_get_payload_t* payload =
                &get->payload.rare->mailbox_get;
            llg_mailbox_message_t* message = mailbox->head;
            if (!mailbox_target_matches(&message->value, &payload->target)) {
                mailbox_type_error();
                return;
            }
            llg_mailbox_target_t target = payload->target;
            llg_value_scope_t* owner = mailbox_snapshot(mailbox, payload->peek);
            /* wake_proc queues, but never runs, the continuation. Copy the
             * destination before wakeup clears the wait record. */
            mailbox_remove_and_wake(get);
            mailbox_deliver(llg_value_scope_object(owner), &target);
            llg_value_scope_end(owner);
            continue;
        }
        if (mailbox->put_head &&
            (mailbox->bound == 0 || mailbox->length < mailbox->bound)) {
            llg_wait_t* put = mailbox->put_head;
            llg_mailbox_value_t* stored = &put->payload.rare->mailbox_put.value;
            llg_mailbox_value_t value = *stored;
            memset(stored, 0, sizeof(*stored));
            mailbox_message_append(mailbox, value);
            mailbox_remove_and_wake(put);
            continue;
        }
        break;
    }
}

static llg_mailbox_t* mailbox_require(llg_mailbox_t* mailbox,
                                      const char* operation) {
    if (mailbox) return mailbox;
    fprintf(stderr, "llg: mailbox %s on a null handle\n", operation);
    llg_last_failure = 1;
    g.finish = 1;
    return NULL;
}

llg_mailbox_t* llg_mailbox_new(sv4_t bound, int kind, uint32_t width,
                               int is_signed, int two_state, int shortreal) {
    if (sv4_is_unknown(bound)) {
        fprintf(stderr, "llg: mailbox bound contains X/Z\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (llg_sv4_width(bound) > 64) {
        fprintf(stderr, "llg: mailbox bound exceeds 64-bit capacity\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (llg_sv4_signed(bound) && sv4_to_i64(bound) < 0) {
        fprintf(stderr, "llg: mailbox bound must be non-negative\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (kind < LLG_MAILBOX_PACKED || kind > LLG_MAILBOX_UNTYPED ||
        (kind == LLG_MAILBOX_PACKED && width == 0) ||
        (kind != LLG_MAILBOX_PACKED && width != 0) ||
        (kind != LLG_MAILBOX_PACKED && (is_signed || two_state)) ||
        (kind != LLG_MAILBOX_REAL && shortreal)) {
        fprintf(stderr, "llg: invalid mailbox element descriptor\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    llg_mailbox_t* mailbox = (llg_mailbox_t*)llg_checked_calloc(
        1, sizeof(*mailbox), "mailbox");
    mailbox->bound = sv4_to_u64(bound);
    mailbox->kind = kind;
    mailbox->width = width;
    mailbox->is_signed = (int8_t)is_signed;
    mailbox->two_state = (int8_t)two_state;
    mailbox->shortreal = (int8_t)shortreal;
    mailbox->next = g.mailboxes;
    g.mailboxes = mailbox;
    return mailbox;
}

uint64_t llg_mailbox_num(const llg_mailbox_t* mailbox) {
    mailbox = mailbox_require((llg_mailbox_t*)mailbox, "num");
    return mailbox ? mailbox->length : 0;
}

llg_co_arm_t llg_arm_mailbox_put_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_value_t value) {
    llg_runtime_service_enter(self, "mailbox::put");
    mailbox = mailbox_require(mailbox, "put");
    if (!mailbox) {
        mailbox_value_destroy(&value);
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (!mailbox_message_kind_matches(mailbox, &value)) {
        fprintf(stderr, "llg: mailbox put value does not match its type\n");
        llg_last_failure = 1;
        g.finish = 1;
        mailbox_value_destroy(&value);
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (mailbox->bound == 0 || mailbox->length < mailbox->bound) {
        mailbox_message_append(mailbox, value);
        mailbox_service_waiters(mailbox);
        return self && self->chain.exiting ? LLG_CO_ARM_EXIT
                                           : LLG_CO_ARM_READY;
    }
    if (!self || !region_can_mutate("mailbox put wait")) {
        mailbox_value_destroy(&value);
        return LLG_CO_ARM_READY;
    }
    llg_wait_t* wait = &self->wait;
    wait->kind = W_MAILBOX_PUT;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_mailbox_put_payload_t* payload =
        &wait_rare_allocate(wait, "mailbox put wait payload")->mailbox_put;
    payload->mailbox = mailbox;
    payload->value = value;
    register_wait();
    mailbox_append_wait(mailbox, wait, 1);
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

int llg_mailbox_try_put_value(llg_mailbox_t* mailbox,
                              llg_mailbox_value_t value) {
    mailbox = mailbox_require(mailbox, "try_put");
    if (!mailbox) {
        mailbox_value_destroy(&value);
        return 0;
    }
    if (!mailbox_message_kind_matches(mailbox, &value)) {
        mailbox_value_destroy(&value);
        return 0;
    }
    if (mailbox->bound != 0 && mailbox->length >= mailbox->bound) {
        mailbox_value_destroy(&value);
        return 0;
    }
    mailbox_message_append(mailbox, value);
    mailbox_service_waiters(mailbox);
    return 1;
}

// 0 is empty, -1 is a type mismatch, and 1 is a successful transfer.
// A failed conversion never changes either the message or the destination.
static int mailbox_take_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek) {
    if (!mailbox || !mailbox->head) return 0;
    if (!mailbox_target_matches(&mailbox->head->value, &target)) return -1;
    llg_value_scope_t* owner = mailbox_snapshot(mailbox, peek);
    mailbox_deliver(llg_value_scope_object(owner), &target);
    llg_value_scope_end(owner);
    mailbox_service_waiters(mailbox);
    return 1;
}

static llg_co_arm_t llg_mailbox_wait_get(llg_proc_t* self,
                                         llg_mailbox_t* mailbox,
                                         llg_mailbox_target_t target,
                                         int peek) {
    if (!self || !region_can_mutate("mailbox get wait"))
        return LLG_CO_ARM_READY;
    llg_wait_t* wait = &self->wait;
    wait->kind = W_MAILBOX_GET;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_mailbox_get_payload_t* payload =
        &wait_rare_allocate(wait, "mailbox get wait payload")->mailbox_get;
    payload->mailbox = mailbox;
    payload->target = target;
    payload->peek = peek;
    register_wait();
    mailbox_append_wait(mailbox, wait, 0);
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_mailbox_get_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_target_t target, int peek) {
    llg_runtime_service_enter(self, "mailbox::get");
    mailbox = mailbox_require(mailbox, "get");
    if (!mailbox) {
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    int result = mailbox_take_value(mailbox, target, peek);
    if (result < 0) {
        mailbox_type_error();
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (result > 0)
        return self && self->chain.exiting ? LLG_CO_ARM_EXIT
                                           : LLG_CO_ARM_READY;
    return llg_mailbox_wait_get(self, mailbox, target, peek);
}

int llg_mailbox_try_get_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek) {
    mailbox = mailbox_require(mailbox, peek ? "try_peek" : "try_get");
    if (!mailbox) return 0;
    return mailbox_take_value(mailbox, target, peek);
}

// ── Named events ──────────────────────────────────────────────────────────────

// Grow one event waiter table to hold at least `needed` entries. The grown
// copy is completed before it replaces the old table, so an allocation failure
// aborts without leaving a partially rebound event. Only `used` entries are
// copied; the rest of a freshly allocated table is unspecified.
static void event_table_reserve(llg_proc_t*** table, size_t* capacity,
                                int needed, int used, const char* what) {
    if (needed < 0 || (size_t)needed <= *capacity) return;
    size_t next = *capacity ? *capacity : 8u;
    while (next < (size_t)needed) {
        if (next > SIZE_MAX / 2u) {
            fprintf(stderr, "llg runtime fatal: %s capacity overflow\n", what);
            abort();
        }
        next *= 2u;
    }
    llg_proc_t** grown = (llg_proc_t**)llg_checked_malloc(
        next, sizeof(*grown), what);
    if (*table && used > 0) memcpy(grown, *table, (size_t)used * sizeof(*grown));
    free(*table);
    *table = grown;
    *capacity = next;
}

// Release any grown waiter tables and clear the event state. Generated
// init/teardown calls this; safe on a zero-initialized object and idempotent.
void llg_event_object_reset(llg_event_object_t* ev) {
    if (!ev) return;
    free(ev->waiters);
    free(ev->triggered_waiters);
    ev->waiters = NULL;
    ev->n_waiters = 0;
    ev->waiters_capacity = 0;
    ev->triggered_waiters = NULL;
    ev->n_triggered_waiters = 0;
    ev->triggered_waiters_capacity = 0;
    ev->triggered_time = 0;
    ev->triggered_generation = 0;
    ev->trigger_count = 0;
    ev->triggered = 0;
}

// Slot arrays share the event-list allocation; a single ordinary event uses
// the existing inline payload. Positions survive waiter-table reallocations.
static llg_event_object_t** event_wait_list_new(int count, int** slots) {
    if (!count) {
        *slots = NULL;
        return NULL;
    }
    llg_event_object_t** events = (llg_event_object_t**)llg_checked_malloc(
        (size_t)count, sizeof(*events) + sizeof(**slots), "indexed event wait list");
    *slots = (int*)(events + count);
    for (int i = 0; i < count; i++) (*slots)[i] = -1;
    return events;
}

static int* event_wait_slot(llg_wait_t* w, llg_event_object_t* ev, int index) {
    llg_event_object_t** events = NULL;
    int* slots = NULL;
    int count = 0;
    switch (w->kind) {
        case W_EVENT:
            events = w->payload.event.evs;
            slots = w->payload.event.event_slots;
            count = w->payload.event.n_evs;
            break;
        case W_EXPR:
            events = w->payload.expression.evs;
            slots = w->payload.expression.event_slots;
            count = w->payload.expression.n_evs;
            break;
        case W_MIXED:
            events = w->payload.rare->mixed.evs;
            slots = w->payload.rare->mixed.event_slots;
            count = w->payload.rare->mixed.n_evs;
            break;
        case W_EVENT_ORDER:
            events = w->payload.rare->order.evs;
            slots = w->payload.rare->order.event_slots;
            count = w->payload.rare->order.n_evs;
            break;
        default:
            return NULL;
    }
    for (int i = 0; i < count; i++)
        if (events[i] == ev && slots[i] == index) return &slots[i];
    return NULL;
}

// Register `p` on `ev`'s waiter table, growing it with checked allocation.
static void event_list_add(llg_event_object_t* ev, llg_proc_t* p, int* slot) {
    if (!ev) return;
    if (ev->n_waiters == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: named-event waiter count overflow\n");
        abort();
    }
    event_table_reserve(&ev->waiters, &ev->waiters_capacity,
                        ev->n_waiters + 1, ev->n_waiters,
                        "named-event waiters");
    *slot = ev->n_waiters;
    ev->waiters[ev->n_waiters++] = p;
}

// Register a process on the persistent same-time-slot state of `ev`. This
// list is deliberately separate from ordinary event waiters: `@(ev)` remains
// edge-triggered and never observes a trigger that happened before it parked.
static void event_triggered_list_add(llg_event_object_t* ev, llg_proc_t* p) {
    if (!ev) return;
    if (ev->n_triggered_waiters == INT_MAX) {
        fprintf(stderr,
                "llg runtime fatal: named-event triggered waiter count overflow\n");
        abort();
    }
    event_table_reserve(&ev->triggered_waiters, &ev->triggered_waiters_capacity,
                        ev->n_triggered_waiters + 1, ev->n_triggered_waiters,
                        "named-event triggered waiters");
    p->wait.payload.event.inline_slot = ev->n_triggered_waiters;
    ev->triggered_waiters[ev->n_triggered_waiters++] = p;
}

// Remove a W_EVENT/W_MIXED waiter from every named-event list it registered
// on — the process may be woken through any ONE of them (or through the
// signal half of a mixed list), and must not stay registered on the others.
static void event_unlink(llg_wait_t* w) {
    llg_event_object_t** events = NULL;
    int* slots = NULL;
    int count = 0;
    if (w->kind == W_EVENT) {
        events = w->payload.event.evs;
        slots = w->payload.event.event_slots;
        count = w->payload.event.n_evs;
    } else if (w->kind == W_EXPR) {
        events = w->payload.expression.evs;
        slots = w->payload.expression.event_slots;
        count = w->payload.expression.n_evs;
    } else if (w->kind == W_MIXED && w->payload.rare) {
        events = w->payload.rare->mixed.evs;
        slots = w->payload.rare->mixed.event_slots;
        count = w->payload.rare->mixed.n_evs;
    } else if (w->kind == W_EVENT_ORDER && w->payload.rare) {
        events = w->payload.rare->order.evs;
        slots = w->payload.rare->order.event_slots;
        count = w->payload.rare->order.n_evs;
    }
    for (int i = 0; i < count; i++) {
        llg_event_object_t* ev = events[i];
        if (!ev) continue;
        int index = slots[i];
        if (index < 0) continue; // already detached into a trigger snapshot
        int last = --ev->n_waiters;
        if (index != last) {
            llg_proc_t* moved = ev->waiters[last];
            int* moved_slot = event_wait_slot(&moved->wait, ev, last);
            if (!moved_slot) abort();
            *moved_slot = index;
            ev->waiters[index] = moved;
        }
        slots[i] = -1;
    }
}

static void event_triggered_unlink(llg_wait_t* w) {
    if (!w || w->kind != W_EVENT_TRIGGERED) return;
    llg_event_object_t* ev = w->payload.event.triggered_ev;
    if (!ev) return;
    int index = w->payload.event.inline_slot;
    if (index < 0) return;
    int last = --ev->n_triggered_waiters;
    if (index != last) {
        llg_proc_t* moved = ev->triggered_waiters[last];
        ev->triggered_waiters[index] = moved;
        moved->wait.payload.event.inline_slot = index;
    }
    w->payload.event.inline_slot = -1;
}

// ── fork/join (coroutine children) ────────────────────────────────────────────

static void llg_kill_proc_tree(llg_proc_t* p); // mutual recursion below

static void process_handle_terminal(llg_proc_t* proc, int status) {
    llg_process_handle_t* handle = proc ? proc->handle : NULL;
    if (!handle) return;
    proc->handle = NULL;
    proc->status = status;
    handle->proc = NULL;
    handle->status = status;
    // Process-await registrations use the same address index as signal waits.
    // Head insertion retains the previous global-list wake order.
    llg_wait_source_t* source = wait_source_find(handle);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
        llg_wait_t* wait;
        while ((wait = wait_source_next(&cursor)) != NULL)
            wake_proc(wait->proc);
        wait_subscription_unlink(&cursor);
    }
    // Drop the process-owned reference after all awaiters have been woken;
    // each awaiter holds its own reference until wake/cancellation.
    llg_process_release(handle);
}

static void process_handle_shutdown(llg_proc_t* proc) {
    llg_process_handle_t* handle = proc ? proc->handle : NULL;
    if (!handle) return;
    proc->handle = NULL;
    proc->status = handle->status == LLG_PROCESS_FINISHED
                       ? LLG_PROCESS_FINISHED
                       : LLG_PROCESS_KILLED;
    handle->proc = NULL;
    if (handle->status != LLG_PROCESS_FINISHED)
        handle->status = LLG_PROCESS_KILLED;
    llg_process_release(handle);
}

static void release_killed_proc_resources(llg_proc_t* proc) {
    value_scopes_unwind(proc);
    activation_unwind_proc(proc);
    llg_frame_release(proc->frame);
    proc->frame = NULL;
    process_local_release_all(proc);
}

// Unlink a suspended or queued proc from every scheduler queue, free its
// pending NBA list, and retire its storage. A named disable may cancel the
// executing coroutine; its destruction is deferred until the scheduler resumes.
static void llg_kill_proc(llg_proc_t* p, int notify_parent) {
    if (!p || p->killed) return;
    p->killed = 1;
    release_program_process(p);
    p->suspended = 0;
    p->wake_pending = 0;
    cancel_proc_nbas(p);

    llg_wait_t* w = &p->wait;
    if (w->kind != W_NONE) {
        if (w->kind == W_ASSERTION) {
            for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
                 assertion = assertion->next) {
                if (assertion->identity == w->payload.rare->assertion.identity &&
                    assertion->kind == LLG_ASSERTION_EXPECT)
                    assertion->expect_active = 0;
            }
        }
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
        if (w->kind == W_SEMAPHORE) semaphore_waiter_unlink(w);
        if (w->kind == W_MAILBOX_GET || w->kind == W_MAILBOX_PUT)
            mailbox_unlink_wait(w);
        wait_payload_release(w);
        g.wait_count--;
    }
    remove_region_entry(p);

    llg_fork_group_t* parent_group = p->grp;
    if (parent_group) {
        p->fork_child->proc = NULL;
        p->fork_child = NULL;
        p->grp = NULL;
    }
    // A process may kill itself indirectly by killing an ancestor or its
    // program origin. Keep its live owners intact until generated code has
    // returned through the active C call chain; the scheduler reaps them at
    // the first safe boundary below.
    if (p != llg_current()) release_killed_proc_resources(p);
    process_handle_terminal(p, LLG_PROCESS_KILLED);
    if (notify_parent && parent_group && !parent_group->terminal)
        llg_fork_group_child_done(parent_group);
    unregister_proc(p);
    p->next_retired = g.retired_procs;
    g.retired_procs = p;
}

// The active coroutine's frames must survive until llg_co_run returns to the
// scheduler. Other cancelled processes can be reclaimed after cancellation
// traversal, including within a long-running caller.
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
        release_killed_proc_resources(proc);
        free_proc_record(proc);
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
                llg_kill_proc_tree_internal(c->proc, 0);
                c->proc = NULL; // freed inline; teardown skips it
            }
            c = next_c;
        }
        grp->remaining = 0;
        grp->terminal = 1;
        grp->prev_g = NULL;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        grp = next_g;
    }
    p->fork_groups = NULL;
    p->fork_groups_tail = NULL;
    p->pending_fork_groups = NULL;
}

// Kill `p` and all of its descendants.
static void llg_kill_proc_tree(llg_proc_t* p) {
    llg_kill_proc_tree_internal(p, 1);
}

static void llg_kill_proc_tree_internal(llg_proc_t* p, int notify_parent) {
    llg_kill_proc_groups(p);
    llg_kill_proc(p, notify_parent);
}

llg_process_handle_t* llg_process_self(llg_proc_t* self) {
    llg_runtime_service_enter(self, "process::self");
    return self ? self->handle : NULL;
}

int llg_process_status(const llg_process_handle_t* handle) {
    return handle ? handle->status : LLG_PROCESS_KILLED;
}

void llg_process_retain(llg_process_handle_t* handle) {
    if (!handle) return;
    if (handle->refs == SIZE_MAX) {
        fprintf(stderr, "llg: process handle reference count overflow\n");
        abort();
    }
    handle->refs++;
}

void llg_process_release(llg_process_handle_t* handle) {
    if (!handle) return;
    if (handle->refs == 0) {
        fprintf(stderr, "llg: process handle reference count underflow\n");
        abort();
    }
    handle->refs--;
    if (handle->refs != 0) return;
    process_handle_unlink(handle);
    free(handle);
}

static llg_process_local_ref_t* process_local_find(llg_proc_t* proc,
                                                    llg_process_handle_t** slot) {
    for (llg_process_local_ref_t* local = proc ? proc->process_locals : NULL;
         local; local = local->next) {
        if (local->slot == slot) return local;
    }
    return NULL;
}

void llg_process_local_register(llg_process_handle_t** slot) {
    llg_proc_t* proc = llg_current();
    if (!proc || !slot || !region_can_mutate("process local registration")) return;
    llg_process_local_ref_t* local = process_local_find(proc, slot);
    if (local) {
        // A repeated declaration is a fresh automatic lifetime (for example,
        // an always-loop iteration). Drop the previous reference and clear
        // the caller's slot before a declaration initializer assigns again.
        llg_process_release(local->value);
        local->value = NULL;
        *slot = NULL;
        return;
    }
    local = (llg_process_local_ref_t*)llg_checked_calloc(
        1, sizeof(*local), "process local reference");
    local->slot = slot;
    local->next = proc->process_locals;
    proc->process_locals = local;
}

static void process_local_release_all(llg_proc_t* proc) {
    while (proc && proc->process_locals) {
        llg_process_local_ref_t* local = proc->process_locals;
        proc->process_locals = local->next;
        llg_process_release(local->value);
        free(local);
    }
}

void llg_process_assign(llg_process_handle_t** target,
                        llg_process_handle_t* source) {
    if (!target || !region_can_mutate("process handle write")) return;
    if (source) llg_process_retain(source);
    if (*target) llg_process_release(*target);
    *target = source;
    llg_proc_t* proc = llg_current();
    llg_process_local_ref_t* local = process_local_find(proc, target);
    if (local) local->value = source;
}

void llg_process_kill(llg_proc_t* self, llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::kill");
    if (!handle || !handle->proc || !region_can_mutate("process control")) return;
    llg_proc_t* target = handle->proc;
    llg_kill_proc_tree(target);
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    if (self && self->killed)
        self->chain.exiting = LLG_EXIT_ABANDON;
    else if (g.finish && self)
        self->chain.exiting = LLG_EXIT_COMPLETE;
}

llg_co_arm_t llg_arm_process_suspend(llg_proc_t* self,
                                     llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::suspend");
    if (!handle || !handle->proc || !region_can_mutate("process suspension"))
        return LLG_CO_ARM_READY;
    llg_proc_t* target = handle->proc;
    if (target->suspended || target->killed || target->completed)
        return LLG_CO_ARM_READY;
    target->suspended = 1;
    target->wake_pending = 0;
    remove_region_entry(target);
    process_status_set(target, LLG_PROCESS_SUSPENDED);
    if (target == self) {
        // Suspending is a blocking control for join_none eligibility, but the
        // wait itself is represented by the stable handle state rather than a
        // second scheduler waiter.
        start_pending_fork_children(target);
        return LLG_CO_ARM_SUSPEND;
    }
    return LLG_CO_ARM_READY;
}

void llg_process_resume(llg_proc_t* self, llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::resume");
    if (!handle || !handle->proc || !region_can_mutate("process resumption")) return;
    llg_proc_t* target = handle->proc;
    if (!target->suspended || target->killed || target->completed) return;
    target->suspended = 0;
    if (target->wait.kind != W_NONE) {
        // The outstanding condition remains registered and must be satisfied
        // before this process becomes runnable again.
        process_status_set(target, LLG_PROCESS_WAITING);
        return;
    }
    if (target->wake_pending) target->wake_pending = 0;
    process_status_set(target, LLG_PROCESS_RUNNING);
    enqueue_region(target, target->region);
}

llg_co_arm_t llg_arm_process_await(llg_proc_t* self,
                                   llg_process_handle_t* handle) {
    llg_runtime_service_enter(self, "process::await");
    if (!self || !region_can_mutate("process await scheduling"))
        return LLG_CO_ARM_READY;
    if (!handle || !handle->proc || handle->proc == self)
        return LLG_CO_ARM_READY;
    llg_wait_t* wait = &self->wait;
    wait->kind = W_PROCESS;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    wait_rare_allocate(wait, "process await payload")->process.target = handle;
    llg_process_retain(handle);
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_semaphore_t* llg_semaphore_new(sv4_t key_count) {
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys)) return NULL;
    llg_semaphore_t* semaphore = (llg_semaphore_t*)llg_checked_calloc(
        1, sizeof(*semaphore), "semaphore");
    semaphore->available = keys;
    semaphore->next_all = g.semaphores;
    g.semaphores = semaphore;
    return semaphore;
}

void llg_semaphore_put(llg_semaphore_t* semaphore, sv4_t key_count) {
    if (!region_can_mutate("semaphore put") ||
        !semaphore_valid(semaphore, "put"))
        return;
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys) || keys == 0) return;
    if (keys > UINT64_MAX - semaphore->available) {
        fprintf(stderr, "llg: semaphore key count overflow in put\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    semaphore->available += keys;
    semaphore_wake_available(semaphore);
}

llg_co_arm_t llg_arm_semaphore_get(llg_proc_t* self,
                                   llg_semaphore_t* semaphore,
                                   sv4_t key_count) {
    llg_runtime_service_enter(self, "semaphore::get");
    if (!region_can_mutate("semaphore get") ||
        !semaphore_valid(semaphore, "get"))
        return LLG_CO_ARM_READY;
    if (!self) {
        fprintf(stderr, "llg: semaphore get requested outside a simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        return LLG_CO_ARM_READY;
    }
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys) || keys == 0)
        return LLG_CO_ARM_READY;
    if (!semaphore->wait_head && semaphore->available >= keys) {
        semaphore->available -= keys;
        return LLG_CO_ARM_READY;
    }

    llg_wait_t* wait = &self->wait;
    llg_semaphore_wait_t* node = (llg_semaphore_wait_t*)llg_checked_calloc(
        1, sizeof(*node), "semaphore waiter");
    node->owner = semaphore;
    node->proc = self;
    node->keys = keys;
    if (semaphore->wait_tail) {
        semaphore->wait_tail->next = node;
    } else {
        semaphore->wait_head = node;
    }
    semaphore->wait_tail = node;
    wait->kind = W_SEMAPHORE;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_semaphore_payload_t* payload =
        &wait_rare_allocate(wait, "semaphore wait payload")->semaphore;
    payload->semaphore = semaphore;
    payload->waiter = node;
    payload->keys = keys;
    process_status_set(self, LLG_PROCESS_WAITING);
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

int llg_semaphore_try_get(llg_semaphore_t* semaphore, sv4_t key_count) {
    if (!region_can_mutate("semaphore try_get") ||
        !semaphore_valid(semaphore, "try_get"))
        return 0;
    uint64_t keys = 0;
    if (!semaphore_key_count(key_count, &keys)) return 0;
    if (keys == 0) return 1;
    // Preserve the specified FIFO ordering: an immediate attempt never skips
    // an already queued request, even when enough keys are currently visible.
    if (semaphore->wait_head || semaphore->available < keys) return 0;
    semaphore->available -= keys;
    return 1;
}

// Detach a live group without changing the order of surviving siblings.
static void fork_group_unlink(llg_fork_group_t* grp) {
    llg_proc_t* parent = grp->parent;
    if (grp->prev_g) grp->prev_g->next_g = grp->next_g;
    else parent->fork_groups = grp->next_g;
    if (grp->next_g) grp->next_g->prev_g = grp->prev_g;
    else parent->fork_groups_tail = grp->prev_g;
    if (parent->pending_fork_groups == grp)
        parent->pending_fork_groups = grp->next_g;
    grp->prev_g = NULL;
}

// One child of `grp` finished. Decrement the live count,
// wake a join/wait_fork waiter whose condition is now met, and move the group
// to the zombie list once the last child is done.
static void llg_fork_group_child_done(llg_fork_group_t* grp) {
    if (!grp || grp->terminal || grp->remaining <= 0) return;
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
    if (wake && parent->wait.kind == W_FORK && parent->wait.payload.rare &&
        parent->wait.payload.rare->fork.group == grp) {
        wake_proc(parent);
    }
    if (grp->remaining == 0) {
        grp->terminal = 1;
        // Unlink from the parent's live-group list; the group and its child
        // list are freed by process_zombie_groups at the next safe point.
        // join_any / join_none groups stay live until the last child finishes
        // so wait_fork still works.
        fork_group_unlink(grp);
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        // wait fork observes only the groups spawned by its own process.
        if (parent->wait.kind == W_FORK_ALL && !parent->fork_groups)
            wake_proc(parent);
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
    grp->prev_g = parent->fork_groups_tail;
    if (grp->prev_g) grp->prev_g->next_g = grp;
    else parent->fork_groups = grp;
    parent->fork_groups_tail = grp;
    if (!parent->pending_fork_groups) parent->pending_fork_groups = grp;
    return grp;
}

static void start_pending_fork_children(llg_proc_t* parent) {
    if (!parent) return;
    llg_fork_group_t* pending = parent->pending_fork_groups;
    parent->pending_fork_groups = NULL;
    for (llg_fork_group_t* grp = pending; grp; grp = grp->next_g) {
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

static llg_proc_t* llg_fork_impl(const llg_co_desc_t* desc,
                                 const char* name, llg_fork_group_t* grp,
                                 llg_frame_t* frame) {
    if (!desc || !desc->fn || !grp || !region_can_mutate("fork scheduling"))
        return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t))
        llg_rt_co_oom(desc->frame_size);
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size,
        "forked process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->grp = grp;
    p->frame = frame;
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_frame_retain(frame);
    llg_rng_state_child(&grp->parent->rng, &p->rng);
    p->program = grp->parent->program;
    p->action_assertion = grp->parent->action_assertion;
    p->is_assertion_action = grp->parent->is_assertion_action;
    p->program_live = 0;
    p->budget_time = g.now;
    grp->remaining++;
    llg_fork_child_t* c = (llg_fork_child_t*)llg_checked_malloc(
        1, sizeof(llg_fork_child_t), "fork child");
    c->proc = p;
    c->next = NULL;
    if (grp->children_tail) grp->children_tail->next = c;
    else grp->children = c;
    grp->children_tail = c;
    p->fork_child = c;
    register_proc(p);
    if (grp->join_kind != LLG_JOIN_NONE) enqueue_region(p, grp->child_region);
    return p;
}

llg_proc_t* llg_fork(const llg_co_desc_t* desc, const char* name,
                     llg_fork_group_t* grp) {
    return llg_fork_impl(desc, name, grp, NULL);
}

llg_proc_t* llg_fork_with_frame(const llg_co_desc_t* desc,
                                const char* name, llg_fork_group_t* grp,
                                llg_frame_t* frame) {
    return llg_fork_impl(desc, name, grp, frame);
}

llg_proc_t* llg_spawn_detached_with_frame(const llg_co_desc_t* desc,
                                          const char* name,
                                          llg_frame_t* frame) {
    llg_proc_t* parent = llg_current();
    if (!desc || !desc->fn || !parent || !region_can_mutate("detached process scheduling"))
        return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t))
        llg_rt_co_oom(desc->frame_size);
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size,
        "detached process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->frame = frame;
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_frame_retain(frame);
    llg_rng_state_child(&parent->rng, &p->rng);
    // Like a fork descendant, the process keeps its origin for program
    // cancellation but never extends that program's lifetime.
    p->program = parent->program;
    p->action_assertion = parent->action_assertion;
    p->is_assertion_action = parent->is_assertion_action;
    p->program_live = 0;
    p->budget_time = g.now;
    llg_region_t region = region_is_reactive(g.current_region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    p->region = region;
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_co_arm_t llg_arm_join(llg_proc_t* self, llg_fork_group_t* grp) {
    llg_runtime_service_enter(self, "join");
    if (!grp || !region_can_mutate("fork wait scheduling"))
        return LLG_CO_ARM_READY;
    if (grp->remaining == 0) {
        // Empty fork groups never receive a child-done callback, so finalize
        // them here before join or wait_fork can observe a permanently live
        // group. The parent is the currently running process.
        fork_group_unlink(grp);
        grp->terminal = 1;
        grp->next_g = g.zombie_groups;
        g.zombie_groups = grp;
        return LLG_CO_ARM_READY;
    }
    if (grp->join_kind == LLG_JOIN_NONE) return LLG_CO_ARM_READY;
    if (grp->join_kind == LLG_JOIN_ANY && grp->resumed)
        return LLG_CO_ARM_READY;
    if (!self) return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_FORK;
    wait_rare_allocate(w, "fork wait payload")->fork.group = grp;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_wait_fork(llg_proc_t* self) {
    llg_runtime_service_enter(self, "wait fork");
    if (!self || self->fork_groups == NULL ||
        !region_can_mutate("fork wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_FORK_ALL;
    wait_rare_allocate(w, "wait-fork payload")->fork_all.parent = self;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    register_wait();
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

void llg_disable_fork(llg_proc_t* self) {
    llg_runtime_service_enter(self, "disable fork");
    if (!region_can_mutate("fork scheduling")) return;
    if (!self) return;
    llg_kill_proc_groups(self);
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
}

void llg_program_exit(llg_proc_t* self) {
    llg_runtime_service_enter(self, "program exit");
    if (!self || !self->program) return;
    if (!region_can_mutate("program exit")) return;
    llg_program_t* origin = self->program;
    // Mark closed before cancellation; recursive unlinking only adjusts counts.
    origin->closed = 1;
    for (;;) {
        llg_proc_t* victim = NULL;
        for (int i = 0; i < g.n_procs; i++) {
            llg_proc_t* proc = g.all_procs[i];
            if (proc && proc->program == origin && !proc->killed &&
                !proc->completed) {
                victim = proc;
                break;
            }
        }
        if (!victim) break;
        llg_kill_proc_tree(victim);
    }
    service_program_completions();
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    // Cancellation already detached all process-owned resources.
    self->chain.exiting = LLG_EXIT_ABANDON;
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
            // The group is being cancelled as a unit. Suppress per-child
            // completion accounting until the group is detached below.
            llg_kill_proc_tree_internal(child->proc, 0);
            child->proc = NULL;
        }
        child = child->next;
    }
    grp->remaining = 0;
    grp->terminal = 1;
    fork_group_unlink(grp);
    grp->next_g = g.zombie_groups;
    g.zombie_groups = grp;

    if (parent->wait.kind == W_FORK && parent->wait.payload.rare &&
        parent->wait.payload.rare->fork.group == grp) {
        wake_proc(parent);
    }
    if (parent->wait.kind == W_FORK_ALL && parent->fork_groups == NULL) {
        wake_proc(parent);
    }
}

void llg_disable_target(llg_proc_t* self, uint32_t declaration,
                        uint32_t instance) {
    llg_runtime_service_enter(self, "disable");
    if (!region_can_mutate("named activation scheduling")) return;
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
    semaphore_service_cancelled_waiters();
    reap_retired_procs();
    if (self && self->killed) self->chain.exiting = LLG_EXIT_ABANDON;
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
            unregister_proc(c->proc);
            free_proc_record(c->proc);
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




static void force_dependency_changed(sv4_t* sig, double* real, int is_real);
static void sig_write(sv4_t* target, sv4_t value);
static void fixed_array_changed(sv4_t* target);
static int pca_real_active(double* target);

enum { LLG_DEPENDENCY_INITIAL_BUCKETS = 64u };

static size_t dependency_bucket(const void* target) {
    uint64_t hash = (uint64_t)(uintptr_t)target;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash & (llg_dependency_bucket_count - 1);
}

static const void* dependency_key(const llg_dependency_binding_t* binding) {
    return binding->target ? (const void*)binding->target
                           : (const void*)binding->real_target;
}

static llg_dependency_binding_t* dependency_bucket_head(const void* target) {
    return llg_dependency_bucket_count
        ? llg_dependency_buckets[dependency_bucket(target)] : NULL;
}

/* Keep the load factor at most one; growth rehashes from the owning list. */
static void dependency_index_reserve(void) {
    if (llg_dependency_binding_count < llg_dependency_bucket_count) return;
    size_t count = llg_dependency_bucket_count
        ? llg_dependency_bucket_count : LLG_DEPENDENCY_INITIAL_BUCKETS;
    while (count <= llg_dependency_binding_count) {
        if (count > SIZE_MAX / 2)
            llg_fatal_allocation("dependency binding index", count, 2);
        count *= 2;
    }
    llg_dependency_binding_t** buckets = (llg_dependency_binding_t**)llg_checked_calloc(
        count, sizeof(*buckets), "dependency binding index");
    free(llg_dependency_buckets);
    llg_dependency_buckets = buckets;
    llg_dependency_bucket_count = count;
    for (llg_dependency_binding_t* binding = llg_dependency_bindings;
         binding; binding = binding->next) {
        size_t bucket = dependency_bucket(dependency_key(binding));
        binding->bucket_next = buckets[bucket];
        buckets[bucket] = binding;
    }
}

static void dependency_bind(sv4_t* target, double* real_target, sv4_t* dependency,
                            const char* what) {
    const void* key = target ? (const void*)target : (const void*)real_target;
    for (llg_dependency_binding_t* binding = dependency_bucket_head(key);
         binding; binding = binding->bucket_next) {
        if (binding->target == target && binding->real_target == real_target &&
            binding->dependency == dependency) return;
    }
    dependency_index_reserve();
    llg_dependency_binding_t* binding = (llg_dependency_binding_t*)llg_checked_malloc(
        1, sizeof(*binding), what);
    binding->target = target;
    binding->real_target = real_target;
    binding->dependency = dependency;
    binding->next = llg_dependency_bindings;
    llg_dependency_bindings = binding;
    size_t bucket = dependency_bucket(key);
    binding->bucket_next = llg_dependency_buckets[bucket];
    llg_dependency_buckets[bucket] = binding;
    ++llg_dependency_binding_count;
}

void llg_dependency_bind(sv4_t* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid dependency binding\n");
        abort();
    }
    dependency_bind(target, NULL, dependency, "dependency binding");
}

void llg_dependency_bind_real(double* target, sv4_t* dependency) {
    if (!target || !dependency) {
        fprintf(stderr, "llg: invalid real dependency binding\n");
        abort();
    }
    dependency_bind(NULL, target, dependency, "real dependency binding");
}

void llg_dependency_changed(sv4_t* dependency) {
    if (!dependency) return;
    uint64_t bit = llg_sv4_word(*dependency, 0, LLG_SV4_BITS) & 1u;
    /* Native strings and containers publish through this marker. A subscriber
     * may terminate the writer without returning through this function. */
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    value[0] = sv4_from_u64(bit ^ 1u, 1, 0);
    sig_write(dependency, value[0]);
    llg_value_scope_end(scope);
}

void llg_dependency_notify(sv4_t* contents, sv4_t* shape, int change) {
    if (change & 1)
        llg_dependency_changed(contents);
    if (change & 2)
        llg_dependency_changed(shape);
}

static int ev_matches(sv4_t old, sv4_t new, int kind) {
    if (kind == LLG_EV_ANY) return !sv4_same(old, new);
    // Edge controls use only the LSB. Read without allocating one-bit values.
    unsigned a = llg_sv4_state(old, 0);
    unsigned b = llg_sv4_state(new, 0);
    if (a == 3) a = 2;
    if (b == 3) b = 2;
    return kind == LLG_EV_POSEDGE ? (a == 0 && b != 0) || (a == 2 && b == 1)
                                  : (a == 1 && b != 1) || (a == 2 && b == 0);
}

static int ev_inline_matches(llg_wait_t* wait, sv4_t value, int kind) {
    if (kind == LLG_EV_ANY) return !wait_inline_same(wait, value);
    const llg_wait_inline_payload_t* single = &wait->payload.single;
    unsigned a = !single->width || (single->words[0].bval & 1u)
        ? 2u : (unsigned)(single->words[0].aval & 1u);
    unsigned b = llg_sv4_state(value, 0);
    if (b == 3) b = 2;
    return kind == LLG_EV_POSEDGE ? (a == 0 && b != 0) || (a == 2 && b == 1)
                                  : (a == 1 && b != 1) || (a == 2 && b == 0);
}

// Signal publication calls this only after establishing that some bit changed.
// Directional controls still inspect the packed value's LSB.
static int ev_matches_changed(sv4_t old, sv4_t new, int kind) {
    return kind == LLG_EV_ANY || ev_matches(old, new, kind);
}

/* Publication precedes callbacks and wait registration. Clocking drives and
 * ##0 may first query a descriptor AFTER its edge, so even automatic locals
 * must be recorded without a current subscriber. Ordinary waits use snapshots,
 * sampled inputs use sampling.c, and named-event .triggered uses event objects;
 * none of those reads this table. Entries with no sequence consumer die at time
 * advance. Registered sequence clocks keep cumulative directional ticks because
 * same-clock delays subtract ticks across slots; their occurrence flags expire.
 * Scoped descriptors are forgotten at final owner release (not lexical exit:
 * an NBA may still retain them). External owners must forget before freeing or
 * reusing a descriptor. Equality is exact pointer identity, never payload or
 * ordering of unrelated pointers. The list owns entries; the index borrows them.
 */
enum {
    CLOCKING_ANY = 1u,
    CLOCKING_POSEDGE = 2u,
    CLOCKING_NEGEDGE = 4u,
    CLOCKING_INDEX_INITIAL_CAPACITY = 16u
};

/* A private object supplies a portable tombstone without a fabricated pointer. */
static llg_clocking_edge_t clocking_deleted_entry;

static size_t clocking_edge_hash(const void* signal) {
    uint64_t hash = (uint64_t)(uintptr_t)signal;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void clocking_index_rebuild(size_t capacity) {
    llg_clocking_edge_t** index = (llg_clocking_edge_t**)llg_checked_calloc(
        capacity, sizeof(*index), "clocking history index");
    for (llg_clocking_edge_t* edge = g.clocking_edges; edge; edge = edge->next) {
        size_t slot = clocking_edge_hash(edge->signal) & (capacity - 1);
        while (index[slot]) slot = (slot + 1) & (capacity - 1);
        index[slot] = edge;
    }
    free(g.clocking_index);
    g.clocking_index = index;
    g.clocking_capacity = capacity;
    g.clocking_used = g.clocking_count;
}

static size_t clocking_edge_slot(sv4_t* signal) {
    size_t slot = clocking_edge_hash(signal) & (g.clocking_capacity - 1);
    while (g.clocking_index[slot]) {
        llg_clocking_edge_t* edge = g.clocking_index[slot];
        if (edge != &clocking_deleted_entry && edge->signal == signal) break;
        slot = (slot + 1) & (g.clocking_capacity - 1);
    }
    return slot;
}

static llg_clocking_edge_t* find_clocking_edge(sv4_t* signal) {
    if (!g.clocking_count || !signal) return NULL;
    return g.clocking_index[clocking_edge_slot(signal)];
}

static llg_clocking_edge_t* clocking_edge_get(sv4_t* signal) {
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (edge) return edge;
    size_t capacity = g.clocking_capacity ? g.clocking_capacity
                                         : CLOCKING_INDEX_INITIAL_CAPACITY;
    if (g.clocking_count >= capacity - capacity / 4) {
        if (capacity > SIZE_MAX / 2)
            llg_fatal_allocation("clocking history index", capacity, 2);
        capacity *= 2;
    }
    /* Rehash tombstones before occupied slots can make a probe unbounded. */
    if (capacity != g.clocking_capacity ||
        g.clocking_used >= capacity - capacity / 4)
        clocking_index_rebuild(capacity);
    size_t slot = clocking_edge_hash(signal) & (capacity - 1);
    while (g.clocking_index[slot] &&
           g.clocking_index[slot] != &clocking_deleted_entry)
        slot = (slot + 1) & (capacity - 1);
    if (!g.clocking_index[slot]) ++g.clocking_used;
    edge = (llg_clocking_edge_t*)llg_checked_calloc(
        1, sizeof(*edge), "clocking event history");
    edge->signal = signal;
    edge->next = g.clocking_edges;
    if (edge->next) edge->next->prev = edge;
    g.clocking_edges = edge;
    g.clocking_index[slot] = edge;
    ++g.clocking_count;
    return edge;
}

void llg_clocking_forget_signal(sv4_t* signal) {
    if (!g.clocking_count || !signal) return;
    size_t slot = clocking_edge_slot(signal);
    llg_clocking_edge_t* edge = g.clocking_index[slot];
    if (!edge) return;
    if (edge->prev) edge->prev->next = edge->next;
    else g.clocking_edges = edge->next;
    if (edge->next) edge->next->prev = edge->prev;
    g.clocking_index[slot] = &clocking_deleted_entry;
    --g.clocking_count;
    free(edge);
}

static void clocking_advance_time(void) {
    llg_clocking_edge_t* edge = g.clocking_edges;
    while (edge) {
        llg_clocking_edge_t* next = edge->next;
        if (edge->keep_ticks) edge->occurred = 0;
        else llg_clocking_forget_signal(edge->signal);
        edge = next;
    }
    if (!g.clocking_count) {
        free(g.clocking_index);
        g.clocking_index = NULL;
        g.clocking_capacity = 0;
        g.clocking_used = 0;
    } else {
        size_t capacity = CLOCKING_INDEX_INITIAL_CAPACITY;
        while (g.clocking_count > capacity - capacity / 4) capacity *= 2;
        /* Bound retained index storage to surviving sequence-clock identities. */
        if (capacity != g.clocking_capacity || g.clocking_used != g.clocking_count)
            clocking_index_rebuild(capacity);
    }
}

static void clocking_record_edge(sv4_t* signal, sv4_t old, sv4_t value) {
    if (!signal) return;
    llg_clocking_edge_t* edge = clocking_edge_get(signal);
    if (edge->time != g.now) edge->occurred = 0;
    edge->time = g.now;
    edge->occurred |= CLOCKING_ANY;
    if (ev_matches(old, value, LLG_EV_POSEDGE)) {
        edge->occurred |= CLOCKING_POSEDGE;
        if (edge->keep_ticks && edge->posedge_count != UINT64_MAX)
            edge->posedge_count++;
    }
    if (ev_matches(old, value, LLG_EV_NEGEDGE)) {
        edge->occurred |= CLOCKING_NEGEDGE;
        if (edge->keep_ticks && edge->negedge_count != UINT64_MAX)
            edge->negedge_count++;
    }
}

static uint64_t assertion_clock_tick(sv4_t* signal, int edge_kind) {
    llg_clocking_edge_t* edge = find_clocking_edge(signal);
    if (!edge) return 0;
    return edge_kind == LLG_EV_POSEDGE ? edge->posedge_count
                                       : edge->negedge_count;
}

static int clocking_event_current(const llg_wait_src_t* srcs, int n) {
    if (!srcs || n <= 0) return 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            llg_clocking_edge_t* edge = find_clocking_edge(srcs[i].sig);
            if (!edge) continue;
            unsigned occurred = srcs[i].kind == LLG_EV_POSEDGE
                                    ? CLOCKING_POSEDGE
                                    : srcs[i].kind == LLG_EV_NEGEDGE
                                          ? CLOCKING_NEGEDGE : CLOCKING_ANY;
            if (edge->time == g.now && (edge->occurred & occurred)) return 1;
        } else if (srcs[i].ev && llg_event_triggered(srcs[i].ev)) {
            return 1;
        }
    }
    return 0;
}

static void free_clocking_drive(llg_clocking_drive_t* drive) {
    if (!drive) return;
    sv4_destroy(&drive->value);
    sv4_destroy(&drive->mask);
    value_scope_release(drive->target_scope);
    free(drive->specs);
    free(drive);
}

static void clocking_drive_enqueue(const llg_clocking_drive_t* drive) {
    llg_nba_t* n = new_clocking_nba(drive->ticks);
    if (!n) return;
    n->target = drive->target;
    n->target_scope = value_scope_retain_target(drive->target);
    n->net_target = drive->net_target;
    n->net_slot = drive->net_slot;
    sv4_copy(&n->value, &drive->value);
    sv4_copy(&n->mask, &drive->mask);
    n->has_mask = drive->has_mask;
    n->range_offset = drive->range_offset;
    n->range_width = drive->range_width;
    n->has_range = drive->has_range;
    n->is_real = drive->is_real;
    n->real_target = drive->real_target;
    n->real_value = drive->real_value;
    enqueue_nba(n);
}

static int clocking_drive_source_matches_signal(
    const llg_clocking_drive_t* drive, sv4_t* signal, sv4_t old, sv4_t value) {
    for (int i = 0; i < drive->n_specs; i++) {
        const llg_wait_src_t* source = &drive->specs[i];
        if (source->sig == signal &&
            ev_matches_changed(old, value, source->kind)) return 1;
    }
    return 0;
}

static int clocking_drive_source_matches_event(
    const llg_clocking_drive_t* drive, llg_event_object_t* event) {
    for (int i = 0; i < drive->n_specs; i++) {
        const llg_wait_src_t* source = &drive->specs[i];
        if (source->ev && source->ev->object == event) return 1;
    }
    return 0;
}

static void clocking_drive_signal_match(sv4_t* signal, sv4_t old, sv4_t value) {
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_signal(drive, signal, old, value)) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        free_clocking_drive(drive);
    }
    g.clocking_drives_tail = g.clocking_drives;
    while (g.clocking_drives_tail && g.clocking_drives_tail->next)
        g.clocking_drives_tail = g.clocking_drives_tail->next;
}

static void clocking_drive_event_match(llg_event_object_t* event) {
    llg_clocking_drive_t** slot = &g.clocking_drives;
    while (*slot) {
        llg_clocking_drive_t* drive = *slot;
        if (!clocking_drive_source_matches_event(drive, event)) {
            slot = &drive->next;
            continue;
        }
        *slot = drive->next;
        drive->next = NULL;
        clocking_drive_enqueue(drive);
        free_clocking_drive(drive);
    }
    g.clocking_drives_tail = g.clocking_drives;
    while (g.clocking_drives_tail && g.clocking_drives_tail->next)
        g.clocking_drives_tail = g.clocking_drives_tail->next;
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
    llg_wait_expression_payload_t* payload = &wait->payload.expression;
    llg_expr_event_spec_t* spec = &payload->expressions[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) {
            spec->real_eval(&value, spec->eval_context);
            if (llg_rt_exiting()) return 0;
        }
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(payload->real_last[index], value, spec->kind);
        payload->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) {
        spec->eval(value, spec->eval_context);
        if (llg_rt_exiting()) {
            llg_value_scope_end(scope);
            return 0;
        }
    }
    else sv4_copy(value, spec->sig);
    int matched = ev_matches(payload->last[index], *value, spec->kind);
    sv4_move(&payload->last[index], value);
    llg_value_scope_end(scope);
    return matched && expression_qualifies(spec);
}

static int deferred_expression_update(llg_deferred_trigger_t* trigger,
                                      int index, sv4_t* sig, double* real) {
    llg_expr_event_spec_t* spec = &trigger->specs[index];
    if (spec->event || !expression_dependency_changed(spec, sig, real)) return 0;
    if (spec->real || spec->real_eval || spec->real_sig) {
        double value;
        if (spec->real_eval) {
            spec->real_eval(&value, spec->eval_context);
            if (llg_rt_exiting()) return 0;
        }
        else if (spec->real_sig) value = *spec->real_sig;
        else return 0;
        int matched = real_ev_matches(trigger->real_last[index], value, spec->kind);
        trigger->real_last[index] = value;
        return matched && expression_qualifies(spec);
    }
    if (!spec->eval && !spec->sig) return 0;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (spec->eval) {
        spec->eval(value, spec->eval_context);
        if (llg_rt_exiting()) {
            llg_value_scope_end(scope);
            return 0;
        }
    }
    else sv4_copy(value, spec->sig);
    int matched = ev_matches(trigger->last[index], *value, spec->kind);
    sv4_move(&trigger->last[index], value);
    llg_value_scope_end(scope);
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

static void sig_publish_changed(sv4_t* target, sv4_t old, sv4_t value,
                                sv4_t published) {
#ifndef LLG_WAVEFORM
    (void)published;
#endif
    clocking_record_edge(target, old, value);
    clocking_drive_signal_match(target, old, value);
    sampled_record_write(target);
    sampled_domain_clock_signal_changed(target, old, value);
    // `disable iff` is an asynchronous, unsampled control. Abort pending
    // attempts at the write boundary, before any waiter or later region can
    // observe the changed value.
    assertion_disable_signal_changed(target);
    // Ordinary accept_on/reject_on controls are also asynchronous. Their
    // predicate is evaluated only after the write is visible, while the
    // synchronous variants are checked at the sampled assertion edge below.
    assertion_abort_condition_changed();
    // Queue the clock event after asynchronous controls have seen the new
    // value. This keeps a clock that also changes an accept/reject condition
    // from being discarded before its sampled control can resolve it.
    assertion_clock_signal_changed(target, old, value);
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
    llg_wave_changed_sv4(target, &published, g.now);
#endif
    llg_wait_source_t* source = wait_source_find(target);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
    }
    llg_wait_t* w;
    while (source && (w = wait_source_next(&cursor)) != NULL) {
        int wake = 0;
        if (w->kind == W_EVENTS_INLINE) {
            llg_wait_inline_payload_t* single = &w->payload.single;
            if (single->specs[0].sig == target) {
                wake = ev_inline_matches(w, *target, single->specs[0].kind);
                if (!wake) {
                    if (wait_inline_fits(target)) wait_inline_copy(w, target);
                    else wait_inline_promote(w, target);
                }
            }
        } else if (w->kind == W_LEVEL_INLINE) {
            if (w->payload.single.specs[0].sig == target &&
                wait_inline_same(w, *target)) wake = 1;
        } else if (w->kind == W_EVENTS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            for (int i = 0; i < payload->n; i++) {
                if (payload->specs[i].sig == target) {
                    if (ev_matches(payload->last[i], *target,
                                   payload->specs[i].kind))
                        wake = 1;
                    sv4_copy(&payload->last[i], target);
                }
            }
        } else if (w->kind == W_MIXED && w->payload.rare) {
            llg_wait_mixed_payload_t* payload = &w->payload.rare->mixed;
            for (int i = 0; i < payload->n; i++) {
                if (payload->specs[i].sig == target) {
                    if (ev_matches(payload->last[i], *target,
                                   payload->specs[i].kind))
                        wake = 1;
                    sv4_copy(&payload->last[i], target);
                }
            }
        } else if (w->kind == W_DEPS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            for (int i = 0; i < payload->n; i++) {
                const llg_wait_dependency_t* dependency = &payload->dependencies[i];
                if (dependency->sig == target) {
                    if (dependency->width) {
                        sv4_t value = sv4_part_select(dependency->value ? *dependency->value : *target,
                            (int64_t)dependency->lsb + dependency->width - 1, dependency->lsb);
                        if (!sv4_same(payload->last[i], value)) wake = 1;
                        sv4_move(&payload->last[i], &value);
                    } else wake = 1;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->payload.expression.n; i++) {
                if (expression_update(w, i, target, NULL)) wake = 1;
            }
        } else if (w->kind == W_LEVEL) {
            llg_wait_level_payload_t* level = &w->payload.rare->level;
            if (level->sig == target && sv4_same(*target, level->value)) wake = 1;
        }
        if (wake) wake_proc(w->proc);
    }
    if (source) wait_subscription_unlink(&cursor);
    deferred_trigger_source_change(target, NULL);
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->target == target) llg_dependency_changed(binding->dependency);
    }
    fixed_array_changed(target);
    force_dependency_changed(target, NULL, 0);
}

static void sig_write(sv4_t* target, sv4_t value) {
    if (region_is_read_only_now(g.current_region)) {
        if (region_private_store("signal write")) sv4_copy(target, &value);
        return;
    }
    if (llg_sv4_width(*target) == llg_sv4_width(value) && sv4_same(*target, value)) return;
    // Callbacks can finish/disable the writer without returning through here.
    // Heap-backed registered owners survive both suspension and stack discard.
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = llg_value_scope_begin(2);
    sv4_t* owned = llg_value_scope_values(snapshots);
    sv4_copy(&owned[0], &value);
    sv4_copy(&owned[1], target);
    value = owned[0]; /* Borrows the registered snapshot until scope end. */
    sv4_t old = owned[1];
    sv4_copy(target, &value);
    sig_publish_changed(target, old, value, value);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
}

static int sig_range_same(const sv4_t* target, uint32_t offset,
                          const sv4_t* value) {
    return llg_sv4_range_same(*target, offset, *value);
}

static void sig_range_copy(sv4_t* target, uint32_t offset,
                           const sv4_t* value) {
    llg_sv4_range_copy(target, offset, *value);
}

// Publish a changed slice without cloning the full packed signal. Waiters
// still subscribe to the canonical full-net address: full-value and selected
// dependency checks below read the already-updated target exactly as before.
static int sig_write_ranges(sv4_t* target,
                            uint32_t first_offset, sv4_t first,
                            uint32_t second_offset, sv4_t second,
                            int has_second) {
    int private_store = 0;
    if (region_is_read_only_now(g.current_region)) {
        if (!region_private_store("signal write")) return 0;
        private_store = 1;
    }
    if (!llg_sv4_width(first) || first_offset > llg_sv4_width(*target) ||
        llg_sv4_width(first) > llg_sv4_width(*target) - first_offset ||
        (has_second &&
         (!llg_sv4_width(second) || second_offset > llg_sv4_width(*target) ||
          llg_sv4_width(second) > llg_sv4_width(*target) - second_offset))) {
        fputs("llg: fatal: invalid signal write range\n", stderr);
        abort();
    }
    int first_changed = !sig_range_same(target, first_offset, &first);
    int second_changed = has_second &&
        !sig_range_same(target, second_offset, &second);
    if (!first_changed && !second_changed) return 0;
    if (private_store) {
        if (first_changed) sig_range_copy(target, first_offset, &first);
        if (second_changed) sig_range_copy(target, second_offset, &second);
        return 0;
    }
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = llg_value_scope_begin(
#ifdef LLG_WAVEFORM
        3
#else
        2
#endif
    );
    sv4_t* edges = llg_value_scope_values(snapshots);
    // Every directional packed edge rule consumes only the LSB. Publication
    // has already established that some bit changed, so these stable one-bit
    // snapshots also preserve any-change behavior through nested callbacks.
    sv4_replace(&edges[0], sv4_bit_select(*target, 0));
    if (first_changed) sig_range_copy(target, first_offset, &first);
    if (second_changed) sig_range_copy(target, second_offset, &second);
    sv4_replace(&edges[1], sv4_bit_select(*target, 0));
#ifdef LLG_WAVEFORM
    sv4_copy(&edges[2], target);
    sv4_t published = edges[2];
#else
    sv4_t published = SV4_EMPTY;
#endif
    sig_publish_changed(target, edges[0], edges[1], published);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
    return 1;
}

static int sig_write_range(sv4_t* target, uint32_t offset, sv4_t value) {
    return sig_write_ranges(target, offset, value, 0, (sv4_t)SV4_EMPTY, 0);
}

// Real equality is bitwise: repeated NaNs with the same payload are
// suppressed, while changes in NaN payload and signed zero are observable.
static void real_write(double* target, double value) {
    if (region_is_read_only_now(g.current_region)) {
        if (region_private_store("real write")) *target = value;
        return;
    }
    double old = *target;
    if (real_same(old, value)) return;
    /* Real locals have stable native owner slots, just like packed descriptors.
     * Keep the slot alive if a callback cancels its receiving process. */
    llg_value_scope_t* target_pin = value_target_pin(target);
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
    llg_wait_source_t* source = wait_source_find(target);
    llg_wait_subscription_t cursor = {0};
    if (source) {
        cursor.source = source;
        wait_subscription_insert(&cursor, &source->head);
    }
    llg_wait_t* w;
    while (source && (w = wait_source_next(&cursor)) != NULL) {
        int wake = 0;
        if (w->kind == W_DEPS) {
            llg_wait_expression_payload_t* payload = &w->payload.expression;
            for (int i = 0; i < payload->n; i++) {
                if (payload->dependencies[i].real == target) {
                    wake = 1;
                    break;
                }
            }
        } else if (w->kind == W_EXPR) {
            for (int i = 0; i < w->payload.expression.n; i++) {
                if (expression_update(w, i, NULL, target)) wake = 1;
            }
        }
        if (wake) wake_proc(w->proc);
    }
    if (source) wait_subscription_unlink(&cursor);
    deferred_trigger_source_change(NULL, target);
    for (llg_dependency_binding_t* binding = dependency_bucket_head(target);
         binding; binding = binding->bucket_next) {
        if (binding->real_target == target) llg_dependency_changed(binding->dependency);
    }
    force_dependency_changed(NULL, target, 1);
    if (target_pin) llg_value_scope_end(target_pin);
}

// ── Procedural force / release ───────────────────────────────────────────────

static sv4_t llg_net_compute(llg_net_t* net);
static void force_recompute_target(sv4_t* target, llg_net_t* net);
static void llg_net_alias_refresh_all(llg_net_t* net);
static void inertial_unlink_pending(llg_inertial_t* driver);
static void llg_net_strength_publish(llg_net_t* net);

// Grow one live-binding table to hold at least `needed` entries. The grown copy
// is completed before it replaces the old table, so an allocation failure
// aborts without a partially rebound registry. Only `used` entries are copied.
static void force_table_reserve(int needed) {
    if (needed <= g.force_capacity) return;
    int capacity = llg_registry_capacity(g.force_capacity, needed);
    llg_force_entry_t* grown = (llg_force_entry_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "force table");
    if (g.force_table)
        memcpy(grown, g.force_table, (size_t)g.force_count * sizeof(*grown));
    free(g.force_table);
    g.force_table = grown;
    g.force_capacity = capacity;
}

static void pca_table_reserve(int needed) {
    if (needed <= g.pca_capacity) return;
    int capacity = llg_registry_capacity(g.pca_capacity, needed);
    llg_pca_binding_t* grown = (llg_pca_binding_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "PCA table");
    if (g.pca_table)
        memcpy(grown, g.pca_table, (size_t)g.pca_count * sizeof(*grown));
    free(g.pca_table);
    g.pca_table = grown;
    g.pca_capacity = capacity;
}

static void pca_real_table_reserve(int needed) {
    if (needed <= g.pca_real_capacity) return;
    int capacity = llg_registry_capacity(g.pca_real_capacity, needed);
    llg_pca_real_binding_t* grown = (llg_pca_real_binding_t*)llg_checked_calloc(
        (size_t)capacity, sizeof(*grown), "real PCA table");
    if (g.pca_real_table)
        memcpy(grown, g.pca_real_table,
               (size_t)g.pca_real_count * sizeof(*grown));
    free(g.pca_real_table);
    g.pca_real_table = grown;
    g.pca_real_capacity = capacity;
}

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
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    sv4_replace(value, sv4_from_u64(active ? 1 : 0, llg_sv4_width(*enable), llg_sv4_signed(*enable)));
    sig_write(enable, *value);
    llg_value_scope_end(scope);
}

void llg_pca_assign(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding) {
        if (g.pca_count == INT_MAX) {
            fprintf(stderr, "llg runtime fatal: PCA binding count overflow\n");
            abort();
        }
        pca_table_reserve(g.pca_count + 1);
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
    sv4_replace(&binding->value, sv4_resize(value, llg_sv4_width(*target), llg_sv4_signed(*target)));
    binding->active = 1;
    pca_set_enable(enable, 1);
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_drive(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active || binding->enable != enable || binding->site != site)
        return;
    sv4_replace(&binding->value, sv4_resize(value, llg_sv4_width(*target), llg_sv4_signed(*target)));
    if (!llg_is_forced(target)) sig_write(target, binding->value);
}

void llg_pca_deassign(sv4_t* target) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_binding_t* binding = pca_binding(target);
    if (!binding || !binding->active) return;
    binding->active = 0;
    sv4_destroy(&binding->value);
    pca_set_enable(binding->enable, 0);
}

void llg_pca_assign_d(double* target, sv4_t* enable, uint64_t site, double value) {
    if (!region_can_mutate("procedural continuous assignment")) return;
    llg_pca_real_binding_t* binding = pca_real_binding(target);
    if (!binding) {
        if (g.pca_real_count == INT_MAX) {
            fprintf(stderr, "llg runtime fatal: real PCA binding count overflow\n");
            abort();
        }
        pca_real_table_reserve(g.pca_real_count + 1);
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
    sv4_destroy(&entry->value);
    sv4_destroy_array(entry->masks, entry->masks ? (size_t)entry->n_parts : 0);
    free(entry->parts);
    free(entry->masks);
    free(entry->reads);
    memset(entry, 0, sizeof(*entry));
}

static sv4_t force_part_mask(const llg_force_part_t* part) {
    if (!part->target || llg_sv4_width(*part->target) >= LLG_SUPPORTED_WIDTH_LIMIT ||
        part->width >= LLG_SUPPORTED_WIDTH_LIMIT) {
        fprintf(stderr, "llg: invalid force target or width\n");
        abort();
    }
    sv4_t mask = sv4_from_u64(0, llg_sv4_width(*part->target), 0);
    sv4_t ones = sv4_fill(1, part->width, 0);
    sv4_part_select_set(&mask, part->left, part->right, ones);
    sv4_destroy(&ones);
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
                llg_sv4_mask_remove(mask, removed);
                sv4_destroy(&removed);
            }
            remains |= sv4_to_bool(*mask);
        }
        if (!remains) force_free_entry(entry);
    }
}

static int force_find_free_slot(void) {
    for (int i = 0; i < g.force_count; i++)
        if (!g.force_table[i].active) return i;
    if (g.force_count == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: force entry count overflow\n");
        abort();
    }
    force_table_reserve(g.force_count + 1);
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
    sv4_t selected = sv4_part_select(*value, (int64_t)high, (int64_t)part->value_lsb);
    if (part->two_state) sv4_replace(&selected, sv4_to_two_state(selected));
    sv4_t updated = sv4_clone(target);
    sv4_part_select_set(&updated, part->left, part->right, selected);
    llg_sv4_masked_merge(target, updated, *mask);
    sv4_destroy(&updated);
    sv4_destroy(&selected);
}

// Union of the active packed force masks on one target for one 64-bit word.
// Strength views report forced bits at strong strength.
static uint64_t force_mask_word(sv4_t* target, size_t word) {
    uint64_t forced = 0;
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        for (int j = 0; j < entry->n_parts; j++) {
            if (entry->parts[j].target != target) continue;
            forced |= llg_sv4_word(entry->masks[j], word, LLG_SV4_BITS) &
                ~(llg_sv4_word(entry->masks[j], word, LLG_SV4_X) |
                  llg_sv4_word(entry->masks[j], word, LLG_SV4_Z));
        }
    }
    return forced;
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
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    if (net) {
        sv4_replace(value, llg_net_compute(net));
    } else {
        llg_pca_binding_t* pca = pca_binding(target);
        sv4_copy(value, pca && pca->active ? &pca->value : target);
    }
    for (int i = 0; i < g.force_count; i++) {
        llg_force_entry_t* entry = &g.force_table[i];
        if (!entry->active || entry->is_real) continue;
        sv4_t streamed = entry->stream_slice
            ? sv4_unstream(entry->value, entry->stream_slice, entry->stream_right_to_left)
            : sv4_clone(&entry->value);
        for (int j = 0; j < entry->n_parts; j++) {
            llg_force_part_t* part = &entry->parts[j];
            if (part->target == target)
                force_apply_part(value, part, &entry->masks[j], &streamed);
        }
        sv4_destroy(&streamed);
    }
    sig_write(target, *value);
    if (net) {
        llg_net_alias_refresh_all(net);
        llg_net_strength_publish(net);
    }
    llg_value_scope_end(scope);
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
        if (entry->eval) {
            llg_value_scope_t* scope = llg_value_scope_begin(1);
            sv4_t* evaluated = llg_value_scope_values(scope);
            entry->eval(evaluated);
            sv4_move(&entry->value, evaluated);
            llg_value_scope_end(scope);
        }
        force_entry_targets(entry);
    }
    entry->evaluating = 0;
}

int llg_force_source_active(const sv4_t* sig, const double* real) {
    int is_real = real != NULL;
    for (int i = 0; i < g.force_count; i++) {
        const llg_force_entry_t* entry = &g.force_table[i];
        if (entry->active &&
            force_read_matches(entry, (sv4_t*)sig, (double*)real, is_real))
            return 1;
    }
    return 0;
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
        sig, NULL, (int64_t)llg_sv4_width(*sig) - 1, 0, llg_sv4_width(*sig), 0, 0
    };
    llg_force_entry_t* entry = force_prepare_packed(&part, 1, 0, 0, NULL, NULL, 0);
    sv4_copy(&entry->value, &value);
    force_entry_targets(entry);
}

void llg_release(sv4_t* sig) {
    if (!sig) return;
    llg_force_part_t part = {
        sig, NULL, (int64_t)llg_sv4_width(*sig) - 1, 0, llg_sv4_width(*sig), 0, 0
    };
    llg_release_parts(&part, 1, 0, 0);
}

// ── Public scheduler API ──────────────────────────────────────────────────────

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
    cancel_proc_nbas(p);
    remove_waiters_entry(&p->wait);
    event_unlink(&p->wait);
    event_triggered_unlink(&p->wait);
    semaphore_waiter_unlink(&p->wait);
    if (p->wait.kind == W_MAILBOX_GET || p->wait.kind == W_MAILBOX_PUT)
        mailbox_unlink_wait(&p->wait);
    wait_payload_release(&p->wait);
    value_scopes_unwind(p);
    activation_unwind_proc(p);
    llg_frame_release(p->frame);
    p->frame = NULL;
    process_local_release_all(p);
    process_handle_shutdown(p);
    free_proc_record(p);
}

static void clocking_copy_observed(void* data);

static void free_region_callbacks(void) {
    while (g.callbacks) {
        llg_region_callback_t* next = g.callbacks->next;
        if (g.callbacks->callback == deferred_assertion_callback)
            free_deferred_assertion_report(
                (llg_deferred_assertion_report_t*)g.callbacks->data);
        else if (g.callbacks->callback == clocking_copy_observed)
            free(g.callbacks->data);
        free(g.callbacks);
        g.callbacks = next;
    }
}

static void free_sampled_values(void) {
    while (g.sampled) {
        llg_sampled_value_t* next = g.sampled->next;
        while (g.sampled->history) {
            llg_sampled_history_t* history = g.sampled->history;
            g.sampled->history = history->next;
            sv4_destroy(&history->value);
            free(history);
        }
        sv4_destroy(&g.sampled->value);
        free(g.sampled);
        g.sampled = next;
    }
    while (g.sampled_domains) {
        llg_sampled_domain_t* next = g.sampled_domains->next;
        while (g.sampled_domains->history) {
            llg_sampled_domain_history_t* history = g.sampled_domains->history;
            g.sampled_domains->history = history->next;
            sv4_destroy(&history->value);
            free(history);
        }
        sv4_destroy(&g.sampled_domains->initial);
        free(g.sampled_domains);
        g.sampled_domains = next;
    }
}

static void free_assertion_clock_events(llg_concurrent_assertion_t* assertion) {
    while (assertion && assertion->clock_events) {
        llg_assertion_clock_event_t* next = assertion->clock_events->next;
        free(assertion->clock_events);
        assertion->clock_events = next;
    }
    if (assertion) {
        assertion->clock_events_tail = NULL;
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            free(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
}

static void sequence_attempt_discard(llg_sequence_attempt_t* attempt);

static void free_assertion_attempts(llg_concurrent_assertion_t* assertion) {
    free_assertion_clock_events(assertion);
    while (assertion->attempts) {
        llg_assertion_attempt_t* next = assertion->attempts->next;
        free(assertion->attempts);
        assertion->attempts = next;
    }
    assertion->attempts_tail = NULL;
    llg_sequence_attempt_t** lists[] = {
        &assertion->sequence_antecedents,
        &assertion->sequence_consequents,
    };
    llg_sequence_attempt_t** tails[] = {
        &assertion->sequence_antecedents_tail,
        &assertion->sequence_consequents_tail,
    };
    for (size_t list_index = 0; list_index < sizeof(lists) / sizeof(lists[0]);
         list_index++) {
        while (*lists[list_index]) {
            llg_sequence_attempt_t* attempt = *lists[list_index];
            *lists[list_index] = attempt->next;
            sequence_attempt_discard(attempt);
        }
        *tails[list_index] = NULL;
    }
}

static void free_assertions(void) {
    while (g.assertions) {
        llg_concurrent_assertion_t* next = g.assertions->next;
        free_assertion_attempts(g.assertions);
        free(g.assertions);
        g.assertions = next;
    }
    g.assertion_tail = NULL;
}

static void free_clocking_edges(void) {
    while (g.clocking_edges) {
        llg_clocking_edge_t* next = g.clocking_edges->next;
        free(g.clocking_edges);
        g.clocking_edges = next;
    }
    free(g.clocking_index);
    g.clocking_index = NULL;
    g.clocking_capacity = 0;
    g.clocking_count = 0;
    g.clocking_used = 0;
}

static void free_clocking_drives(void) {
    while (g.clocking_drives) {
        llg_clocking_drive_t* next = g.clocking_drives->next;
        free_clocking_drive(g.clocking_drives);
        g.clocking_drives = next;
    }
    g.clocking_drives_tail = NULL;
}

static void free_q_queues(void) {
    while (g.q_queues) {
        llg_q_queue_t* queue = g.q_queues;
        g.q_queues = queue->next;
        while (queue->head) {
            llg_q_entry_t* entry = queue->head;
            queue->head = entry->next;
            free(entry);
        }
        free(queue);
    }
}

static void free_mailboxes(void) {
    while (g.mailboxes) {
        llg_mailbox_t* mailbox = g.mailboxes;
        g.mailboxes = mailbox->next;
        while (mailbox->head) {
            llg_mailbox_message_t* message = mailbox_message_pop(mailbox);
            mailbox_value_destroy(&message->value);
            free(message);
        }
        free(mailbox);
    }
}

void llg_rt_cleanup(void) {
    value_scopes_unwind(NULL);
    for (int i = 0; i < g.pca_count; ++i) sv4_destroy(&g.pca_table[i].value);
    while (root_reference_top) llg_ref_scope_end(root_reference_top);
    for (int i = 0; i < g.force_count; i++) force_free_entry(&g.force_table[i]);
    while (g.inertial_drivers) {
        llg_inertial_t* driver = g.inertial_drivers;
        g.inertial_drivers = driver->next_all;
        *driver->handle = NULL;
        sv4_destroy(&driver->current);
        sv4_destroy(&driver->value);
        sv4_destroy(&driver->mask);
        free(driver);
    }
    free_all_nbas();
    free_deferred_triggers();
    free_deferred_assertions();
    free_assertion_rules();
    // Groups own only child-list nodes; process objects are owned once by
    // all_procs and are released separately below.
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        if (p && p->fork_groups) {
            free_group_storage(p->fork_groups);
            p->fork_groups = NULL;
            p->fork_groups_tail = NULL;
            p->pending_fork_groups = NULL;
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
            sv4_destroy_array(g.strobes->work, (size_t)g.strobes->n);
            free(g.strobes->work);
        }
        free(g.strobes);
        g.strobes = next;
    }
    g.strobe_tail = NULL;
    free(g.mon.fmt);
    if (g.mon.last) sv4_destroy_array(g.mon.last, (size_t)g.mon.n);
    if (g.mon.work) sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
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
    free_assertions();
    free_clocking_edges();
    free_clocking_drives();
    free_q_queues();
    llg_string_destroy(&g.time_format.suffix);

    for (int i = 0; i < g.n_procs; i++) {
        if (g.all_procs[i]) free_proc_storage(g.all_procs[i]);
    }
    for (int level = 0; level < g.proc_free_levels; level++)
        free(g.proc_free_bits[level]);
    free(g.wait_sources);
    free(g.all_procs);
    g.all_procs = NULL;
    g.all_procs_capacity = 0;
    free(g.force_table);
    g.force_table = NULL;
    g.force_capacity = 0;
    free(g.pca_table);
    g.pca_table = NULL;
    g.pca_capacity = 0;
    free(g.pca_real_table);
    g.pca_real_table = NULL;
    g.pca_real_capacity = 0;
    free_mailboxes();
    reap_retired_procs();
    while (g.programs) {
        llg_program_t* next = g.programs->next;
        free(g.programs);
        g.programs = next;
    }
    while (g.semaphores) {
        llg_semaphore_t* semaphore = g.semaphores;
        g.semaphores = semaphore->next_all;
        while (semaphore->wait_head) {
            llg_semaphore_wait_t* next = semaphore->wait_head->next;
            free(semaphore->wait_head);
            semaphore->wait_head = next;
        }
        semaphore->wait_tail = NULL;
        free(semaphore);
    }
    // External HDL references may keep terminal process identities alive, but
    // no handle may retain a pointer into the context being reset below.
    llg_process_handle_t* handle = g.process_handles;
    while (handle) {
        llg_process_handle_t* next = handle->next;
        handle->linked = 0;
        handle->prev_link = NULL;
        handle->next = NULL;
        handle = next;
    }
    g.process_handles = NULL;
    if (!llg_file_defer_cleanup) llg_file_cleanup();
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
    free(llg_dependency_buckets);
    llg_dependency_buckets = NULL;
    llg_dependency_bucket_count = 0;
    llg_dependency_binding_count = 0;
    llg_rt_co_cache_release();
}

void llg_rt_init_with_args_and_precision(int argc, char** argv,
                                         uint64_t precision_fs) {
    llg_clear_final_timeformat();
    llg_rt_cleanup();
    llg_warn_host_stack_limit();
    llg_last_failure = 0;
    llg_last_config_error = 0;
    memset(llg_severity_counts, 0, sizeof(llg_severity_counts));
    memset(llg_assertion_failure_counts, 0, sizeof(llg_assertion_failure_counts));
    llg_assertion_cover_count = 0;
    llg_assertion_vacuous_total = 0;
    llg_assertion_event_order = 0;
    finals_release(); // a fresh run never inherits final registrations
    llg_n_finals = 0;
    if (precision_fs == 0) {
        fprintf(stderr, "llg: runtime precision must be non-zero\n");
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_timeformat_defaults(precision_fs);
    if (!configure_limits() || !configure_stop_policy() || !configure_output_files()) {
        llg_last_failure = 1;
        llg_last_config_error = 1;
        g.config_error = 1;
        return;
    }
    llg_configured_zero_loop_limit = g.zero_loop_limit;
    llg_configured_process_step_limit = g.process_step_limit;
    llg_configured_stop_policy = g.stop_policy;
    g.initialized = 1;
    g.current_region = LLG_REGION_PREPONED;
    llg_rng_state_seed(&g.rng_root, LLG_RNG_DEFAULT_SEED);
    g.argc = argc > 0 ? argc : 0;
    g.argv = g.argc > 0 ? argv : NULL;
}

void llg_rt_init_with_args(int argc, char** argv) {
    llg_rt_init_with_args_and_precision(argc, argv, 1);
}

void llg_rt_init_with_precision(uint64_t precision_fs) {
    llg_rt_init_with_args_and_precision(0, NULL, precision_fs);
}

void llg_rt_init(void) {
    llg_rt_init_with_args_and_precision(0, NULL, 1);
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
    if (value) llg_sv4_set_state(value, index, (unsigned)state);
}

static void llg_plusarg_mask_top(sv4_t* value) {
    if (value) llg_sv4_mask_top(value);
}

static void llg_plusarg_unknown(sv4_t* value) {
    if (value) sv4_replace(value, sv4_x(llg_sv4_width(*value), llg_sv4_signed(*value)));
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
        negative = text[start++] == '-';
    }
    int digits = 0, unknown = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        if (llg_plusarg_unknown_digit(text[i])) unknown = 1;
        else if (llg_plusarg_digit((unsigned char)text[i], 10) < 0) return 0;
        digits = 1;
    }
    if (!digits) return 0;
    if (unknown) { llg_plusarg_unknown(output); return 1; }
    // The enclosing parser supplies a zeroed, exact-destination-width owner.
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        llg_sv4_mul_add_known(output, 10, (uint32_t)(text[i] - '0'));
    }
    if (negative) llg_sv4_negate_known(output);
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
        llg_sv4_negate_known(output);
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
    sv4_replace(output, sv4_zero(llg_sv4_width(*output), llg_sv4_signed(*output)));
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
            for (size_t i = length; i > 0 && bit < llg_sv4_width(*output); --i) {
                unsigned char byte = (unsigned char)text[i - 1];
                for (int j = 0; j < 8 && bit + (uint32_t)j < llg_sv4_width(*output); ++j) {
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
            sv4_replace(output, sv4_from_real(real, llg_sv4_width(*output), llg_sv4_signed(*output)));
            return 1;
        }
        default: return 0;
    }
}

static void llg_plusarg_to_two_state(sv4_t* value) {
    if (!value) return;
    llg_sv4_two_state_inplace(value);
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
    if (!out || width == 0 || width >= LLG_SUPPORTED_WIDTH_LIMIT) return 0;
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
        sv4_move(out, &parsed);
    }
    sv4_destroy(&parsed);
    llg_plusarg_format_free(&format);
    return converted;
}

// Parse only the magnitude required by this argument. Leading zeroes consume no
// limb storage, and base-2 input does not reserve four bits for every digit.
// Accumulate integers exactly before applying the packed-to-real rounding order.
static int llg_plusarg_integral_real(const char* text, size_t length,
                                     char conversion, double* output) {
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' ? 16u : 10u;
    size_t start = 0;
    int negative = length && text[0] == '-';
    if (length && (text[0] == '-' || text[0] == '+')) ++start;
    if (base != 10u && length - start >= 2 && text[start] == '0' &&
        ((base == 2u && (text[start + 1] == 'b' || text[start + 1] == 'B')) ||
         (base == 8u && (text[start + 1] == 'o' || text[start + 1] == 'O')) ||
         (base == 16u && (text[start + 1] == 'x' || text[start + 1] == 'X'))))
        start += 2;
    int digits = 0, unknown = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        if (llg_plusarg_unknown_digit((unsigned char)text[i])) unknown = 1;
        else if (llg_plusarg_digit((unsigned char)text[i], (int)base) < 0) return 0;
        digits = 1;
    }
    if (!digits) return 0;
    if (unknown && (negative || base == 10u)) { *output = 0.0; return 1; }
    uint64_t* limbs = NULL;
    size_t used = 0, capacity = 0;
    const size_t maximum = (LLG_SUPPORTED_WIDTH_LIMIT - 1u + 63u) / 64u;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        int digit = llg_plusarg_digit((unsigned char)text[i], (int)base);
        uint64_t carry = digit < 0 ? 0u : (unsigned)digit;
        for (size_t limb = 0; limb < used; ++limb) {
            uint64_t low = (limbs[limb] & UINT32_MAX) * base + carry;
            uint64_t high = (limbs[limb] >> 32) * base + (low >> 32);
            limbs[limb] = (high << 32) | (low & UINT32_MAX);
            carry = high >> 32;
        }
        if (carry) {
            if (used == maximum) {
                free(limbs);
                llg_fatal_allocation("integral plusarg width", used + 1u, 64u);
            }
            if (used == capacity) {
                size_t next = capacity ? capacity * 2u : 1u;
                if (next > maximum) next = maximum;
                uint64_t* replacement = llg_checked_malloc(next, sizeof(*limbs), "integral plusarg limbs");
                if (used) memcpy(replacement, limbs, used * sizeof(*limbs));
                free(limbs);
                limbs = replacement;
                capacity = next;
            }
            limbs[used++] = carry;
        }
        if (used == maximum && (limbs[used - 1u] >> 63u)) {
            free(limbs);
            llg_fatal_allocation("integral plusarg width", LLG_SUPPORTED_WIDTH_LIMIT, 1u);
        }
    }
    double result = 0.0;
    for (size_t i = used; i > 0; --i) result = ldexp(result, 64) + (double)limbs[i - 1u];
    free(limbs);
    *output = negative && result != 0.0 ? -result : result;
    return 1;
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
            converted = llg_plusarg_integral_real(
                value, value_len, format.conversion, out);
            if (!converted) *out = 0.0;
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
    llg_string_t result = {0};
    if (converted) result = llg_string_bytes(value, value_len);
    llg_plusarg_format_free(&format);
    /* No parser-owned buffers may remain across a notifying string write. */
    if (converted) llg_string_move(out, result);
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
        if (llg_severity_counts[LLG_SEVERITY_INFO] != 0 ||
            llg_severity_counts[LLG_SEVERITY_WARNING] != 0 ||
            llg_severity_counts[LLG_SEVERITY_ERROR] != 0 ||
            llg_severity_counts[LLG_SEVERITY_FATAL] != 0) {
            fprintf(stderr,
                    "llg: severity counts: info=%llu warning=%llu error=%llu fatal=%llu\n",
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_INFO],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_WARNING],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_ERROR],
                    (unsigned long long)llg_severity_counts[LLG_SEVERITY_FATAL]);
        }
        if (llg_assertion_failure_counts[LLG_ASSERTION_ASSERT] != 0 ||
            llg_assertion_failure_counts[LLG_ASSERTION_ASSUME] != 0 ||
            llg_assertion_failure_counts[LLG_ASSERTION_EXPECT] != 0 ||
            llg_assertion_cover_count != 0) {
            if (llg_assertion_failure_counts[LLG_ASSERTION_EXPECT] != 0) {
                fprintf(stderr,
                        "llg: assertion counts: assert_failed=%llu assume_failed=%llu expect_failed=%llu cover=%llu\n",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_EXPECT],
                        (unsigned long long)llg_assertion_cover_count);
            } else {
                fprintf(stderr,
                        "llg: assertion counts: assert_failed=%llu assume_failed=%llu cover=%llu\n",
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSERT],
                        (unsigned long long)llg_assertion_failure_counts[LLG_ASSERTION_ASSUME],
                        (unsigned long long)llg_assertion_cover_count);
            }
        }
        if (llg_assertion_vacuous_total != 0)
            fprintf(stderr, "llg: assertion vacuous=%llu\n",
                    (unsigned long long)llg_assertion_vacuous_total);
    }
}

void llg_rt_finish_with_level(int verbosity, const char* location) {
    llg_proc_t* current = llg_current();
    llg_runtime_service_enter(current, "$finish");
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $finish verbosity %d\n", verbosity);
        abort();
    }
    run_deferred_assertions_now();
    report_finish(verbosity, location);
    g.finish = 1;
    if (current) current->chain.exiting = LLG_EXIT_COMPLETE;
}

void llg_rt_finish(void) {
    llg_rt_finish_with_level(0, NULL);
}

void llg_rt_request_finish(void) {
    g.finish = 1;
    if (g.current) g.current->chain.exiting = LLG_EXIT_COMPLETE;
}

int llg_rt_exiting(void) {
    return g.current && g.current->chain.exiting != LLG_EXIT_NONE;
}

void llg_rt_mark_failed(void) {
    llg_last_failure = 1;
}

static void report_stop(int verbosity, const char* location) {
    if (verbosity >= 1) {
        fprintf(stderr, "llg: $stop at time %llu",
                (unsigned long long)g.now);
        if (location && location[0] != '\0') fprintf(stderr, " at %s", location);
        fputc('\n', stderr);
    }
    if (verbosity >= 2) {
        fprintf(stderr, "llg: simulation statistics: processes=%d\n", g.n_procs);
    }
}

static int resume_stopped_process(void) {
    if (!g.suspended) return 0;
    if (!g.stop_proc) {
        g.suspended = 0;
        return 1;
    }
    llg_proc_t* process = g.stop_proc;
    if (process->killed || process->completed) {
        fprintf(stderr, "llg runtime fatal: stopped process is no longer resumable\n");
        llg_last_failure = 1;
        g.finish = 1;
        g.suspended = 0;
        g.stop_proc = NULL;
        return 0;
    }
    g.stop_proc = NULL;
    g.suspended = 0;
    g.current_region = g.stop_region;
    enqueue_region(process, g.stop_region);
    return 1;
}

llg_co_arm_t llg_arm_stop(llg_proc_t* self, int verbosity,
                          const char* location) {
    llg_runtime_service_enter(self, "$stop");
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $stop verbosity %d\n", verbosity);
        abort();
    }
    if (!g.running || !g.process_turn_active || !self || self != g.current ||
        g.suspended || g.stop_proc) {
        fprintf(stderr, "llg runtime fatal: $stop requires a running simulation process\n");
        llg_last_failure = 1;
        g.finish = 1;
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    report_stop(verbosity, location);
    g.stop_proc = self;
    g.stop_region = self->region;
    g.suspended = 1;
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

void llg_rt_request_stop(int verbosity, const char* location) {
    if (verbosity < 0 || verbosity > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $stop verbosity %d\n", verbosity);
        abort();
    }
    report_stop(verbosity, location);
    if (!g.running || !g.process_turn_active || !g.current) {
        fprintf(stderr,
                "llg: warning: deferred $stop outside a process turn ignored\n");
        return;
    }
    g.deferred_stop = 1;
}

int llg_rt_set_stop_policy(int policy) {
    if (policy != LLG_STOP_POLICY_RESUME && policy != LLG_STOP_POLICY_EXIT) return 0;
    if (g.running) return 0;
    llg_stop_policy_override = 1;
    llg_configured_stop_policy = policy;
    g.stop_policy = policy;
    return 1;
}

int llg_rt_stop_policy(void) {
    return g.initialized ? g.stop_policy : llg_configured_stop_policy;
}

int llg_rt_is_suspended(void) { return g.suspended != 0; }

int llg_rt_resume(void) {
    if (g.running) return 0;
    return resume_stopped_process();
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
    return llg_schedule_region_callback_id(region, callback, data, ticks, NULL);
}

int llg_schedule_region_callback_id(llg_region_t region,
                                    llg_region_callback_fn callback,
                                    void* data, uint64_t ticks,
                                    llg_region_callback_id_t* id) {
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
    if (id) {
        id->generation = llg_event_generation;
        id->sequence = entry->sequence;
    }
    insert_region_callback(entry);
    return 1;
}

int llg_cancel_region_callback(llg_region_callback_id_t id) {
    if (id.generation != llg_event_generation) return 0;
    for (llg_region_callback_t** slot = &g.callbacks; *slot;
         slot = &(*slot)->next) {
        if ((*slot)->sequence != id.sequence) continue;
        llg_region_callback_t* entry = *slot;
        *slot = entry->next;
        free(entry);
        return 1;
    }
    return 0;
}

int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback_after(region, callback, data, 0);
}

int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data) {
    return llg_schedule_region_callback(region, callback, data);
}

static llg_sampled_value_t* find_sampled_value(const sv4_t* signal) {
    if (!signal) return NULL;
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        if (item->signal == signal) return item;
    }
    return NULL;
}

static void report_unregistered_sampled_signal(void) {
    fprintf(stderr, "llg: sampled value requested for an unregistered signal\n");
    llg_last_failure = 1;
    g.finish = 1;
}

static void sampled_record_write(sv4_t* signal) {
    llg_sampled_value_t* item = find_sampled_value(signal);
    if (!item) return;
    llg_sampled_history_t* last = item->history;
    if (last && last->time == g.now) {
        sv4_copy(&last->value, signal);
        return;
    }
    llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
        1, sizeof(*history), "sampled history");
    history->time = g.now;
    history->value = sv4_clone(signal);
    history->next = item->history;
    item->history = history;
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
    item->value = sv4_clone(signal);
    item->history = NULL;
    llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
        1, sizeof(*history), "sampled history");
    history->time = g.now;
    history->value = sv4_clone(signal);
    history->next = NULL;
    item->history = history;
    item->next = g.sampled;
    g.sampled = item;
}

const sv4_t* llg_sampled_value(const sv4_t* signal) {
    llg_sampled_value_t* item = find_sampled_value(signal);
    if (item) return &item->value;
    report_unregistered_sampled_signal();
    return NULL;
}

int llg_sampled_copy(const sv4_t* signal, sv4_t* out) {
    if (!out) return 0;
    const sv4_t* value = llg_sampled_value(signal);
    if (!value) return 0;
    sv4_copy(out, value);
    return 1;
}

static llg_sampled_domain_t* find_sampled_domain(uint64_t identity) {
    for (llg_sampled_domain_t* domain = g.sampled_domains; domain;
         domain = domain->next) {
        if (domain->identity == identity) return domain;
    }
    return NULL;
}

static void report_missing_sampled_domain(uint64_t identity) {
    fprintf(stderr, "llg: sampled-value domain %llu is not registered\n",
            (unsigned long long)identity);
    llg_last_failure = 1;
    g.finish = 1;
}

int llg_sampled_domain_register(uint64_t identity, sv4_t* clock, int edge,
                                llg_sampled_domain_eval_fn value,
                                llg_sampled_domain_eval_fn gate, void* data) {
    if (!clock || !value || (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE)) {
        fprintf(stderr, "llg: invalid sampled-value domain registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    if (find_sampled_domain(identity)) {
        fprintf(stderr, "llg: duplicate sampled-value domain %llu\n",
                (unsigned long long)identity);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_sampled_domain_t* domain = (llg_sampled_domain_t*)llg_checked_malloc(
        1, sizeof(*domain), "sampled-value domain");
    domain->identity = identity;
    domain->clock = clock;
    domain->edge = edge;
    domain->value = value;
    domain->gate = gate;
    domain->data = data;
    domain->initial = value(data);
    domain->history = NULL;
    domain->next = g.sampled_domains;
    g.sampled_domains = domain;
    return 1;
}

static void sampled_domain_clock_signal_changed(sv4_t* signal, sv4_t old,
                                                sv4_t value) {
    if (!signal) return;
    for (llg_sampled_domain_t* domain = g.sampled_domains; domain;
         domain = domain->next) {
        if (domain->clock != signal ||
            !ev_matches_changed(old, value, domain->edge))
            continue;
        if (domain->gate) {
            sv4_t gate = domain->gate(domain->data);
            int enabled = sv4_to_bool(gate);
            sv4_destroy(&gate);
            if (!enabled) continue;
        }
        llg_sampled_domain_history_t* history =
            (llg_sampled_domain_history_t*)llg_checked_malloc(
                1, sizeof(*history), "sampled-value domain history");
        history->time = g.now;
        history->sequence = g.sampled_domain_sequence++;
        history->value = domain->value(domain->data);
        history->next = domain->history;
        domain->history = history;
    }
}

sv4_t llg_sampled_domain_past(uint64_t identity, uint64_t ticks) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_missing_sampled_domain(identity);
        return sv4_x(1, 0);
    }
    if (ticks == 0) return sv4_clone(&domain->initial);
    // $past counts time steps strictly before the evaluating one in which
    // the clocking event occurred (IEEE 1800-2009 §16.9.3). A tick in the
    // current time step is not one of them, but the latest tick before it is,
    // as when procedural code evaluates $past between clock edges. Repeated
    // edges in one time step count once.
    llg_sampled_domain_history_t* history = domain->history;
    while (history && history->time == g.now) history = history->next;
    for (uint64_t index = 1; history && index < ticks; index++) {
        uint64_t time = history->time;
        while (history && history->time == time) history = history->next;
    }
    return sv4_clone(history ? &history->value : &domain->initial);
}

static int sampled_domain_lsb_one(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) != 0;
}

static int sampled_domain_lsb_zero(sv4_t value) {
    if (llg_sv4_width(value) == 0 || llg_sv4_word(value, 0, LLG_SV4_X) & 1ULL || llg_sv4_word(value, 0, LLG_SV4_Z) & 1ULL) return 0;
    return (llg_sv4_word(value, 0, LLG_SV4_BITS) & 1ULL) == 0;
}

int llg_sampled_domain_status(uint64_t identity, int kind) {
    llg_sampled_domain_t* domain = find_sampled_domain(identity);
    if (!domain) {
        report_missing_sampled_domain(identity);
        return 0;
    }
    sv4_t current = domain->history ? domain->history->value : domain->initial;
    sv4_t previous = domain->history && domain->history->next
                         ? domain->history->next->value
                         : domain->initial;
    switch (kind) {
        case 0: return sampled_domain_lsb_one(current) && !sampled_domain_lsb_one(previous);
        case 1: return sampled_domain_lsb_zero(current) && !sampled_domain_lsb_zero(previous);
        case 2: return sv4_same(current, previous);
        case 3: return !sv4_same(current, previous);
        default:
            fprintf(stderr, "llg: invalid sampled-value status kind %d\n", kind);
            llg_last_failure = 1;
            g.finish = 1;
            return 0;
    }
}

static void sample_preponed_values(void) {
    g.assertion_edges_pending = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
    }
    // The scheduler revisits PREPONED for zero-delay deltas in the same time
    // slot. #1step samples are fixed at the slot boundary and must not observe
    // values written by later active/NBA iterations.
    if (g.sampled_time_valid && g.sampled_time == g.now) return;
    g.sampled_time = g.now;
    g.sampled_time_valid = 1;
    for (llg_sampled_value_t* item = g.sampled; item; item = item->next) {
        sv4_copy(&item->value, item->signal);
        llg_sampled_history_t* last = item->history;
        if (last && last->time == g.now) {
            sv4_copy(&last->value, &item->value);
            continue;
        }
        llg_sampled_history_t* history = (llg_sampled_history_t*)llg_checked_malloc(
            1, sizeof(*history), "sampled history");
        history->time = g.now;
        history->value = sv4_clone(&item->value);
        history->next = item->history;
        item->history = history;
    }
}

typedef struct {
    sv4_t* source;
    sv4_t* sample;
} llg_clocking_observed_t;

static void clocking_copy_observed(void* data) {
    llg_clocking_observed_t* copy = (llg_clocking_observed_t*)data;
    sv4_copy(copy->sample, copy->source);
    free(copy);
}

int llg_clocking_sample_observed(sv4_t* source, sv4_t* sample) {
    if (!source || !sample) return 0;
    if (!find_sampled_value(source)) {
        report_unregistered_sampled_signal();
        return 0;
    }
    llg_clocking_observed_t* copy = (llg_clocking_observed_t*)llg_checked_malloc(
        1, sizeof(*copy), "clocking observed sample");
    copy->source = source;
    copy->sample = sample;
    if (!llg_schedule_region_callback(LLG_REGION_OBSERVED,
                                      clocking_copy_observed, copy)) {
        free(copy);
        return 0;
    }
    return 1;
}

int llg_clocking_sample_history(sv4_t* source, sv4_t* sample, uint64_t ticks) {
    if (!source || !sample) return 0;
    llg_sampled_value_t* item = find_sampled_value(source);
    if (!item) {
        report_unregistered_sampled_signal();
        return 0;
    }
    uint64_t target = g.now < ticks ? 0 : g.now - ticks;
    llg_sampled_history_t* selected = NULL;
    for (llg_sampled_history_t* history = item->history; history;
         history = history->next) {
        if (history->time > target) continue;
        if (!selected || selected->time < history->time) selected = history;
    }
    sv4_copy(sample, selected ? &selected->value : &item->value);
    return 1;
}

uint64_t llg_time_precision_fs(void) { return g.design_precision_fs; }

uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs) {
    if (unit_fs == 0) {
        fprintf(stderr, "llg runtime fatal: zero time unit\n");
        abort();
    }
    // Portable 64x64 -> 128 multiplication using 32-bit halves.
    uint64_t a0 = (uint32_t)g.now, a1 = g.now >> 32;
    uint64_t b0 = (uint32_t)precision_fs, b1 = precision_fs >> 32;
    uint64_t p0 = a0 * b0;
    uint64_t t = a1 * b0 + (p0 >> 32);
    uint64_t middle = (uint32_t)t;
    uint64_t high = a1 * b1 + (t >> 32);
    t = a0 * b1 + middle;
    high += t >> 32;
    uint64_t low = (t << 32) | (uint32_t)p0;
    if (high >= unit_fs) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    uint64_t quotient = 0, remainder = high;
    for (unsigned i = 64; i > 0; --i) {
        int overflow = (remainder >> 63) != 0;
        remainder = (remainder << 1) | ((low >> (i - 1)) & 1u);
        quotient <<= 1;
        if (overflow || remainder >= unit_fs) {
            remainder -= unit_fs;
            quotient |= 1u;
        }
    }
    if (remainder >= unit_fs - remainder) {
        if (quotient == UINT64_MAX) {
            fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
            abort();
        }
        ++quotient;
    }
    return quotient;
}

static void llg_timeformat_error(const char* message, llg_string_t* suffix) {
    fprintf(stderr, "llg: $timeformat %s\n", message);
    if (suffix) llg_string_destroy(suffix);
    llg_last_failure = 1;
    g.finish = 1;
}

void llg_timeformat(sv4_t units, sv4_t precision, llg_string_t suffix,
                    sv4_t minimum_field_width) {
    if (!region_can_mutate("$timeformat state update")) {
        llg_string_destroy(&suffix);
        return;
    }
    if (sv4_is_unknown(units) || !sv4_fits_i64(units)) {
        llg_timeformat_error("units must be a known signed integer", &suffix);
        return;
    }
    if (sv4_is_unknown(precision) || !sv4_fits_i64(precision)) {
        llg_timeformat_error("precision must be a known signed integer", &suffix);
        return;
    }
    if (sv4_is_unknown(minimum_field_width) ||
        !sv4_fits_i64(minimum_field_width)) {
        llg_timeformat_error("minimum field width must be a known signed integer",
                             &suffix);
        return;
    }
    int64_t units_value = sv4_to_i64(units);
    int64_t precision_value = sv4_to_i64(precision);
    int64_t width_value = sv4_to_i64(minimum_field_width);
    uint64_t unit_fs = units_value <= 0 ? llg_time_unit_from_exponent(units_value) : 0;
    if (!unit_fs) {
        llg_timeformat_error("units must be between -15 and 0", &suffix);
        return;
    }
    if (precision_value < 0 ||
        precision_value > (int64_t)LLG_TIMEFORMAT_MAX_PRECISION) {
        llg_timeformat_error("precision is out of range for the formatter", &suffix);
        return;
    }
    if (width_value < 0 || width_value > (int64_t)(LLG_SUPPORTED_WIDTH_LIMIT * 2u + 256u)) {
        llg_timeformat_error("minimum field width is out of range", &suffix);
        return;
    }
    llg_string_move(&g.time_format.suffix, suffix);
    g.time_format.unit_fs = unit_fs;
    g.time_format.precision = (int)precision_value;
    g.time_format.minimum_field_width = (int)width_value;
}

int llg_rt_process_count(void) {
    int count = 0;
    for (int i = 0; i < g.n_procs; i++)
        if (g.all_procs[i]) count++;
    return count;
}

static llg_proc_t* spawn_in_region(const llg_co_desc_t* desc,
                                   const char* name, llg_region_t region,
                                   llg_program_t* program, int is_initial) {
    if (g.config_error || !desc || !desc->fn || !region_valid(region)) return NULL;
    if (!callback_region_allowed(region, 0)) return NULL;
    if (desc->frame_size < sizeof(llg_co_frame_t) ||
        desc->frame_size > SIZE_MAX - sizeof(llg_proc_t)) {
        llg_rt_co_oom(desc->frame_size);
    }
    llg_proc_t* p = (llg_proc_t*)llg_checked_calloc(
        1, sizeof(*p) + desc->frame_size, "process and coroutine root frame");
#ifdef LLG_CO_DEBUG
    memset(LLG_CO_ROOT(&p->chain), LLG_CO_POISON_BYTE, desc->frame_size);
#endif
    p->name = name;
    llg_co_start(&p->chain, desc, p);
    p->program = is_initial ? program : NULL;
    p->program_live = program && is_initial;
    if (p->program_live) {
        if (program->live_initials == SIZE_MAX || g.program_processes == SIZE_MAX) {
            fprintf(stderr, "llg: program initial accounting overflow\n");
            abort();
        }
        program->had_initial = 1;
        program->live_initials++;
        g.program_processes++;
    }
    p->handle = process_handle_new(p);
    p->status = LLG_PROCESS_RUNNING;
    llg_rng_state_child(&g.rng_root, &p->rng);
    p->budget_time = g.now;
    p->region = region;
    register_proc(p);
    enqueue_region(p, region);
    return p;
}

llg_proc_t* llg_spawn_in_region(const llg_co_desc_t* desc,
                                const char* name, llg_region_t region) {
    return spawn_in_region(desc, name, region, NULL, 0);
}

llg_proc_t* llg_spawn_program_in_region(const llg_co_desc_t* desc,
                                         const char* name,
                                         llg_region_t region,
                                         uint64_t instance, int is_initial) {
    if (region != LLG_REGION_REACTIVE) {
        fprintf(stderr,
                "llg: program process must be spawned in a reactive region\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    llg_program_t* program = g.programs;
    while (program && program->instance != instance) program = program->next;
    if (!program) {
        program = llg_checked_calloc(1, sizeof(*program), "program origin");
        program->instance = instance;
        program->next = g.programs;
        g.programs = program;
    }
    if (program->closed) {
        fprintf(stderr, "llg: cannot spawn into a completed program\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    return spawn_in_region(desc, name, region, program, is_initial);
}

llg_proc_t* llg_spawn(const llg_co_desc_t* desc, const char* name) {
    return llg_spawn_in_region(desc, name, LLG_REGION_ACTIVE);
}

static llg_proc_t* initialize_instance(llg_proc_t* p,
    const llg_co_desc_t* desc, const void* record, size_t offset) {
    if (!p) return NULL;
    if (!record || offset < sizeof(llg_co_frame_t) ||
        desc->frame_size < sizeof(record) ||
        offset > desc->frame_size - sizeof(record)) {
        llg_rt_co_bad_state(LLG_CO_ROOT(&p->chain), "invalid root instance record offset");
    }
    memcpy((unsigned char*)LLG_CO_ROOT(&p->chain) + offset, &record, sizeof(record));
    return p;
}

llg_proc_t* llg_spawn_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, const void* record, size_t record_offset) {
    return initialize_instance(llg_spawn_in_region(desc, name, region),
                               desc, record, record_offset);
}

llg_proc_t* llg_spawn_program_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, uint64_t instance, int is_initial,
    const void* record, size_t record_offset) {
    return initialize_instance(llg_spawn_program_in_region(desc, name, region,
                                                          instance, is_initial),
                               desc, record, record_offset);
}

llg_frame_t* llg_proc_frame(llg_proc_t* self) {
    return self ? self->frame : NULL;
}

static void proc_complete(llg_proc_t* self) {
    if (!self || self->completed || self->killed) return;
    // Natural process termination is the other join_none eligibility
    // boundary.  Release children before unwinding the creator's activation
    // and frame; their copied captures remain retained by the child process.
    start_pending_fork_children(self);
    self->completed = 1;
    process_status_set(self, LLG_PROCESS_FINISHED);
    process_handle_terminal(self, LLG_PROCESS_FINISHED);
    value_scopes_unwind(self);
    activation_unwind_proc(self);
    llg_frame_release(self->frame);
    self->frame = NULL;
    process_local_release_all(self);
    if (self->grp) llg_fork_group_child_done(self->grp);
    release_program_process(self);
    service_program_completions();
    semaphore_service_cancelled_waiters();
}

llg_co_arm_t llg_arm_time(llg_proc_t* self, uint64_t ticks) {
    llg_runtime_service_enter(self, "delay");
    if (!self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_TIME;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr,
                "llg: fatal: simulation time overflow at %llu while scheduling a delay of %llu tick(s)\n",
                (unsigned long long)g.now, (unsigned long long)ticks);
        abort();
    }
    w->payload.timer.time = g.now + ticks;
    if (ticks == 0) {
        // `#0` yields into the INACTIVE region of the current time step
        // (LRM §4.4.2): it runs after the active region drains and before
        // the NBA region commits.
        w->resume_region = region_is_reactive(self->region)
                               ? LLG_REGION_RE_INACTIVE
                               : LLG_REGION_INACTIVE;
        insert_zero_wait(w, w->resume_region);
    } else {
        insert_timed(w);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
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

llg_co_arm_t llg_arm_any(llg_proc_t* self, sv4_t** sigs, int n) {
    llg_runtime_service_enter(self, "signal wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENTS;
    w->resume_region = take_wait_resume_region(self);
    if (n == 1 && wait_inline_fits(sigs[0])) {
        w->kind = W_EVENTS_INLINE;
        w->payload.single.specs[0] = (llg_event_spec_t){sigs[0], LLG_EV_ANY};
        wait_inline_copy(w, sigs[0]);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "event wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "event wait snapshots");
    for (int i = 0; i < n; i++) {
        payload->specs[i].sig = sigs[i];
        payload->specs[i].kind = LLG_EV_ANY;
        payload->last[i] = sv4_clone(sigs[i]);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_any_dependencies(llg_proc_t* self,
                                      const llg_wait_dependency_t* deps,
                                      int n) {
    llg_runtime_service_enter(self, "dependency wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_DEPS;
    w->resume_region = take_wait_resume_region(self);
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_wait_dependency_t), "typed event dependencies");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "packed-prefix wait snapshots");
    for (int i = 0; i < n; i++) {
        if ((deps[i].sig == NULL) == (deps[i].real == NULL)) {
            fprintf(stderr, "llg: typed wait dependency must name one storage kind\n");
            abort();
        }
        payload->dependencies[i] = deps[i];
        if (deps[i].width) {
            sv4_t* value = deps[i].value ? deps[i].value : deps[i].sig;
            if (!value || deps[i].real || deps[i].lsb >= llg_sv4_width(*value) ||
                deps[i].width > llg_sv4_width(*value) - deps[i].lsb) {
                fprintf(stderr, "llg: invalid packed-prefix wait dependency\n"); abort();
            }
            payload->last[i] = sv4_part_select(
                *value, (int64_t)deps[i].lsb + deps[i].width - 1,
                deps[i].lsb);
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_any_events(llg_proc_t* self,
                                const llg_event_spec_t* specs, int n) {
    llg_runtime_service_enter(self, "edge wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENTS;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    if (n == 1 && wait_inline_fits(specs[0].sig)) {
        w->kind = W_EVENTS_INLINE;
        w->payload.single.specs[0] = specs[0];
        wait_inline_copy(w, specs[0].sig);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
    llg_wait_expression_payload_t* payload = &w->payload.expression;
    payload->n = n;
    payload->specs = (llg_event_spec_t*)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_spec_t), "edge wait specifications");
    payload->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "edge wait snapshots");
    for (int i = 0; i < n; i++) {
        payload->specs[i].sig = specs[i].sig;
        payload->specs[i].kind = specs[i].kind;
        payload->last[i] = sv4_clone(specs[i].sig);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_edge(llg_proc_t* self, sv4_t* sig, int posedge) {
    llg_event_spec_t spec;
    spec.sig = sig;
    spec.kind = posedge ? LLG_EV_POSEDGE : LLG_EV_NEGEDGE;
    return llg_arm_any_events(self, &spec, 1);
}

llg_co_arm_t llg_arm_level(llg_proc_t* self, sv4_t* sig, sv4_t value) {
    llg_runtime_service_enter(self, "level wait");
    if (!self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_LEVEL;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    if (wait_inline_fits(&value)) {
        w->kind = W_LEVEL_INLINE;
        w->payload.single.specs[0] = (llg_event_spec_t){sig, LLG_EV_ANY};
        wait_inline_copy(w, &value);
        register_wait();
        return LLG_CO_ARM_SUSPEND;
    }
    llg_wait_level_payload_t* payload =
        &wait_rare_allocate(w, "level wait payload")->level;
    payload->sig = sig;
    sv4_copy(&payload->value, &value);
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

// ── Named events (see llg_rt.h) ──────────────────────────────────────────────

// Return the outcome of one event observed by a wait_order waiter:
//  1 completes the sequence, 0 keeps waiting, and -1 takes the failure arm.
// Repeated occurrences of already-consumed events are ignored, while an
// event that is still ahead in the sequence is an ordering violation.
static int event_order_match(llg_wait_t* w, llg_event_object_t* ev) {
    llg_wait_order_payload_t* order =
        w->payload.rare ? &w->payload.rare->order : NULL;
    if (!order || !order->sequence || order->next < 0 ||
        order->next >= order->n_order)
        return -1;
    if (order->sequence[order->next] == ev) {
        order->next++;
        return order->next == order->n_order ? 1 : 0;
    }
    for (int i = 0; i < order->next; i++) {
        if (order->sequence[i] == ev) return 0;
    }
    return -1;
}

static void event_trigger_object_unchecked(llg_event_object_t* ev) {
    if (!ev) return;

    // The state is tied to both the current simulation time and this runtime
    // generation. Comparing the generation avoids stale `.triggered` state
    // when a generated model is initialized again after cleanup; comparing
    // the time preserves all zero-delay deltas in the current slot.
    ev->triggered = 1;
    ev->triggered_time = g.now;
    ev->triggered_generation = llg_event_generation;
    ev->trigger_count++;
    clocking_drive_event_match(ev);

    // Snapshot and detach everyone first: wake_proc unlinks the waiter from
    // every event list it registered on, which must not fight the iteration
    // over this event's own table. Wake order is the snapshot order, i.e. the
    // current table order: deterministic, and equal to registration order
    // unless earlier partial unlinks (swap-with-last) reordered it. The
    // snapshots are heap scratch because the waiter tables grow without a
    // fixed ceiling.
    int n_triggered = ev->n_triggered_waiters;
    llg_proc_t** triggered = n_triggered
        ? (llg_proc_t**)llg_checked_malloc(
              (size_t)n_triggered, sizeof(*triggered),
              "triggered event wake snapshot")
        : NULL;
    if (triggered)
        memcpy(triggered, ev->triggered_waiters,
               (size_t)n_triggered * sizeof(*triggered));
    for (int i = 0; i < n_triggered; i++)
        triggered[i]->wait.payload.event.inline_slot = -1;
    ev->n_triggered_waiters = 0;
    for (int i = 0; i < n_triggered; i++) wake_proc(triggered[i]);
    free(triggered);

    int n = ev->n_waiters;
    llg_proc_t** wake = n
        ? (llg_proc_t**)llg_checked_malloc(
              (size_t)n, sizeof(*wake), "event wake snapshot")
        : NULL;
    if (wake)
        memcpy(wake, ev->waiters, (size_t)n * sizeof(*wake));
    for (int i = 0; i < n; i++) {
        int* slot = event_wait_slot(&wake[i]->wait, ev, i);
        if (!slot) abort();
        *slot = -1;
    }
    ev->n_waiters = 0;
    for (int i = 0; i < n; i++) {
        llg_wait_t* w = &wake[i]->wait;
        if (w->kind == W_EVENT_ORDER) {
            int result = event_order_match(w, ev);
            if (result != 0) {
                *w->payload.rare->order.result = result;
                wake_proc(wake[i]);
            } else {
                event_list_add(ev, wake[i], event_wait_slot(w, ev, -1));
            }
            continue;
        }
        int matched = w->kind != W_EXPR;
        if (!matched) {
            llg_wait_expression_payload_t* expression = &w->payload.expression;
            for (int j = 0; j < expression->n; j++) {
                if (expression->expressions[j].event_object == ev &&
                    expression_qualifies(&expression->expressions[j]))
                    matched = 1;
            }
        }
        if (matched) wake_proc(wake[i]);
        else event_list_add(ev, wake[i], event_wait_slot(w, ev, -1));
    }
    free(wake);
    deferred_trigger_event(ev);
}

static void event_trigger_object(llg_event_object_t* ev) {
    if (!region_can_mutate("event scheduling")) return;
    event_trigger_object_unchecked(ev);
}

static void clocking_event_callback(void* data) {
    llg_event_t* event = data;
    event_trigger_object_unchecked(event ? event->object : NULL);
}

int llg_clocking_event_observed(llg_event_t* event) {
    if (!event) return 0;
    // Event handles are runtime-owned until cleanup. Earlier Observed sample
    // callbacks are FIFO, so this callback sees the complete block publication.
    return llg_schedule_region_callback(LLG_REGION_OBSERVED, clocking_event_callback, event);
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

uint64_t llg_event_trigger_count(const llg_event_t* ev) {
    return ev && ev->object ? ev->object->trigger_count : 0;
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

llg_co_arm_t llg_arm_event(llg_proc_t* self, llg_event_t* ev) {
    const llg_event_t* list[1] = {ev};
    return llg_arm_events(self, list, 1);
}

llg_co_arm_t llg_arm_events(llg_proc_t* self,
                            const llg_event_t* const* evs, int n) {
    llg_runtime_service_enter(self, "event wait");
    if (n <= 0 || !self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->payload.event.n_evs = n;
    if (n == 1) {
        w->payload.event.evs = &w->payload.event.inline_ev;
        w->payload.event.event_slots = &w->payload.event.inline_slot;
        w->payload.event.inline_slot = -1;
    } else {
        w->payload.event.evs = event_wait_list_new(n, &w->payload.event.event_slots);
    }
    for (int i = 0; i < n; i++) {
        w->payload.event.evs[i] = evs[i] ? evs[i]->object : NULL;
        event_list_add(w->payload.event.evs[i], self, &w->payload.event.event_slots[i]);
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_event_triggered(llg_proc_t* self,
                                     const llg_event_t* ev) {
    llg_runtime_service_enter(self, "event triggered wait");
    if (!self || !region_can_mutate("wait scheduling") ||
        llg_event_triggered(ev))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT_TRIGGERED;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    w->payload.event.triggered_ev = ev ? ev->object : NULL;
    w->payload.event.inline_slot = -1;
    event_triggered_list_add(w->payload.event.triggered_ev, self);
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_assertion(llg_proc_t* self, uint64_t identity) {
    llg_runtime_service_enter(self, "assertion wait");
    if (!self || !region_can_mutate("expect scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_ASSERTION;
    // An assertion result is observed before Reactive actions, so resume the
    // procedural expect continuation in Reactive after its action callback
    // has been queued.
    w->resume_region = LLG_REGION_REACTIVE;
    wait_rare_allocate(w, "assertion wait payload")->assertion.identity = identity;
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_order(llg_proc_t* self,
                           const llg_event_t* const* evs, int n,
                           int* result) {
    llg_runtime_service_enter(self, "wait_order");
    if (n <= 0 || !result || !self || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    w->kind = W_EVENT_ORDER;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_order_payload_t* order =
        &wait_rare_allocate(w, "wait_order payload")->order;
    order->n_order = n;
    order->next = 0;
    *result = 0;
    order->result = result;
    order->sequence = (llg_event_object_t**)llg_checked_malloc(
        (size_t)n, sizeof(llg_event_object_t*), "wait_order sequence");
    order->evs = event_wait_list_new(n, &order->event_slots);
    order->n_evs = 0;
    for (int i = 0; i < n; i++) {
        llg_event_object_t* object = evs[i] ? evs[i]->object : NULL;
        order->sequence[i] = object;
        if (!object) continue;
        int seen = 0;
        for (int j = 0; j < order->n_evs; j++) {
            if (order->evs[j] == object) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            order->evs[order->n_evs] = object;
            event_list_add(object, self, &order->event_slots[order->n_evs++]);
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_mixed(llg_proc_t* self,
                           const llg_wait_src_t* srcs, int n) {
    llg_runtime_service_enter(self, "mixed wait");
    if (!self || n < 0 || !region_can_mutate("wait scheduling"))
        return LLG_CO_ARM_READY;
    llg_wait_t* w = &self->wait;
    int nsig = 0;
    int nev = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) nsig++;
        else nev++;
    }
    w->kind = W_MIXED;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_mixed_payload_t* mixed =
        &wait_rare_allocate(w, "mixed wait payload")->mixed;
    mixed->n = nsig;
    mixed->specs = nsig ? (llg_event_spec_t*)llg_checked_malloc(
        (size_t)nsig, sizeof(llg_event_spec_t), "mixed wait specifications") : NULL;
    mixed->last = nsig ? (sv4_t*)llg_checked_calloc(
        (size_t)nsig, sizeof(sv4_t), "mixed wait snapshots") : NULL;
    mixed->n_evs = nev;
    mixed->evs = nev ? event_wait_list_new(nev, &mixed->event_slots) : NULL;
    int si = 0;
    int ei = 0;
    for (int i = 0; i < n; i++) {
        if (srcs[i].sig) {
            mixed->specs[si].sig = srcs[i].sig;
            mixed->specs[si].kind = srcs[i].kind;
            mixed->last[si] = sv4_clone(srcs[i].sig);
            si++;
        } else {
            mixed->evs[ei] = srcs[i].ev ? srcs[i].ev->object : NULL;
            event_list_add(mixed->evs[ei], self, &mixed->event_slots[ei]);
            ei++;
        }
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_clocking_cycle(llg_proc_t* self,
                                    const llg_wait_src_t* srcs, int n,
                                    int accept_current) {
    llg_runtime_service_enter(self, "clocking cycle wait");
    if (!srcs || n <= 0 || !self ||
        !region_can_mutate("clocking cycle wait"))
        return LLG_CO_ARM_READY;
    if (accept_current && clocking_event_current(srcs, n))
        return LLG_CO_ARM_READY;
    return llg_arm_mixed(self, srcs, n);
}

llg_co_arm_t llg_arm_expressions(llg_proc_t* self,
                                 const llg_expr_event_spec_t* specs, int n) {
    if (n < 0) abort();
    llg_runtime_service_enter(self, "expression wait");
    if (!self || !region_can_mutate("wait scheduling")) {
        release_expression_contexts(specs, n);
        return LLG_CO_ARM_READY;
    }
    llg_wait_t* w = &self->wait;
    w->kind = W_EXPR;
    w->resume_region = region_is_reactive(self->region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    llg_wait_expression_payload_t* expression = &w->payload.expression;
    expression->n = n;
    expression->n_evs = 0;
    expression->expressions = (llg_expr_event_spec_t*)llg_checked_calloc(
        (size_t)n, sizeof(llg_expr_event_spec_t), "expression event descriptors");
    expression->last = (sv4_t*)llg_checked_calloc(
        (size_t)n, sizeof(sv4_t), "expression event snapshots");
    expression->real_last = (double*)llg_checked_malloc(
        (size_t)n, sizeof(double), "real expression event snapshots");
    expression->evs = event_wait_list_new(n, &expression->event_slots);
    for (int i = 0; i < n; i++) {
        if (specs[i].n_reads < 0 || specs[i].n_dependencies < 0) abort();
        expression->expressions[i] = specs[i];
        expression->expressions[i].event_object = specs[i].event ? specs[i].event->object : NULL;
        expression->expressions[i].reads = NULL;
        expression->expressions[i].dependencies = NULL;
        if (specs[i].n_reads) {
            if (!specs[i].reads) abort();
            expression->expressions[i].reads = (sv4_t**)llg_checked_malloc(
                (size_t)specs[i].n_reads, sizeof(sv4_t*), "expression dependencies");
            memcpy(expression->expressions[i].reads, specs[i].reads,
                   (size_t)specs[i].n_reads * sizeof(sv4_t*));
        }
        if (specs[i].n_dependencies) {
            if (!specs[i].dependencies) abort();
            expression->expressions[i].dependencies = (llg_wait_dependency_t*)llg_checked_malloc(
                (size_t)specs[i].n_dependencies, sizeof(llg_wait_dependency_t),
                "typed expression dependencies");
            for (int j = 0; j < specs[i].n_dependencies; j++) {
                if ((specs[i].dependencies[j].sig == NULL) ==
                    (specs[i].dependencies[j].real == NULL))
                    abort();
            }
            memcpy(expression->expressions[i].dependencies, specs[i].dependencies,
                   (size_t)specs[i].n_dependencies * sizeof(llg_wait_dependency_t));
        }
    }
    // Adopt every descriptor/context before invoking user code. An evaluator
    // can finish the process on its first call; teardown must also discover
    // contexts belonging to later entries which have not been evaluated yet.
    for (int i = 0; i < n; i++) {
        if (specs[i].event) {
            llg_event_object_t* object = specs[i].event->object;
            int seen = 0;
            for (int j = 0; j < expression->n_evs; j++)
                if (expression->evs[j] == object) seen = 1;
            if (!seen) {
                expression->evs[expression->n_evs] = object;
                event_list_add(object, self,
                               &expression->event_slots[expression->n_evs++]);
            }
        } else if (specs[i].real || specs[i].real_eval || specs[i].real_sig) {
            if (specs[i].real_eval)
                specs[i].real_eval(&expression->real_last[i], specs[i].eval_context);
            else if (specs[i].real_sig)
                expression->real_last[i] = *specs[i].real_sig;
            else abort();
        } else if (specs[i].eval) {
            specs[i].eval(&expression->last[i], specs[i].eval_context);
        } else if (specs[i].sig) {
            expression->last[i] = sv4_clone(specs[i].sig);
        } else {
            abort();
        }
        if (self->chain.exiting) break;
    }
    if (self->chain.exiting) {
        event_unlink(w);
        wait_payload_release(w);
        return LLG_CO_ARM_EXIT;
    }
    register_wait();
    return LLG_CO_ARM_SUSPEND;
}

uint64_t llg_repeat_count(sv4_t value) {
    sv4_t count = sv4_repeat_count(value);
    for (int i = 1; i < llg_sv4_nlimbs(llg_sv4_width(count)); i++) {
        if (llg_sv4_word(count, i, LLG_SV4_BITS)) {
            fprintf(stderr,
                    "llg runtime fatal: nonblocking repeat count exceeds 64 bits\n");
            abort();
        }
    }
    uint64_t result = sv4_to_u64(count);
    sv4_destroy(&count);
    return result;
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
            trigger->last[i] = sv4_clone(specs[i].sig);
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

static llg_nba_t* new_nba_in_region(uint64_t ticks, llg_region_t region) {
    if (!region_can_mutate("nonblocking scheduling")) return NULL;
    llg_proc_t* owner = g.in_deferred_action ? NULL : llg_current();
    if (ticks > UINT64_MAX - g.now || g.nba_sequence == UINT64_MAX) {
        fprintf(stderr, "llg: fatal: nonblocking assignment time or sequence overflow\n");
        abort();
    }
    llg_nba_t* n = (llg_nba_t*)llg_checked_calloc(1, sizeof(llg_nba_t), "nonblocking assignment");
    n->target = NULL;
    n->net_target = NULL;
    n->net_slot = -1;
    n->event_target = NULL;
    n->is_real = 0;
    n->is_event = 0;
    n->has_mask = 0;
    n->has_range = 0;
    n->real_target = NULL;
    n->real_value = 0.0;
    n->is_string = 0;
    n->string_target = NULL;
    n->string_value = (llg_string_t){0};
    n->tag_view = NULL;
    n->time = g.now + ticks;
    n->sequence = g.nba_sequence++;
    n->region = region;
    n->owner = owner;
    return n;
}

static llg_nba_t* new_nba(uint64_t ticks) {
    return new_nba_in_region(
        ticks,
        region_is_reactive(g.current_region) ? LLG_REGION_RE_NBA : LLG_REGION_NBA);
}

static llg_nba_t* new_clocking_nba(uint64_t ticks) {
    return new_nba_in_region(ticks, LLG_REGION_RE_NBA);
}

static void nba_queue_append(llg_nba_queue_t* queue, llg_nba_t* n) {
    n->queue_next = NULL;
    n->queue_prev = queue->tail;
    if (queue->tail) queue->tail->queue_next = n;
    else queue->head = n;
    queue->tail = n;
}

static void nba_queue_remove(llg_nba_queue_t* queue, llg_nba_t* n) {
    if (n->queue_prev) n->queue_prev->queue_next = n->queue_next;
    else queue->head = n->queue_next;
    if (n->queue_next) n->queue_next->queue_prev = n->queue_prev;
    else queue->tail = n->queue_prev;
    n->queue_next = NULL;
    n->queue_prev = NULL;
}

static void nba_owner_append(llg_proc_t* owner, llg_nba_t* n) {
    n->owner_next = NULL;
    n->owner_prev = owner->nba_tail;
    if (owner->nba_tail) owner->nba_tail->owner_next = n;
    else owner->nba_head = n;
    owner->nba_tail = n;
}

static void nba_owner_remove(llg_nba_t* n) {
    llg_proc_t* owner = n->owner;
    if (!owner) return;
    if (n->owner_prev) n->owner_prev->owner_next = n->owner_next;
    else owner->nba_head = n->owner_next;
    if (n->owner_next) n->owner_next->owner_prev = n->owner_prev;
    else owner->nba_tail = n->owner_prev;
    n->owner_next = NULL;
    n->owner_prev = NULL;
    n->owner = NULL;
}

static llg_nba_bucket_t* delayed_nba_bucket(uint64_t time) {
    llg_nba_bucket_t** slot = &g.delayed_nba_buckets;
    while (*slot && (*slot)->time < time) slot = &(*slot)->next;
    if (*slot && (*slot)->time == time) return *slot;
    llg_nba_bucket_t* bucket = (llg_nba_bucket_t*)llg_checked_calloc(
        1, sizeof(*bucket), "delayed nonblocking assignment bucket");
    bucket->time = time;
    bucket->next = *slot;
    *slot = bucket;
    return bucket;
}

static void enqueue_nba(llg_nba_t* n) {
    if (!n) return;
    if (n->time == g.now) {
        nba_queue_append(&g.nba_queues[n->region], n);
        if (n->owner) nba_owner_append(n->owner, n);
    } else {
        llg_nba_bucket_t* bucket = delayed_nba_bucket(n->time);
        nba_queue_append(&bucket->queues[n->region], n);
        // Future NBAs retain their destinations and values independently of
        // the issuing process. They intentionally do not join its cancellable
        // current-slot list.
        n->owner = NULL;
    }
}

static sv4_t nba_range_slice(uint32_t target_width, sv4_t value,
                             sv4_select_plan_t plan, int reverse,
                             uint32_t* offset, uint32_t* width) {
    if (plan.storage_width != target_width) {
        fputs("llg runtime fatal: selected NBA storage width mismatch\n", stderr);
        abort();
    }
    *offset = plan.storage_lsb;
    *width = plan.count;
    return sv4_select_plan_slice(value, &plan, reverse);
}

static void nba_capture_range(llg_nba_t* n, uint32_t target_width,
                              sv4_t value, sv4_select_plan_t plan,
                              int reverse) {
    n->value = nba_range_slice(target_width, value, plan, reverse,
                               &n->range_offset, &n->range_width);
    n->has_range = 1;
}

static int nba_mask_contiguous(uint32_t target_width, sv4_t value, sv4_t mask,
                               uint32_t* offset, uint32_t* width) {
    uint32_t limit = target_width;
    if (llg_sv4_width(value) < limit) limit = llg_sv4_width(value);
    if (llg_sv4_width(mask) < limit) limit = llg_sv4_width(mask);
    uint32_t first = limit;
    uint32_t last = 0;
    uint32_t count = 0;
    for (uint32_t bit = 0; bit < limit; ++bit) {
        if (!((llg_sv4_word(mask, bit / 64u, LLG_SV4_BITS) >> (bit % 64u)) & 1u)) continue;
        if (first == limit) first = bit;
        last = bit;
        ++count;
    }
    if (!count) {
        *offset = *width = 0;
        return 1;
    }
    if (count != last - first + 1) return 0;
    *offset = first;
    *width = count;
    return 1;
}

static void nba_capture_masked(llg_nba_t* n, uint32_t target_width,
                               sv4_t value, sv4_t mask) {
    uint32_t offset;
    uint32_t width;
    if (nba_mask_contiguous(target_width, value, mask, &offset, &width)) {
        n->range_offset = offset;
        n->range_width = width;
        n->has_range = 1;
        sv4_select_plan_t plan = {
            target_width, llg_sv4_width(value), offset, offset, width,
        };
        n->value = sv4_select_plan_slice(value, &plan, 0);
        return;
    }
    sv4_copy(&n->value, &value);
    sv4_copy(&n->mask, &mask);
    n->has_mask = 1;
}

static void promote_delayed_nbas(void) {
    while (g.delayed_nba_buckets && g.delayed_nba_buckets->time == g.now) {
        llg_nba_bucket_t* bucket = g.delayed_nba_buckets;
        g.delayed_nba_buckets = bucket->next;
        for (int region = 0; region < LLG_REGION_COUNT; ++region) {
            llg_nba_queue_t* due = &bucket->queues[region];
            llg_nba_queue_t* current = &g.nba_queues[region];
            if (!due->head) continue;
            // Everything in a future bucket was issued before work could run
            // at that time, so it precedes any current-slot entry.
            if (current->head) {
                due->tail->queue_next = current->head;
                current->head->queue_prev = due->tail;
                current->head = due->head;
            } else {
                *current = *due;
            }
        }
        free(bucket);
    }
}

static void cancel_proc_nbas(llg_proc_t* proc) {
    while (proc && proc->nba_head) {
        llg_nba_t* n = proc->nba_head;
        nba_queue_remove(&g.nba_queues[n->region], n);
        nba_owner_remove(n);
        nba_destroy(n);
    }
}

static void free_all_nbas(void) {
    for (int region = 0; region < LLG_REGION_COUNT; ++region) {
        llg_nba_queue_t* queue = &g.nba_queues[region];
        while (queue->head) {
            llg_nba_t* n = queue->head;
            nba_queue_remove(queue, n);
            nba_owner_remove(n);
            nba_destroy(n);
        }
    }
    while (g.delayed_nba_buckets) {
        llg_nba_bucket_t* bucket = g.delayed_nba_buckets;
        g.delayed_nba_buckets = bucket->next;
        for (int region = 0; region < LLG_REGION_COUNT; ++region) {
            llg_nba_queue_t* queue = &bucket->queues[region];
            while (queue->head) {
                llg_nba_t* n = queue->head;
                nba_queue_remove(queue, n);
                nba_destroy(n);
            }
        }
        free(bucket);
    }
}

void llg_nba_after(sv4_t* target, sv4_t value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    sv4_copy(&n->value, &value);
    enqueue_nba(n);
}

void llg_nba_net_after(llg_net_t* net, int slot, sv4_t value, uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    sv4_copy(&n->value, &value);
    enqueue_nba(n);
}

void llg_nba_net_masked_after(llg_net_t* net, int slot, sv4_t value,
                              sv4_t mask, uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    nba_capture_masked(n, llg_sv4_width(*net->drivers[slot]), value, mask);
    enqueue_nba(n);
}

void llg_nba_net_selected_after(llg_net_t* net, int slot, sv4_t value,
                                sv4_select_plan_t plan, int reverse,
                                uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    nba_capture_range(n, llg_sv4_width(*net->drivers[slot]), value, plan, reverse);
    enqueue_nba(n);
}

static void clocking_drive_schedule(const llg_clocking_drive_t* drive,
                                    const llg_wait_src_t* specs, int n_specs) {
    if (!specs || n_specs <= 0 || !region_can_mutate("clocking drive scheduling"))
        return;
    if (clocking_event_current(specs, n_specs)) {
        clocking_drive_enqueue(drive);
        return;
    }
    llg_clocking_drive_t* pending = (llg_clocking_drive_t*)llg_checked_calloc(
        1, sizeof(*pending), "pending clocking drive");
    pending->specs = (llg_wait_src_t*)llg_checked_malloc(
        (size_t)n_specs, sizeof(*pending->specs), "clocking drive event sources");
    memcpy(pending->specs, specs, (size_t)n_specs * sizeof(*specs));
    pending->n_specs = n_specs;
    pending->target = drive->target;
    pending->target_scope = value_scope_retain_target(drive->target);
    pending->net_target = drive->net_target;
    pending->net_slot = drive->net_slot;
    pending->real_target = drive->real_target;
    sv4_copy(&pending->value, &drive->value);
    sv4_copy(&pending->mask, &drive->mask);
    pending->has_mask = drive->has_mask;
    pending->range_offset = drive->range_offset;
    pending->range_width = drive->range_width;
    pending->has_range = drive->has_range;
    pending->is_real = drive->is_real;
    pending->real_value = drive->real_value;
    pending->ticks = drive->ticks;
    if (g.clocking_drives_tail) g.clocking_drives_tail->next = pending;
    else g.clocking_drives = pending;
    g.clocking_drives_tail = pending;
}

void llg_clocking_nba_sync_after(sv4_t* target, sv4_t value, uint64_t ticks,
                                 const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = value;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_net_sync_after(llg_net_t* net, int slot, sv4_t value,
                                     uint64_t ticks,
                                     const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = value;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_sync_masked_after(
    sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks,
    const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = value;
    drive.mask = mask;
    drive.has_mask = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_net_sync_masked_after(
    llg_net_t* net, int slot, sv4_t value, sv4_t mask, uint64_t ticks,
    const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = value;
    drive.mask = mask;
    drive.has_mask = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_sync_selected_after(
    sv4_t* target, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, const llg_wait_src_t* specs, int n_specs) {
    if (!target) return;
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = nba_range_slice(llg_sv4_width(*target), value, plan, reverse,
                                  &drive.range_offset, &drive.range_width);
    drive.has_range = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
    sv4_destroy(&drive.value);
}

void llg_clocking_nba_net_sync_selected_after(
    llg_net_t* net, int slot, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, const llg_wait_src_t* specs, int n_specs) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = nba_range_slice(llg_sv4_width(*net->drivers[slot]), value, plan,
                                  reverse, &drive.range_offset,
                                  &drive.range_width);
    drive.has_range = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
    sv4_destroy(&drive.value);
}

void llg_clocking_nba_d_sync_after(double* target, double value, uint64_t ticks,
                                   const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.real_target = target;
    drive.real_value = value;
    drive.is_real = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
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
    if (!target) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_masked(n, llg_sv4_width(*target), value, mask);
    enqueue_nba(n);
}

void llg_nba_selected_after(sv4_t* target, sv4_t value,
                            sv4_select_plan_t plan, int reverse,
                            uint64_t ticks) {
    if (!target) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_range(n, llg_sv4_width(*target), value, plan, reverse);
    enqueue_nba(n);
}

void llg_nba_tagged_selected_after(sv4_t* target, sv4_t value,
                                   sv4_select_plan_t plan, int reverse,
                                   uint64_t ticks,
                                   const llg_ref_tag_check_t* checks,
                                   size_t check_count, const char* location) {
    if (!target) return;
    if (!check_count || !checks) {
        llg_nba_selected_after(target, value, plan, reverse, ticks);
        return;
    }
    if (check_count > (SIZE_MAX - sizeof(llg_ref_view_t)) / sizeof(*checks)) {
        fputs("llg: fatal: tagged nonblocking assignment check count overflow\n", stderr);
        abort();
    }
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    // One allocation holds the view header and its check array; the checks
    // carry only plans, widths and static member-name literals.
    llg_ref_view_t* view = (llg_ref_view_t*)llg_checked_malloc(
        1, sizeof(llg_ref_view_t) + check_count * sizeof(*checks),
        "tagged nonblocking assignment checks");
    llg_ref_tag_check_t* copied = (llg_ref_tag_check_t*)(view + 1);
    memcpy(copied, checks, check_count * sizeof(*checks));
    *view = (llg_ref_view_t){
        .parent = NULL,
        .plan = plan,
        .tag_check_count = check_count,
        .tag_checks = copied,
        .location = location,
    };
    n->tag_view = view;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_range(n, llg_sv4_width(*target), value, plan, reverse);
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

void llg_ba_from(sv4_t* target, const sv4_t* value) {
    llg_ba(target, *value);
}
/* Lazy fixed cells never move: queued writes and selected dependencies retain
 * descriptor identity. Unvisited cells share only a read-only default. */
struct llg_fixed_inertial {
    const void* site;
    llg_inertial_t* handle;
    struct llg_fixed_inertial* next;
};

struct llg_fixed_cell {
    uint64_t index;
    sv4_t value;
    llg_fixed_array_t* array;
    struct llg_fixed_inertial* inertial;
    struct llg_fixed_cell *bucket_next, *next, *address_next;
};

static _Noreturn void fixed_bad_state(const char* reason) {
    fprintf(stderr, "llg runtime fatal: %s\n", reason);
    abort();
}

#define LLG_FIXED_INDEX_INITIAL_CAPACITY 16u
static llg_fixed_cell_t** fixed_addresses;
static size_t fixed_address_capacity, fixed_address_count;

static size_t fixed_hash(uint64_t key) {
    key ^= key >> 30;
    key *= UINT64_C(0xbf58476d1ce4e5b9);
    key ^= key >> 27;
    key *= UINT64_C(0x94d049bb133111eb);
    return (size_t)(key ^ (key >> 31));
}

static size_t fixed_grow(size_t capacity) {
    if (capacity > SIZE_MAX / 2) llg_fatal_allocation("fixed array index overflow", capacity, 2);
    return capacity ? capacity * 2 : LLG_FIXED_INDEX_INITIAL_CAPACITY;
}

static void fixed_address_reserve(void) {
    if (fixed_address_count < fixed_address_capacity / 2) return;
    size_t capacity = fixed_grow(fixed_address_capacity);
    llg_fixed_cell_t** buckets = llg_checked_calloc(capacity, sizeof(*buckets), "fixed cell address index");
    for (size_t i = 0; i < fixed_address_capacity; ++i) {
        llg_fixed_cell_t* cell = fixed_addresses[i];
        while (cell) {
            llg_fixed_cell_t* next = cell->address_next;
            size_t slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (capacity - 1);
            cell->address_next = buckets[slot];
            buckets[slot] = cell;
            cell = next;
        }
    }
    free(fixed_addresses);
    fixed_addresses = buckets;
    fixed_address_capacity = capacity;
}

static void fixed_array_changed(sv4_t* target) {
    if (!fixed_address_capacity) return;
    size_t slot = fixed_hash((uint64_t)(uintptr_t)target) & (fixed_address_capacity - 1);
    for (llg_fixed_cell_t* cell = fixed_addresses[slot]; cell; cell = cell->address_next) {
        if (&cell->value == target) {
            llg_dependency_changed(cell->array->contents);
            return;
        }
    }
}

llg_inertial_t** llg_fixed_array_inertial(sv4_t* target, const void* site) {
    if (fixed_address_capacity) {
        size_t slot = fixed_hash((uint64_t)(uintptr_t)target) & (fixed_address_capacity - 1);
        for (llg_fixed_cell_t* cell = fixed_addresses[slot]; cell; cell = cell->address_next) {
            if (&cell->value != target) continue;
            for (struct llg_fixed_inertial* entry = cell->inertial; entry; entry = entry->next)
                if (entry->site == site) return &entry->handle;
            struct llg_fixed_inertial* entry = llg_checked_calloc(1, sizeof(*entry), "fixed inertial site");
            entry->site = site;
            entry->next = cell->inertial;
            cell->inertial = entry;
            return &entry->handle;
        }
    }
    fixed_bad_state("inertial target is not a fixed cell");
}

static sv4_t fixed_read(const llg_fixed_array_t*, uint64_t);
static void fixed_ranges_destroy(llg_fixed_range_t*);
static llg_fixed_range_t* fixed_ranges_copy(const llg_fixed_array_t*, uint64_t, uint64_t, uint64_t, int);
static void fixed_array_snapshot(llg_fixed_array_t*, const llg_fixed_array_t*, int);
static void fixed_array_apply(llg_fixed_array_t*, const llg_fixed_array_t*);

static llg_fixed_cell_t* fixed_find(const llg_fixed_array_t* array, uint64_t index) {
    if (!array->capacity) return NULL;
    size_t slot = fixed_hash(index) & (array->capacity - 1);
    for (llg_fixed_cell_t* cell = array->buckets[slot]; cell; cell = cell->bucket_next)
        if (cell->index == index) return cell;
    return NULL;
}

void llg_fixed_array_init(llg_fixed_array_t* array, uint64_t total,
                          sv4_t initial, sv4_t* contents) {
    if (!array || !total || array->total || !llg_sv4_width(initial))
        fixed_bad_state("invalid fixed array initialization");
    array->total = total;
    array->initial = initial; /* Consumes the fresh default owner. */
    array->contents = contents;
}

const sv4_t* llg_fixed_array_peek(const llg_fixed_array_t* array, uint64_t index) {
    if (!array || index >= array->total) fixed_bad_state("invalid fixed array index");
    if (array->owner) {
        if (array->origin == UINT64_MAX) return &array->initial;
        return llg_fixed_array_peek(array->owner, array->origin + index);
    }
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (!cell && array->ranges) return llg_fixed_array_cell((llg_fixed_array_t*)array, index);
    return cell ? &cell->value : &array->initial;
}

sv4_t* llg_fixed_array_cell(llg_fixed_array_t* array, uint64_t index) {
    if (!array || index >= array->total) fixed_bad_state("invalid fixed array index");
    if (array->owner) {
        if (array->origin == UINT64_MAX) fixed_bad_state("invalid writable fixed view");
        return llg_fixed_array_cell(array->owner, array->origin + index);
    }
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (cell) return &cell->value;
    if (array->count >= array->capacity / 2) {
        size_t capacity = fixed_grow(array->capacity);
        llg_fixed_cell_t** buckets = llg_checked_calloc(capacity, sizeof(*buckets), "fixed array index");
        for (cell = array->cells; cell; cell = cell->next) {
            size_t slot = fixed_hash(cell->index) & (capacity - 1);
            cell->bucket_next = buckets[slot];
            buckets[slot] = cell;
        }
        free(array->buckets);
        array->buckets = buckets;
        array->capacity = capacity;
    }
    fixed_address_reserve();
    cell = llg_checked_calloc(1, sizeof(*cell), "fixed array cell");
    cell->index = index;
    cell->array = array;
    cell->value = fixed_read(array, index);
    size_t slot = fixed_hash(index) & (array->capacity - 1);
    cell->bucket_next = array->buckets[slot];
    array->buckets[slot] = cell;
    cell->next = array->cells;
    array->cells = cell;
    ++array->count;
    slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (fixed_address_capacity - 1);
    cell->address_next = fixed_addresses[slot];
    fixed_addresses[slot] = cell;
    ++fixed_address_count;
    return &cell->value;
}

void llg_fixed_array_reset(llg_fixed_array_t* array, sv4_t initial) {
    fixed_ranges_destroy(array->ranges);
    array->ranges = NULL;
    for (llg_fixed_cell_t* cell = array->cells; cell; cell = cell->next)
        sv4_copy(&cell->value, &initial);
    sv4_replace(&array->initial, initial);
}

void llg_fixed_array_destroy(void* object) {
    llg_fixed_array_t* array = object;
    fixed_ranges_destroy(array->ranges);
    llg_fixed_cell_t* cell = array->cells;
    while (cell) {
        llg_fixed_cell_t* next = cell->next;
        size_t slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (fixed_address_capacity - 1);
        llg_fixed_cell_t** link = &fixed_addresses[slot];
        while (*link != cell) link = &(*link)->address_next;
        *link = cell->address_next;
        --fixed_address_count;
        llg_clocking_forget_signal(&cell->value);
        sv4_destroy(&cell->value);
        while (cell->inertial) {
            struct llg_fixed_inertial* next = cell->inertial->next;
            free(cell->inertial);
            cell->inertial = next;
        }
        free(cell);
        cell = next;
    }
    if (!fixed_address_count) {
        free(fixed_addresses);
        fixed_addresses = NULL;
        fixed_address_capacity = 0;
    }
    free(array->buckets);
    sv4_destroy(&array->initial);
    *array = (llg_fixed_array_t){0};
}


static void fixed_array_simple_stream_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src,
                          int two_state, int nba, uint32_t slice) {
    if (region_is_read_only_now(g.current_region) && !region_private_store("fixed array write"))
        return;
    llg_value_scope_t* target_pin = value_target_pin(dst);
    if (dst->total != src->total) fixed_bad_state("fixed array copy shape mismatch");
    uint32_t width = llg_sv4_width(src->initial);
    if (slice && width % slice && slice % width) fixed_bad_state("unaligned fixed stream slice");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    sv4_t initial = sv4_cast(src->initial, llg_sv4_width(dst->initial), llg_sv4_signed(dst->initial));
    if (slice && slice < width) sv4_replace(&initial, sv4_stream(initial, slice, 1));
    if (two_state) {
        sv4_t converted = sv4_to_two_state(initial);
        sv4_move(&initial, &converted);
    }
    llg_fixed_array_init(snapshot, src->total, initial, NULL);
    for (llg_fixed_cell_t* cell = src->cells; cell; cell = cell->next) {
        sv4_t value = sv4_cast(cell->value, llg_sv4_width(dst->initial), llg_sv4_signed(dst->initial));
        if (slice && slice < width) sv4_replace(&value, sv4_stream(value, slice, 1));
        if (two_state) {
            sv4_t converted = sv4_to_two_state(value);
            sv4_move(&value, &converted);
        }
        uint64_t index = cell->index;
        if (slice) {
            uint64_t group = slice <= width ? 1 : slice / width;
            uint64_t low = src->total - 1 - index;
            uint64_t base = low / group * group;
            uint64_t count = src->total - base < group ? src->total - base : group;
            index = base + count - 1 - low % group;
        }
        if (!sv4_same(value, snapshot->initial))
            sv4_move(llg_fixed_array_cell(snapshot, index), &value);
        sv4_destroy(&value);
    }
    /* Allocate all missing destinations before the first publication. */
    for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
        (void)llg_fixed_array_cell(dst, cell->index);
    if (nba) {
        llg_nba_t* update = new_nba(0);
        if (update) {
            update->fixed_target = dst;
            update->target_scope = value_scope_retain_target(dst);
            update->fixed_value = llg_checked_calloc(1, sizeof(*snapshot), "fixed array NBA snapshot");
            llg_fixed_array_init(update->fixed_value, snapshot->total, sv4_clone(&snapshot->initial), NULL);
            for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
                sv4_copy(llg_fixed_array_cell(update->fixed_value, cell->index), &cell->value);
            enqueue_nba(update);
        }
    } else {
        fixed_array_apply(dst, snapshot);
    }
    llg_value_scope_end(scope);
    if (target_pin) llg_value_scope_end(target_pin);
}

static int fixed_compare_leaf(const sv4_t* left, const sv4_t* right, int case_eq) {
    sv4_t result = case_eq ? sv4_case_eq(*left, *right) : sv4_eq(*left, *right);
    int state = llg_sv4_state(result, 0);
    sv4_destroy(&result);
    return state;
}

/* Runtime `with` source over descriptor storage: reads each selected cell
 * through `peek`, so untouched cells are not materialized by the stream. */
sv4_t llg_fixed_array_stream_source(const llg_fixed_array_t* array,
                                    int64_t declaration_left, int64_t declaration_right,
                                    uint32_t element_width, sv4_t fallback,
                                    int selector_kind, sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_fixed_stream_bounds(selector_kind, first, second, declaration_left,
                            declaration_right, &left, &right, &count);
    uint32_t width = count ? llg_fixed_stream_width(selector_kind, first, second, element_width) : 0;
    if (!width) {
        sv4_t empty = SV4_EMPTY;
        return empty;
    }
    sv4_t packed = sv4_zero(width, 0);
    uint32_t cursor = width;
    for (size_t i = 0; i < count; ++i) {
        int64_t offset = llg_fixed_stream_storage_offset(
            declaration_left, declaration_right, llg_fixed_stream_index_at(left, right, i));
        sv4_part_select_set(&packed, (int64_t)cursor - 1, (int64_t)(cursor - element_width),
                            offset < 0 ? fallback : *llg_fixed_array_peek(array, (uint64_t)offset));
        cursor -= element_width;
    }
    return packed;
}

sv4_t llg_fixed_array_compare(const llg_fixed_array_t* left,
                             const llg_fixed_array_t* right, int case_eq, int negate) {
    if (left->total != right->total) fixed_bad_state("fixed comparison shape mismatch");
    if (left->owner || right->owner || left->ranges || right->ranges) {
        int result = 1;
        for (uint64_t index = 0; index < left->total; ++index) {
            sv4_t a = fixed_read(left, index), b = fixed_read(right, index);
            int state = fixed_compare_leaf(&a, &b, case_eq);
            sv4_destroy(&a); sv4_destroy(&b);
            if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
            if (state > 1) result = 2;
        }
        return result > 1 ? sv4_x(1, 0) : sv4_from_u64((uint64_t)(result ^ !!negate), 1, 0);
    }
    int result = 1;
    size_t represented = left->count;
    for (llg_fixed_cell_t* cell = left->cells; cell; cell = cell->next) {
        int state = fixed_compare_leaf(&cell->value, llg_fixed_array_peek(right, cell->index), case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    for (llg_fixed_cell_t* cell = right->cells; cell; cell = cell->next) {
        if (fixed_find(left, cell->index)) continue;
        ++represented;
        int state = fixed_compare_leaf(&left->initial, &cell->value, case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    if (represented < left->total) {
        int state = fixed_compare_leaf(&left->initial, &right->initial, case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    return result > 1 ? sv4_x(1, 0) : sv4_from_u64((uint64_t)(result ^ !!negate), 1, 0);
}

void llg_fixed_array_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src,
                          int two_state, int nba) {
    llg_fixed_array_stream_copy(dst, src, two_state, nba, 0);
}

void llg_fixed_array_fill(llg_fixed_array_t* dst, sv4_t value, int two_state, int nba) {
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* source = llg_value_scope_object(scope);
    llg_fixed_array_init(source, dst->total, sv4_clone(&value), NULL);
    llg_fixed_array_copy(dst, source, two_state, nba);
    llg_value_scope_end(scope);
}

struct llg_fixed_range {
    uint64_t start, count, source;
    int two_state;
    sv4_t value;
    llg_fixed_image_t* image;
    llg_fixed_range_t* next;
};

struct llg_fixed_image {
    size_t refs, count;
    llg_fixed_array_t* sources;
    uint64_t bit_count, element_cells;
    uint32_t slice, cell_width;
    int two_state, merge;
};

static void fixed_image_release(llg_fixed_image_t* image) {
    if (!image || --image->refs) return;
    for (size_t i = 0; i < image->count; ++i) llg_fixed_array_destroy(&image->sources[i]);
    free(image->sources);
    free(image);
}

static void fixed_ranges_destroy(llg_fixed_range_t* range) {
    while (range) {
        llg_fixed_range_t* next = range->next;
        sv4_destroy(&range->value);
        fixed_image_release(range->image);
        free(range);
        range = next;
    }
}

static sv4_t fixed_convert(sv4_t value, uint32_t width, int sign, int two_state) {
    sv4_t result = sv4_cast(value, width, sign);
    if (two_state) sv4_replace(&result, sv4_to_two_state(result));
    return result;
}

static sv4_t fixed_image_read(const llg_fixed_image_t* image, uint64_t index) {
    if (image->merge) {
        uint64_t first = index / image->element_cells * image->element_cells;
        int equal = 1;
        for (uint64_t i = 0; i < image->element_cells; ++i) {
            sv4_t left = fixed_read(&image->sources[0], first + i);
            sv4_t right = fixed_read(&image->sources[1], first + i);
            int state = fixed_compare_leaf(&left, &right, 0);
            sv4_destroy(&left); sv4_destroy(&right);
            if (state != 1) { equal = 0; break; }
        }
        if (!equal) return image->two_state ? sv4_zero(image->cell_width, 0) : sv4_x(image->cell_width, 0);
        return fixed_read(&image->sources[0], index);
    }
    sv4_t result = sv4_zero(image->cell_width, 0);
    sv4_t cached = SV4_EMPTY;
    const llg_fixed_array_t* cached_source = NULL;
    uint64_t cached_index = UINT64_MAX;
    for (uint32_t bit = 0; bit < image->cell_width; ++bit) {
        uint64_t position = index * image->cell_width + (image->cell_width - 1 - bit);
        if (position >= image->bit_count) continue;
        if (image->slice) {
            uint64_t first_size = image->bit_count % image->slice;
            if (!first_size) first_size = image->slice;
            position = position < first_size ? image->bit_count - first_size + position
                : image->bit_count - first_size - ((position - first_size) / image->slice + 1) * image->slice
                    + (position - first_size) % image->slice;
        }
        const llg_fixed_array_t* source = NULL;
        uint64_t local = position;
        uint32_t width = 0;
        for (size_t i = 0; i < image->count; ++i) {
            width = llg_sv4_width(image->sources[i].initial);
            uint64_t bits = image->sources[i].total * width;
            if (local < bits) { source = &image->sources[i]; break; }
            local -= bits;
        }
        if (!source) fixed_bad_state("fixed stream source cursor overflow");
        uint64_t cell = local / width;
        if (cached_source != source || cached_index != cell) {
            sv4_replace(&cached, fixed_read(source, cell));
            cached_source = source;
            cached_index = cell;
        }
        unsigned state = llg_sv4_state(cached, width - 1 - local % width);
        llg_sv4_set_state(&result, bit, image->two_state && state > 1 ? 0 : state);
    }
    sv4_destroy(&cached);
    return result;
}

static sv4_t fixed_read(const llg_fixed_array_t* array, uint64_t index) {
    if (index >= array->total) fixed_bad_state("fixed read exceeds view");
    if (array->owner) {
        return array->origin == UINT64_MAX ? sv4_clone(&array->initial)
            : fixed_read(array->owner, array->origin + index);
    }
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (cell) return sv4_clone(&cell->value);
    for (const llg_fixed_range_t* range = array->ranges; range; range = range->next) {
        if (index < range->start || index - range->start >= range->count) continue;
        sv4_t value = range->image ? fixed_image_read(range->image, range->source + index - range->start)
            : sv4_clone(&range->value);
        sv4_t converted = fixed_convert(value, llg_sv4_width(array->initial), llg_sv4_signed(array->initial), range->two_state);
        sv4_destroy(&value);
        return converted;
    }
    return sv4_clone(&array->initial);
}

static llg_fixed_range_t* fixed_range_clone(const llg_fixed_range_t* range, uint64_t start, uint64_t count, uint64_t source, int two_state) {
    llg_fixed_range_t* result = llg_checked_calloc(1, sizeof(*result), "fixed value range");
    result->start = start; result->count = count; result->source = source;
    result->two_state = two_state || range->two_state;
    result->value = sv4_clone(&range->value);
    result->image = range->image;
    if (result->image) {
        if (result->image->refs == SIZE_MAX) llg_fatal_allocation("fixed image references", SIZE_MAX, 1);
        ++result->image->refs;
    }
    return result;
}

/* Earlier ranges take precedence, so lists keep their order. A range wholly
 * shadowed by an earlier one is dropped; repeated row copies therefore keep
 * the list bounded by the number of distinct intervals instead of growing. */
static void fixed_range_append(llg_fixed_range_t** head, llg_fixed_range_t*** tail, llg_fixed_range_t* range) {
    for (const llg_fixed_range_t* earlier = *head; earlier; earlier = earlier->next) {
        if (earlier->start <= range->start
            && range->start + range->count <= earlier->start + earlier->count) {
            range->next = NULL;
            fixed_ranges_destroy(range);
            return;
        }
    }
    range->next = NULL;
    **tail = range;
    *tail = &range->next;
}

static llg_fixed_range_t* fixed_ranges_copy(const llg_fixed_array_t* array, uint64_t first, uint64_t count, uint64_t target, int two_state) {
    llg_fixed_range_t* result = NULL;
    llg_fixed_range_t** tail = &result;
    for (const llg_fixed_range_t* range = array->ranges; range; range = range->next) {
        uint64_t begin = range->start > first ? range->start : first;
        uint64_t end = range->start + range->count < first + count ? range->start + range->count : first + count;
        if (begin >= end) continue;
        fixed_range_append(&result, &tail, fixed_range_clone(range, target + begin - first, end - begin, range->source + begin - range->start, two_state));
    }
    return result;
}

void llg_fixed_array_view_init(llg_fixed_array_t* view, llg_fixed_array_t* owner, uint64_t origin, uint64_t total, int two_state) {
    if (!view || !owner || !total || view->total) fixed_bad_state("invalid fixed view initialization");
    llg_fixed_array_init(view, total, two_state ? sv4_zero(llg_sv4_width(owner->initial), llg_sv4_signed(owner->initial))
        : sv4_x(llg_sv4_width(owner->initial), llg_sv4_signed(owner->initial)), NULL);
    view->owner = owner->owner ? owner->owner : owner;
    view->origin = origin == UINT64_MAX || owner->origin == UINT64_MAX || origin > owner->total || total > owner->total - origin
        ? UINT64_MAX : origin + owner->origin;
}

static void fixed_array_snapshot(llg_fixed_array_t* result, const llg_fixed_array_t* source, int two_state) {
    const llg_fixed_array_t* owner = source->owner ? source->owner : source;
    uint64_t origin = source->owner ? source->origin : 0;
    sv4_t initial = fixed_convert(origin == UINT64_MAX ? source->initial : owner->initial,
        llg_sv4_width(source->initial), llg_sv4_signed(source->initial), two_state);
    llg_fixed_array_init(result, source->total, initial, NULL);
    if (origin == UINT64_MAX) return;
    result->ranges = fixed_ranges_copy(owner, origin, source->total, 0, two_state);
    for (llg_fixed_cell_t* cell = owner->cells; cell; cell = cell->next) {
        if (cell->index < origin || cell->index - origin >= source->total) continue;
        sv4_t value = fixed_convert(cell->value, llg_sv4_width(initial), llg_sv4_signed(initial), two_state);
        sv4_move(llg_fixed_array_cell(result, cell->index - origin), &value);
    }
}

struct fixed_publication { sv4_t* target; sv4_t old, value, replacement; };
struct fixed_transaction { size_t count; struct fixed_publication* cells; llg_fixed_range_t* ranges; sv4_t initial; };

static void fixed_transaction_destroy(void* object) {
    struct fixed_transaction* transaction = object;
    for (size_t i = 0; i < transaction->count; ++i) {
        sv4_destroy(&transaction->cells[i].old);
        sv4_destroy(&transaction->cells[i].value);
        sv4_destroy(&transaction->cells[i].replacement);
    }
    free(transaction->cells);
    fixed_ranges_destroy(transaction->ranges);
    sv4_destroy(&transaction->initial);
}

static void fixed_array_apply(llg_fixed_array_t* destination, const llg_fixed_array_t* snapshot) {
    llg_fixed_array_t* dst = destination->owner ? destination->owner : destination;
    uint64_t origin = destination->owner ? destination->origin : 0;
    if (origin == UINT64_MAX) return;
    if (destination->total != snapshot->total) fixed_bad_state("fixed publication shape mismatch");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(struct fixed_transaction), fixed_transaction_destroy);
    struct fixed_transaction* prepared = llg_value_scope_object(scope);
    for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
        (void)llg_fixed_array_cell(dst, origin + cell->index);
    prepared->ranges = fixed_ranges_copy(snapshot, 0, snapshot->total, origin, 0);
    if (origin || destination->total != dst->total) {
        llg_fixed_range_t** tail = &prepared->ranges;
        while (*tail) tail = &(*tail)->next;
        /* Source ranges take precedence over this interval's uniform default. */
        llg_fixed_range_t uniform = {0}; uniform.value = snapshot->initial;
        fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(&uniform, origin, snapshot->total, 0, 0));
        /* Retained ranges lie outside the interval; their relative order is kept. */
        for (llg_fixed_range_t* old = dst->ranges; old; old = old->next) {
            uint64_t end = old->start + old->count, limit = origin + snapshot->total;
            uint64_t before = end < origin ? end : origin;
            if (before > old->start)
                fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(old, old->start, before - old->start, old->source, 0));
            uint64_t after = old->start > limit ? old->start : limit;
            if (after < end)
                fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(old, after, end - after, old->source + after - old->start, 0));
        }
        prepared->initial = sv4_clone(&dst->initial);
    } else prepared->initial = sv4_clone(&snapshot->initial);
    prepared->cells = llg_checked_calloc(dst->count, sizeof(*prepared->cells), "fixed publication values");
    for (llg_fixed_cell_t* cell = dst->cells; cell; cell = cell->next) {
        if (cell->index < origin || cell->index - origin >= snapshot->total) continue;
        struct fixed_publication* entry = &prepared->cells[prepared->count++];
        entry->target = &cell->value;
        entry->old = sv4_clone(&cell->value);
        entry->value = fixed_read(snapshot, cell->index - origin);
        entry->replacement = sv4_clone(&entry->value);
    }
    int default_changed = !sv4_same(dst->initial, prepared->initial) || dst->ranges || prepared->ranges;
    /* Every owner and publication record is prepared before any value becomes visible. */
    sv4_move(&dst->initial, &prepared->initial);
    llg_fixed_range_t* old_ranges = dst->ranges;
    dst->ranges = prepared->ranges; prepared->ranges = old_ranges;
    for (size_t i = 0; i < prepared->count; ++i)
        sv4_move(prepared->cells[i].target, &prepared->cells[i].replacement);
    /* Observer callbacks run only after the complete image has been committed.
     * A private Postponed evaluation stores helper-owned cells unpublished. */
    for (size_t i = 0; i < prepared->count && !g.private_evaluation; ++i) {
        struct fixed_publication* entry = &prepared->cells[i];
        if (!sv4_same(entry->old, entry->value)) sig_publish_changed(entry->target, entry->old, entry->value, entry->value);
    }
    if (default_changed) llg_dependency_changed(dst->contents);
    llg_value_scope_end(scope);
}

static void fixed_snapshot_publish(llg_fixed_array_t* dst, llg_fixed_array_t* snapshot, int nba) {
    if (dst->owner && dst->origin == UINT64_MAX) return;
    if (nba) {
        llg_nba_t* update = new_nba(0);
        if (update) {
            update->fixed_target = dst;
            update->target_scope = value_scope_retain_target(dst);
            update->fixed_value = llg_checked_calloc(1, sizeof(*snapshot), "fixed array NBA image");
            *update->fixed_value = *snapshot;
            /* Snapshot cell identities are not exposed; keep their reverse-index owner accurate. */
            for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next) cell->array = update->fixed_value;
            *snapshot = (llg_fixed_array_t){0};
            enqueue_nba(update);
        }
    } else fixed_array_apply(dst, snapshot);
}

void llg_fixed_array_stream_segments(llg_fixed_array_t* dst, const llg_fixed_array_t* const* sources, size_t count, int two_state, int nba, uint32_t slice) {
    if (region_is_read_only_now(g.current_region) && !region_private_store("fixed array stream"))
        return;
    llg_value_scope_t* pin = value_target_pin(dst);
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    if (!count) fixed_bad_state("empty fixed stream");
    if (count == 1 && !slice && dst->total == sources[0]->total && llg_sv4_width(dst->initial) == llg_sv4_width(sources[0]->initial)) {
        fixed_array_snapshot(snapshot, sources[0], two_state);
    } else {
        llg_fixed_image_t* image = llg_checked_calloc(1, sizeof(*image), "fixed stream image");
        image->refs = 1; image->count = count; image->slice = slice;
        image->cell_width = llg_sv4_width(dst->initial); image->two_state = two_state;
        image->sources = llg_checked_calloc(count, sizeof(*image->sources), "fixed stream sources");
        for (size_t i = 0; i < count; ++i) {
            fixed_array_snapshot(&image->sources[i], sources[i], 0);
            uint64_t width = llg_sv4_width(sources[i]->initial);
            if (sources[i]->total > (UINT64_MAX - image->bit_count) / width) fixed_bad_state("fixed stream width overflow");
            image->bit_count += sources[i]->total * width;
        }
        if (dst->total > UINT64_MAX / image->cell_width || image->bit_count > dst->total * image->cell_width) fixed_bad_state("fixed stream exceeds destination");
        llg_fixed_array_init(snapshot, dst->total, sv4_zero(image->cell_width, llg_sv4_signed(dst->initial)), NULL);
        llg_fixed_range_t* range = llg_checked_calloc(1, sizeof(*range), "fixed stream range");
        range->count = dst->total; range->image = image; snapshot->ranges = range;
    }
    fixed_snapshot_publish(dst, snapshot, nba);
    llg_value_scope_end(scope);
    if (pin) llg_value_scope_end(pin);
}

void llg_fixed_array_stream_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src, int two_state, int nba, uint32_t slice) {
    uint32_t width = llg_sv4_width(src->initial);
    if (!dst->owner && !src->owner && !dst->ranges && !src->ranges && dst->total == src->total
        && llg_sv4_width(dst->initial) == width && (!slice || !(width % slice) || !(slice % width)))
        fixed_array_simple_stream_copy(dst, src, two_state, nba, slice);
    else { const llg_fixed_array_t* sources[] = {src}; llg_fixed_array_stream_segments(dst, sources, 1, two_state, nba, slice); }
}

void llg_fixed_array_merge(llg_fixed_array_t* dst, const llg_fixed_array_t* left, const llg_fixed_array_t* right, uint64_t element_cells, int two_state) {
    if (!element_cells || dst->total != left->total || dst->total != right->total || dst->total % element_cells) fixed_bad_state("fixed conditional shape mismatch");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    llg_fixed_image_t* image = llg_checked_calloc(1, sizeof(*image), "fixed conditional image");
    image->refs = 1; image->count = 2; image->merge = 1; image->element_cells = element_cells;
    image->cell_width = llg_sv4_width(dst->initial); image->two_state = two_state;
    image->sources = llg_checked_calloc(2, sizeof(*image->sources), "fixed conditional sources");
    fixed_array_snapshot(&image->sources[0], left, 0); fixed_array_snapshot(&image->sources[1], right, 0);
    llg_fixed_array_init(snapshot, dst->total, two_state ? sv4_zero(image->cell_width, 0) : sv4_x(image->cell_width, 0), NULL);
    llg_fixed_range_t* range = llg_checked_calloc(1, sizeof(*range), "fixed conditional range");
    range->count = dst->total; range->image = image; snapshot->ranges = range;
    fixed_array_apply(dst, snapshot);
    llg_value_scope_end(scope);
}

// ── IEEE stochastic analysis queues ─────────────────────────────────────────

/* Output publication can invoke a callback that never returns. Keep its
 * packed operand discoverable by process cleanup rather than on the C stack. */
static void llg_q_write_integer(sv4_t* target, int64_t number) {
    if (!target) return;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* value = llg_value_scope_values(scope);
    sv4_replace(value, sv4_from_i64(number, target->width));
    llg_ba(target, *value);
    llg_value_scope_end(scope);
}

static void llg_q_set_status(sv4_t* status, int code) {
    llg_q_write_integer(status, (int64_t)code);
}

static int llg_q_read_integer(sv4_t value, int64_t* result,
                              const char* operation, const char* argument) {
    if (llg_sv4_width(value) == 0 || sv4_is_unknown(value) || !sv4_fits_i64(value)) {
        fprintf(stderr,
                "llg runtime fatal: %s %s must be a known integer "
                "representable as int64_t\n",
                operation, argument);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    *result = sv4_to_i64(value);
    return 1;
}

static llg_q_queue_t* llg_q_find(int64_t id) {
    for (llg_q_queue_t* queue = g.q_queues; queue; queue = queue->next) {
        if (queue->id == id) return queue;
    }
    return NULL;
}

static int llg_q_checked_add_u64(uint64_t left, uint64_t right,
                                 uint64_t* result, const char* what) {
    if (right > UINT64_MAX - left) {
        fprintf(stderr, "llg runtime fatal: stochastic queue %s overflow\n", what);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    *result = left + right;
    return 1;
}

static int llg_q_round_average(uint64_t total, uint64_t count,
                               uint64_t* result) {
    if (count == 0) {
        *result = 0;
        return 1;
    }
    uint64_t quotient = total / count;
    uint64_t remainder = total % count;
    // Round to the nearest integer, with exact halves rounded upward. The
    // comparison avoids overflowing `remainder * 2`.
    if (remainder >= count - remainder) {
        if (quotient == UINT64_MAX) {
            fprintf(stderr,
                    "llg runtime fatal: stochastic queue average overflow\n");
            llg_last_failure = 1;
            g.finish = 1;
            return 0;
        }
        ++quotient;
    }
    *result = quotient;
    return 1;
}

static int llg_q_write_stat(sv4_t* target, uint64_t value) {
    if (!target) return 1;
    if (value > (uint64_t)INT64_MAX) {
        fprintf(stderr,
                "llg runtime fatal: stochastic queue statistic exceeds "
                "signed integer range\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_q_write_integer(target, (int64_t)value);
    return 1;
}

static int llg_q_wait(const llg_q_entry_t* entry, uint64_t* result) {
    if (g.now < entry->arrival) {
        fprintf(stderr, "llg runtime fatal: stochastic queue time moved backwards\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    *result = g.now - entry->arrival;
    return 1;
}

void llg_q_initialize(sv4_t q_id_value, sv4_t q_type_value,
                      sv4_t max_length_value, sv4_t* status) {
    int64_t q_id;
    int64_t q_type;
    int64_t max_length;
    llg_q_set_status(status, LLG_Q_OK);
    if (!llg_q_read_integer(q_id_value, &q_id, "$q_initialize", "q_id")) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    if (!llg_q_read_integer(q_type_value, &q_type, "$q_initialize", "q_type")) {
        llg_q_set_status(status, LLG_Q_BAD_TYPE);
        return;
    }
    if (!llg_q_read_integer(max_length_value, &max_length, "$q_initialize",
                            "max_length")) {
        llg_q_set_status(status, LLG_Q_BAD_LENGTH);
        return;
    }
    if (q_type != 1 && q_type != 2) {
        llg_q_set_status(status, LLG_Q_BAD_TYPE);
        return;
    }
    if (max_length <= 0) {
        llg_q_set_status(status, LLG_Q_BAD_LENGTH);
        return;
    }
    if (llg_q_find(q_id)) {
        llg_q_set_status(status, LLG_Q_DUPLICATE_ID);
        return;
    }
    llg_q_queue_t* queue = (llg_q_queue_t*)malloc(sizeof(*queue));
    if (!queue) {
        llg_q_set_status(status, LLG_Q_NO_MEMORY);
        return;
    }
    memset(queue, 0, sizeof(*queue));
    queue->id = q_id;
    queue->type = (int)q_type;
    queue->capacity = (uint64_t)max_length;
    queue->next = g.q_queues;
    g.q_queues = queue;
}

void llg_q_add(sv4_t q_id_value, sv4_t job_id_value,
               sv4_t inform_id_value, sv4_t* status) {
    int64_t q_id;
    int64_t job_id;
    int64_t inform_id;
    llg_q_set_status(status, LLG_Q_OK);
    if (!llg_q_read_integer(q_id_value, &q_id, "$q_add", "q_id")) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    llg_q_queue_t* queue = llg_q_find(q_id);
    if (!queue) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    if (queue->length >= queue->capacity) {
        llg_q_set_status(status, LLG_Q_FULL);
        return;
    }
    if (!llg_q_read_integer(job_id_value, &job_id, "$q_add", "job_id") ||
        !llg_q_read_integer(inform_id_value, &inform_id, "$q_add", "inform_id")) {
        return;
    }
    uint64_t interarrival_sum = queue->interarrival_sum;
    if (queue->has_arrival) {
        if (g.now < queue->last_arrival) {
            fprintf(stderr,
                    "llg runtime fatal: stochastic queue time moved backwards\n");
            llg_last_failure = 1;
            g.finish = 1;
            return;
        }
        uint64_t interval = g.now - queue->last_arrival;
        if (!llg_q_checked_add_u64(interarrival_sum, interval, &interarrival_sum,
                                   "interarrival sum"))
            return;
    } else {
        // The first arrival is measured from simulation time zero, matching
        // the standard's queue statistics examples.
        interarrival_sum = g.now;
    }
    if (queue->arrivals == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: stochastic queue arrival count overflow\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    llg_q_entry_t* entry = (llg_q_entry_t*)malloc(sizeof(*entry));
    if (!entry) {
        llg_q_set_status(status, LLG_Q_NO_MEMORY);
        return;
    }
    entry->next = NULL;
    entry->job_id = job_id;
    entry->inform_id = inform_id;
    entry->arrival = g.now;
    if (!queue->head) {
        queue->head = entry;
        queue->tail = entry;
    } else {
        queue->tail->next = entry;
        queue->tail = entry;
    }
    queue->interarrival_sum = interarrival_sum;
    queue->has_arrival = 1;
    queue->last_arrival = g.now;
    ++queue->arrivals;
    ++queue->length;
    if (queue->length > queue->maximum_length) queue->maximum_length = queue->length;
}

void llg_q_remove(sv4_t q_id_value, sv4_t* job_id, sv4_t* inform_id,
                  sv4_t* status) {
    int64_t q_id;
    llg_q_set_status(status, LLG_Q_OK);
    if (!llg_q_read_integer(q_id_value, &q_id, "$q_remove", "q_id")) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    llg_q_queue_t* queue = llg_q_find(q_id);
    if (!queue) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    if (!queue->head) {
        llg_q_set_status(status, LLG_Q_EMPTY);
        return;
    }
    llg_q_entry_t* entry = queue->head;
    if (queue->type == 2 && queue->head != queue->tail) {
        entry = queue->tail;
        llg_q_entry_t* previous = queue->head;
        while (previous->next != queue->tail) previous = previous->next;
        previous->next = NULL;
        queue->tail = previous;
    }
    const int64_t job = entry->job_id;
    const int64_t information = entry->inform_id;
    if (queue->head == entry) queue->head = entry->next;
    if (queue->tail == entry) queue->tail = NULL;
    --queue->length;
    uint64_t wait;
    if (llg_q_wait(entry, &wait) &&
        (!queue->has_shortest_wait || wait < queue->shortest_wait)) {
        queue->shortest_wait = wait;
        queue->has_shortest_wait = 1;
    }
    free(entry);
    /* Finish the dequeue before publication can reenter the queue or exit.
     * The snapshots are native scalars, so there is no detached entry owner. */
    llg_q_write_integer(job_id, job);
    llg_q_write_integer(inform_id, information);
}

sv4_t llg_q_full(sv4_t q_id_value, sv4_t* status) {
    int64_t q_id;
    llg_q_set_status(status, LLG_Q_OK);
    if (!llg_q_read_integer(q_id_value, &q_id, "$q_full", "q_id")) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return sv4_from_i64(0, 32);
    }
    llg_q_queue_t* queue = llg_q_find(q_id);
    if (!queue) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return sv4_from_i64(0, 32);
    }
    return sv4_from_i64(queue->length >= queue->capacity, 32);
}

void llg_q_exam(sv4_t q_id_value, sv4_t stat_code_value,
                sv4_t* stat_value, sv4_t* status) {
    int64_t q_id;
    int64_t stat_code;
    llg_q_set_status(status, LLG_Q_OK);
    if (!llg_q_read_integer(q_id_value, &q_id, "$q_exam", "q_id")) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    llg_q_queue_t* queue = llg_q_find(q_id);
    if (!queue) {
        llg_q_set_status(status, LLG_Q_UNKNOWN_ID);
        return;
    }
    if (!llg_q_read_integer(stat_code_value, &stat_code, "$q_exam", "stat_code")) {
        llg_q_set_status(status, LLG_Q_BAD_TYPE);
        return;
    }
    uint64_t value = 0;
    switch (stat_code) {
    case 1:
        value = queue->length;
        break;
    case 2:
        if (!llg_q_round_average(queue->interarrival_sum, queue->arrivals, &value)) return;
        break;
    case 3:
        value = queue->maximum_length;
        break;
    case 4:
        value = queue->has_shortest_wait ? queue->shortest_wait : 0;
        break;
    case 5: {
        for (llg_q_entry_t* entry = queue->head; entry; entry = entry->next) {
            uint64_t wait;
            if (!llg_q_wait(entry, &wait)) return;
            if (wait > value) value = wait;
        }
        break;
    }
    case 6: {
        uint64_t total = 0;
        for (llg_q_entry_t* entry = queue->head; entry; entry = entry->next) {
            uint64_t wait;
            if (!llg_q_wait(entry, &wait) ||
                !llg_q_checked_add_u64(total, wait, &total, "wait sum"))
                return;
        }
        if (!llg_q_round_average(total, queue->length, &value)) return;
        break;
    }
    default:
        // The standard status table has no separate statistic-selector code;
        // use the documented unsupported-selector value and leave the output
        // value untouched, while keeping the operation deterministic.
        llg_q_set_status(status, LLG_Q_BAD_TYPE);
        return;
    }
    (void)llg_q_write_stat(stat_value, value);
}

static int llg_ref_report_tag_failure(const llg_ref_view_t* view,
                                      const sv4_t* parent) {
    size_t failed = 0;
    if (llg_ref_view_valid(view, parent, &failed)) return 1;
    const char* member = "<unknown>";
    const char* location = "<unknown>";
    if (view && failed < view->tag_check_count && view->tag_checks) {
        if (view->tag_checks[failed].member_name)
            member = view->tag_checks[failed].member_name;
        if (view->location) location = view->location;
    }
    llg_rt_mark_failed();
    fprintf(stderr,
            "llg: runtime error: access to inactive tagged-union member %s at %s\n",
            member, location);
    fflush(stderr);
    return 0;
}

sv4_t llg_rt_ref_read(const llg_ref_t* ref) {
    if (!ref || (llg_ref_kind_t)ref->kind != LLG_REF_TAGGED_VIEW)
        return llg_ref_read(ref);
    const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
    if (!view || !view->parent) {
        fputs("llg runtime fatal: invalid tagged reference view\n", stderr);
        abort();
    }
    sv4_t parent = llg_rt_ref_read(view->parent);
    int valid = llg_ref_report_tag_failure(view, &parent);
    sv4_t result = valid ? sv4_select_plan_read(parent, &view->plan)
                         : (ref->two_state ? sv4_zero(ref->width, ref->is_signed)
                                           : sv4_x(ref->width, ref->is_signed));
    sv4_destroy(&parent);
    if (ref->two_state) sv4_replace(&result, sv4_to_two_state(result));
    sv4_replace(&result, sv4_cast(result, ref->width, ref->is_signed));
    return result;
}

void llg_ref_write(llg_ref_t* ref, sv4_t value) {
    if (!ref) return;
    /* Signal publication can terminate the current coroutine. Both snapshots
     * belong to its unwind stack, not merely to this C stack frame. */
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    values[0] = sv4_cast(value, ref->width, ref->is_signed);
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->retained_write)
            (void)ref->retained_write(ref->retained, values[0]);
        else if (ref->queue_write)
            (void)ref->queue_write(ref->queue, ref->queue_identity, values[0]);
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        llg_ref_composite_t* composite = (llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; i++) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[1], sv4_part_select(values[0],
                (int64_t)remaining + part->width - 1, remaining));
            llg_ref_write(part, values[1]);
        }
        if (remaining) abort();
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
        (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        sv4_replace(&values[1], sv4_fill(1, ref->width, 0));
        llg_ref_write_masked(ref, values[0], values[1]);
        goto cleanup;
    }
    if (!ref->base) goto cleanup;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_WHOLE) {
        llg_ba(ref->base, values[0]);
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
        if (ref->index != UINT64_MAX && ref->index < ref->array_size)
            llg_ba(&ref->base[ref->index], values[0]);
        goto cleanup;
    }
    sv4_copy(&values[1], ref->base);
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_BIT:
        sv4_bit_select_set(&values[1], ref->index, values[0]);
        break;
    case LLG_REF_PART:
        sv4_part_select_set(&values[1], ref->left, ref->right, values[0]);
        break;
    case LLG_REF_PACKED_PLAN:
        sv4_select_plan_set(&values[1], (const sv4_select_plan_t*)ref->retained, values[0]);
        break;
    case LLG_REF_INDEXED:
        sv4_idx_part_select_set(&values[1], ref->index, ref->indexed_width,
                                ref->indexed_negative, values[0]);
        break;
    default: goto cleanup;
    }
    llg_ba(ref->base, values[1]);
cleanup:
    llg_value_scope_end(scope);
}

void llg_ref_write_masked(llg_ref_t* ref, sv4_t value, sv4_t mask) {
    if (!ref || !ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(4);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_cast(value, ref->width, ref->is_signed));
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    sv4_replace(&values[1], sv4_cast(mask, ref->width, 0));
    sv4_replace(&values[1], sv4_to_two_state(values[1]));
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; i++) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[2], sv4_part_select(values[0], (int64_t)remaining + part->width - 1, remaining));
            sv4_replace(&values[3], sv4_part_select(values[1], (int64_t)remaining + part->width - 1, remaining));
            if (sv4_to_bool(values[3])) llg_ref_write_masked(part, values[2], values[3]);
        }
        if (remaining) abort();
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
               (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        if ((llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
            sv4_replace(&values[2], llg_rt_ref_read(view->parent));
            if (!llg_ref_report_tag_failure(view, &values[2])) goto cleanup;
        }
        sv4_replace(&values[2], sv4_zero(view->plan.storage_width, 0));
        sv4_replace(&values[3], sv4_zero(view->plan.storage_width, 0));
        sv4_select_plan_set(&values[2], &view->plan, values[0]);
        sv4_select_plan_set(&values[3], &view->plan, values[1]);
        llg_ref_write_masked(view->parent, values[2], values[3]);
    } else if (sv4_to_bool(values[1])) {
        sv4_replace(&values[2], llg_ref_read(ref));
        if (llg_sv4_width(values[2]) != ref->width) abort();
        llg_sv4_masked_merge(&values[2], values[0], values[1]);
        llg_ref_write(ref, values[2]);
    }
cleanup:
    llg_value_scope_end(scope);
}

void llg_ref_nba_masked(llg_ref_t* ref, sv4_t value, sv4_t mask, uint64_t ticks) {
    if (!ref || !ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(4);
    sv4_t* values = llg_value_scope_values(scope);
    values[0] = sv4_cast(value, ref->width, ref->is_signed);
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    values[1] = sv4_cast(mask, ref->width, 0);
    sv4_replace(&values[1], sv4_to_two_state(values[1]));
    if (!sv4_to_bool(values[1])) goto cleanup;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; ++i) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[2], sv4_part_select(values[0], (int64_t)remaining + part->width - 1, remaining));
            sv4_replace(&values[3], sv4_part_select(values[1], (int64_t)remaining + part->width - 1, remaining));
            llg_ref_nba_masked(part, values[2], values[3], ticks);
        }
        if (remaining) abort();
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
               (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        if ((llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
            sv4_replace(&values[2], llg_rt_ref_read(view->parent));
            if (!llg_ref_report_tag_failure(view, &values[2])) goto cleanup;
        }
        sv4_replace(&values[2], sv4_zero(view->plan.storage_width, 0));
        values[3] = sv4_zero(view->plan.storage_width, 0);
        sv4_select_plan_set(&values[2], &view->plan, values[0]);
        sv4_select_plan_set(&values[3], &view->plan, values[1]);
        llg_ref_nba_masked(view->parent, values[2], values[3], ticks);
    } else if (ref->base) {
        sv4_t* target = ref->base;
        if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
            if (ref->index == UINT64_MAX || ref->index >= ref->array_size) goto cleanup;
            target = &ref->base[ref->index];
        } else if ((llg_ref_kind_t)ref->kind != LLG_REF_WHOLE) {
            values[2] = sv4_zero(llg_sv4_width(*target), llg_sv4_signed(*target));
            values[3] = sv4_zero(llg_sv4_width(*target), 0);
            switch ((llg_ref_kind_t)ref->kind) {
            case LLG_REF_BIT:
                sv4_bit_select_set(&values[2], ref->index, values[0]);
                sv4_bit_select_set(&values[3], ref->index, values[1]);
                break;
            case LLG_REF_PART:
                sv4_part_select_set(&values[2], ref->left, ref->right, values[0]);
                sv4_part_select_set(&values[3], ref->left, ref->right, values[1]);
                break;
            case LLG_REF_INDEXED:
                sv4_idx_part_select_set(&values[2], ref->index, ref->indexed_width, ref->indexed_negative, values[0]);
                sv4_idx_part_select_set(&values[3], ref->index, ref->indexed_width, ref->indexed_negative, values[1]);
                break;
            case LLG_REF_PACKED_PLAN:
                sv4_select_plan_set(&values[2], (const sv4_select_plan_t*)ref->retained, values[0]);
                sv4_select_plan_set(&values[3], (const sv4_select_plan_t*)ref->retained, values[1]);
                break;
            default: abort();
            }
            llg_nba_masked(target, values[2], values[3], ticks);
            goto cleanup;
        }
        llg_nba_masked(target, values[0], values[1], ticks);
    }
cleanup:
    llg_value_scope_end(scope);
}

/* Consumes a freshly produced input value before invoking any notification.
 * The caller must not destroy its shallow descriptor after this transfer. */
static void llg_ref_write_owned(llg_ref_t* ref, sv4_t value) {
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    owned[0] = value;
    llg_ref_write(ref, owned[0]);
    llg_value_scope_end(scope);
}

void llg_ref_write_bit(llg_ref_t* ref, uint64_t index, sv4_t value) {
    if (!ref || index >= ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(3);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_zero(ref->width, 0));
    sv4_replace(&values[1], sv4_zero(ref->width, 0));
    sv4_replace(&values[2], sv4_from_u64(1, 1, 0));
    sv4_bit_select_set(&values[0], index, value);
    sv4_bit_select_set(&values[1], index, values[2]);
    llg_ref_write_masked(ref, values[0], values[1]);
    llg_value_scope_end(scope);
}

void llg_nba_d(double* target, double value) {
    llg_nba_d_after(target, value, 0);
}

void llg_ba_d(double* target, double value) {
    if (llg_is_real_forced(target) || pca_real_active(target)) return;
    real_write(target, value);
}

// ── Collapsed inout nets ──────────────────────────────────────────────────────

static uint32_t llg_net_index_priority(int index) {
    uint32_t value = (uint32_t)index + UINT32_C(0x9e3779b9);
    value ^= value >> 16;
    value *= UINT32_C(0x7feb352d);
    value ^= value >> 15;
    value *= UINT32_C(0x846ca68b);
    value ^= value >> 16;
    return value;
}

static uint32_t llg_net_index_max(const llg_net_t* net, int index) {
    return index < 0 ? 0 : net->driver_index[index].max_high;
}

static void llg_net_index_recompute(llg_net_t* net, int index) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    node->max_high = node->high;
    uint32_t left = llg_net_index_max(net, node->left);
    uint32_t right = llg_net_index_max(net, node->right);
    if (left > node->max_high) node->max_high = left;
    if (right > node->max_high) node->max_high = right;
}

static int llg_net_index_before(const llg_net_t* net, int left, int right) {
    const llg_net_driver_index_t* a = &net->driver_index[left];
    const llg_net_driver_index_t* b = &net->driver_index[right];
    return a->low < b->low || (a->low == b->low && left < right);
}

static int llg_net_index_rotate_left(llg_net_t* net, int root) {
    int next = net->driver_index[root].right;
    net->driver_index[root].right = net->driver_index[next].left;
    net->driver_index[next].left = root;
    llg_net_index_recompute(net, root);
    llg_net_index_recompute(net, next);
    return next;
}

static int llg_net_index_rotate_right(llg_net_t* net, int root) {
    int next = net->driver_index[root].left;
    net->driver_index[root].left = net->driver_index[next].right;
    net->driver_index[next].right = root;
    llg_net_index_recompute(net, root);
    llg_net_index_recompute(net, next);
    return next;
}

static int llg_net_index_insert_at(llg_net_t* net, int root, int index) {
    if (root < 0) return index;
    if (llg_net_index_before(net, index, root)) {
        net->driver_index[root].left =
            llg_net_index_insert_at(net, net->driver_index[root].left, index);
        if (net->driver_index[net->driver_index[root].left].priority <
            net->driver_index[root].priority)
            root = llg_net_index_rotate_right(net, root);
    } else {
        net->driver_index[root].right =
            llg_net_index_insert_at(net, net->driver_index[root].right, index);
        if (net->driver_index[net->driver_index[root].right].priority <
            net->driver_index[root].priority)
            root = llg_net_index_rotate_left(net, root);
    }
    llg_net_index_recompute(net, root);
    return root;
}

static int llg_net_index_merge(llg_net_t* net, int left, int right) {
    if (left < 0) return right;
    if (right < 0) return left;
    if (net->driver_index[left].priority < net->driver_index[right].priority) {
        net->driver_index[left].right =
            llg_net_index_merge(net, net->driver_index[left].right, right);
        llg_net_index_recompute(net, left);
        return left;
    }
    net->driver_index[right].left =
        llg_net_index_merge(net, left, net->driver_index[right].left);
    llg_net_index_recompute(net, right);
    return right;
}

static int llg_net_index_remove_at(llg_net_t* net, int root, int index) {
    if (root < 0) return -1;
    if (root == index)
        return llg_net_index_merge(net, net->driver_index[root].left,
                                   net->driver_index[root].right);
    if (llg_net_index_before(net, index, root))
        net->driver_index[root].left =
            llg_net_index_remove_at(net, net->driver_index[root].left, index);
    else
        net->driver_index[root].right =
            llg_net_index_remove_at(net, net->driver_index[root].right, index);
    llg_net_index_recompute(net, root);
    return root;
}

static int llg_net_has_index(const llg_net_t* net) {
    return net->driver_index && net->overlap_scratch && net->n_drivers > 0;
}

void llg_net_index_reset(llg_net_t* net) {
    if (!net || !net->driver_index) return;
    net->index_root = -1;
    for (int index = 0; index < net->n_drivers; index++) {
        llg_net_driver_index_t* node = &net->driver_index[index];
        node->low = 0;
        node->high = 0;
        node->max_high = 0;
        node->priority = llg_net_index_priority(index);
        node->left = -1;
        node->right = -1;
        node->active = 0;
    }
}

static void llg_net_index_remove(llg_net_t* net, int index) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    if (!node->active) return;
    net->index_root = llg_net_index_remove_at(net, net->index_root, index);
    node->left = -1;
    node->right = -1;
    node->active = 0;
}

static void llg_net_index_insert(llg_net_t* net, int index,
                                 uint32_t low, uint32_t high) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    node->low = low;
    node->high = high;
    node->max_high = high;
    node->left = -1;
    node->right = -1;
    node->active = 1;
    net->index_root = llg_net_index_insert_at(net, net->index_root, index);
}

static void llg_net_index_collect(const llg_net_t* net, int root,
                                  uint32_t low, uint32_t high, int* count) {
    if (root < 0) return;
    const llg_net_driver_index_t* node = &net->driver_index[root];
    if (node->left >= 0 &&
        net->driver_index[node->left].max_high >= low)
        llg_net_index_collect(net, node->left, low, high, count);
    if (node->low <= high && node->high >= low)
        net->overlap_scratch[(*count)++] = root;
    if (node->low <= high)
        llg_net_index_collect(net, node->right, low, high, count);
}

static int llg_net_all_z(const sv4_t* value) {
    int limbs = (int)((llg_sv4_width(*value) + 63u) / 64u);
    for (int limb = 0; limb < limbs; limb++) {
        uint32_t remaining = llg_sv4_width(*value) - (uint32_t)limb * 64u;
        uint64_t mask = remaining >= 64u
            ? UINT64_MAX : (UINT64_C(1) << remaining) - UINT64_C(1);
        if ((llg_sv4_word(*value, limb, LLG_SV4_BITS) & mask) || (llg_sv4_word(*value, limb, LLG_SV4_X) & mask) ||
            (llg_sv4_word(*value, limb, LLG_SV4_Z) & mask) != mask)
            return 0;
    }
    return 1;
}

static sv4_t llg_net_compute_range(llg_net_t* net, uint32_t low,
                                   uint32_t width) {
    int count = 0;
    llg_net_index_collect(net, net->index_root, low, low + width - 1u, &count);
    return sv4_resolve_strengths_range(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->overlap_scratch, count, net->width, low, width,
        net->is_signed, net->resolution);
}

static sv4_t llg_net_compute(llg_net_t* net) {
    if (llg_net_has_index(net))
        return llg_net_compute_range(net, 0, net->width);
    return sv4_resolve_strengths(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->n_drivers, net->width, net->is_signed, net->resolution);
}

static void llg_net_alias_refresh(llg_net_alias_t* alias) {
    if (!alias || !alias->storage) return;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_copy(&owned[0], alias->storage);
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        const llg_net_alias_part_t* part = &alias->parts[i];
        if (!part->net || part->bit_count == 0 ||
            (uint64_t)part->signal_bit + part->bit_count > llg_sv4_width(owned[0]) ||
            (uint64_t)part->group_bit + part->bit_count > llg_sv4_width(part->net->resolved))
            continue;
        sv4_t bits = sv4_part_select(part->net->resolved,
                                     (int64_t)part->group_bit + part->bit_count - 1,
                                     part->group_bit);
        sv4_part_select_set(&owned[0], (int64_t)part->signal_bit + part->bit_count - 1,
                            part->signal_bit, bits);
        sv4_destroy(&bits);
    }
    // The visible cell is a first-class dependency/waveform target. Route
    // updates through the ordinary signal writer so waiters and waveform
    // callbacks observe canonical alias changes.
    sig_write(&alias->visible, owned[0]);
    if (alias->publication_target) sig_write(alias->publication_target, owned[0]);
    llg_value_scope_end(scope);

}

static void llg_net_alias_refresh_all(llg_net_t* net) {
    if (!net) return;
    for (int i = 0; i < net->n_aliases; i++)
        llg_net_alias_refresh(net->aliases[i]);
}


// ── Strength views ───────────────────────────────────────────────────────────
//
// Only nets with a strength consumer carry a view, so ordinary resolution
// keeps its value-only word-parallel path. Every driver contributes one
// strength interval per bit on the signed scale (-7 = Su0 .. 7 = Su1): a
// known value is a point, X spans [-strength0, strength1], and a highz
// endpoint leaves only the other side, which models L/H. The resolved range
// is the hull of the outcomes of every level choice inside those intervals
// (IEEE 1364-2001 7.10.1-7.10.4): the strongest level wins, equal opposite
// levels give X on a wire and the wired value on wand/wor. Only the strongest
// definite and possible levels of each value decide that hull.

static int llg_net_top_level(const uint64_t* levels, uint64_t bit) {
    for (int level = LLG_STRENGTH_SUPPLY; level > LLG_STRENGTH_HIGHZ; level--)
        if (levels[level] & bit) return level;
    return -1;
}

static uint8_t llg_net_strength_code(int lo, int hi) {
    return (uint8_t)(((hi + 7) << 4) | (lo + 7));
}

static uint8_t llg_net_strength_hull(int mode, int k0, int k1, int p0, int p1) {
    int hi;
    int lo;
    if (mode == LLG_RESOLVE_WAND) {
        hi = p1 >= 0 && p1 > k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 >= k1 ? -p0 : k1 >= 0 ? k1 : 0;
    } else if (mode == LLG_RESOLVE_WOR) {
        hi = p1 >= 0 && p1 >= k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 > k1 ? -p0 : k1 >= 0 ? k1 : 0;
    } else {
        hi = p1 >= 0 && p1 >= k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 >= k1 ? -p0 : k1 >= 0 ? k1 : 0;
    }
    return llg_net_strength_code(lo, hi);
}

static uint8_t llg_net_forced_strength(const sv4_t* resolved, uint64_t bit) {
    switch (llg_sv4_state(*resolved, bit)) {
    case 0: return llg_net_strength_code(-LLG_STRENGTH_STRONG, -LLG_STRENGTH_STRONG);
    case 1: return llg_net_strength_code(LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG);
    case 2: return llg_net_strength_code(-LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG);
    default: return llg_net_strength_code(0, 0);
    }
}

static sv4_t llg_net_strength_levels(llg_net_t* net) {
    uint32_t width = net->width;
    size_t words = ((size_t)width + 63u) / 64u;
    size_t out_words = ((size_t)width * 8u + 63u) / 64u;
    uint64_t* out = (uint64_t*)llg_checked_calloc(out_words, sizeof(uint64_t),
                                                  "net strength view");
    int forced = llg_is_forced(&net->resolved);
    int mode = net->resolution;
    int implicit = -1;
    int implicit_strength = LLG_STRENGTH_PULL;
    if (mode == LLG_RESOLVE_TRI0 || mode == LLG_RESOLVE_SUPPLY0) implicit = 0;
    if (mode == LLG_RESOLVE_TRI1 || mode == LLG_RESOLVE_SUPPLY1) implicit = 1;
    if (mode == LLG_RESOLVE_SUPPLY0 || mode == LLG_RESOLVE_SUPPLY1)
        implicit_strength = LLG_STRENGTH_SUPPLY;
    for (size_t word = 0; word < words; word++) {
        uint32_t remaining = width - (uint32_t)(word * 64u);
        uint64_t m = remaining >= 64u ? UINT64_MAX
                                      : (UINT64_C(1) << remaining) - UINT64_C(1);
        uint64_t known0[8] = {0};
        uint64_t known1[8] = {0};
        uint64_t possible0[8] = {0};
        uint64_t possible1[8] = {0};
        if (implicit == 0) known0[implicit_strength] = possible0[implicit_strength] = m;
        if (implicit == 1) known1[implicit_strength] = possible1[implicit_strength] = m;
        for (int d = 0; d < net->n_drivers; d++) {
            const sv4_t* v = net->drivers[d];
            if (!v) continue;
            uint8_t s0 = net->strength0 ? net->strength0[d] : LLG_STRENGTH_STRONG;
            uint8_t s1 = net->strength1 ? net->strength1[d] : LLG_STRENGTH_STRONG;
            if (s0 > LLG_STRENGTH_SUPPLY || s1 > LLG_STRENGTH_SUPPLY) {
                fputs("llg runtime fatal: invalid net drive strength\n", stderr);
                abort();
            }
            uint64_t valid = word < llg_sv4_words(*v)
                ? (llg_sv4_width(*v) - (uint32_t)(word * 64u) >= 64u
                       ? UINT64_MAX
                       : (UINT64_C(1) << (llg_sv4_width(*v) - (uint32_t)(word * 64u))) -
                             UINT64_C(1))
                : 0;
            uint64_t x = llg_sv4_word(*v, word, LLG_SV4_X) & valid & m;
            uint64_t z = (llg_sv4_word(*v, word, LLG_SV4_Z) | ~valid) & m;
            uint64_t bits = llg_sv4_word(*v, word, LLG_SV4_BITS);
            uint64_t k0 = ~bits & ~(x | z) & m;
            uint64_t k1 = bits & ~(x | z) & m;
            if (s0 != LLG_STRENGTH_HIGHZ) {
                known0[s0] |= k0;
                possible0[s0] |= k0 | x;
            }
            if (s1 != LLG_STRENGTH_HIGHZ) {
                known1[s1] |= k1;
                possible1[s1] |= k1 | x;
            }
        }
        uint64_t forced_bits = forced ? force_mask_word(&net->resolved, word) : 0;
        for (uint32_t offset = 0; offset < 64u && offset < remaining; offset++) {
            uint64_t bit = UINT64_C(1) << offset;
            uint64_t net_bit = (uint64_t)word * 64u + offset;
            uint8_t code = forced_bits & bit
                ? llg_net_forced_strength(&net->resolved, net_bit)
                : llg_net_strength_hull(mode, llg_net_top_level(known0, bit),
                                        llg_net_top_level(known1, bit),
                                        llg_net_top_level(possible0, bit),
                                        llg_net_top_level(possible1, bit));
            out[net_bit / 8u] |= (uint64_t)code << ((net_bit % 8u) * 8u);
        }
    }
    sv4_t view = sv4_from_limbs(out, NULL, NULL, width * 8u, 0);
    free(out);
    return view;
}

// Publish the strength view after the value. Unchanged strengths are not
// rewritten, so only a real strength change wakes `%v` monitors. A pending
// net propagation delay publishes the view with the delayed value instead.
static void llg_net_strength_publish(llg_net_t* net) {
    if (!net || !net->strength) return;
    if (net->propagation && net->propagation->pending) return;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(&owned[0], llg_net_strength_levels(net));
    sig_write(net->strength, owned[0]);
    llg_value_scope_end(scope);
}

// Startup: initialize the view from the all-Z drivers without waking readers.
void llg_net_strength_reset(llg_net_t* net) {
    if (!net || !net->strength) return;
    sv4_replace(net->strength, llg_net_strength_levels(net));
}

static void llg_net_publish(llg_net_t* net, sv4_t resolved) {
    if (net->propagation_enabled) {
        llg_inertial_assign(&net->propagation, &net->resolved, resolved,
                            net->propagation_rise, net->propagation_fall,
                            net->propagation_turn_off);
        if (net->propagation) net->propagation->publication_net = net;
    } else {
        sig_write(&net->resolved, resolved);
        llg_net_alias_refresh_all(net);
    }
}

static void llg_net_publish_range(llg_net_t* net, uint32_t low,
                                  sv4_t resolved) {
    if (sig_write_range(&net->resolved, low, resolved))
        llg_net_alias_refresh_all(net);
}

static void llg_net_publish_ranges(llg_net_t* net,
                                   uint32_t first_low, sv4_t first,
                                   uint32_t second_low, sv4_t second) {
    if (sig_write_ranges(&net->resolved, first_low, first,
                         second_low, second, 1))
        llg_net_alias_refresh_all(net);
}

void llg_net_resolve(llg_net_t* net) {
    if (!net || !region_can_mutate("net resolution")) return;
    if (llg_is_forced(&net->resolved)) {
        force_recompute_target(&net->resolved, net);
    } else {
        llg_value_scope_t* scope = llg_value_scope_begin(1);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute(net));
        llg_net_publish(net, owned[0]);
        llg_value_scope_end(scope);
    }
    llg_net_strength_publish(net);
}

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    if (!net || !region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers || !net->drivers[idx]) return;
    sv4_t replacement = sv4_resize(value, net->width, net->is_signed);
    sv4_t* slot = net->drivers[idx];
    if (sv4_same(*slot, replacement)) {
        sv4_destroy(&replacement);
        return;
    }
    uint32_t old_low = 0;
    uint32_t old_high = 0;
    int old_active = 0;
    if (llg_net_has_index(net)) {
        llg_net_driver_index_t* node = &net->driver_index[idx];
        old_low = node->low;
        old_high = node->high;
        old_active = node->active;
        llg_net_index_remove(net, idx);
    }
    sv4_move(slot, &replacement);
    int new_active = !llg_net_all_z(slot);
    if (llg_net_has_index(net) && new_active)
        llg_net_index_insert(net, idx, 0, net->width - 1u);
    // A selected force may cover only part of the net. Recompute through the
    // force path so unforced bits still publish underlying driver changes.
    if (llg_is_forced(&net->resolved)) {
        llg_net_resolve(net);
        return;
    }
    if (!llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_resolve(net);
        return;
    }
    uint32_t low = old_active ? old_low : 0;
    uint32_t high = old_active ? old_high : net->width - 1u;
    if (!old_active && !new_active) return;
    if (new_active) {
        if (!old_active || low > 0) low = 0;
        if (!old_active || high < net->width - 1u) high = net->width - 1u;
    }
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(owned, llg_net_compute_range(net, low, high - low + 1u));
    llg_net_publish_range(net, low, owned[0]);
    llg_value_scope_end(scope);
    llg_net_strength_publish(net);
}

static int llg_net_range_same(const sv4_t* target, uint32_t offset,
                              const sv4_t* value) {
    return llg_sv4_range_same(*target, offset, *value);
}

static void llg_net_range_fill_z(sv4_t* target, uint32_t offset,
                                 uint32_t width) {
    llg_sv4_range_fill(target, offset, width, 3);
}

static void llg_net_range_copy(sv4_t* target, uint32_t offset,
                               const sv4_t* value) {
    llg_sv4_range_copy(target, offset, *value);
}

static void llg_net_write_slice(llg_net_t* net, int idx, sv4_t selected,
                                uint32_t new_low) {
    sv4_t* slot = net->drivers[idx];
    uint32_t old_low = 0;
    uint32_t old_high = net->width - 1u;
    int old_active = 1;
    if (llg_net_has_index(net)) {
        llg_net_driver_index_t* node = &net->driver_index[idx];
        old_low = node->low;
        old_high = node->high;
        old_active = node->active;
    }
    int new_active = llg_sv4_width(selected) && !llg_net_all_z(&selected);
    uint32_t new_high = new_active ? new_low + llg_sv4_width(selected) - 1u : new_low;
    if (old_active == new_active &&
        (!new_active || (old_low == new_low && old_high == new_high &&
                         llg_net_range_same(slot, new_low, &selected)))) {
        return;
    }
    if (llg_net_has_index(net)) llg_net_index_remove(net, idx);
    if (llg_net_has_index(net)) {
        if (old_active) llg_net_range_fill_z(slot, old_low, old_high - old_low + 1u);
    } else {
        llg_net_range_fill_z(slot, 0, net->width);
        old_low = 0;
        old_high = net->width - 1u;
    }
    if (new_active) {
        llg_net_range_copy(slot, new_low, &selected);
        if (llg_net_has_index(net)) llg_net_index_insert(net, idx, new_low, new_high);
    }
    if (llg_is_forced(&net->resolved)) {
        llg_net_resolve(net);
        return;
    }
    if (!llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_resolve(net);
        return;
    }
    if (!old_active && !new_active) return;
    if (old_active && new_active &&
        (old_high + 1u < new_low || new_high + 1u < old_low)) {
        llg_value_scope_t* scope = llg_value_scope_begin(2);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute_range(
            net, old_low, old_high - old_low + 1u));
        sv4_replace(&owned[1], llg_net_compute_range(
            net, new_low, new_high - new_low + 1u));
        llg_net_publish_ranges(net, old_low, owned[0], new_low, owned[1]);
        llg_value_scope_end(scope);
        llg_net_strength_publish(net);
        return;
    }
    uint32_t low = old_active ? old_low : new_low;
    uint32_t high = old_active ? old_high : new_high;
    if (new_active) {
        if (!old_active || new_low < low) low = new_low;
        if (!old_active || new_high > high) high = new_high;
    }
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(owned, llg_net_compute_range(net, low, high - low + 1u));
    llg_net_publish_range(net, low, owned[0]);
    llg_value_scope_end(scope);
    llg_net_strength_publish(net);
}

void llg_net_write_selected(llg_net_t* net, int idx, sv4_t value,
                            sv4_select_plan_t plan, int reverse) {
    if (!net || !region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers || !net->drivers[idx] ||
        plan.storage_width != net->width) {
        fputs("llg: fatal: invalid selected net driver\n", stderr);
        abort();
    }
    sv4_t selected = sv4_select_plan_slice(value, &plan, reverse);
    llg_net_write_slice(net, idx, selected, plan.storage_lsb);
    sv4_destroy(&selected);
}

/* Grow one net's alias list to hold at least one more entry. The old table
 * stays live until the grown copy is complete, so an allocation failure aborts
 * without leaving the net partially rebound. */
static void llg_net_alias_reserve(llg_net_t* net) {
    if (net->n_aliases < net->alias_capacity) return;
    if (net->alias_capacity < 0) {
        fputs("llg: fatal: net alias capacity is invalid\n", stderr);
        abort();
    }
    int capacity = net->alias_capacity ? net->alias_capacity : 4;
    while (capacity <= net->n_aliases) {
        if (capacity > INT_MAX / 2) {
            fputs("llg: fatal: net alias capacity overflow\n", stderr);
            abort();
        }
        capacity *= 2;
    }
    llg_net_alias_t** grown = (llg_net_alias_t**)llg_checked_malloc(
        (size_t)capacity, sizeof(*grown), "net alias table");
    for (int i = 0; i < net->n_aliases; i++) grown[i] = net->aliases[i];
    free(net->aliases);
    net->aliases = grown;
    net->alias_capacity = capacity;
}

void llg_net_alias_bind(llg_net_alias_t* alias) {
    if (!alias || !alias->parts || alias->n_parts == 0) return;
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        llg_net_t* net = alias->parts[i].net;
        if (!net) continue;
        int seen = 0;
        for (int j = 0; j < net->n_aliases; j++)
            if (net->aliases[j] == alias) seen = 1;
        if (seen) continue;
        llg_net_alias_reserve(net);
        net->aliases[net->n_aliases++] = alias;
    }
    llg_net_alias_refresh(alias);
}

void llg_net_alias_clear(llg_net_t* net) {
    if (!net) return;
    free(net->aliases);
    net->aliases = NULL;
    net->n_aliases = 0;
    net->alias_capacity = 0;
}

sv4_t llg_net_alias_read(llg_net_alias_t* alias) {
    // Driver/force/propagation commits publish this view before readers run.
    // Observing it, including from Postponed, must never perform a write.
    return alias ? sv4_clone(&alias->visible) : sv4_from_u64(0, 1, 0);
}

void llg_net_alias_write(llg_net_alias_t* alias, sv4_t value) {
    if (!alias || !alias->parts || !region_can_mutate("net alias write")) return;
    /* A publication callback may reenter or terminate the current process.
     * Both the input snapshot and each contribution must outlive that edge. */
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_copy(&owned[0], &value);
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        const llg_net_alias_part_t* part = &alias->parts[i];
        if (!part->net) continue;
        int seen = 0;
        for (uint32_t j = 0; j < i; j++) {
            const llg_net_alias_part_t* prior = &alias->parts[j];
            if (prior->net == part->net && prior->slot == part->slot) seen = 1;
        }
        if (seen) continue;
        sv4_replace(&owned[1], sv4_fill(3, part->net->width, part->net->is_signed));
        for (uint32_t j = i; j < alias->n_parts; j++) {
            const llg_net_alias_part_t* mapped = &alias->parts[j];
            if (mapped->net != part->net || mapped->slot != part->slot ||
                mapped->bit_count == 0 ||
                (uint64_t)mapped->signal_bit + mapped->bit_count > llg_sv4_width(owned[0]) ||
                (uint64_t)mapped->group_bit + mapped->bit_count > llg_sv4_width(owned[1]))
                continue;
            sv4_t bits = sv4_part_select(owned[0],
                                         (int64_t)mapped->signal_bit + mapped->bit_count - 1,
                                         mapped->signal_bit);
            sv4_part_select_set(&owned[1], (int64_t)mapped->group_bit + mapped->bit_count - 1,
                                mapped->group_bit, bits);
            sv4_destroy(&bits);
        }
        llg_net_write(part->net, part->slot, owned[1]);
        sv4_destroy(&owned[1]);
    }
    llg_value_scope_end(scope);
}

static int inertial_bit(const sv4_t* value, uint32_t bit) {
    if (!value || bit >= llg_sv4_width(*value)) return 0;
    return (int)llg_sv4_state(*value, bit);
}

static int inertial_masked_same(const sv4_t* a, const sv4_t* b,
                                const sv4_t* mask) {
    return llg_sv4_masked_same(*a, *b, mask);
}

static void inertial_merge(sv4_t* target, const sv4_t* value,
                           const sv4_t* mask) {
    llg_sv4_masked_copy(target, *value, *mask);
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
    int has_transition = 0;
    uint32_t width = llg_sv4_width(*old_value) < llg_sv4_width(*new_value)
                         ? llg_sv4_width(*old_value)
                         : llg_sv4_width(*new_value);
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
        // UINT64_MAX is a valid delay. Track whether a transition was found
        // separately so the maximum delay is not mistaken for "no transition"
        // and silently converted to zero.
        if (!has_transition || ticks < selected) selected = ticks;
        has_transition = 1;
    }
    return has_transition ? selected : 0;
}

static void inertial_unlink_pending(llg_inertial_t* driver) {
    if (!driver->pending) return;
    llg_inertial_t** entry = &g.inertial_pending;
    while (*entry && *entry != driver) entry = &(*entry)->next_pending;
    if (*entry) *entry = driver->next_pending;
    driver->pending = 0;
    driver->next_pending = NULL;
    sv4_destroy(&driver->value);
    sv4_destroy(&driver->mask);
    driver->has_mask = 0;

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
        sv4_copy(&driver->current, target);
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
        driver->publication_net = NULL;
        driver->slot = slot;
        sv4_copy(&driver->current, target);
    }
    driver->region = region_is_reactive(g.current_region)
                         ? LLG_REGION_REACTIVE
                         : LLG_REGION_ACTIVE;
    value = sv4_resize(value, llg_sv4_width(*target), llg_sv4_signed(*target));
    sv4_t selected_mask = SV4_EMPTY;
    if (mask) {
        selected_mask = sv4_resize(*mask, llg_sv4_width(*target), 0);
    }
    const sv4_t* effective_mask = mask ? &selected_mask : NULL;
    uint64_t ticks;
    llg_inertial_t** entry;
    if (driver->pending) {
        // Unchanged expression values keep the original propagation time.
        if (driver->has_mask == (effective_mask != NULL) &&
            (!effective_mask || sv4_same(driver->mask, *effective_mask)) &&
            inertial_masked_same(&driver->value, &value, effective_mask))
            goto cleanup;
        inertial_unlink_pending(driver);
    }
    if (effective_mask ? inertial_masked_same(&driver->current, &value, effective_mask)
                       : sv4_same(driver->current, value))
        goto cleanup;
    driver->has_mask = effective_mask != NULL;
    if (effective_mask) sv4_copy(&driver->mask, effective_mask);
    driver->rise = rise;
    driver->fall = fall;
    driver->turn_off = turn_off;
    ticks = inertial_transition_ticks(
        &driver->current, &value, effective_mask, rise, fall, turn_off);
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling an inertial update\n");
        abort();
    }
    sv4_move(&driver->value, &value);
    driver->time = g.now + ticks;
    driver->pending = 1;
    entry = &g.inertial_pending;
    while (*entry && (*entry)->time <= driver->time) entry = &(*entry)->next_pending;
    driver->next_pending = *entry;
    *entry = driver;
cleanup:
    sv4_destroy(&value);
    sv4_destroy(&selected_mask);

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

    // Detach before publishing: callbacks may schedule another update on driver.
    sv4_t value = SV4_EMPTY;
    sv4_t mask = SV4_EMPTY;
    sv4_move(&value, &driver->value);
    sv4_move(&mask, &driver->mask);
    int has_mask = driver->has_mask;
    driver->has_mask = 0;
    llg_net_t* publication_net = driver->publication_net;
    if (has_mask) {
        sv4_t merged = sv4_clone(driver->target);
        inertial_merge(&merged, &value, &mask);
        sv4_move(&value, &merged);
    }
    uint32_t range_offset = 0;
    uint32_t range_width = 0;
    int selected_net = driver->net && has_mask &&
        nba_mask_contiguous(driver->net->width, value, mask,
                            &range_offset, &range_width);
    if (selected_net) {
        sv4_t selected = range_width
            ? sv4_part_select(value, range_offset + range_width - 1u,
                              range_offset)
            : (sv4_t)SV4_EMPTY;
        sv4_t current = sv4_fill(3, driver->net->width,
                                 driver->net->is_signed);
        if (range_width)
            llg_net_range_copy(&current, range_offset, &selected);
        sv4_copy(&driver->current, &current);
        sv4_destroy(&current);
        llg_net_write_slice(driver->net, driver->slot, selected, range_offset);
        sv4_destroy(&selected);
    } else if (driver->net) {
        sv4_copy(&driver->current, &value);
        llg_net_write(driver->net, driver->slot, value);
    } else {
        sv4_copy(&driver->current, &value);
        llg_ba(driver->target, value);
    }
    if (publication_net) {
        llg_net_alias_refresh_all(publication_net);
        llg_net_strength_publish(publication_net);
    }
    sv4_destroy(&mask);
    sv4_destroy(&value);
}

static int nba_due(llg_region_t region) {
    promote_delayed_nbas();
    return g.nba_queues[region].head != NULL;
}

// SV 11.9 checks a member assignment against the tag current when it is
// performed. A member NBA is performed here, so a blocking or other-process
// retag after issue makes the queued member write a runtime error; the
// write is dropped rather than storing one member's payload under another
// member's tag (SV 7.3.2).
static int nba_tag_commit_valid(const llg_ref_view_t* view, const sv4_t* target) {
    size_t failed = 0;
    if (llg_ref_view_valid(view, target, &failed)) return 1;
    const char* member = "<unknown>";
    if (failed < view->tag_check_count && view->tag_checks[failed].member_name)
        member = view->tag_checks[failed].member_name;
    llg_rt_mark_failed();
    fprintf(stderr,
            "llg: runtime error: nonblocking write to tagged-union member %s at %s "
            "found an inactive tag at commit\n",
            member, view->location ? view->location : "<unknown>");
    fflush(stderr);
    return 0;
}

static void apply_nba(llg_nba_t* next) {
    if (next->fixed_target) {
        fixed_array_apply(next->fixed_target, next->fixed_value);
    } else if (next->is_event) {
        event_trigger_object(next->event_target);
    } else if (next->is_string) {
        if (next->string_target) {
            llg_string_move(next->string_target, next->string_value);
            next->string_value = (llg_string_t){0};
        }
    } else if (next->is_real) {
        if (!llg_is_real_forced(next->real_target) && !pca_real_active(next->real_target))
            real_write(next->real_target, next->real_value);
    } else {
        sv4_t* target = next->target;
        if (next->net_target) {
            if (next->net_slot < 0 || next->net_slot >= next->net_target->n_drivers)
                return;
            target = next->net_target->drivers[next->net_slot];
        } else if (llg_is_forced(target) || pca_active(target)) return;
        if (!target) return;
        if (next->tag_view && !nba_tag_commit_valid(next->tag_view, target)) return;
        sv4_t value = (next->has_mask || next->has_range)
                          ? sv4_clone(target)
                          : sv4_clone(&next->value);
        if (next->has_mask) {
            llg_sv4_masked_merge(&value, next->value, next->mask);
        } else if (next->has_range) {
            if (next->range_width) {
                sv4_select_plan_t plan = {
                    llg_sv4_width(value), next->range_width, next->range_offset, 0,
                    next->range_width,
                };
                sv4_select_plan_set(&value, &plan, next->value);
            }
        }
        if (next->net_target) llg_net_write(next->net_target, next->net_slot, value);
        else sig_write(target, value);
        sv4_destroy(&value);
    }
}

static void commit_nbas(llg_region_t region) {
    promote_delayed_nbas();
    llg_nba_queue_t* queue = &g.nba_queues[region];
    while (queue->head) {
        llg_nba_t* next = queue->head;
        nba_queue_remove(queue, next);
        nba_owner_remove(next);
        apply_nba(next);
        nba_destroy(next);
    }
}
// Checked size arithmetic for per-conversion formatting scratch.
static size_t llg_format_size_add(size_t a, size_t b) {
    if (b > SIZE_MAX - a) llg_fatal_allocation("format scratch", a, b);
    return a + b;
}

static size_t llg_format_scratch_size(size_t payload, size_t precision) {
    size_t cap = llg_format_size_add(payload, precision);
    cap = llg_format_size_add(cap, (size_t)g.time_format.precision);
    cap = llg_format_size_add(cap, g.time_format.suffix.len);
    return llg_format_size_add(cap, 512u);
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
            int has_width;
            int width;
            int zero;
            p = llg_parse_legacy_spec(p, &has_width, &width, &zero);
            c = *p;
            if (c) ++p;
            if (c == '%') {
                llg_append(out, cap, &len, '%');
            } else if ((c == 'd' || c == 'h' || c == 'b' || c == 'o' || c == 't') &&
                       argi < n) {
                size_t tmp_cap = llg_format_scratch_size(llg_sv4_width(args[argi]), 0);
                char* tmp = llg_checked_malloc(tmp_cap, 1, "packed format");
                size_t tmp_len;
                if (c == 't') {
                    tmp_len = llg_format_time_integer(args[argi++],
                                                      g.design_precision_fs,
                                                      tmp, tmp_cap);
                    if (!has_width && !zero) width = g.time_format.minimum_field_width;
                } else {
                    sv4_format(c, args[argi++], tmp, tmp_cap);
                    tmp_len = strlen(tmp);
                }
                while (width > 0 && (size_t)width > tmp_len && len + 1 < cap) {
                    out[len++] = ' ';
                    width--;
                }
                for (size_t i = 0; i < tmp_len && len + 1 < cap; i++)
                    out[len++] = tmp[i];
                free(tmp);
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
        size_t extra = (size_t)llg_sv4_width(args[i]) + 2u;
        if ((size_t)g.time_format.minimum_field_width > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.minimum_field_width;
        if ((size_t)g.time_format.precision > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.precision;
        if (g.time_format.suffix.len > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += g.time_format.suffix.len;
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
        else if (args[i].kind == LLG_FMT_PACKED || args[i].kind == LLG_FMT_STRENGTH)
            sv4_destroy(&args[i].value.packed);
        memset(&args[i], 0, sizeof(args[i]));
    }
}

static llg_fmt_arg_t llg_fmt_arg_clone(const llg_fmt_arg_t* value) {
    llg_fmt_arg_t result = *value;
    if (value->kind == LLG_FMT_STRING)
        result.value.string = llg_string_clone(&value->value.string);
    else if (value->kind == LLG_FMT_PACKED || value->kind == LLG_FMT_STRENGTH)
        result.value.packed = sv4_clone(&value->value.packed);
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

static int llg_decimal_increment(char* digits, size_t* length, size_t cap) {
    for (size_t i = *length; i > 0; --i) {
        if (digits[i - 1] != '9') {
            digits[i - 1]++;
            return 1;
        }
        digits[i - 1] = '0';
    }
    if (*length >= cap) return 0;
    memmove(digits + 1, digits, *length);
    digits[0] = '1';
    (*length)++;
    return 1;
}

static int llg_time_format_exponents(int* source, int* display) {
    int display_exponent = llg_time_unit_exponent(g.time_format.unit_fs);
    if (display_exponent == INT_MIN) return 0;
    int source_exponent = llg_time_unit_exponent(g.design_precision_fs);
    if (source_exponent == INT_MIN) source_exponent = display_exponent;
    *source = source_exponent;
    *display = display_exponent;
    return 1;
}

// Convert an integral time argument from its owning scope's unit to the
// design-wide `$timeformat` unit, rounding the discarded decimal digits half
// up.  The conversion operates on decimal digits so wide four-state values do
// not pass through a host integer or floating-point type.
static size_t llg_format_time_integer(sv4_t value, uint64_t source_unit_fs,
                                      char* raw, size_t cap) {
    size_t decimal_cap = (size_t)llg_sv4_width(value) + 3u;
    size_t scaled_cap = llg_format_scratch_size(llg_sv4_width(value), 0);
    char* decimal = llg_checked_malloc(decimal_cap, 1, "time digits");
    char* scaled = llg_checked_malloc(scaled_cap, 1, "scaled time digits");
    sv4_to_dec_string(value, decimal, decimal_cap);
    size_t decimal_len = strlen(decimal);
    size_t len = 0;
    int source_exponent;
    int display_exponent;
    int metadata_exponent;
    int negative;
    const char* digits;
    size_t digits_len;
    int scale;
    size_t scaled_len;
    int precision;
    if (decimal_len == 0) goto cleanup;
    if (strchr("xXzZ", decimal[0])) {
        llg_append_text(raw, cap, &len, decimal, decimal_len);
        llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                        g.time_format.suffix.len);
        goto cleanup;
    }
    if (!llg_time_format_exponents(&source_exponent, &display_exponent)) {
        source_exponent = display_exponent = 0;
    }
    metadata_exponent = llg_time_unit_exponent(source_unit_fs);
    if (metadata_exponent != INT_MIN) source_exponent = metadata_exponent;
    negative = decimal[0] == '-';
    digits = decimal + (negative ? 1 : 0);
    digits_len = decimal_len - (negative ? 1u : 0u);
    scale = source_exponent - display_exponent + g.time_format.precision;
    scaled_len = 0;
    if (scale >= 0) {
        // Multiplying zero by a power of ten must not manufacture trailing
        // zero digits; keeping its canonical representation also preserves
        // the expected `%0t` spelling at time zero.
        if (digits_len == 1 && digits[0] == '0') {
            scaled[0] = '0';
            scaled_len = 1;
        } else {
            size_t max_scaled = scaled_cap - 1u;
            if (digits_len > max_scaled || (size_t)scale > max_scaled - digits_len)
                llg_fatal_allocation("formatted time", 1,
                                     digits_len + (size_t)scale + 1u);
            memcpy(scaled, digits, digits_len);
            scaled_len = digits_len;
            for (int i = 0; i < scale; ++i)
                scaled[scaled_len++] = '0';
        }
    } else {
        size_t drop = (size_t)(-scale);
        size_t keep = drop < digits_len ? digits_len - drop : 0;
        if (keep) memcpy(scaled, digits, keep);
        scaled_len = keep;
        if (scaled_len == 0) scaled[scaled_len++] = '0';
        // If the value has fewer digits than the discarded scale, the
        // leading discarded decimal digits are zero (for example 7/10^9),
        // so inspect the first actually discarded digit only when it exists.
        int round_up = drop <= digits_len && digits[digits_len - drop] >= '5';
        if (round_up) (void)llg_decimal_increment(scaled, &scaled_len, scaled_cap);
    }
    precision = g.time_format.precision;
    if (negative) llg_append(raw, cap, &len, '-');
    if (scaled_len > (size_t)precision) {
        size_t integer_len = scaled_len - (size_t)precision;
        llg_append_text(raw, cap, &len, scaled, integer_len);
        if (precision) {
            llg_append(raw, cap, &len, '.');
            llg_append_text(raw, cap, &len, scaled + integer_len,
                            (size_t)precision);
        }
    } else {
        llg_append(raw, cap, &len, '0');
        if (precision) {
            llg_append(raw, cap, &len, '.');
            for (size_t i = scaled_len; i < (size_t)precision; ++i)
                llg_append(raw, cap, &len, '0');
            llg_append_text(raw, cap, &len, scaled, scaled_len);
        }
    }
    llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                    g.time_format.suffix.len);
cleanup:
    free(scaled);
    free(decimal);
    return len;
}

static size_t llg_format_time_real(double value, uint64_t source_unit_fs,
                                   char* raw, size_t cap) {
    int source_exponent;
    int display_exponent;
    if (!llg_time_format_exponents(&source_exponent, &display_exponent)) {
        source_exponent = display_exponent = 0;
    }
    int metadata_exponent = llg_time_unit_exponent(source_unit_fs);
    if (metadata_exponent != INT_MIN) source_exponent = metadata_exponent;
    double scale = (double)llg_time_unit_from_exponent(source_exponent) /
                   (double)llg_time_unit_from_exponent(display_exponent);
    // C's printf family is allowed to honor the process rounding mode.  The
    // simulator's time formatter instead uses the standard decimal rule for
    // discarded digits (exact halves away from zero), so round in the scaled
    // decimal domain before asking snprintf only to render the fixed digits.
    double scaled = value * scale;
    double factor = 1.0;
    for (int i = 0; i < g.time_format.precision && isfinite(factor); ++i)
        factor *= 10.0;
    if (isfinite(scaled)) {
        double rounded = round(scaled * factor);
        if (isfinite(rounded)) scaled = rounded / factor;
    }
    int written = snprintf(raw, cap, "%.*f", g.time_format.precision,
                           scaled);
    size_t len = written < 0 ? 0 : (size_t)written;
    if (len >= cap) len = cap ? cap - 1 : 0;
    llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                    g.time_format.suffix.len);
    return len;
}

static size_t llg_format_raw2(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (llg_sv4_width(value) + 63u) / 64u;
    uint32_t last_bits = llg_sv4_width(value) % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        // SFormat::formatRaw2 flattens X/Z to zero and emits the native
        // little-endian limb bytes, including the complete last 32-bit half
        // for values whose width is between 33 and 64 bits.
        uint64_t bits = llg_sv4_word(value, i, LLG_SV4_BITS) & ~(llg_sv4_word(value, i, LLG_SV4_X) | llg_sv4_word(value, i, LLG_SV4_Z));
        size_t bytes = (i == words - 1 && last_bits <= 32) ? sizeof(uint32_t)
                                                            : sizeof(uint64_t);
        for (size_t j = 0; j < bytes && len < cap; j++)
            raw[len++] = (char)(bits >> (j * 8));
    }
    return len;
}

static size_t llg_format_raw4(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (llg_sv4_width(value) + 63u) / 64u;
    uint32_t last_bits = llg_sv4_width(value) % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        uint64_t unknown = llg_sv4_word(value, i, LLG_SV4_X) | llg_sv4_word(value, i, LLG_SV4_Z);
        uint64_t bits = llg_sv4_word(value, i, LLG_SV4_BITS);
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
    for (uint32_t bit = llg_sv4_width(value); bit > 0; bit--) {
        uint32_t index = bit - 1;
        uint64_t mask = UINT64_C(1) << (index % 64u);
        uint32_t limb = index / 64u;
        const char* text;
        if (llg_sv4_word(value, limb, LLG_SV4_X) & mask)
            text = "StX";
        else if (llg_sv4_word(value, limb, LLG_SV4_Z) & mask)
            text = "HiZ";
        else
            text = llg_sv4_word(value, limb, LLG_SV4_BITS) & mask ? "St1" : "St0";
        llg_append_text(raw, cap, &len, text, strlen(text));
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

// Format a net strength view (llg_net_t.strength) with IEEE 1364-2001
// 17.1.1.5 / Tables 69-71: a mnemonic for one level, otherwise two digits
// (max then min strength for 0/1; strength0 then strength1 for X). L and H
// always use the mnemonic of their driven level.
static void llg_format_strength_byte(uint8_t code, char text[4]) {
    static const char* const names[8] = {"Hi", "Sm", "Me", "We", "La", "Pu", "St", "Su"};
    int lo = (int)(code & 0x0fu) - 7;
    int hi = (int)(code >> 4) - 7;
    if (lo < -7 || hi > 7 || lo > hi) {
        lo = -LLG_STRENGTH_STRONG;
        hi = LLG_STRENGTH_STRONG;
    }
    char value;
    int first;
    int second;
    if (hi < 0) {
        value = '0';
        first = -lo;
        second = -hi;
    } else if (lo > 0) {
        value = '1';
        first = hi;
        second = lo;
    } else if (lo == 0 && hi == 0) {
        memcpy(text, "HiZ", 4);
        return;
    } else if (hi == 0) {
        value = 'L';
        first = second = -lo;
    } else if (lo == 0) {
        value = 'H';
        first = second = hi;
    } else {
        value = 'X';
        first = -lo;
        second = hi;
    }
    if (first == second) {
        memcpy(text, names[first], 2);
    } else {
        text[0] = (char)('0' + first);
        text[1] = (char)('0' + second);
    }
    text[2] = value;
    text[3] = 0;
}

static size_t llg_format_strength_view(sv4_t view, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t bits = llg_sv4_width(view) / 8u;
    for (uint32_t bit = bits; bit > 0; bit--) {
        uint32_t index = (bit - 1u) * 8u;
        uint64_t word = llg_sv4_word(view, index / 64u, LLG_SV4_BITS);
        char text[4];
        llg_format_strength_byte((uint8_t)(word >> (index % 64u)), text);
        llg_append_text(raw, cap, &len, text, 3);
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

static size_t llg_format_char(sv4_t value, char* raw, size_t cap) {
    if (cap == 0 || llg_sv4_width(value) == 0) return 0;
    uint64_t unknown = llg_sv4_word(value, 0, LLG_SV4_X) | llg_sv4_word(value, 0, LLG_SV4_Z);
    raw[0] = (char)(unknown & 0xffu ? 0xffu : llg_sv4_word(value, 0, LLG_SV4_BITS) & 0xffu);
    return 1;
}

static sv4_t llg_string_to_display_packed(const llg_string_t* value) {
    size_t max_bytes = (size_t)(LLG_SUPPORTED_WIDTH_LIMIT - 1u) / 8u;
    if (value->len > max_bytes) {
        fprintf(stderr,
                "llg runtime fatal: string display conversion exceeds packed width\n");
        abort();
    }
    uint32_t width = value->len ? (uint32_t)(value->len * 8u) : 8u;
    return llg_string_to_packed(llg_string_clone(value), width, 0);
}

static size_t llg_format_pattern_packed(sv4_t value, char* raw, size_t cap) {
    size_t digits_cap = (size_t)llg_sv4_width(value) + 3u;
    char* digits = llg_checked_malloc(digits_cap, 1, "pattern digits");
    int has_unknown = sv4_is_unknown(value);
    int all_x = has_unknown;
    int all_z = has_unknown;
    for (int i = 0; i < llg_sv4_nlimbs(llg_sv4_width(value)); i++) {
        uint64_t mask = llg_sv4_limb_mask(llg_sv4_width(value), i);
        all_x &= (llg_sv4_word(value, i, LLG_SV4_X) & mask) == mask;
        all_z &= (llg_sv4_word(value, i, LLG_SV4_Z) & mask) == mask;
    }
    int base;
    if ((llg_sv4_width(value) < 8u && !llg_sv4_signed(value)) ||
        (has_unknown && llg_sv4_width(value) <= 64u && !all_x && !all_z)) {
        base = 'b';
    } else if (llg_sv4_width(value) <= 32u || llg_sv4_signed(value) || all_x || all_z) {
        base = 'd';
    } else {
        base = 'h';
    }
    sv4_format((char)base, value, digits, digits_cap);
    size_t digits_len = strlen(digits);
    size_t len = 0;
    const char* digit_text = digits;
    int include_base = !(base == 'd' && llg_sv4_width(value) == 32u && llg_sv4_signed(value) && !has_unknown);
    if (digits_len && digits[0] == '-') {
        llg_append(raw, cap, &len, '-');
        digit_text++;
        digits_len--;
    }
    if (include_base) {
        char prefix[64];
        int written = snprintf(prefix, sizeof(prefix), "%u'%s%c", llg_sv4_width(value),
                               llg_sv4_signed(value) ? "s" : "", base);
        if (written > 0) llg_append_text(raw, cap, &len, prefix, (size_t)written);
    }
    llg_append_text(raw, cap, &len, digit_text, digits_len);
    free(digits);
    return len;
}

static void llg_emit_field(char* out, size_t cap, size_t* len,
                            const char* value, size_t value_len,
                            llg_fmt_spec_t spec, char conversion) {
    size_t field_cap = llg_format_size_add(value_len, (size_t)spec.precision);
    field_cap = llg_format_size_add(field_cap, 8u);
    char* field = llg_checked_malloc(field_cap, 1, "formatted field");
    size_t n = value_len;
    if (n > field_cap - 1) n = field_cap - 1;
    memcpy(field, value, n);
    field[n] = 0;
    if (spec.has_precision && conversion == 's' && n > (size_t)spec.precision)
        n = (size_t)spec.precision;
    if (spec.has_precision && strchr("dhbox", conversion)) {
        size_t sign = n && field[0] == '-' ? 1u : 0u;
        size_t digits = n - sign;
        while (digits < (size_t)spec.precision && n + 1 < field_cap) {
            memmove(field + sign + 1, field + sign, digits + 1);
            field[sign] = '0';
            n++;
            digits++;
        }
    }
    if (spec.alternate && strchr("hbo", conversion) && n > 0 &&
        !(n == 1 && strchr("xXzZ", field[0]))) {
        const char* prefix = conversion == 'h' ? "0x" : conversion == 'o' ? "0" : "0b";
        size_t prefix_len = strlen(prefix);
        if (n + prefix_len < field_cap) {
            memmove(field + prefix_len, field, n + 1);
            memcpy(field, prefix, prefix_len);
            n += prefix_len;
        }
    }
    int numeric = strchr("dhbotxfeg", conversion) != NULL;
    if (numeric && n > 0 && field[0] != '-' && spec.plus) {
        if (n + 1 < field_cap) {
            memmove(field + 1, field, n + 1);
            field[0] = '+';
            n++;
        }
    } else if (numeric && n > 0 && field[0] != '-' && spec.space) {
        if (n + 1 < field_cap) {
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
            free(field);
            return;
        }
    }
    if (!spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
    llg_append_text(out, cap, len, field, n);
    if (spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
    free(field);

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
        size_t payload = 0;
        if (arg->kind == LLG_FMT_PACKED || arg->kind == LLG_FMT_STRENGTH)
            payload = (size_t)llg_sv4_width(arg->value.packed) * 4u;
        else if (arg->kind == LLG_FMT_STRING) {
            if (arg->value.string.len > SIZE_MAX / 8u)
                llg_fatal_allocation("string display", arg->value.string.len, 8u);
            payload = arg->value.string.len * 8u;
        }
        size_t raw_cap = llg_format_scratch_size(payload, (size_t)spec.precision);
        raw_cap = llg_format_size_add(raw_cap, (size_t)spec.width);
        char* raw = llg_checked_malloc(raw_cap, 1, "typed format");
        size_t raw_len = 0;
        if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_PACKED) {
            sv4_format(conversion == 'x' ? 'h' : conversion, arg->value.packed,
                       raw, raw_cap);
            raw_len = strlen(raw);
        } else if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            sv4_format(conversion == 'x' ? 'h' : conversion, packed, raw, raw_cap);
            raw_len = strlen(raw);
            sv4_destroy(&packed);
        } else if (conversion == 't' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_time_integer(arg->value.packed,
                                              arg->time_unit_fs, raw, raw_cap);
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 't' && arg->kind == LLG_FMT_REAL) {
            raw_len = llg_format_time_real(arg->value.real, arg->time_unit_fs,
                                           raw, raw_cap);
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 'c' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_char(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'c' && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            raw_len = llg_format_char(packed, raw, raw_cap);
            sv4_destroy(&packed);
        } else if (conversion == 'u' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw2(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'z' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw4(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'v' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_strength(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'v' && arg->kind == LLG_FMT_STRENGTH) {
            raw_len = llg_format_strength_view(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_PACKED) {
            // Aggregate pattern formatting is rejected by lowering until the
            // owned aggregate representation is available.  A packed scalar
            // follows ConstantValue::toString's base-selection and literal
            // prefix rules, which is the scalar case of Slang's pattern
            // visitor.
            raw_len = llg_format_pattern_packed(arg->value.packed, raw, raw_cap);
        } else if (strchr("feg", conversion) && arg->kind == LLG_FMT_REAL) {
            char real_fmt[128];
            size_t spec_len = (size_t)(p - start);
            if (spec_len >= sizeof(real_fmt) - 1) spec_len = sizeof(real_fmt) - 2;
            memcpy(real_fmt, start, spec_len);
            real_fmt[spec_len] = 0;
            int written = snprintf(raw, raw_cap, real_fmt, arg->value.real);
            raw_len = written < 0 ? 0 : (size_t)written < raw_cap
                                           ? (size_t)written
                                           : raw_cap - 1;
        } else if (conversion == 's' && arg->kind == LLG_FMT_PACKED) {
            llg_string_t value = llg_string_from_packed(arg->value.packed);
            raw_len = value.len;
            if (raw_len > raw_cap) raw_len = raw_cap;
            if (raw_len) memcpy(raw, value.data, raw_len);
            llg_string_destroy(&value);
        } else if (conversion == 's' && arg->kind == LLG_FMT_STRING) {
            raw_len = arg->value.string.len;
            if (raw_len > raw_cap) raw_len = raw_cap;
            if (raw_len) memcpy(raw, arg->value.string.data, raw_len);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_STRING) {
            // Keep a string pattern visibly distinct from `%s`, matching the
            // quote-delimited form produced by Slang's pattern formatter.
            size_t value_len = arg->value.string.len;
            if (value_len + 2u <= raw_cap) {
                raw[0] = '"';
                if (value_len) memcpy(raw + 1, arg->value.string.data, value_len);
                raw[value_len + 1] = '"';
                raw_len = value_len + 2u;
            } else {
                raw[0] = '"';
                raw_len = raw_cap;
                if (raw_len > 1) {
                    size_t copy = raw_len - 2u;
                    memcpy(raw + 1, arg->value.string.data, copy);
                    raw[raw_len - 1] = '"';
                }
            }
        } else {
            free(raw);
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        llg_emit_field(out, cap, &len, raw, raw_len, spec, conversion);
        free(raw);
    }
    out[len] = 0;
    return len;
}

static void llg_print_typed_to(uint32_t descriptor, const char* fmt,
                               llg_fmt_arg_t* args, int n, const char* scope,
                               int newline);

static void llg_print_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                            const char* scope, int newline) {
    llg_print_typed_to(1u, fmt, args, n, scope, newline);
}

static void llg_file_set_message(char* target, size_t capacity, const char* message) {
    if (!capacity) return;
    const char* source = message ? message : "";
    size_t length = 0;
    while (length < capacity - 1u && source[length]) ++length;
    // Diagnostic text may be reused or shortened in place. Bound the read and
    // accept overlap without strncpy's implicit padding/truncation contract.
    memmove(target, source, length);
    target[length] = 0;
}

static void llg_file_global_failure(const char* message) {
    llg_file_global_error = 1;
    llg_file_set_message(llg_file_global_message, sizeof(llg_file_global_message), message);
}

static void llg_file_slot_failure(llg_file_slot_t* slot, const char* message) {
    slot->error = 1;
    llg_file_set_message(slot->message, sizeof(slot->message), message);
}

static void llg_file_init_table(void) {
    if (llg_files_initialized) return;
    memset(llg_file_slots, 0, sizeof(llg_file_slots));
    llg_file_slots[LLG_FILE_STDIN].stream = stdin;
    llg_file_slots[LLG_FILE_STDOUT].stream = stdout;
    llg_file_slots[LLG_FILE_STDERR].stream = stderr;
    for (unsigned i = 0; i < 3u; ++i) llg_file_slots[i].open = 1;
    llg_files_initialized = 1;
    llg_file_global_error = 0;
    llg_file_global_message[0] = 0;
}

static uint32_t llg_file_mcd_bit(unsigned slot) {
    if (slot == LLG_FILE_STDOUT) return 1u;
    if (slot >= LLG_FILE_MCD_FIRST && slot < LLG_FILE_MCD_END)
        return UINT32_C(1) << (slot - 2u);
    return 0;
}

static int llg_file_selected(uint32_t descriptor, unsigned slot) {
    if (descriptor & LLG_FILE_FD_TAG)
        return (descriptor & ~LLG_FILE_FD_TAG) == slot;
    return (descriptor & llg_file_mcd_bit(slot)) != 0;
}

static int llg_file_mask_valid(uint32_t descriptor) {
    llg_file_init_table();
    if (!descriptor) {
        llg_file_global_failure("invalid file descriptor");
        return 0;
    }
    if (descriptor & LLG_FILE_FD_TAG) {
        uint32_t index = descriptor & ~LLG_FILE_FD_TAG;
        if (index >= LLG_FILE_SLOTS ||
            (index >= 3u && index < LLG_FILE_FD_FIRST) ||
            !llg_file_slots[index].open || !llg_file_slots[index].stream) {
            llg_file_global_failure("invalid or closed file descriptor");
            return 0;
        }
        return 1;
    }
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (llg_file_selected(descriptor, i) &&
            (!llg_file_slots[i].open || !llg_file_slots[i].stream)) {
            llg_file_global_failure("invalid or closed multichannel descriptor");
            return 0;
        }
    }
    return 1;
}

static int llg_file_single_ordinary(uint32_t descriptor, llg_file_slot_t** out) {
    if (!(descriptor & LLG_FILE_FD_TAG) || !llg_file_mask_valid(descriptor)) {
        llg_file_global_failure("file input/position operation requires an FD, not an MCD");
        return 0;
    }
    *out = &llg_file_slots[descriptor & ~LLG_FILE_FD_TAG];
    return 1;
}

uint32_t llg_file_descriptor(sv4_t value) {
    // A descriptor is a 32-bit bit pattern, not a nonnegative signed integer.
    if (llg_sv4_width(value) == 0 || llg_sv4_width(value) > 32 || sv4_is_unknown(value)) {
        llg_file_global_failure("file descriptor is not a known 32-bit value");
        return 0;
    }
    uint32_t descriptor = (uint32_t)sv4_to_u64(value);
    if (descriptor == 0) llg_file_global_failure("invalid file descriptor");
    return descriptor;
}

uint32_t llg_file_open(llg_string_t path, llg_string_t mode, int has_mode) {
    llg_file_init_table();
    const char* selected_mode = has_mode ? mode.data : "w";
    size_t mode_len = has_mode ? mode.len : 1u;
    const char* selected_path = path.data ? path.data : "";
    char* path_copy = (char*)llg_checked_malloc(path.len + 1u, 1, "file path");
    memcpy(path_copy, selected_path, path.len);
    path_copy[path.len] = 0;
    char* mode_copy = (char*)llg_checked_malloc(mode_len + 1u, 1, "file mode");
    memcpy(mode_copy, selected_mode ? selected_mode : "", mode_len);
    mode_copy[mode_len] = 0;
    llg_string_destroy(&path);
    llg_string_destroy(&mode);

    unsigned slot_index = LLG_FILE_SLOTS;
    for (unsigned i = has_mode ? LLG_FILE_FD_FIRST : LLG_FILE_MCD_FIRST;
         i < (has_mode ? LLG_FILE_SLOTS : LLG_FILE_MCD_END); i++) {
        if (!llg_file_slots[i].open) {
            slot_index = i;
            break;
        }
    }
    if (slot_index == LLG_FILE_SLOTS) {
        llg_file_global_failure("file descriptor table is full");
        free(path_copy);
        free(mode_copy);
        return 0;
    }

    static const char* const valid_modes[] = {
        "r", "w", "a", "r+", "w+", "a+",
        "rb", "wb", "ab", "r+b", "w+b", "a+b", "rb+", "wb+", "ab+",
    };
    int mode_valid = 0;
    for (size_t i = 0; i < sizeof(valid_modes) / sizeof(valid_modes[0]); i++) {
        if (strcmp(mode_copy, valid_modes[i]) == 0) {
            mode_valid = 1;
            break;
        }
    }
    if (!mode_valid) {
        llg_file_global_failure("unsupported file open mode");
        free(path_copy);
        free(mode_copy);
        return 0;
    }
    // Files the simulation creates follow LLG_SIM_OUT_DIR; reads ("r", "r+")
    // keep resolving from the current directory.
    if (mode_copy[0] == 'w' || mode_copy[0] == 'a') {
        char* resolved = llg_output_path(path_copy);
        free(path_copy);
        path_copy = resolved;
    }
    FILE* stream = fopen(path_copy, mode_copy);
    if (!stream) {
        char message[160];
        snprintf(message, sizeof(message), "file open failed: %s", strerror(errno));
        llg_file_global_failure(message);
        free(path_copy);
        free(mode_copy);
        return 0;
    }
    free(path_copy);
    free(mode_copy);
    llg_file_slots[slot_index].stream = stream;
    llg_file_slots[slot_index].open = 1;
    llg_file_slots[slot_index].owned = 1;
    llg_file_slots[slot_index].error = 0;
    llg_file_slots[slot_index].eof = 0;
    llg_file_slots[slot_index].pushback_len = 0;
    llg_file_slots[slot_index].message[0] = 0;
    return has_mode ? LLG_FILE_FD_TAG | slot_index : llg_file_mcd_bit(slot_index);
}

// Cancel deferred output before the slot can be reused by a later fopen.
static uint32_t llg_file_without_slot(uint32_t descriptor, unsigned slot) {
    if (descriptor & LLG_FILE_FD_TAG)
        return llg_file_selected(descriptor, slot) ? 0 : descriptor;
    return descriptor & ~llg_file_mcd_bit(slot);
}

void llg_file_close(uint32_t descriptor) {
    if (!llg_file_mask_valid(descriptor)) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (!llg_file_selected(descriptor, i)) continue;
        if (g.mon.typed) g.mon.descriptor = llg_file_without_slot(g.mon.descriptor, i);
        for (llg_strobe_t* e = g.strobes; e; e = e->next)
            if (e->typed) e->descriptor = llg_file_without_slot(e->descriptor, i);
        llg_file_slot_t* slot = &llg_file_slots[i];
        // Preopened streams are borrowed from the host. Invalidate their HDL
        // descriptors without closing the host's diagnostic/output channel.
        int result = slot->owned ? fclose(slot->stream) : 0;
        slot->stream = NULL;
        slot->open = 0;
        slot->owned = 0;
        slot->pushback_len = 0;
        if (result != 0) llg_file_slot_failure(slot, "file close failed");
    }
}

int llg_file_flush(uint32_t descriptor, int all) {
    llg_file_init_table();
    if (!all && !llg_file_mask_valid(descriptor)) return -1;
    int result = 0;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (all ? (!llg_file_slots[i].open || i == LLG_FILE_STDIN)
                : !llg_file_selected(descriptor, i)) continue;
        if (!llg_file_slots[i].open || !llg_file_slots[i].stream) {
            llg_file_global_failure("invalid or closed file descriptor");
            result = -1;
            continue;
        }
        if (fflush(llg_file_slots[i].stream) != 0) {
            llg_file_slot_failure(&llg_file_slots[i], "file flush failed");
            result = -1;
        }
    }
    return result;
}

void llg_file_rewind(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return;
    rewind(slot->stream);
    slot->error = 0;
    slot->eof = 0;
    slot->pushback_len = 0;
    slot->message[0] = 0;
}

int64_t llg_file_tell(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return -1;
    long position = ftell(slot->stream);
    if (position < 0) {
        llg_file_slot_failure(slot, "file tell failed");
        return -1;
    }
    if ((uint64_t)position < slot->pushback_len) {
        llg_file_slot_failure(slot, "file tell position precedes pushed-back characters");
        return -1;
    }
    return (int64_t)position - (int64_t)slot->pushback_len;
}

int llg_file_seek(uint32_t descriptor, sv4_t offset, sv4_t operation) {
    llg_file_slot_t* slot;
    int64_t signed_offset;
    if (!llg_file_single_ordinary(descriptor, &slot) ||
        !sv4_to_index_i64(offset, &signed_offset) || sv4_is_unknown(operation) ||
        llg_sv4_width(operation) == 0 || sv4_to_u64(operation) > 2u) {
        llg_file_global_failure("invalid file seek arguments");
        return -1;
    }
    int whence = (int)sv4_to_u64(operation);
    if (whence == SEEK_CUR) {
        if (signed_offset < INT64_MIN + (int64_t)slot->pushback_len) {
            llg_file_slot_failure(slot, "file seek offset underflow");
            return -1;
        }
        signed_offset -= (int64_t)slot->pushback_len;
    }
    if (signed_offset < (int64_t)LONG_MIN || signed_offset > (int64_t)LONG_MAX ||
        fseek(slot->stream, (long)signed_offset, whence) != 0) {
        llg_file_slot_failure(slot, "file seek failed");
        return -1;
    }
    slot->eof = 0;
    slot->pushback_len = 0;
    return 0;
}

int llg_file_error(uint32_t descriptor, llg_string_t* message) {
    const char* text = "";
    int result = 0;
    if (!llg_file_mask_valid(descriptor)) {
        result = 1;
        text = llg_file_global_message;
    } else {
        for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
            if (!llg_file_selected(descriptor, i)) continue;
            llg_file_slot_t* slot = &llg_file_slots[i];
            if (ferror(slot->stream)) llg_file_slot_failure(slot, "host stream error");
            if (slot->error) {
                result = 1;
                text = slot->message;
                break;
            }
        }
    }
    if (message) llg_string_move(message, llg_string_bytes(text, strlen(text)));
    return result;
}

int llg_file_eof(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return -1;
    return slot->eof != 0;
}

// ── File input ──────────────────────────────────────────────────────────────

static int llg_file_getc_slot(llg_file_slot_t* slot) {
    if (!slot || !slot->open || !slot->stream) return EOF;
    int value;
    if (slot->pushback_len) {
        value = slot->pushback[--slot->pushback_len];
        slot->eof = 0;
        return value;
    }
    value = fgetc(slot->stream);
    if (value == EOF) {
        if (feof(slot->stream)) slot->eof = 1;
        if (ferror(slot->stream)) llg_file_slot_failure(slot, "file input failed");
    } else {
        slot->eof = 0;
    }
    return value;
}

static int llg_file_ungetc_slot(llg_file_slot_t* slot, int value) {
    if (!slot || !slot->open || !slot->stream || value == EOF || value < 0 || value > UCHAR_MAX)
        return EOF;
    if (slot->pushback_len >= LLG_FILE_PUSHBACK) {
        llg_file_slot_failure(slot, "file input pushback limit exceeded");
        return EOF;
    }
    slot->pushback[slot->pushback_len++] = (unsigned char)value;
    // A successful standard-library ungetc clears the stream EOF indicator;
    // mirror that behavior even though the bounded stack keeps bytes outside
    // the host FILE buffer.
    clearerr(slot->stream);
    slot->eof = 0;
    return value;
}

int llg_file_getc(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return EOF;
    return llg_file_getc_slot(slot);
}

int llg_file_ungetc(uint32_t descriptor, sv4_t character) {
    llg_file_slot_t* slot;
    int64_t value;
    if (!llg_file_single_ordinary(descriptor, &slot) ||
        !sv4_to_index_i64(character, &value) || value < 0 || value > UCHAR_MAX) {
        llg_file_global_failure("invalid file ungetc arguments");
        return EOF;
    }
    return llg_file_ungetc_slot(slot, (int)value);
}

int llg_file_gets(uint32_t descriptor, llg_string_t* target) {
    llg_file_slot_t* slot;
    if (!target || !llg_file_single_ordinary(descriptor, &slot)) return 0;
    size_t capacity = 128u;
    size_t length = 0;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "file input line");
    for (;;) {
        int value = llg_file_getc_slot(slot);
        if (value == EOF) break;
        if (length == capacity) {
            if (capacity > (SIZE_MAX / 2u)) {
                free(bytes);
                llg_fatal_allocation("file input line", capacity, 2u);
            }
            capacity *= 2u;
            unsigned char* replacement = (unsigned char*)realloc(bytes, capacity);
            if (!replacement) {
                free(bytes);
                llg_fatal_allocation("file input line", capacity, 1u);
            }
            bytes = replacement;
        }
        bytes[length++] = (unsigned char)value;
        if (value == '\n') break;
    }
    if (length == 0) {
        free(bytes);
        return 0;
    }
    llg_string_t line = llg_string_bytes((const char*)bytes, length);
    free(bytes);
    /* Commit owns the string before it notifies; no line buffer remains on
     * the abandoned stack if a dependency callback calls $finish. */
    llg_string_move(target, line);
    return length > (size_t)INT_MAX ? INT_MAX : (int)length;
}

int llg_file_gets_packed(uint32_t descriptor, llg_ref_t* target) {
    if (!target || target->width == 0) return 0;
    llg_string_t value = {0};
    int result = llg_file_gets(descriptor, &value);
    if (result) llg_ref_write_owned(target, llg_string_to_packed(value, target->width,
                                                            target->is_signed));
    else llg_string_destroy(&value);
    return result;
}

typedef struct {
    llg_file_slot_t* file;
    const unsigned char* bytes;
    size_t length;
    size_t position;
    int input_failure;
} llg_scan_input_t;

static int llg_scan_get(llg_scan_input_t* input) {
    int value;
    if (input->file) {
        value = llg_file_getc_slot(input->file);
    } else if (input->position >= input->length) {
        value = EOF;
    } else {
        value = input->bytes[input->position++];
    }
    return value;
}

static int llg_scan_unget(llg_scan_input_t* input, int value) {
    if (value == EOF) return 0;
    if (input->file) return llg_file_ungetc_slot(input->file, value) != EOF;
    if (input->position == 0) return 0;
    input->position--;
    return 1;
}

static int llg_scan_skip_space(llg_scan_input_t* input) {
    int value;
    do {
        value = llg_scan_get(input);
    } while (value != EOF && isspace((unsigned char)value));
    if (value != EOF) (void)llg_scan_unget(input, value);
    else input->input_failure = 1;
    return value != EOF;
}

static int llg_scan_token(llg_scan_input_t* input, size_t limit,
                          unsigned char** result, size_t* length) {
    size_t capacity = limit != SIZE_MAX && limit < 128u ? limit + 1u : 128u;
    if (capacity == 0) capacity = 1;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "file input token");
    size_t used = 0;
    for (;;) {
        int value = llg_scan_get(input);
        if (value == EOF || isspace((unsigned char)value)) {
            if (value != EOF) (void)llg_scan_unget(input, value);
            else if (used == 0) input->input_failure = 1;
            break;
        }
        if (limit != SIZE_MAX && used >= limit) {
            (void)llg_scan_unget(input, value);
            break;
        }
        if (used == capacity) {
            if (capacity > SIZE_MAX / 2u) {
                free(bytes);
                llg_fatal_allocation("file input token", capacity, 2u);
            }
            capacity *= 2u;
            unsigned char* replacement = (unsigned char*)realloc(bytes, capacity);
            if (!replacement) {
                free(bytes);
                llg_fatal_allocation("file input token", capacity, 1u);
            }
            bytes = replacement;
        }
        bytes[used++] = (unsigned char)value;
    }
    if (used == 0) {
        free(bytes);
        return 0;
    }
    *result = bytes;
    *length = used;
    return 1;
}

static int llg_scan_chars(llg_scan_input_t* input, size_t count,
                          unsigned char** result, size_t* length) {
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(count ? count : 1u, 1,
                                                               "file input characters");
    size_t used = 0;
    while (used < count) {
        int value = llg_scan_get(input);
        if (value == EOF) {
            if (used == 0) input->input_failure = 1;
            break;
        }
        bytes[used++] = (unsigned char)value;
    }
    if (used == 0) {
        free(bytes);
        return 0;
    }
    *result = bytes;
    *length = used;
    return 1;
}

static int llg_scan_digit(unsigned char value, unsigned base) {
    if (value >= '0' && value <= '9') {
        int digit = (int)(value - '0');
        return digit < (int)base ? digit : -1;
    }
    if (value >= 'a' && value <= 'f') {
        int digit = (int)(value - 'a') + 10;
        return digit < (int)base ? digit : -1;
    }
    if (value >= 'A' && value <= 'F') {
        int digit = (int)(value - 'A') + 10;
        return digit < (int)base ? digit : -1;
    }
    return -1;
}

// Read only this conversion's input item. In particular, a comma or colon
// belongs to the next directive, not to an all-or-nothing whitespace token.
static int llg_scan_numeric(llg_scan_input_t* input, char conversion, size_t limit,
                            unsigned char** result, size_t* length) {
    size_t capacity = 64u, used = 0;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "numeric input");
    int real = conversion == 'f' || conversion == 'e' || conversion == 'g';
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' || conversion == 'x' ? 16u : 10u;
    int digits = 0, dot = 0, exponent = 0, exponent_digits = 0;
    int decimal_unknown = 0;
    while (used < limit) {
        int c = llg_scan_get(input);
        if (c == EOF) {
            if (!used) input->input_failure = 1;
            break;
        }
        int accept = 0;
        if (used == 0 && (c == '+' || c == '-') &&
            (real || base == 10u)) accept = 1;
        else if (real) {
            if (c >= '0' && c <= '9') {
                accept = 1;
                if (exponent) exponent_digits = 1; else digits = 1;
            } else if (c == '.' && !dot && !exponent) {
                accept = 1; dot = 1;
            } else if ((c == 'e' || c == 'E') && digits && !exponent) {
                accept = 1; exponent = 1;
            } else if ((c == '+' || c == '-') && used &&
                       (bytes[used - 1u] == 'e' || bytes[used - 1u] == 'E')) accept = 1;
        } else {
            size_t start = used && (bytes[0] == '+' || bytes[0] == '-') ? 1u : 0u;
            // Preserve the existing C-style auto-radix %i extension only.
            if (conversion == 'i' && used == start + 1u && bytes[start] == '0' &&
                (c == 'x' || c == 'X' || c == 'b' || c == 'B' || c == 'o' || c == 'O')) {
                base = c == 'x' || c == 'X' ? 16u : c == 'b' || c == 'B' ? 2u : 8u;
                digits = 0; accept = 1;
            } else {
                if (conversion == 'i' && used == start + 1u && bytes[start] == '0') base = 8u;
                if (c == '_' && digits) accept = 1;
                else if (!decimal_unknown && llg_scan_digit((unsigned char)c, base) >= 0) {
                    accept = 1; digits = 1;
                } else if (c == 'x' || c == 'X' || c == 'z' || c == 'Z' || c == '?') {
                    if (base != 10u || (!digits && used == start)) {
                        accept = 1; digits = 1;
                        if (base == 10u) decimal_unknown = 1;
                    }
                }
            }
        }
        if (!accept) { (void)llg_scan_unget(input, c); break; }
        if (used == capacity) {
            if (capacity > SIZE_MAX / 2u) llg_fatal_allocation("numeric input", capacity, 2u);
            capacity *= 2u;
            unsigned char* next = (unsigned char*)realloc(bytes, capacity);
            if (!next) llg_fatal_allocation("numeric input", capacity, 1u);
            bytes = next;
        }
        bytes[used++] = (unsigned char)c;
    }
    if (!digits || (real && exponent && !exponent_digits)) { free(bytes); return 0; }
    *result = bytes;
    *length = used;
    return 1;
}

static void llg_scan_set_bit(sv4_t* value, uint32_t bit, int state) {
    llg_sv4_set_state(value, bit, (unsigned)state);
}

static int llg_scan_unknown(unsigned char value) {
    return value == 'x' || value == 'X' ? 2 :
           value == 'z' || value == 'Z' || value == '?' ? 3 : 0;
}

static int llg_scan_integer(const unsigned char* bytes, size_t length,
                            char conversion, uint32_t width, int is_signed,
                            sv4_t* result) {
    size_t begin = 0;
    int negative = 0;
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' || conversion == 'x' ? 16u : 10u;
    if (length && (bytes[0] == '+' || bytes[0] == '-')) {
        negative = bytes[0] == '-';
        begin = 1;
    }
    if (conversion == 'i' && begin + 2u <= length && bytes[begin] == '0') {
        unsigned char prefix = bytes[begin + 1u];
        if (prefix == 'x' || prefix == 'X') { base = 16u; begin += 2u; }
        else if (prefix == 'b' || prefix == 'B') { base = 2u; begin += 2u; }
        else if (prefix == 'o' || prefix == 'O') { base = 8u; begin += 2u; }
        else base = 8u;
    }
    if (begin == length) return 0;
    int unknown = 0;
    for (size_t i = begin; i < length; i++) {
        if (bytes[i] == '_') continue;
        int state = llg_scan_unknown(bytes[i]);
        if (state) {
            unknown = unknown && unknown != state ? 2 : state;
            continue;
        }
        if (llg_scan_digit(bytes[i], base) < 0) return 0;
    }
    if (unknown && base == 10u) {
        sv4_replace(result, sv4_fill((uint8_t)(unknown == 3 ? 3 : 2), width, (int8_t)is_signed));
        return 1;
    }
    sv4_replace(result, sv4_zero(width, (int8_t)is_signed));
    if (base == 10u) {
        for (size_t i = begin; i < length; i++) {
            if (bytes[i] == '_') continue;
            unsigned digit = (unsigned)llg_scan_digit(bytes[i], base);
            llg_sv4_mul_add_known(result, 10, digit);
        }
    } else {
        uint32_t bits_per_digit = base == 16u ? 4u : base == 8u ? 3u : 1u;
        uint32_t bit = 0;
        for (size_t i = length; i > begin; i--) {
            unsigned char digit_char = bytes[i - 1u];
            if (digit_char == '_') continue;
            int state = llg_scan_unknown(digit_char);
            if (state) {
                for (uint32_t part = 0; part < bits_per_digit; part++)
                    llg_scan_set_bit(result, bit + part, state);
            } else {
                unsigned digit = (unsigned)llg_scan_digit(digit_char, base);
                for (uint32_t part = 0; part < bits_per_digit; part++)
                    llg_scan_set_bit(result, bit + part, (digit >> part) & 1u);
            }
            if (bit <= UINT32_MAX - bits_per_digit) bit += bits_per_digit;
        }
    }
    llg_plusarg_mask_top(result);
    if (negative) sv4_replace(result, sv4_neg(*result));
    return 1;
}

static int llg_scan_bytes_to_packed(const unsigned char* bytes, size_t length,
                                    uint32_t width, int is_signed, sv4_t* result) {
    sv4_replace(result, sv4_zero(width, (int8_t)is_signed));
    size_t capacity = ((size_t)width + 7u) / 8u;
    size_t used = length < capacity ? length : capacity;
    for (size_t i = 0; i < used; i++) {
        unsigned char value = bytes[length - 1u - i];
        for (unsigned bit = 0; bit < 8u; bit++)
            llg_scan_set_bit(result, (uint32_t)(i * 8u + bit), (value >> bit) & 1u);
    }
    return 1;
}

static int llg_scan_assign(const unsigned char* bytes, size_t length, char conversion,
                           const llg_file_input_target_t* target, uint32_t width,
                           int is_signed) {
    if (!target) return 0;
    if (conversion == 's' || conversion == 'c') {
        if (target->kind == LLG_FILE_INPUT_STRING && target->string) {
            llg_string_move(target->string, llg_string_bytes((const char*)bytes, length));
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t value = SV4_EMPTY;
            llg_scan_bytes_to_packed(bytes, length, width, is_signed, &value);
            llg_ref_write_owned(target->packed, value);
            return 1;
        }
        return 0;
    }
    if (conversion == 'f' || conversion == 'e' || conversion == 'g') {
        char* text = (char*)llg_checked_malloc(length + 1u, 1, "file input real token");
        memcpy(text, bytes, length);
        text[length] = 0;
        char* end = NULL;
        double value = strtod(text, &end);
        int valid = end == text + length;
        free(text);
        if (!valid) return 0;
        if (target->kind == LLG_FILE_INPUT_REAL && target->real) {
            llg_ba_d(target->real, target->shortreal ? (double)(float)value : value);
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t packed = sv4_from_real(value, width, (int8_t)is_signed);
            llg_ref_write_owned(target->packed, packed);
            return 1;
        }
        return 0;
    }
    if (target->kind != LLG_FILE_INPUT_PACKED || !target->packed) return 0;
    sv4_t value = SV4_EMPTY;
    if (!llg_scan_integer(bytes, length, conversion, width, is_signed, &value)) {
        sv4_destroy(&value);
        return 0;
    }
    llg_ref_write_owned(target->packed, value);
    return 1;
}

static void llg_scan_token_destroy(void* object) {
    free(*(unsigned char**)object);
}

static int llg_scan_conversion(llg_scan_input_t* input, char conversion,
                               size_t width, int suppressed,
                               const llg_file_input_target_t* target) {
    unsigned char* bytes = NULL;
    size_t length = 0;
    int ok;
    if (conversion == 'c') {
        ok = llg_scan_chars(input, width ? width : 1u, &bytes, &length);
    } else {
        if (!llg_scan_skip_space(input)) return 0;
        ok = conversion == 's'
            ? llg_scan_token(input, width ? width : SIZE_MAX, &bytes, &length)
            : llg_scan_numeric(input, conversion, width ? width : SIZE_MAX, &bytes, &length);
    }
    if (!ok) return input->input_failure ? -1 : 0;
    if (suppressed) {
        // The lexical conversion above still runs; only assignment is suppressed.
        free(bytes);
        return 2;
    }
    if (!target) {
        free(bytes);
        return 0;
    }
    uint32_t target_width = target->packed ? target->packed->width : 32u;
    int target_signed = target->packed ? target->packed->is_signed : 1;
    llg_value_scope_t* token_scope = llg_value_scope_begin_object(
        sizeof(unsigned char*), llg_scan_token_destroy);
    *(unsigned char**)llg_value_scope_object(token_scope) = bytes;
    ok = llg_scan_assign(bytes, length, conversion, target, target_width, target_signed);
    llg_value_scope_end(token_scope);
    return ok ? 1 : 0;
}

static int llg_scan_format(llg_scan_input_t* input, const char* format,
                           const llg_file_input_target_t* targets, int target_count) {
    if (!format || target_count < 0) return 0;
    int assigned = 0;
    int matched = 0;
    int target_index = 0;
    size_t length = strlen(format);
    for (size_t i = 0; i < length;) {
        unsigned char format_char = (unsigned char)format[i++];
        if (isspace(format_char)) {
            while (i < length && isspace((unsigned char)format[i])) i++;
            (void)llg_scan_skip_space(input);
            continue;
        }
        if (format_char != '%') {
            int value = llg_scan_get(input);
            if (value != format_char) {
                if (value == EOF) input->input_failure = 1;
                (void)llg_scan_unget(input, value);
                break;
            }
            continue;
        }
        if (i >= length) break;
        if (format[i] == '%') {
            i++;
            int value = llg_scan_get(input);
            if (value != '%') {
                if (value == EOF) input->input_failure = 1;
                (void)llg_scan_unget(input, value);
                break;
            }
            continue;
        }
        int suppressed = 0;
        if (format[i] == '*') { suppressed = 1; i++; }
        size_t width = 0;
        while (i < length && isdigit((unsigned char)format[i])) {
            unsigned digit = (unsigned)(format[i++] - '0');
            if (width > (SIZE_MAX - digit) / 10u) width = SIZE_MAX;
            else width = width * 10u + digit;
        }
        while (i < length && (format[i] == 'l' || format[i] == 'L' ||
                              format[i] == 'j' ||
                              format[i] == 'z' || format[i] == 't')) i++;
        if (i >= length) break;
        char conversion = format[i++];
        if (conversion >= 'A' && conversion <= 'Z') conversion = (char)(conversion - 'A' + 'a');
        if (conversion != 'd' && conversion != 'i' && conversion != 'u' &&
            conversion != 'o' && conversion != 'x' && conversion != 'h' &&
            conversion != 'b' && conversion != 'c' && conversion != 's' &&
            conversion != 'f' && conversion != 'e' && conversion != 'g') break;
        const llg_file_input_target_t* target = NULL;
        if (!suppressed) {
            if (target_index >= target_count) break;
            target = &targets[target_index++];
        }
        int converted = llg_scan_conversion(input, conversion, width, suppressed, target);
        if (converted < 0) break;
        if (converted == 0) break;
        matched = 1;
        if (converted == 1) assigned++;
    }
    return assigned || matched || !input->input_failure ? assigned : -1;
}

int llg_file_scanf(uint32_t descriptor, const char* format,
                   const llg_file_input_target_t* targets, int target_count) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return 0;
    llg_scan_input_t input = {slot, NULL, 0, 0, 0};
    return llg_scan_format(&input, format, targets, target_count);
}

int llg_string_scanf(const char* source, size_t source_length,
                     const char* format,
                     const llg_file_input_target_t* targets, int target_count) {
    llg_scan_input_t input = {NULL, (const unsigned char*)source, source_length, 0, 0};
    return llg_scan_format(&input, format, targets, target_count);
}

static int llg_file_read_byte(llg_file_slot_t* slot, unsigned char* output) {
    int value = llg_file_getc_slot(slot);
    if (value == EOF) return 0;
    *output = (unsigned char)value;
    return 1;
}

int llg_file_read_packed(uint32_t descriptor, llg_ref_t* target) {
    llg_file_slot_t* slot;
    if (!target || !llg_file_single_ordinary(descriptor, &slot) || target->width == 0)
        return 0;
    sv4_t value = llg_ref_read(target);
    size_t bytes = ((size_t)target->width + 7u) / 8u;
    int read = 0;
    for (size_t index = 0; index < bytes; index++) {
        unsigned char byte;
        if (!llg_file_read_byte(slot, &byte)) break;
        size_t bit_base = (bytes - 1u - index) * 8u;
        for (unsigned bit = 0; bit < 8u; bit++)
            llg_scan_set_bit(&value, (uint32_t)(bit_base + bit), (byte >> bit) & 1u);
        read++;
    }
    if (read) llg_ref_write_owned(target, value);
    else sv4_destroy(&value);
    return read;
}

static int fixed_file_read_array(uint32_t descriptor, sv4_t* values, llg_fixed_array_t* fixed, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    llg_file_slot_t* slot;
    if ((!values && !fixed) || total == 0 || elem_width == 0 || !dimensions || dimension_count <= 0 ||
        !llg_file_single_ordinary(descriptor, &slot)) return 0;
    /* IEEE 1364-2001 17.2.4.4 / 1800-2009 21.3.4.4: a memory
       is read from its lowest address toward its highest, not in declaration
       order. Rank-one descending storage therefore walks backward. */
    int reverse_storage = dimension_count == 1 && dimensions[0] > dimensions[1];
    uint64_t offset = reverse_storage ? total - 1u : 0;
    if (has_start) {
        int64_t index;
        int64_t low = dimensions[0] < dimensions[1] ? dimensions[0] : dimensions[1];
        int64_t high = dimensions[0] > dimensions[1] ? dimensions[0] : dimensions[1];
        if (!sv4_to_index_i64(start, &index) || index < low || index > high) {
            llg_file_slot_failure(slot, "file read start index is out of bounds");
            return 0;
        }
        offset = dimensions[0] >= dimensions[1]
                     ? (uint64_t)((int64_t)dimensions[0] - index)
                     : (uint64_t)(index - (int64_t)dimensions[0]);
    }
    if (offset >= total) {
        llg_file_slot_failure(slot, "file read start index is out of bounds");
        return 0;
    }
    uint64_t available = reverse_storage ? offset + 1u : total - offset;
    uint64_t requested = available;
    if (has_count) {
        int64_t value;
        if (!sv4_to_index_i64(count, &value) || value < 0) {
            llg_file_slot_failure(slot, "file read count is out of bounds");
            return 0;
        }
        requested = (uint64_t)value;
        if (requested > available) requested = available;
    }
    size_t bytes_per_element = ((size_t)elem_width + 7u) / 8u;
    int result = 0;
    for (uint64_t element = 0; element < requested; element++) {
        uint64_t position = reverse_storage ? offset - element : offset + element;
        sv4_t value = sv4_clone(fixed ? llg_fixed_array_peek(fixed, position) : &values[position]);
        int read = 0;
        for (size_t index = 0; index < bytes_per_element; index++) {
            unsigned char byte;
            if (!llg_file_read_byte(slot, &byte)) break;
            size_t bit_base = (bytes_per_element - 1u - index) * 8u;
            for (unsigned bit = 0; bit < 8u; bit++)
                llg_scan_set_bit(&value, (uint32_t)(bit_base + bit), (byte >> bit) & 1u);
            read++;
        }
        if (!read) { sv4_destroy(&value); break; }
        llg_sv4_set_signed(&value, (int8_t)elem_signed);
        if (elem_two_state) sv4_replace(&value, sv4_to_two_state(value));
        llg_value_scope_t* value_scope = llg_value_scope_begin(1);
        sv4_t* owned = llg_value_scope_values(value_scope);
        owned[0] = value;
        llg_ba(fixed ? llg_fixed_array_cell(fixed, position) : &values[position], owned[0]);
        llg_value_scope_end(value_scope);
        result += read;
        if ((size_t)read < bytes_per_element) break;
    }
    return result;
}

static void llg_file_write_typed(uint32_t descriptor, const char* output,
                                 size_t length, int newline) {
    if (!llg_file_mask_valid(descriptor)) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (!llg_file_selected(descriptor, i)) continue;
        llg_file_slot_t* slot = &llg_file_slots[i];
        if (fwrite(output, 1, length, slot->stream) != length ||
            (newline && fputc('\n', slot->stream) == EOF) ||
            fflush(slot->stream) != 0) {
            llg_file_slot_failure(slot, "file output failed");
        }
    }
}

static char* llg_typed_line_alloc(const char* fmt, llg_fmt_arg_t* args, int n,
                                  const char* scope, size_t* length) {
    size_t cap = strlen(fmt) + (scope ? strlen(scope) : 0) + 64u;
    for (int i = 0; i < n; i++) {
        size_t extra = 64u;
        if (args[i].kind == LLG_FMT_PACKED || args[i].kind == LLG_FMT_STRENGTH) {
            if (llg_sv4_width(args[i].value.packed) > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            extra += (size_t)llg_sv4_width(args[i].value.packed) * 8u;
        }
        if (args[i].kind == LLG_FMT_STRING) {
            if (args[i].value.string.len > SIZE_MAX - extra)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            if (args[i].value.string.len > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("formatted string", args[i].value.string.len, 8u);
            extra += args[i].value.string.len * 8u;
        }
        if ((size_t)g.time_format.minimum_field_width > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.minimum_field_width;
        if ((size_t)g.time_format.precision > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.precision;
        if (g.time_format.suffix.len > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += g.time_format.suffix.len;
        if (extra > SIZE_MAX - cap) llg_fatal_allocation("typed formatted line", cap, extra);
        cap += extra;
    }
    // Include explicit field widths/precisions and repeated scope conversions.
    for (const char* p = fmt; *p;) {
        if (*p++ != '%') continue;
        const char* start = p - 1;
        llg_fmt_spec_t spec;
        p = llg_parse_typed_spec(start, p, &spec);
        cap = llg_format_size_add(cap, (size_t)spec.width);
        cap = llg_format_size_add(cap, (size_t)spec.precision);
        cap = llg_format_size_add(cap, scope ? strlen(scope) : 0);
        if (*p) ++p;
    }
    char* out = llg_checked_malloc(cap, 1, "typed formatted line");
    *length = llg_format_typed(out, cap, fmt, args, n, scope);
    return out;
}

llg_string_t llg_string_format_typed(llg_string_t format, llg_fmt_arg_t* args,
                                     int n, const char* scope) {
    const char* text = format.data ? format.data : "";
    size_t length = 0;
    char* output = llg_typed_line_alloc(text, args, n, scope, &length);
    llg_string_t result = llg_string_bytes(output, length);
    free(output);
    llg_string_destroy(&format);
    llg_fmt_args_destroy(args, n);
    return result;
}

static void llg_print_typed_to(uint32_t descriptor, const char* fmt,
                               llg_fmt_arg_t* args, int n, const char* scope,
                               int newline) {
    size_t len = 0;
    char* out = llg_typed_line_alloc(fmt, args, n, scope, &len);
    llg_file_write_typed(descriptor, out, len, newline);
    free(out);
}

void llg_file_display_typed(uint32_t descriptor, const char* fmt,
                            llg_fmt_arg_t* args, int n, const char* scope,
                            int newline) {
    llg_print_typed_to(descriptor, fmt, args, n, scope, newline);
    llg_fmt_args_destroy(args, n);
}

int llg_file_read_array(uint32_t descriptor, sv4_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    return fixed_file_read_array(descriptor, values, NULL, elem_width, elem_signed, elem_two_state, total, dimensions, dimension_count, has_start, start, has_count, count);
}

int llg_fixed_file_read_array(uint32_t descriptor, llg_fixed_array_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    return fixed_file_read_array(descriptor, NULL, values, elem_width, elem_signed, elem_two_state, total, dimensions, dimension_count, has_start, start, has_count, count);
}

// ── Memory file tasks ────────────────────────────────────────────────────────

enum {
    LLG_MEMORY_TOKEN_EOF = 0,
    LLG_MEMORY_TOKEN_DATA = 1,
    LLG_MEMORY_TOKEN_ADDRESS = 2,
    LLG_MEMORY_TOKEN_ERROR = -1,
};

typedef struct {
    sv4_t value;
    uint64_t digits;
    int too_wide;
    int64_t address;
    int invalid_index;
} llg_memory_value_t;

static void llg_memory_warning(const char* path, const char* format, ...) {
    va_list args;
    fprintf(stderr, "llg: memory file `%s`: ", path ? path : "");
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
    fputc('\n', stderr);
}

static char* llg_memory_path_copy(llg_string_t path) {
    char* copy = (char*)llg_checked_malloc(path.len + 1u, 1, "memory file path");
    if (path.len && path.data) memcpy(copy, path.data, path.len);
    copy[path.len] = 0;
    llg_string_destroy(&path);
    return copy;
}

// Append one binary or hexadecimal digit, retaining the least significant
// token-capacity bits. The caller diagnoses an over-width token separately.
static void llg_memory_append_digit(llg_memory_value_t* value, unsigned bits,
                                    int state, unsigned numeric) {
    if (value->too_wide) return;
    if (value->digits >= (LLG_SUPPORTED_WIDTH_LIMIT - 1u) / bits) {
        value->too_wide = 1;
        return;
    }
    uint32_t required = (uint32_t)((value->digits + 1u) * bits);
    if (required > llg_sv4_width(value->value)) {
        uint32_t capacity = llg_sv4_width(value->value) ? llg_sv4_width(value->value) * 2u : 64u;
        if (capacity >= LLG_SUPPORTED_WIDTH_LIMIT) capacity = LLG_SUPPORTED_WIDTH_LIMIT - 1u;
        sv4_replace(&value->value, sv4_resize(value->value, capacity, 0));
    }
    llg_sv4_append_digit(&value->value, bits, state == 1 ? 2u : state == 2 ? 3u : 0u, numeric);
    ++value->digits;
}

static int llg_memory_digit(int c, int radix, int* state, unsigned* numeric) {
    if (c == 'x' || c == 'X') {
        *state = 1;
        *numeric = 0;
        return 1;
    }
    if (c == 'z' || c == 'Z') {
        *state = 2;
        *numeric = 0;
        return 1;
    }
    if (c >= '0' && c <= '1') {
        *state = 0;
        *numeric = (unsigned)(c - '0');
        return 1;
    }
    if (radix == 16) {
        if (c >= '2' && c <= '9') {
            *state = 0;
            *numeric = (unsigned)(c - '0');
            return 1;
        }
        if (c >= 'a' && c <= 'f') {
            *state = 0;
            *numeric = (unsigned)(c - 'a' + 10);
            return 1;
        }
        if (c >= 'A' && c <= 'F') {
            *state = 0;
            *numeric = (unsigned)(c - 'A' + 10);
            return 1;
        }
    }
    return 0;
}

static int llg_memory_next_noncomment(FILE* stream) {
    for (;;) {
        int c = fgetc(stream);
        if (c == EOF || !isspace((unsigned char)c)) {
            if (c != '/') return c;
            int next = fgetc(stream);
            if (next == '/') {
                while ((c = fgetc(stream)) != EOF && c != '\n') {}
                continue;
            }
            if (next == '*') {
                int previous = 0;
                int closed = 0;
                while ((c = fgetc(stream)) != EOF) {
                    if (previous == '*' && c == '/') {
                        closed = 1;
                        break;
                    }
                    previous = c;
                }
                if (!closed) return -2;
                continue;
            }
            if (next != EOF) (void)ungetc(next, stream);
            return '/';
        }
    }
}

static void llg_memory_consume_bad_token(FILE* stream) {
    int c;
    while ((c = fgetc(stream)) != EOF) {
        if (isspace((unsigned char)c)) break;
        if (c == '/') {
            (void)ungetc(c, stream);
            break;
        }
    }
}

static int llg_memory_parse_digits(FILE* stream, int radix, int first,
                                   llg_memory_value_t* value) {
    const unsigned bits = radix == 2 ? 1u : 4u;
    int c = first;
    int saw_digit = 0;
    int malformed = 0;
    while (c != EOF) {
        int state;
        unsigned numeric;
        if (llg_memory_digit(c, radix, &state, &numeric)) {
            llg_memory_append_digit(value, bits, state, numeric);
            saw_digit = 1;
        } else if (c == '_') {
            // Underscores are separators inside a memory word.
        } else if (isspace((unsigned char)c)) {
            break;
        } else if (c == '/') {
            (void)ungetc(c, stream);
            break;
        } else {
            malformed = 1;
            llg_memory_consume_bad_token(stream);
            break;
        }
        c = fgetc(stream);
    }
    if (!saw_digit || malformed || value->too_wide) return LLG_MEMORY_TOKEN_ERROR;
    sv4_replace(&value->value, sv4_resize(value->value,
                (uint32_t)(value->digits * bits), 0));
    return LLG_MEMORY_TOKEN_DATA;
}

static int llg_memory_next_token(FILE* stream, int radix,
                                 llg_memory_value_t* value) {
    sv4_destroy(&value->value);
    memset(value, 0, sizeof(*value));
    int c = llg_memory_next_noncomment(stream);
    if (c == EOF) return LLG_MEMORY_TOKEN_EOF;
    if (c == -2) return LLG_MEMORY_TOKEN_ERROR;
    if (c == '@') {
        c = fgetc(stream);
        int negative = 0;
        uint64_t magnitude = 0;
        int saw_digit = 0;
        if (c == '-' || c == '+') {
            negative = c == '-';
            c = fgetc(stream);
        }
        int state;
        unsigned numeric;
        while (c != EOF) {
            if (llg_memory_digit(c, 16, &state, &numeric) && state == 0) {
                saw_digit = 1;
                if (magnitude > (UINT64_MAX - numeric) / 16u) {
                    value->invalid_index = 1;
                } else if (!value->invalid_index) {
                    magnitude = magnitude * 16u + numeric;
                }
            } else if (c == '_') {
                // Underscores are separators inside an address.
            } else if (isspace((unsigned char)c)) {
                break;
            } else if (c == '/') {
                (void)ungetc(c, stream);
                break;
            } else {
                llg_memory_consume_bad_token(stream);
                return LLG_MEMORY_TOKEN_ERROR;
            }
            c = fgetc(stream);
        }
        if (!saw_digit) return LLG_MEMORY_TOKEN_ERROR;
        uint64_t limit = negative ? (uint64_t)INT64_MAX + 1u : (uint64_t)INT64_MAX;
        if (magnitude > limit) value->invalid_index = 1;
        if (!value->invalid_index) {
            value->address = negative
                                 ? (magnitude == limit ? INT64_MIN : -(int64_t)magnitude)
                                 : (int64_t)magnitude;
        }
        return LLG_MEMORY_TOKEN_ADDRESS;
    }
    if (!llg_memory_digit(c, radix, &(int){0}, &(unsigned){0})) {
        llg_memory_consume_bad_token(stream);
        return LLG_MEMORY_TOKEN_ERROR;
    }
    return llg_memory_parse_digits(stream, radix, c, value);
}

static int llg_memory_extent(int32_t left_value, int32_t right_value,
                             uint64_t* extent) {
    int64_t left = left_value;
    int64_t right = right_value;
    uint64_t distance = left >= right ? (uint64_t)(left - right)
                                      : (uint64_t)(right - left);
    if (distance == UINT64_MAX) return 0;
    *extent = distance + 1u;
    return 1;
}

static int llg_memory_address_offset(int64_t address, int32_t left_value,
                                     int32_t right_value, uint64_t* offset) {
    int64_t left = left_value;
    int64_t right = right_value;
    if (address < (left < right ? left : right) ||
        address > (left > right ? left : right)) return 0;
    *offset = left >= right ? (uint64_t)(left - address)
                            : (uint64_t)(address - left);
    return 1;
}

static int llg_memory_descriptor(const char* path, uint64_t total,
                                 const int32_t* dims, int n_dims,
                                 const uint64_t* strides, uint64_t origin,
                                 uint64_t view_total, uint64_t* inner_total) {
    if (!dims || !strides || n_dims <= 0 || total == 0 || view_total == 0 ||
        origin >= total || view_total > total - origin) {
        llg_memory_warning(path, "memory descriptor is invalid");
        return 0;
    }
    uint64_t expected_total = 1;
    for (int dimension = 0; dimension < n_dims; ++dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            expected_total > UINT64_MAX / extent) {
            llg_memory_warning(path, "memory descriptor dimension overflows");
            return 0;
        }
        expected_total *= extent;
    }
    if (expected_total != view_total) {
        llg_memory_warning(path, "memory descriptor size does not match its bounds");
        return 0;
    }
    uint64_t expected_stride = 1;
    for (int dimension = n_dims - 1; dimension >= 0; --dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            strides[dimension] != expected_stride ||
            expected_stride > UINT64_MAX / extent) {
            llg_memory_warning(path, "memory descriptor stride is invalid");
            return 0;
        }
        expected_stride *= extent;
    }
    uint64_t max_offset = 0;
    for (int dimension = 0; dimension < n_dims; ++dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            extent == 0 ||
            (extent - 1u) > UINT64_MAX / strides[dimension] ||
            max_offset > UINT64_MAX - (extent - 1u) * strides[dimension]) {
            llg_memory_warning(path, "memory descriptor range overflows");
            return 0;
        }
        max_offset += (extent - 1u) * strides[dimension];
    }
    if (max_offset >= total - origin) {
        llg_memory_warning(path, "memory descriptor exceeds its source array");
        return 0;
    }
    uint64_t outer_extent;
    if (!llg_memory_extent(dims[0], dims[1], &outer_extent) ||
        outer_extent == 0 || view_total % outer_extent != 0) {
        llg_memory_warning(path, "memory descriptor outer extent is invalid");
        return 0;
    }
    *inner_total = view_total / outer_extent;
    return 1;
}

static int llg_memory_view_index(int64_t address, uint64_t inner_ordinal,
                                 const int32_t* dims, int n_dims,
                                 const uint64_t* strides, uint64_t origin,
                                 uint64_t view_total, uint64_t total,
                                 uint64_t* index) {
    uint64_t outer_extent;
    if (!llg_memory_extent(dims[0], dims[1], &outer_extent) ||
        outer_extent == 0 || inner_ordinal >= view_total / outer_extent) {
        return 0;
    }
    uint64_t outer_offset;
    if (!llg_memory_address_offset(address, dims[0], dims[1], &outer_offset)) {
        return 0;
    }
    if (outer_offset > UINT64_MAX / strides[0]) return 0;
    uint64_t flat = origin + outer_offset * strides[0];
    if (flat < origin) return 0;
    uint64_t ordinal = inner_ordinal;
    for (int dimension = n_dims - 1; dimension >= 1; --dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            extent == 0) {
            return 0;
        }
        uint64_t coordinate = ordinal % extent;
        ordinal /= extent;
        uint64_t offset = dims[2 * dimension] >= dims[2 * dimension + 1]
                              ? extent - 1u - coordinate
                              : coordinate;
        if (offset > UINT64_MAX / strides[dimension] ||
            flat > UINT64_MAX - offset * strides[dimension]) {
            return 0;
        }
        flat += offset * strides[dimension];
    }
    if (ordinal != 0 || flat >= total) return 0;
    *index = flat;
    return 1;
}

static uint64_t llg_memory_range_length(int64_t first, int64_t last) {
    uint64_t distance = first >= last ? (uint64_t)(first - last)
                                      : (uint64_t)(last - first);
    return distance == UINT64_MAX ? UINT64_MAX : distance + 1u;
}

static int llg_memory_bounds(const char* path, uint64_t total,
                             const int32_t* dims, int n_dims,
                             const uint64_t* strides, uint64_t origin,
                             uint64_t view_total, sv4_t start, sv4_t finish,
                             int has_start, int has_finish, int addressing_policy,
                             int64_t* first, int64_t* last,
                             uint64_t* inner_total) {
    if (!llg_memory_descriptor(path, total, dims, n_dims, strides, origin,
                               view_total, inner_total)) {
        return 0;
    }
    int32_t left_value = dims[0], right_value = dims[1];
    int64_t left = left_value, right = right_value;
    if (has_start && !sv4_to_index_i64(start, first)) {
        llg_memory_warning(path, "start address is unknown, negative-width, or out of range");
        return 0;
    }
    if (has_finish && !sv4_to_index_i64(finish, last)) {
        llg_memory_warning(path, "finish address is unknown, negative-width, or out of range");
        return 0;
    }
    if (!has_start) {
        *first = addressing_policy == LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009
                     ? (left < right ? left : right)
                     : left;
    }
    if (!has_finish) {
        *last = addressing_policy == LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009
                    ? (left > right ? left : right)
                    : right;
    }
    uint64_t ignored_offset;
    if (!llg_memory_address_offset(*first, left_value, right_value, &ignored_offset) ||
        !llg_memory_address_offset(*last, left_value, right_value, &ignored_offset)) {
        llg_memory_warning(path, "selected range includes an address outside the destination memory");
        return 0;
    }
    return 1;
}

static int llg_memory_in_requested_range(int64_t address, int64_t first,
                                         int64_t last) {
    return first <= last ? address >= first && address <= last
                         : address <= first && address >= last;
}

static int llg_memory_enum_value_allowed(sv4_t value,
                                         const sv4_t* enum_values,
                                         uint32_t enum_count) {
    if (!enum_values || enum_count == 0) return 1;
    for (uint32_t index = 0; index < enum_count; ++index) {
        sv4_t match = sv4_case_eq(value, enum_values[index]);
        int allowed = sv4_to_bool(match);
        sv4_destroy(&match);
        if (allowed) return 1;
    }
    return 0;
}

// Memory-file words carry no signed marker. When a known word is wider than an
// enum base, its discarded bits must be redundant for that base: zeroes for an
// unsigned base, or copies of the retained sign bit for a signed base.
static int llg_memory_enum_word_fits_width(sv4_t value, uint32_t width,
                                            int8_t is_signed) {
    if (sv4_is_unknown(value) || llg_sv4_width(value) <= width) return 1;
    int sign = is_signed && width != 0
                   ? (int)((llg_sv4_word(value, (width - 1u) / 64u, LLG_SV4_BITS) >> ((width - 1u) % 64u)) & 1u)
                   : 0;
    for (uint32_t bit = width; bit < llg_sv4_width(value); ++bit) {
        int high = (int)((llg_sv4_word(value, bit / 64u, LLG_SV4_BITS) >> (bit % 64u)) & 1u);
        if (high != sign) return 0;
    }
    return 1;
}

// Memory words are unsigned based digits. Only a leading X/Z digit pads with
// its state; a known leading one still zero-extends, even into signed storage.
static sv4_t llg_memory_word_cast(sv4_t word, uint32_t width, int8_t is_signed) {
    int8_t extend_unknown = 0;
    if (llg_sv4_width(word) != 0 && width > llg_sv4_width(word)) {
        uint32_t bit = llg_sv4_width(word) - 1u;
        extend_unknown = llg_sv4_state(word, bit) >= 2u;
    }
    sv4_t result = sv4_resize(word, width, extend_unknown);
    llg_sv4_set_signed(&result, is_signed);
    return result;
}

static void fixed_memory_read_view(llg_string_t path, sv4_t* memory, llg_fixed_array_t* fixed, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix) {
    char* filename = llg_memory_path_copy(path);
    FILE* stream = fopen(filename, "r");
    if (!stream) {
        llg_memory_warning(filename, "open for reading failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, &first, &last, &inner_total)) {
        fclose(stream);
        free(filename);
        return;
    }
    uint64_t range = llg_memory_range_length(first, last);
    uint64_t expected = range > UINT64_MAX / inner_total
                            ? UINT64_MAX
                            : range * inner_total;
    uint64_t written = 0;
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    uint64_t inner = 0;
    int warned_extra = 0;
    int warned_unknown = 0;
    int saw_address = 0;
    llg_memory_value_t token = {0};
    for (;;) {
        int kind = llg_memory_next_token(stream, radix, &token);
        if (kind == LLG_MEMORY_TOKEN_EOF) break;
        if (kind == LLG_MEMORY_TOKEN_ERROR) {
            llg_memory_warning(filename, "malformed or over-width memory token");
            continue;
        }
        if (kind == LLG_MEMORY_TOKEN_ADDRESS) {
            saw_address = 1;
            int64_t address = token.address;
            if (token.invalid_index) {
                llg_memory_warning(filename, "address jump is not a known index");
                sv4_destroy(&token.value);
                fclose(stream);
                free(filename);
                return;
            } else {
                current = address;
                inner = 0;
                if (!llg_memory_address_offset(address, dims[0], dims[1], &(uint64_t){0}) ||
                    !llg_memory_in_requested_range(address, first, last)) {
                    llg_memory_warning(filename,
                        "address jump is outside the destination memory or selected range; load terminated");
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
            }
            continue;
        }
        if (!llg_memory_in_requested_range(current, first, last)) {
            if (!warned_extra) {
                llg_memory_warning(filename, "memory file contains more words than the selected range");
                warned_extra = 1;
            }
        } else {
            uint64_t index;
            if (inner >= inner_total ||
                !llg_memory_view_index(current, inner, dims, n_dims, strides,
                                       origin, view_total, total, &index)) {
                if (!warned_extra) {
                    llg_memory_warning(filename, "selected address is outside the destination memory");
                    warned_extra = 1;
                }
            } else {
                // Normalize two-state data before a narrowing conversion so a
                // low X/Z digit cannot hide a known out-of-range enum high bit.
                // Four-state enum membership remains an exact state comparison.
                int had_unknown = sv4_is_unknown(token.value);
                if (two_state && had_unknown) {
                    sv4_replace(&token.value, sv4_to_two_state(token.value));
                }
                if (enum_count != 0 &&
                    !llg_memory_enum_word_fits_width(token.value, elem_width,
                                                     elem_signed)) {
                    llg_memory_warning(filename,
                        "numeric memory data does not fit the enum base type; load terminated");
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
                sv4_t converted = llg_memory_word_cast(token.value, elem_width, elem_signed);
                if (two_state && had_unknown) {
                    if (!warned_unknown) {
                        llg_memory_warning(filename, "X/Z memory data converted to a two-state element");
                        warned_unknown = 1;
                    }
                }
                if (!llg_memory_enum_value_allowed(converted, enum_values, enum_count)) {
                    llg_memory_warning(filename,
                        "memory data value is not a member of the enum; load terminated");
                    sv4_destroy(&converted);
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
                llg_ba(fixed ? llg_fixed_array_cell(fixed, index) : &memory[index], converted);
                sv4_destroy(&converted);
                written++;
            }
        }
        if (inner + 1u >= inner_total) {
            inner = 0;
            if (current == last) {
                current = step > 0 ? INT64_MAX : INT64_MIN;
            } else if ((step > 0 && current < INT64_MAX) ||
                       (step < 0 && current > INT64_MIN)) {
                current += step;
            }
        } else {
            ++inner;
        }
    }
    sv4_destroy(&token.value);
    if (written < expected &&
        (!saw_address || addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001)) {
        llg_memory_warning(filename, "memory file contains too few words for the selected range");
    } else if (written > expected && !warned_extra &&
               addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001) {
        llg_memory_warning(filename, "memory file contains more words than the selected range");
    }
    if (ferror(stream)) llg_memory_warning(filename, "read failed");
    fclose(stream);
    free(filename);
}

static void fixed_memory_write_view(llg_string_t path, sv4_t* memory, llg_fixed_array_t* fixed, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix) {
    (void)two_state;
    (void)enum_values;
    (void)enum_count;
    char* requested = llg_memory_path_copy(path);
    char* filename = llg_output_path(requested);
    free(requested);
    FILE* stream = fopen(filename, "w");
    if (!stream) {
        llg_memory_warning(filename, "open for writing failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, &first, &last, &inner_total)) {
        fclose(stream);
        free(filename);
        return;
    }
    size_t capacity = (size_t)elem_width + 2u;
    char* digits = (char*)llg_checked_malloc(capacity, 1, "memory file word");
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    int warned_extra = 0;
    uint64_t inner = 0;
    for (;;) {
        uint64_t index;
        if (!llg_memory_view_index(current, inner, dims, n_dims, strides,
                                   origin, view_total, total, &index)) {
            if (!warned_extra) {
                llg_memory_warning(filename, "selected address is outside the source memory");
                warned_extra = 1;
            }
        } else {
            sv4_format(radix == 2 ? 'b' : 'h', *(fixed ? llg_fixed_array_peek(fixed, index) : &memory[index]), digits, capacity);
            if (fputs(digits, stream) == EOF || fputc('\n', stream) == EOF) {
                llg_memory_warning(filename, "write failed");
                break;
            }
        }
        if (inner + 1u >= inner_total) {
            inner = 0;
            if (current == last) break;
            if ((step > 0 && current == INT64_MAX) ||
                (step < 0 && current == INT64_MIN)) break;
            current += step;
        } else {
            ++inner;
        }
    }
    free(digits);
    if (fclose(stream) != 0) llg_memory_warning(filename, "close after writing failed");
    (void)elem_width;
    (void)elem_signed;
    free(filename);
}

void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int addressing_policy,
                     const sv4_t* enum_values, uint32_t enum_count, int radix) {
    uint64_t stride = 1;
    llg_memory_read_view(path, memory, total, elem_width, elem_signed, two_state,
                         dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                         start, finish, has_start, has_finish, addressing_policy,
                         enum_values, enum_count, radix);
}

void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int addressing_policy,
                      const sv4_t* enum_values, uint32_t enum_count, int radix) {
    uint64_t stride = 1;
    llg_memory_write_view(path, memory, total, elem_width, elem_signed, two_state,
                          dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                          start, finish, has_start, has_finish, addressing_policy,
                          enum_values, enum_count, radix);
}

static void llg_file_cleanup(void) {
    if (!llg_files_initialized) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (llg_file_slots[i].owned && llg_file_slots[i].open && llg_file_slots[i].stream)
            fclose(llg_file_slots[i].stream);
    }
    memset(llg_file_slots, 0, sizeof(llg_file_slots));
    llg_files_initialized = 0;
    llg_file_global_error = 0;
    llg_file_global_message[0] = 0;
}

void llg_memory_read_view(llg_string_t path, sv4_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix) {
    fixed_memory_read_view(path, memory, NULL, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, enum_values, enum_count, radix);
}
void llg_fixed_memory_read_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix) {
    fixed_memory_read_view(path, NULL, memory, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, enum_values, enum_count, radix);
}

void llg_memory_write_view(llg_string_t path, sv4_t* memory, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix) {
    fixed_memory_write_view(path, memory, NULL, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, enum_values, enum_count, radix);
}
void llg_fixed_memory_write_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix) {
    fixed_memory_write_view(path, NULL, memory, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, enum_values, enum_count, radix);
}

static const char* llg_severity_name(int severity) {
    switch (severity) {
        case LLG_SEVERITY_INFO: return "info";
        case LLG_SEVERITY_WARNING: return "warning";
        case LLG_SEVERITY_ERROR: return "error";
        case LLG_SEVERITY_FATAL: return "fatal";
        default: return "invalid";
    }
}

static void llg_report_severity_typed(int severity, const char* fmt,
                                      llg_fmt_arg_t* args, int n,
                                      const char* scope, const char* location) {
    if (severity < LLG_SEVERITY_INFO || severity > LLG_SEVERITY_FATAL) {
        fprintf(stderr, "llg runtime fatal: invalid severity level %d\n", severity);
        abort();
    }
    if (n < 0) {
        fprintf(stderr, "llg runtime fatal: negative severity argument count\n");
        abort();
    }
    if (llg_severity_counts[severity] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: severity counter overflow\n");
        abort();
    }
    size_t len = 0;
    char* out = llg_typed_line_alloc(fmt, args, n, scope, &len);
    llg_severity_counts[severity]++;
    fprintf(stderr, "llg: severity %s: %s: ",
            llg_severity_name(severity),
            location && location[0] ? location : "<unknown>");
    fwrite(out, 1, len, stderr);
    fputc('\n', stderr);
    fflush(stderr);
    free(out);
}

void llg_rt_severity_typed(int severity, const char* fmt, llg_fmt_arg_t* args,
                           int n, const char* scope, const char* location) {
    llg_report_severity_typed(severity, fmt, args, n, scope, location);
    llg_fmt_args_destroy(args, n);
}

void llg_rt_fatal_typed(int finish_number, const char* fmt,
                        llg_fmt_arg_t* args, int n,
                        const char* scope, const char* location) {
    if (finish_number < 0 || finish_number > 2) {
        fprintf(stderr, "llg runtime fatal: invalid $fatal finish number %d\n",
                finish_number);
        llg_fmt_args_destroy(args, n);
        abort();
    }
    llg_report_severity_typed(LLG_SEVERITY_FATAL, fmt, args, n, scope, location);
    llg_fmt_args_destroy(args, n);
    llg_rt_finish_with_level(finish_number, location);
}

uint64_t llg_rt_severity_count(int severity) {
    if (severity < LLG_SEVERITY_INFO || severity > LLG_SEVERITY_FATAL) return 0;
    return llg_severity_counts[severity];
}

static const char* llg_assertion_name(int kind) {
    switch (kind) {
        case LLG_ASSERTION_ASSERT: return "assert";
        case LLG_ASSERTION_ASSUME: return "assume";
        case LLG_ASSERTION_EXPECT: return "expect";
        default: return "invalid";
    }
}

static void assertion_record_failure(int kind) {
    if (kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        llg_assertion_failure_counts[kind] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: invalid assertion kind or counter overflow\n");
        abort();
    }
    llg_assertion_failure_counts[kind]++;
}

static void assertion_report_failure(int kind, const char* label,
                                      const char* location) {
    if (llg_severity_counts[LLG_SEVERITY_ERROR] == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: severity counter overflow\n");
        abort();
    }
    llg_severity_counts[LLG_SEVERITY_ERROR]++;
    fprintf(stderr, "llg: assertion %s failed: %s",
            llg_assertion_name(kind),
            location && location[0] ? location : "<unknown>");
    if (label && label[0]) fprintf(stderr, " (%s)", label);
    fputc('\n', stderr);
    fflush(stderr);
}

void llg_assertion_failure(int kind, uint64_t identity, const char* label,
                           const char* location) {
    (void)identity;
    assertion_record_failure(kind);
    assertion_report_failure(kind, label, location);
}

void llg_assertion_cover(uint64_t identity, const char* label, const char* location) {
    (void)identity;
    (void)label;
    (void)location;
    if (llg_assertion_cover_count == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: assertion coverage counter overflow\n");
        abort();
    }
    llg_assertion_cover_count++;
}

uint64_t llg_assertion_count(int kind) {
    if (kind == LLG_ASSERTION_COVER) return llg_assertion_cover_count;
    if (kind >= LLG_ASSERTION_ASSERT && kind <= LLG_ASSERTION_EXPECT)
        return llg_assertion_failure_counts[kind];
    return 0;
}

uint64_t llg_assertion_vacuous_count(void) {
    return llg_assertion_vacuous_total;
}

static llg_concurrent_assertion_t* find_assertion(uint64_t identity) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (assertion->identity == identity) return assertion;
    }
    return NULL;
}

static int path_matches_selector(const char* path, const char* selector) {
    if (!path || !selector || !path[0] || !selector[0]) return 0;
    if (strcmp(path, selector) == 0) return 1;
    size_t path_len = strlen(path);
    size_t selector_len = strlen(selector);
    if (path_len > selector_len &&
        path[path_len - selector_len - 1] == '.' &&
        strcmp(path + path_len - selector_len, selector) == 0)
        return 1;
    return selector_len < path_len &&
           strncmp(path, selector, selector_len) == 0 &&
           path[selector_len] == '.';
}

static int assertion_matches_scope(const llg_concurrent_assertion_t* assertion,
                                   const char* selector) {
    if (!selector || !selector[0]) return 0;
    if (path_matches_selector(assertion->scope, selector)) return 1;
    const char* scope = assertion->scope ? assertion->scope : "";
    const char* label = assertion->label ? assertion->label : "";
    if (!label[0]) return 0;
    size_t scope_len = strlen(scope);
    size_t label_len = strlen(label);
    size_t full_len = scope_len + (scope_len != 0) + label_len;
    char* full = (char*)llg_checked_malloc(full_len + 1, 1,
                                           "assertion hierarchy selector");
    if (scope_len != 0) {
        memcpy(full, scope, scope_len);
        full[scope_len] = '.';
    }
    memcpy(full + scope_len + (scope_len != 0), label, label_len);
    full[full_len] = '\0';
    int matched = path_matches_selector(full, selector);
    free(full);
    return matched;
}

static int assertion_matches_type(const llg_concurrent_assertion_t* assertion,
                                  uint64_t assertion_type,
                                  uint64_t directive_type) {
    // Table 20-6: concurrent assertions use bit 1 and expect uses bit 16.
    // Immediate/unique report classes are intentionally ignored because this
    // runtime registry owns only sampled concurrent instances.
    uint64_t type_bit = assertion->kind == LLG_ASSERTION_EXPECT ? 16u : 1u;
    if ((assertion_type & type_bit) == 0) return 0;
    uint64_t directive_bit = assertion->kind == LLG_ASSERTION_COVER
                                 ? 2u
                                 : assertion->kind == LLG_ASSERTION_ASSUME ? 4u : 1u;
    return (directive_type & directive_bit) != 0;
}

static int assertion_control_failure(const char* reason) {
    fprintf(stderr, "llg: assertion control error: %s\n", reason);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

static int assertion_control_arg(const sv4_t* value, uint64_t* result) {
    if (!value || llg_sv4_width(*value) == 0 || llg_sv4_width(*value) > 64 ||
        sv4_is_unknown(*value))
        return 0;
    *result = sv4_to_u64(*value);
    return 1;
}

static int assertion_has_attempts(const llg_concurrent_assertion_t* assertion) {
    return assertion->attempts || assertion->sequence_antecedents ||
           assertion->sequence_consequents;
}

static int deferred_selected(int kind, const char* label, const char* scope,
                              uint64_t types, uint64_t directives,
                              const char* const* scopes, int count) {
    uint64_t directive = kind == LLG_ASSERTION_COVER ? 2u :
                         kind == LLG_ASSERTION_ASSUME ? 4u : 1u;
    if (!(types & 4u) || !(directives & directive)) return 0; // #0 deferred
    if (!count) return 1;
    llg_concurrent_assertion_t view = {0};
    view.scope = scope;
    view.label = label;
    for (int i = 0; i < count; ++i)
        if (assertion_matches_scope(&view, scopes[i])) return 1;
    return 0;
}

int llg_deferred_assertion_enabled(int kind, const char* label, const char* scope) {
    for (llg_assertion_rule_t* rule = llg_assertion_rules; rule; rule = rule->next) {
        const char* selectors[] = {rule->scope};
        if (deferred_selected(kind, label, scope, rule->assertion_type,
                              rule->directive_type, selectors, rule->scope ? 1 : 0))
            return rule->enabled;
    }
    return 1;
}

static void assertion_remember_control(int enabled, uint64_t types,
                                       uint64_t directives, const char* scope) {
    // Replace equal selector/mask rules; repeated control in a loop must not
    // retain an unbounded history of identical commands.
    llg_assertion_rule_t** link = &llg_assertion_rules;
    llg_assertion_rule_t* rule = NULL;
    while (*link) {
        if ((*link)->assertion_type == types && (*link)->directive_type == directives &&
            ((!scope && !(*link)->scope) ||
             (scope && (*link)->scope && strcmp(scope, (*link)->scope) == 0))) {
            rule = *link;
            *link = rule->next;
            break;
        }
        link = &(*link)->next;
    }
    if (!rule) {
        rule = llg_checked_calloc(1, sizeof(*rule), "assertion control rule");
        if (scope) {
            size_t length = strlen(scope);
            rule->scope = llg_checked_malloc(length + 1u, 1, "assertion control scope");
            memcpy(rule->scope, scope, length + 1u);
        }
        rule->assertion_type = types;
        rule->directive_type = directives;
    }
    rule->enabled = enabled;
    rule->next = llg_assertion_rules;
    llg_assertion_rules = rule;
}

static void assertion_default_failure_action(void* data);

static void assertion_kill_actions(llg_concurrent_assertion_t* assertion) {
    // Cancellation changes the process table; restart each scan.
    for (;;) {
        llg_proc_t* victim = NULL;
        for (int i = 0; i < g.n_procs; ++i) {
            llg_proc_t* p = g.all_procs[i];
            if (p && p->is_assertion_action &&
                p->action_assertion == assertion->identity && !p->killed && !p->completed) {
                victim = p;
                break;
            }
        }
        if (!victim) break;
        llg_kill_proc_tree(victim);
    }
    llg_region_callback_t** link = &g.callbacks;
    while (*link) {
        llg_region_callback_t* entry = *link;
        if (entry->callback == assertion_default_failure_action && entry->data == assertion) {
            *link = entry->next;
            free(entry);
        } else link = &entry->next;
    }
}

static void assertion_kill_deferred(uint64_t types, uint64_t directives,
                                    const char* const* scopes, int count) {
    llg_deferred_assertion_report_t** link = &g.deferred_assertions;
    g.deferred_assertion_tail = NULL;
    while (*link) {
        llg_deferred_assertion_report_t* report = *link;
        if (deferred_selected(report->kind, report->label, report->scope,
                              types, directives, scopes, count)) {
            *link = report->next;
            free_deferred_assertion_report(report);
        } else {
            g.deferred_assertion_tail = report;
            link = &report->next;
        }
    }
    llg_region_callback_t** callback = &g.callbacks;
    while (*callback) {
        llg_region_callback_t* entry = *callback;
        llg_deferred_assertion_report_t* report = entry->data;
        if (entry->callback == deferred_assertion_callback && report &&
            deferred_selected(report->kind, report->label, report->scope,
                              types, directives, scopes, count)) {
            *callback = entry->next;
            free_deferred_assertion_report(report);
            free(entry);
        } else callback = &entry->next;
    }
}

int llg_assertion_control(int kind, const sv4_t* args, int n_args,
                          const char* const* scopes, int n_scopes) {
    if (!region_can_mutate("assertion control")) return 0;
    if (kind < LLG_ASSERTION_CONTROL_ON || kind > LLG_ASSERTION_CONTROL_FULL ||
        n_args < 0 || n_args > 4 || n_scopes < 0 ||
        (n_args != 0 && !args) || (n_scopes != 0 && !scopes))
        return assertion_control_failure("invalid control argument shape");

    for (int i = 0; i < n_scopes; ++i)
        if (!scopes[i] || !scopes[i][0])
            return assertion_control_failure("empty assertion scope selector");

    uint64_t values[4] = {0, 0, 0, 0};
    for (int index = 0; index < n_args; index++) {
        if (!assertion_control_arg(&args[index], &values[index]))
            return assertion_control_failure("control arguments must be known 64-bit integers");
    }

    int operation = kind;
    uint64_t assertion_type = UINT64_C(255);
    uint64_t directive_type = UINT64_C(7);
    if (kind == LLG_ASSERTION_CONTROL_FULL) {
        if (n_args < 1) return assertion_control_failure("$assertcontrol requires control_type");
        uint64_t control_type = values[0];
        if (control_type < 3 || control_type > 5)
            return assertion_control_failure("bounded $assertcontrol supports only ON, OFF, and KILL");
        operation = control_type - 3;
        if (n_args > 1) assertion_type = values[1];
        if (n_args > 2) directive_type = values[2];
        // The optional fourth argument is `levels`. This bounded registry
        // implements the standard level-0 (all descendants) selector; other
        // hierarchy-depth policies remain fail-closed until represented in
        // the owned assertion catalog.
        if (n_args > 3 && values[3] != 0)
            return assertion_control_failure("bounded assertion control supports only level 0");
        if (assertion_type > 255 || directive_type > 7)
            return assertion_control_failure("unsupported assertion or directive type");
    } else if (n_args > 1) {
        return assertion_control_failure("assertion control task takes one level argument");
    } else if (n_args == 1 && values[0] != 0) {
        return assertion_control_failure("bounded assertion control supports only level 0");
    }

    if (n_scopes == 0)
        assertion_remember_control(operation == LLG_ASSERTION_CONTROL_ON,
                                   assertion_type, directive_type, NULL);
    for (int i = 0; i < n_scopes; ++i)
        assertion_remember_control(operation == LLG_ASSERTION_CONTROL_ON,
                                   assertion_type, directive_type, scopes[i]);
    if (operation == LLG_ASSERTION_CONTROL_KILL)
        assertion_kill_deferred(assertion_type, directive_type, scopes, n_scopes);

    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (kind == LLG_ASSERTION_CONTROL_FULL &&
            !assertion_matches_type(assertion, assertion_type, directive_type))
            continue;
        int selected = n_scopes == 0;
        for (int index = 0; !selected && index < n_scopes; index++)
            selected = assertion_matches_scope(assertion, scopes[index]);
        if (!selected) continue;
        switch (operation) {
            case LLG_ASSERTION_CONTROL_ON:
                assertion->enabled = 1;
                break;
            case LLG_ASSERTION_CONTROL_OFF:
                assertion->enabled = 0;
                break;
            case LLG_ASSERTION_CONTROL_KILL:
                assertion->enabled = 0;
                assertion_kill_actions(assertion);
                free_assertion_attempts(assertion);
                assertion->edge_pending = 0;
                if (assertion->kind == LLG_ASSERTION_EXPECT &&
                    assertion->expect_active) {
                    assertion->expect_active = 0;
                    wake_assertion_waiter(assertion->identity);
                }
                break;
            default:
                return assertion_control_failure("invalid assertion control operation");
        }
    }
    if (operation == LLG_ASSERTION_CONTROL_KILL) {
        llg_proc_t* current = llg_current();
        service_program_completions();
        semaphore_service_cancelled_waiters();
        reap_retired_procs();
        if (current && current->killed)
            current->chain.exiting = LLG_EXIT_ABANDON;
        else if (current && g.finish)
            current->chain.exiting = LLG_EXIT_COMPLETE;
    }
    return 1;
}

int llg_assertion_expect_start(uint64_t identity) {
    if (!region_can_mutate("expect scheduling")) return 0;
    llg_concurrent_assertion_t* assertion = find_assertion(identity);
    llg_proc_t* current = llg_current();
    if (!assertion || assertion->kind != LLG_ASSERTION_EXPECT || !current ||
        assertion->expect_active)
        return assertion_control_failure("expect has no unique inactive assertion instance");
    free_assertion_attempts(assertion);
    assertion->expect_active = 1;
    assertion->edge_pending = 0;
    assertion->sequence_cycle = 0;
    return 1;
}

static void assertion_attempt_enqueue(llg_concurrent_assertion_t* assertion) {
    llg_assertion_attempt_t* attempt = (llg_assertion_attempt_t*)llg_checked_calloc(
        1, sizeof(*attempt), "concurrent assertion attempt");
    attempt->due = 1;
    if (assertion->attempts_tail) {
        assertion->attempts_tail->next = attempt;
    } else {
        assertion->attempts = attempt;
    }
    assertion->attempts_tail = attempt;
}

static llg_sequence_attempt_t* sequence_attempt_from_data(void* data) {
    return (llg_sequence_attempt_t*)data;
}

sv4_t* llg_sequence_local_addr(void* data, uint32_t slot) {
    llg_sequence_attempt_t* attempt = sequence_attempt_from_data(data);
    if (!attempt || !attempt->graph || slot >= attempt->graph->local_count ||
        !attempt->locals) {
        fprintf(stderr, "llg runtime fatal: invalid sequence local slot %u\n",
                (unsigned)slot);
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    return &attempt->locals[slot];
}

sv4_t llg_sequence_local_read(void* data, uint32_t slot) {
    sv4_t* value = llg_sequence_local_addr(data, slot);
    return value ? sv4_clone(value) : sv4_x(1, 0);
}

void llg_sequence_local_write(sv4_t* target, sv4_t value) {
    if (!target) {
        fprintf(stderr, "llg runtime fatal: null sequence local target\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    /* Local assertion storage is private to one attempt. It has no signal
     * waiters, force/PCA drivers, or scheduler-visible notifications, so the
     * match-item write is intentionally a direct value replacement even while
     * the enclosing assertion is being resolved in Observed. */
    sv4_copy(target, &value);
}

static int sequence_locals_same(const sv4_t* left, const sv4_t* right,
                                uint32_t count) {
    if (count == 0) return 1;
    if (!left || !right) return 0;
    for (uint32_t index = 0; index < count; index++)
        if (!sv4_same(left[index], right[index])) return 0;
    return 1;
}

static sv4_t* sequence_locals_clone(const llg_sequence_graph_t* graph,
                                    const sv4_t* locals) {
    if (!graph || graph->local_count == 0) return NULL;
    if (!locals) {
        fprintf(stderr, "llg runtime fatal: missing sequence thread locals\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    sv4_t* copy = (sv4_t*)llg_checked_calloc(
        graph->local_count, sizeof(*copy), "concurrent assertion sequence thread locals");
    for (uint32_t i = 0; i < graph->local_count; ++i)
        copy[i] = sv4_clone(&locals[i]);
    return copy;
}

static void sequence_scope_retain(llg_sequence_scope_t* scope) {
    if (scope) {
        if (scope->refs == SIZE_MAX) { fprintf(stderr, "llg: sequence scope reference overflow\n"); abort(); }
        scope->refs++;
    }
}

static void sequence_scope_release(llg_sequence_scope_t* scope) {
    while (scope && --scope->refs == 0) {
        llg_sequence_scope_t* parent = scope->parent;
        free(scope);
        scope = parent;
    }
}

static int sequence_scope_closed(const llg_sequence_scope_t* scope) {
    for (; scope; scope = scope->parent) if (scope->matched) return 1;
    return 0;
}

static int sequence_scope_allows(const llg_sequence_scope_t* scope,
                                 const llg_assertion_clock_event_t* event) {
    for (; scope; scope = scope->parent) {
        if (!scope->matched) continue;
        if (scope->time != event->time || scope->clock != event->signal ||
            scope->edge != event->edge || scope->tick != event->tick) return 0;
    }
    return 1;
}

static void sequence_token_free(llg_sequence_token_t* token) {
    if (!token) return;
    sequence_scope_release(token->scope);
    sv4_destroy_array(token->locals, token->local_count);
    free(token->locals);
    free(token);
}

static void sequence_tokens_free(llg_sequence_token_t* tokens) {
    while (tokens) {
        llg_sequence_token_t* next = tokens->next;
        sequence_token_free(tokens);
        tokens = next;
    }
}

static llg_sequence_token_t* sequence_token_copy(
    const llg_sequence_graph_t* graph, const llg_sequence_token_t* source) {
    llg_sequence_token_t* token = llg_checked_calloc(1, sizeof(*token), "sequence token");
    *token = *source;
    token->next = NULL;
    token->local_count = graph->local_count;
    token->locals = sequence_locals_clone(graph, source->locals);
    sequence_scope_retain(token->scope);
    return token;
}

static int sequence_token_same(const llg_sequence_graph_t* graph,
                               const llg_sequence_token_t* a,
                               const llg_sequence_token_t* b) {
    return a->state == b->state && a->transition == b->transition &&
        a->scope == b->scope && a->entered_time == b->entered_time &&
        a->entered_tick == b->entered_tick && a->entered_clock == b->entered_clock &&
        a->entered_edge == b->entered_edge && a->entered_order == b->entered_order &&
        sequence_locals_same(a->locals, b->locals, graph->local_count);
}

/* Takes ownership, including the scope reference and local snapshot. */
static void sequence_token_push(const llg_sequence_graph_t* graph,
                                llg_sequence_token_t** list,
                                llg_sequence_token_t* token) {
    for (llg_sequence_token_t* old = *list; old; old = old->next) {
        if (!sequence_token_same(graph, old, token)) continue;
        if (token->checked && (!old->checked || old->last_order < token->last_order)) {
            old->checked = 1;
            old->last_order = token->last_order;
        }
        sequence_token_free(token);
        return;
    }
    token->next = *list;
    *list = token;
}

static void sequence_endpoints_free(llg_sequence_endpoint_t* endpoint) {
    while (endpoint) {
        llg_sequence_endpoint_t* next = endpoint->next;
        sv4_destroy_array(endpoint->locals, endpoint->local_count);
        free(endpoint->locals);
        free(endpoint);
        endpoint = next;
    }
}

static void sequence_endpoint_add(llg_sequence_attempt_t* attempt,
                                   const llg_sequence_token_t* token, int empty) {
    llg_sequence_endpoint_t* endpoint = llg_checked_calloc(1, sizeof(*endpoint), "sequence endpoint");
    endpoint->local_count = attempt->graph->local_count;
    endpoint->locals = sequence_locals_clone(attempt->graph, token->locals);
    endpoint->clock = token->entered_clock;
    endpoint->edge = token->entered_edge;
    endpoint->time = token->entered_time;
    endpoint->tick = token->entered_tick;
    endpoint->order = token->entered_order;
    endpoint->empty = empty;
    endpoint->next = attempt->endpoints;
    attempt->endpoints = endpoint;
    if (!empty) attempt->matched = 1;
}

static void sequence_match_items(const llg_sequence_graph_t* graph,
                                 llg_sequence_attempt_t* attempt,
                                 uint32_t start, uint32_t count) {
    if (count == 0) return;
    if (!graph->match || start > graph->match_item_count || count > graph->match_item_count - start) {
        fprintf(stderr, "llg runtime fatal: invalid sequence match-item range\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    for (uint32_t index = 0; index < count && !g.finish; index++) graph->match(start + index, attempt);
}

static void sequence_token_anchor(llg_sequence_token_t* token,
                                   const llg_assertion_clock_event_t* event) {
    token->entered_time = event->time;
    token->entered_tick = event->tick;
    token->entered_order = event->order;
    token->entered_clock = event->signal;
    token->entered_edge = event->edge;
    token->checked = 0;
    token->last_order = 0;
}

/* A coincident destination edge is usable even when delivered before the
 * source. History is bounded to one physical time slot, not the whole run. */
static const llg_assertion_clock_event_t* sequence_coincident(
    const llg_concurrent_assertion_t* assertion, sv4_t* clock, int edge, uint64_t time) {
    for (const llg_assertion_clock_event_t* event = assertion->clock_history;
         event; event = event->next)
        if (event->signal == clock && event->edge == edge && event->time == time) return event;
    return NULL;
}

static int sequence_choose_event(const llg_concurrent_assertion_t* assertion,
                                  const llg_sequence_token_t* token,
                                  const llg_sequence_transition_t* transition,
                                  const llg_assertion_clock_event_t* current,
                                  llg_assertion_clock_event_t* selected,
                                  uint64_t* elapsed) {
    sv4_t* clock = transition->clock ? transition->clock : assertion->clock;
    int edge = transition->clock ? transition->edge : assertion->edge;
    int same = clock == token->entered_clock && edge == token->entered_edge;
    if (same) {
        if (!token->checked && transition->min_delay == 0 && token->entered_time == current->time) {
            *selected = (llg_assertion_clock_event_t){ .signal = clock, .edge = edge,
                .time = token->entered_time, .tick = token->entered_tick, .order = token->entered_order };
            *elapsed = 0;
            return 1;
        }
        if (current->signal != clock || current->edge != edge || current->tick < token->entered_tick) return 0;
        *selected = *current;
        *elapsed = current->tick - token->entered_tick;
        return 1;
    }
    if (!((transition->min_delay == 0 && transition->max_delay == 0) ||
          (transition->min_delay == 1 && transition->max_delay == 1))) {
        fprintf(stderr, "llg: invalid cross-clock sequence boundary\n");
        llg_last_failure = 1; g.finish = 1; return 0;
    }
    const llg_assertion_clock_event_t* candidate = NULL;
    if (transition->min_delay == 0 && current->time == token->entered_time)
        candidate = sequence_coincident(assertion, clock, edge, token->entered_time);
    if (!candidate && current->signal == clock && current->edge == edge &&
        (transition->min_delay == 0 ? current->time >= token->entered_time : current->time > token->entered_time))
        candidate = current;
    if (!candidate) return 0;
    *selected = *candidate;
    *elapsed = transition->min_delay;
    return 1;
}

int llg_sequence_local_inherited(void* data, uint32_t slot) {
    llg_sequence_attempt_t* attempt = sequence_attempt_from_data(data);
    return attempt && attempt->graph && slot < attempt->graph->local_count &&
        attempt->inherited && attempt->inherited[slot];
}

static llg_sequence_attempt_t* sequence_attempt_new(
    const llg_sequence_graph_t* graph, uint64_t due_cycle,
    const llg_sequence_graph_t* source, const sv4_t* values) {
    llg_sequence_attempt_t* attempt = llg_checked_calloc(1, sizeof(*attempt), "sequence attempt");
    attempt->graph = graph;
    attempt->due_cycle = due_cycle;
    if (graph->local_count) {
        attempt->locals = llg_checked_calloc(graph->local_count, sizeof(*attempt->locals), "sequence locals");
        attempt->inherited = llg_checked_calloc(graph->local_count, 1, "sequence inherited locals");
        for (uint32_t i = 0; i < graph->local_count; i++) {
            const llg_sequence_local_t* local = &graph->locals[i];
            attempt->locals[i] = sv4_x(local->width, local->is_signed);
            if (local->two_state) sv4_replace(&attempt->locals[i], sv4_to_two_state(attempt->locals[i]));
            if (!source || !values || !local->declaration) continue;
            for (uint32_t j = 0; j < source->local_count; j++) {
                const llg_sequence_local_t* from = &source->locals[j];
                if (from->declaration != local->declaration) continue;
                if (from->width != local->width || from->is_signed != local->is_signed || from->two_state != local->two_state) {
                    fprintf(stderr, "llg: inconsistent assertion local type across implication\n");
                    llg_last_failure = 1; g.finish = 1; break;
                }
                sv4_copy(&attempt->locals[i], &values[j]);
                attempt->inherited[i] = 1;
                break;
            }
        }
    }
    return attempt;
}

static int sequence_start(llg_sequence_attempt_t* attempt,
                           const llg_concurrent_assertion_t* assertion,
                           const llg_assertion_clock_event_t* current) {
    llg_assertion_clock_event_t event = *current;
    if (attempt->launch_pending) {
        llg_sequence_token_t source = { .entered_time = attempt->launch.time,
            .entered_tick = attempt->launch.tick, .entered_order = attempt->launch.order,
            .entered_clock = attempt->launch.clock, .entered_edge = attempt->launch.edge };
        llg_sequence_transition_t boundary = { .clock = attempt->graph->leading_clock,
            .edge = attempt->graph->leading_edge, .min_delay = attempt->launch_strict,
            .max_delay = attempt->launch_strict };
        uint64_t elapsed = 0;
        if (!sequence_choose_event(assertion, &source, &boundary, current, &event, &elapsed)) return 0;
        if (elapsed < boundary.min_delay) return 0;
    }
    // Consequent-private initializers run when that consequent actually starts;
    // inherited declaration cells have already been copied from its endpoint.
    if (attempt->graph->init) attempt->graph->init(attempt);
    if (g.finish) return 0;
    llg_sequence_token_t seed = { .state = attempt->graph->start,
        .transition = UINT32_MAX, .locals = attempt->locals };
    sequence_token_anchor(&seed, &event);
    if (attempt->graph->admits_empty) sequence_endpoint_add(attempt, &seed, 1);
    attempt->tokens = sequence_token_copy(attempt->graph, &seed);
    if (attempt->locals) sv4_destroy_array(attempt->locals, attempt->graph->local_count);
    free(attempt->locals);
    free(attempt->inherited);
    attempt->locals = NULL;
    attempt->inherited = NULL;
    attempt->started = 1;
    return 1;
}

/* Pending tokens own ONE outgoing edge. This prevents replaying an already
 * consumed cross-clock boundary just because a sibling edge remains pending.
 * Zero-delay closure consumes no tick and first_match cancellation is scoped
 * to the dynamic invocation, leaving tied endpoints and outer alternatives. */
static int sequence_attempt_step(llg_sequence_attempt_t* attempt,
                                  const llg_concurrent_assertion_t* assertion,
                                  uint64_t cycle, sv4_t* event_clock, int event_edge,
                                  uint64_t event_time, uint64_t event_order,
                                  uint64_t event_tick, int* accepted) {
    const llg_sequence_graph_t* graph = attempt->graph;
    llg_assertion_clock_event_t current = { .signal = event_clock, .edge = event_edge,
        .time = event_time, .order = event_order, .tick = event_tick };
    (void)cycle;
    sequence_endpoints_free(attempt->endpoints);
    attempt->endpoints = NULL;
    *accepted = 0;
    if (!attempt->started && !sequence_start(attempt, assertion, &current)) return !g.finish;
    llg_sequence_token_t* work = attempt->tokens;
    llg_sequence_token_t* processed = NULL;
    llg_sequence_token_t* next = NULL;
    attempt->tokens = NULL;
    while (work && !g.finish) {
        llg_sequence_token_t* token = work;
        work = token->next;
        token->next = NULL;
        int duplicate = 0;
        for (llg_sequence_token_t* old = processed; old; old = old->next)
            if (sequence_token_same(graph, old, token)) { duplicate = 1; break; }
        if (duplicate) { sequence_token_free(token); continue; }
        token->next = processed;
        processed = token;
        if (token->transition == UINT32_MAX) {
            if (token->state == graph->accept) { sequence_endpoint_add(attempt, token, 0); continue; }
            for (uint32_t i = 0; i < graph->transition_count; i++) {
                if (graph->transitions[i].from != token->state) continue;
                llg_sequence_token_t* edge = sequence_token_copy(graph, token);
                edge->transition = i;
                sequence_token_push(graph, &work, edge);
            }
            continue;
        }
        const llg_sequence_transition_t* edge = &graph->transitions[token->transition];
        llg_assertion_clock_event_t event;
        uint64_t elapsed = 0;
        if (!sequence_choose_event(assertion, token, edge, &current, &event, &elapsed)) {
            if (!sequence_scope_closed(token->scope)) sequence_token_push(graph, &next, sequence_token_copy(graph, token));
            continue;
        }
        if (!sequence_scope_allows(token->scope, &event)) continue;
        if (edge->max_delay != LLG_SEQUENCE_UNBOUNDED && elapsed > edge->max_delay) continue;
        if (elapsed < edge->min_delay || (token->checked && token->last_order == event.order)) {
            if (!sequence_scope_closed(token->scope) &&
                (edge->max_delay == LLG_SEQUENCE_UNBOUNDED || elapsed < edge->max_delay))
                sequence_token_push(graph, &next, sequence_token_copy(graph, token));
            continue;
        }
        token->checked = 1;
        token->last_order = event.order;
        if (edge->max_delay == LLG_SEQUENCE_UNBOUNDED || elapsed < edge->max_delay)
            sequence_token_push(graph, &next, sequence_token_copy(graph, token));
        llg_sequence_token_t* destination = sequence_token_copy(graph, token);
        destination->state = edge->to;
        destination->transition = UINT32_MAX;
        sequence_token_anchor(destination, &event);
        attempt->locals = destination->locals;
        int matches = edge->atom == LLG_SEQUENCE_EPSILON || (graph->atom && graph->atom(edge->atom, attempt));
        if (matches && edge->exit_scope) {
            llg_sequence_scope_t* scope = destination->scope;
            if (!scope || scope->identity != edge->exit_scope) {
                fprintf(stderr, "llg: unbalanced first_match scope\n");
                llg_last_failure = 1; g.finish = 1; matches = 0;
            } else if (scope->matched && (scope->time != event.time || scope->tick != event.tick ||
                       scope->clock != event.signal || scope->edge != event.edge)) matches = 0;
            else {
                scope->matched = 1; scope->time = event.time; scope->tick = event.tick;
                scope->clock = event.signal; scope->edge = event.edge;
                destination->scope = scope->parent;
                sequence_scope_retain(destination->scope);
                sequence_scope_release(scope);
            }
        }
        if (matches && edge->enter_scope) {
            llg_sequence_scope_t* scope = llg_checked_calloc(1, sizeof(*scope), "first_match invocation");
            scope->refs = 1;
            scope->identity = edge->enter_scope;
            scope->parent = destination->scope; // transfer the token's parent reference
            destination->scope = scope;
        }
        if (matches) sequence_match_items(graph, attempt, edge->match_start, edge->match_count);
        attempt->locals = NULL;
        if (matches && !g.finish) sequence_token_push(graph, &work, destination);
        else sequence_token_free(destination);
    }
    sequence_tokens_free(work);
    sequence_tokens_free(processed);
    llg_sequence_token_t** link = &next;
    while (*link) {
        llg_sequence_token_t* token = *link;
        if (sequence_scope_closed(token->scope) || (graph->first_match && attempt->endpoints)) {
            *link = token->next;
            sequence_token_free(token);
        } else link = &token->next;
    }
    attempt->tokens = next;
    *accepted = attempt->endpoints != NULL;
    return !g.finish && next != NULL;
}

static void sequence_attempt_append(llg_sequence_attempt_t** head,
                                    llg_sequence_attempt_t** tail,
                                    llg_sequence_attempt_t* attempt) {
    if (*tail) (*tail)->next = attempt;
    else *head = attempt;
    *tail = attempt;
}

static void sequence_attempt_discard(llg_sequence_attempt_t* attempt) {
    if (!attempt) return;
    sequence_tokens_free(attempt->tokens);
    sequence_endpoints_free(attempt->endpoints);
    if (attempt->locals) sv4_destroy_array(attempt->locals, attempt->graph->local_count);
    free(attempt->locals);
    free(attempt->inherited);
    free(attempt);
}

static int sequence_cycle_next(llg_concurrent_assertion_t* assertion, uint64_t* cycle) {
    if (assertion->sequence_cycle == UINT64_MAX) {
        fprintf(stderr, "llg: concurrent assertion sequence clock-cycle counter overflow\n");
        llg_last_failure = 1; g.finish = 1; return 0;
    }
    *cycle = assertion->sequence_cycle++;
    return 1;
}

static int sequence_spawn_consequents(llg_concurrent_assertion_t* assertion,
                                      llg_sequence_attempt_t* antecedent, uint64_t cycle) {
    llg_sequence_endpoint_t* endpoint = antecedent->endpoints;
    antecedent->endpoints = NULL;
    while (endpoint) {
        llg_sequence_endpoint_t* next = endpoint->next;
        if (!(endpoint->empty && assertion->overlapped)) {
            antecedent->matched = 1;
            llg_sequence_attempt_t* consequent = sequence_attempt_new(
                assertion->consequent_sequence, cycle, antecedent->graph, endpoint->locals);
            consequent->launch_pending = 1;
            consequent->launch = *endpoint;
            consequent->launch.next = NULL;
            consequent->launch.locals = NULL;
            // An empty endpoint is before its start; |=> then starts at that
            // start, not at the next clock. Nonempty endpoints consume one tick.
            consequent->launch_strict = !assertion->overlapped && !endpoint->empty;
            sequence_attempt_append(&assertion->sequence_consequents,
                                    &assertion->sequence_consequents_tail, consequent);
        }
        sv4_destroy_array(endpoint->locals, endpoint->local_count);
        free(endpoint->locals);
        free(endpoint);
        endpoint = next;
    }
    return !g.finish;
}

static void assertion_action(llg_concurrent_assertion_t* assertion,
                             const llg_co_desc_t* desc) {
    if (!desc || g.finish) return;
    const char* name = assertion->label && assertion->label[0]
                           ? assertion->label
                           : "concurrent assertion action";
    llg_proc_t* proc = llg_spawn_in_region(desc, name, LLG_REGION_REACTIVE);
    if (proc) {
        proc->is_assertion_action = 1;
        proc->action_assertion = assertion->identity;
    }
}

static void assertion_vacuous(void) {
    if (llg_assertion_vacuous_total == UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: assertion vacuity counter overflow\n");
        abort();
    }
    llg_assertion_vacuous_total++;
}

static void assertion_default_failure_action(void* data) {
    // Assertion records outlive callbacks; cleanup discards callbacks first.
    llg_concurrent_assertion_t* assertion = data;
    assertion_report_failure(assertion->kind, assertion->label,
                              assertion->location);
}

static void assertion_result(llg_concurrent_assertion_t* assertion, int success,
                             int vacuous) {
    if (vacuous) assertion_vacuous();
    if (success) {
        // Cover counts and pass actions represent a non-vacuous match. An
        // implication with a false antecedent is accounted separately but is
        // not a coverage hit. Assert/assume pass actions still run for their
        // vacuous success, as required by assertion action semantics.
        if (assertion->kind == LLG_ASSERTION_COVER && !vacuous)
            llg_assertion_cover(assertion->identity, assertion->label,
                                assertion->location);
        if (assertion->kind != LLG_ASSERTION_COVER || !vacuous)
            assertion_action(assertion, assertion->pass_desc);
    } else {
        if (assertion->kind == LLG_ASSERTION_COVER) {
            assertion_action(assertion, assertion->fail_desc);
        } else {
            assertion_record_failure(assertion->kind);
            if (assertion->fail_desc) {
                // Even an explicit null else is a generated action function.
                assertion_action(assertion, assertion->fail_desc);
            } else {
                (void)llg_schedule_region_callback(
                    LLG_REGION_REACTIVE, assertion_default_failure_action,
                    assertion);
            }
        }
    }
    if (assertion->kind == LLG_ASSERTION_EXPECT && assertion->expect_active) {
        assertion->expect_active = 0;
        wake_assertion_waiter(assertion->identity);
    }
}

static void assertion_disable_signal_changed(sv4_t* signal) {
    if (!signal) return;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (assertion->disable == signal && sv4_to_bool(*signal))
            free_assertion_attempts(assertion);
    }
}

static void assertion_abort_attempts(llg_concurrent_assertion_t* assertion) {
    if (!assertion || !assertion->abort_condition) return;
    free_assertion_clock_events(assertion);
    const int success = assertion->abort_reject ? 0 : 1;
    while (assertion->attempts) {
        llg_assertion_attempt_t* attempt = assertion->attempts;
        assertion->attempts = attempt->next;
        if (!assertion->attempts) assertion->attempts_tail = NULL;
        assertion_result(assertion, success, assertion->abort_reject ? 0 : 1);
        free(attempt);
        if (g.finish) return;
    }
    llg_sequence_attempt_t** lists[] = {
        &assertion->sequence_antecedents,
        &assertion->sequence_consequents,
    };
    llg_sequence_attempt_t** tails[] = {
        &assertion->sequence_antecedents_tail,
        &assertion->sequence_consequents_tail,
    };
    for (size_t list_index = 0; list_index < sizeof(lists) / sizeof(lists[0]);
         list_index++) {
        while (*lists[list_index]) {
            llg_sequence_attempt_t* attempt = *lists[list_index];
            *lists[list_index] = attempt->next;
            assertion_result(assertion, success, assertion->abort_reject ? 0 : 1);
            sequence_attempt_discard(attempt);
            if (g.finish) return;
        }
        *tails[list_index] = NULL;
    }
}

static void assertion_abort_condition_changed(void) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if ((assertion->enabled || assertion_has_attempts(assertion)) &&
            (assertion->kind != LLG_ASSERTION_EXPECT || assertion->expect_active) &&
            assertion->abort_condition && !assertion->abort_sync &&
            assertion->abort_condition(assertion->data)) {
            assertion_abort_attempts(assertion);
            if (g.finish) return;
        }
    }
}

static int sequence_graph_uses_clock(const llg_sequence_graph_t* graph,
                                     sv4_t* signal, int edge) {
    if (!graph || !signal) return 0;
    for (uint32_t index = 0; index < graph->transition_count; index++) {
        const llg_sequence_transition_t* transition = &graph->transitions[index];
        if (transition->clock == signal && transition->edge == edge) return 1;
    }
    return 0;
}

static int assertion_sequence_uses_clock(llg_concurrent_assertion_t* assertion,
                                         sv4_t* signal, int edge) {
    return assertion &&
           (sequence_graph_uses_clock(assertion->antecedent_sequence, signal,
                                      edge) ||
            sequence_graph_uses_clock(assertion->consequent_sequence, signal,
                                      edge));
}

static void assertion_clock_event_append(llg_concurrent_assertion_t* assertion,
                                         sv4_t* signal, int edge,
                                         uint64_t order) {
    llg_assertion_clock_event_t* event = (llg_assertion_clock_event_t*)llg_checked_calloc(
        1, sizeof(*event), "concurrent assertion clock event");
    event->signal = signal;
    event->edge = edge;
    event->time = g.now;
    event->order = order;
    event->tick = assertion_clock_tick(signal, edge);
    if (assertion->clock_events_tail)
        assertion->clock_events_tail->next = event;
    else
        assertion->clock_events = event;
    assertion->clock_events_tail = event;
    if (assertion->clock_history && assertion->clock_history->time != g.now) {
        while (assertion->clock_history) {
            llg_assertion_clock_event_t* next = assertion->clock_history->next;
            free(assertion->clock_history);
            assertion->clock_history = next;
        }
        assertion->clock_history_tail = NULL;
    }
    llg_assertion_clock_event_t* saved = llg_checked_calloc(1, sizeof(*saved), "sequence clock history");
    *saved = *event;
    saved->next = NULL;
    if (assertion->clock_history_tail) assertion->clock_history_tail->next = saved;
    else assertion->clock_history = saved;
    assertion->clock_history_tail = saved;
}

static void assertion_clock_signal_changed(sv4_t* signal, sv4_t old,
                                           sv4_t value) {
    if (!signal) return;
    if (llg_assertion_event_order == UINT64_MAX) {
        fprintf(stderr, "llg: concurrent assertion event-order overflow\n");
        llg_last_failure = 1;
        g.finish = 1;
        return;
    }
    uint64_t order = llg_assertion_event_order++;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if ((!assertion->enabled && !assertion_has_attempts(assertion)) ||
            (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active))
            continue;
        if (assertion->disable && sv4_to_bool(*assertion->disable)) continue;
        if (assertion->clock == signal &&
            ev_matches_changed(old, value, assertion->edge)) {
            if (assertion->consequent_sequence)
                assertion_clock_event_append(assertion, signal, assertion->edge,
                                             order);
            else
                assertion->edge_pending = 1;
            g.assertion_edges_pending = 1;
            continue;
        }
        if (assertion->consequent_sequence &&
            ev_matches(old, value, LLG_EV_POSEDGE) &&
            assertion_sequence_uses_clock(assertion, signal, LLG_EV_POSEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_POSEDGE,
                                         order);
            g.assertion_edges_pending = 1;
        } else if (assertion->consequent_sequence &&
                   ev_matches(old, value, LLG_EV_NEGEDGE) &&
                   assertion_sequence_uses_clock(assertion, signal,
                                                 LLG_EV_NEGEDGE)) {
            assertion_clock_event_append(assertion, signal, LLG_EV_NEGEDGE,
                                         order);
            g.assertion_edges_pending = 1;
        }
    }
}

static int run_sequence_concurrent_assertion(llg_concurrent_assertion_t* assertion,
                                             uint64_t cycle,
                                             sv4_t* event_clock,
                                             int event_edge, uint64_t event_time,
                                             uint64_t event_order,
                                             uint64_t event_tick, int root_event) {
    llg_sequence_attempt_t** antecedent_link = &assertion->sequence_antecedents;
    while (*antecedent_link) {
        llg_sequence_attempt_t* attempt = *antecedent_link;
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted && !sequence_spawn_consequents(assertion, attempt, cycle)) return 0;
        if (!alive) {
            *antecedent_link = attempt->next;
            if (assertion->sequence_antecedents_tail == attempt)
                assertion->sequence_antecedents_tail = NULL;
            if (!attempt->matched) assertion_result(assertion, 1, 1);
            sequence_attempt_discard(attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        } else {
            antecedent_link = &attempt->next;
        }
        if (g.finish) return 0;
    }
    if (assertion->sequence_antecedents_tail == NULL) {
        for (llg_sequence_attempt_t* item = assertion->sequence_antecedents;
             item; item = item->next)
            assertion->sequence_antecedents_tail = item;
    }

    if (root_event && assertion->enabled && assertion->antecedent_sequence) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->antecedent_sequence, cycle, NULL, NULL);
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted && !sequence_spawn_consequents(assertion, attempt, cycle)) {
            sequence_attempt_discard(attempt);
            return 0;
        }
        if (alive) {
            sequence_attempt_append(&assertion->sequence_antecedents,
                                    &assertion->sequence_antecedents_tail, attempt);
        } else {
            if (!attempt->matched) assertion_result(assertion, 1, 1);
            sequence_attempt_discard(attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
        }
    } else if (root_event && assertion->enabled) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(
            assertion->consequent_sequence, cycle, NULL, NULL);
        sequence_attempt_append(&assertion->sequence_consequents,
                                &assertion->sequence_consequents_tail, attempt);
    }

    llg_sequence_attempt_t** consequent_link = &assertion->sequence_consequents;
    while (*consequent_link) {
        llg_sequence_attempt_t* attempt = *consequent_link;
        int accepted = 0;
        int alive = sequence_attempt_step(
            attempt, assertion, cycle, event_clock, event_edge, event_time, event_order,
            event_tick, &accepted);
        if (accepted || !alive) {
            *consequent_link = attempt->next;
            if (assertion->sequence_consequents_tail == attempt)
                assertion->sequence_consequents_tail = NULL;
            sequence_attempt_discard(attempt);
            assertion_result(assertion, accepted, 0);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)
                return 1;
            if (g.finish) return 0;
        } else {
            consequent_link = &attempt->next;
        }
    }
    if (assertion->sequence_consequents_tail == NULL) {
        for (llg_sequence_attempt_t* item = assertion->sequence_consequents;
             item; item = item->next)
            assertion->sequence_consequents_tail = item;
    }
    return 1;
}

static void run_concurrent_assertion(llg_concurrent_assertion_t* assertion) {
    // A clock transition is observed after Active/NBA writes, while every
    // predicate reads the immutable Preponed snapshot from this time slot.
    if ((!assertion->enabled && !assertion_has_attempts(assertion)) ||
        (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)) {
        assertion->edge_pending = 0;
        free_assertion_clock_events(assertion);
        return;
    }
    if (assertion->disable && sv4_to_bool(*assertion->disable)) {
        assertion->edge_pending = 0;
        free_assertion_attempts(assertion);
        return;
    }
    if (assertion->consequent_sequence) {
        while (assertion->clock_events) {
            llg_assertion_clock_event_t* event = assertion->clock_events;
            assertion->clock_events = event->next;
            if (!assertion->clock_events) assertion->clock_events_tail = NULL;
            int root_event = event->signal == assertion->clock &&
                             event->edge == assertion->edge;
            uint64_t cycle = 0;
            if (!sequence_cycle_next(assertion, &cycle)) {
                free(event);
                return;
            }
            if (root_event && assertion->abort_condition &&
                assertion->abort_condition(assertion->data)) {
                int had_pending = assertion->attempts != NULL ||
                                  assertion->sequence_antecedents != NULL ||
                                  assertion->sequence_consequents != NULL;
                assertion_abort_attempts(assertion);
                // A synchronous accept/reject control also controls the new
                // attempt begun at this sampled leading-clock edge.
                if (!had_pending && assertion->enabled && !g.finish)
                    assertion_result(assertion, assertion->abort_reject ? 0 : 1,
                                     assertion->abort_reject ? 0 : 1);
                free(event);
                if (g.finish) return;
                continue;
            }
            (void)run_sequence_concurrent_assertion(
                assertion, cycle, event->signal, event->edge, event->time,
                event->order, event->tick, root_event);
            free(event);
            if (g.finish) return;
        }
        return;
    }

    int edge = assertion->edge_pending;
    assertion->edge_pending = 0;
    if (!edge) return;

    if (assertion->abort_condition && assertion->abort_condition(assertion->data)) {
        int had_pending = assertion->attempts != NULL ||
                          assertion->sequence_antecedents != NULL ||
                          assertion->sequence_consequents != NULL;
        assertion_abort_attempts(assertion);
        // A synchronous accept/reject control also controls the new attempt
        // begun at this sampled edge. Emit one result even when no older
        // attempt was pending, matching the per-clock evaluation contract.
        if (!had_pending && assertion->enabled && !g.finish)
            assertion_result(assertion, assertion->abort_reject ? 0 : 1,
                             assertion->abort_reject ? 0 : 1);
        return;
    }

    while (assertion->attempts) {
        llg_assertion_attempt_t* attempt = assertion->attempts;
        assertion->attempts = attempt->next;
        if (!assertion->attempts) assertion->attempts_tail = NULL;
        int success = assertion->consequent(assertion->data) != 0;
        assertion_result(assertion, success, 0);
        free(attempt);
        if (g.finish) return;
    }

    if (!assertion->enabled ||
        (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active)) return;

    int antecedent = assertion->antecedent == NULL ||
                     assertion->antecedent(assertion->data) != 0;
    if (!antecedent) {
        assertion_result(assertion, 1, 1);
    } else if (assertion->overlapped) {
        assertion_result(assertion, assertion->consequent(assertion->data) != 0,
                         0);
    } else {
        assertion_attempt_enqueue(assertion);
    }
}

static int run_concurrent_assertions(void) {
    g.assertion_edges_pending = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        run_concurrent_assertion(assertion);
        if (g.finish) return 0;
    }
    return 1;
}

static void flush_assertion_attempts(void) {
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next)
        free_assertion_attempts(assertion);
}

int llg_assertion_register_control(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    if (!g.initialized || g.running || g.config_error || !clock || !consequent ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition) ||
        (pass_desc && !pass_desc->fn) || (fail_desc && !fail_desc->fn)) {
        fprintf(stderr, "llg: invalid concurrent assertion registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t* assertion =
        (llg_concurrent_assertion_t*)llg_checked_calloc(
            1, sizeof(*assertion), "concurrent assertion");
    assertion->clock = clock;
    assertion->edge = edge;
    assertion->disable = disable;
    assertion->antecedent = antecedent;
    assertion->consequent = consequent;
    assertion->abort_condition = abort_condition;
    assertion->pass_desc = pass_desc;
    assertion->fail_desc = fail_desc;
    assertion->data = data;
    assertion->kind = kind;
    assertion->overlapped = overlapped;
    assertion->abort_reject = abort_reject;
    assertion->abort_sync = abort_sync;
    assertion->identity = identity;
    assertion->label = label;
    assertion->location = location;
    assertion->scope = scope;
    assertion->enabled = 1;
    assertion->expect_active = 0;
    if (g.assertion_tail) {
        g.assertion_tail->next = assertion;
    } else {
        g.assertions = assertion;
    }
    g.assertion_tail = assertion;
    return 1;
}

int llg_assertion_register(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_desc,
        fail_desc, data, kind, overlapped, 0, 0,
        identity, label, location, scope);
}

void llg_deferred_assertion_scoped(int kind, int passed, uint64_t identity,
                            const char* label, const char* location, const char* scope,
                            llg_deferred_assertion_fn action,
                            llg_frame_t* frame) {
    if (kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_COVER ||
        (passed != 0 && passed != 1)) {
        fprintf(stderr, "llg runtime fatal: invalid deferred assertion result\n");
        llg_frame_release(frame);
        abort();
    }
    if (!llg_deferred_assertion_enabled(kind, label, scope)) {
        llg_frame_release(frame);
        return;
    }
    if (!action && frame) {
        // A frame is meaningful only for a selected action. This also keeps
        // malformed embedding calls from leaking an owned capture.
        llg_frame_release(frame);
        frame = NULL;
    }
    llg_proc_t* current = llg_current();
    uint64_t owner = g.in_deferred_action || !current
                         ? 0
                         : current->assertion_owner;
    for (llg_deferred_assertion_report_t* report = g.deferred_assertions;
         report; report = report->next) {
        if (report->owner == owner && report->time == g.now &&
            report->identity == identity) {
            llg_frame_release(report->frame);
            report->kind = kind;
            report->passed = passed;
            report->label = label;
            report->location = location;
            report->scope = scope;
            report->action = action;
            report->frame = frame;
            return;
        }
    }
    llg_deferred_assertion_report_t* report =
        (llg_deferred_assertion_report_t*)llg_checked_calloc(
            1, sizeof(*report), "deferred assertion report");
    report->owner = owner;
    report->time = g.now;
    report->kind = kind;
    report->passed = passed;
    report->identity = identity;
    report->label = label;
    report->location = location;
    report->scope = scope;
    report->action = action;
    report->frame = frame;
    if (g.deferred_assertion_tail) {
        g.deferred_assertion_tail->next = report;
    } else {
        g.deferred_assertions = report;
    }
    g.deferred_assertion_tail = report;
}

// Compatibility entrypoint for embedding callers without hierarchy metadata.
void llg_deferred_assertion(int kind, int passed, uint64_t identity,
                            const char* label, const char* location,
                            llg_deferred_assertion_fn action, llg_frame_t* frame) {
    llg_deferred_assertion_scoped(kind, passed, identity, label, location, "", action, frame);
}

static int valid_sequence_graph(const llg_sequence_graph_t* graph,
                                sv4_t* root_clock, int root_edge) {
    if (!graph || graph->states == 0 || graph->start >= graph->states ||
        graph->accept >= graph->states ||
        (graph->transition_count != 0 && !graph->transitions) ||
        graph->first_match_state_count != 0 ||
        (graph->local_count != 0 && !graph->locals))
        return 0;
    for (uint32_t index = 0; index < graph->local_count; index++) {
        const llg_sequence_local_t* local = &graph->locals[index];
        if (local->width == 0 || local->width >= LLG_SUPPORTED_WIDTH_LIMIT ||
            (local->two_state != 0 && local->two_state != 1))
            return 0;
    }
    for (uint32_t index = 0; index < graph->first_match_state_count; index++)
        if (graph->first_match_states[index] >= graph->states) return 0;
    for (uint32_t index = 0; index < graph->transition_count; index++) {
        const llg_sequence_transition_t* transition = &graph->transitions[index];
        if (transition->from >= graph->states || transition->to >= graph->states ||
            transition->max_delay < transition->min_delay ||
            (transition->atom != LLG_SEQUENCE_EPSILON && !graph->atom))
            return 0;
        if ((transition->clock && transition->edge != LLG_EV_POSEDGE &&
             transition->edge != LLG_EV_NEGEDGE) ||
            (!transition->clock && transition->edge != 0))
            return 0;
        sv4_t* destination = transition->clock ? transition->clock : root_clock;
        int destination_edge = transition->clock ? transition->edge : root_edge;
        int exact_boundary = (transition->min_delay == 0 && transition->max_delay == 0) ||
                             (transition->min_delay == 1 && transition->max_delay == 1);
        if (!exact_boundary) {
            sv4_t* initial = graph->leading_clock ? graph->leading_clock : root_clock;
            int initial_edge = graph->leading_clock ? graph->leading_edge : root_edge;
            if (transition->from == graph->start &&
                (destination != initial || destination_edge != initial_edge)) return 0;
            for (uint32_t j = 0; j < graph->transition_count; j++) {
                const llg_sequence_transition_t* previous = &graph->transitions[j];
                if (previous->to != transition->from) continue;
                sv4_t* source = previous->clock ? previous->clock : root_clock;
                int source_edge = previous->clock ? previous->edge : root_edge;
                if (source != destination || source_edge != destination_edge) return 0;
            }
        }
        if (transition->match_count > graph->match_item_count ||
            transition->match_start > graph->match_item_count -
                transition->match_count ||
            (transition->match_count != 0 && !graph->match))
            return 0;
    }
    return 1;
}

int llg_assertion_register_sequence_control(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope) {
    if (!g.initialized || g.running || g.config_error || !clock ||
        !valid_sequence_graph(consequent, clock, edge) ||
        (antecedent && !valid_sequence_graph(antecedent, clock, edge)) ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        (overlapped != 0 && overlapped != 1) ||
        (abort_reject != 0 && abort_reject != 1) ||
        (abort_sync != 0 && abort_sync != 1) ||
        ((abort_reject || abort_sync) && !abort_condition) ||
        (pass_desc && !pass_desc->fn) || (fail_desc && !fail_desc->fn)) {
        fprintf(stderr, "llg: invalid concurrent sequence assertion registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t* assertion =
        (llg_concurrent_assertion_t*)llg_checked_calloc(
            1, sizeof(*assertion), "concurrent sequence assertion");
    assertion->clock = clock;
    assertion->edge = edge;
    assertion->disable = disable;
    assertion->pass_desc = pass_desc;
    assertion->fail_desc = fail_desc;
    assertion->abort_condition = abort_condition;
    assertion->data = data;
    assertion->kind = kind;
    assertion->overlapped = overlapped;
    assertion->abort_reject = abort_reject;
    assertion->abort_sync = abort_sync;
    assertion->identity = identity;
    assertion->label = label;
    assertion->location = location;
    assertion->scope = scope;
    assertion->enabled = 1;
    assertion->expect_active = 0;
    assertion->antecedent_sequence = antecedent;
    assertion->consequent_sequence = consequent;
    /* Registration precedes execution. Only sequence consumers need ticks
     * across slots, including explicit clocks in either transition graph. */
    clocking_edge_get(clock)->keep_ticks = 1;
    const llg_sequence_graph_t* graphs[] = {antecedent, consequent};
    for (size_t gi = 0; gi < sizeof(graphs) / sizeof(graphs[0]); ++gi) {
        if (!graphs[gi]) continue;
        for (uint32_t ti = 0; ti < graphs[gi]->transition_count; ++ti) {
            sv4_t* source = graphs[gi]->transitions[ti].clock;
            if (source) clocking_edge_get(source)->keep_ticks = 1;
        }
    }
    if (g.assertion_tail)
        g.assertion_tail->next = assertion;
    else
        g.assertions = assertion;
    g.assertion_tail = assertion;
    return 1;
}

int llg_assertion_register_sequence(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    return llg_assertion_register_sequence_control(
        clock, edge, disable, antecedent, consequent, NULL, pass_desc,
        fail_desc, data, kind, overlapped, 0, 0,
        identity, label, location, scope);
}

static int llg_fmt_arg_same(const llg_fmt_arg_t* a, const llg_fmt_arg_t* b) {
    if (a->kind != b->kind) return 0;
    if (a->kind == LLG_FMT_PACKED || a->kind == LLG_FMT_STRENGTH)
        return sv4_same(a->value.packed, b->value.packed);
    if (a->kind == LLG_FMT_REAL) return real_same(a->value.real, b->value.real);
    return a->value.string.len == b->value.string.len &&
           (!a->value.string.len ||
            memcmp(a->value.string.data, b->value.string.data,
                   a->value.string.len) == 0);
}

static void reset_monitor_state(void) {
    if (g.mon.active) {
        free(g.mon.fmt);
        if (g.mon.last) sv4_destroy_array(g.mon.last, (size_t)g.mon.n);
        if (g.mon.work) sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
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
    llg_file_monitor_with_typed_reads(1u, fmt, n, eval, scope, reads, n_reads);
}

void llg_file_monitor_with_typed_reads(
    uint32_t descriptor, const char* fmt, int n, llg_display_eval_fn eval,
    const char* scope, const llg_display_read_t* reads, int n_reads) {
    if (!region_can_mutate("monitor scheduling")) return;
    reset_monitor_state();
    g.mon.active = 1;
    g.mon.enabled = 1;
    g.mon.dirty = 1;
    g.mon.force_report = 1;
    g.mon.n = n;
    g.mon.typed = 1;
    g.mon.descriptor = descriptor;
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
    llg_file_strobe_typed(1u, fmt, n, eval, scope);
}

void llg_file_strobe_typed(uint32_t descriptor, const char* fmt, int n,
                           llg_display_eval_fn eval, const char* scope) {
    if (!region_can_mutate("strobe scheduling")) return;
    llg_strobe_t* e = (llg_strobe_t*)llg_checked_malloc(
        1, sizeof(llg_strobe_t), "typed strobe");
    e->fmt = (char*)llg_checked_malloc(strlen(fmt) + 1, 1, "strobe format");
    strcpy(e->fmt, fmt);
    e->n = n;
    e->eval = NULL;
    e->typed = 1;
    e->descriptor = descriptor;
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
        if (!g.mon.descriptor) return;
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
            llg_print_typed_to(g.mon.descriptor, g.mon.fmt, g.mon.typed_work,
                               g.mon.n, g.mon.scope, 1);
        }
        llg_fmt_args_destroy(g.mon.typed_work, g.mon.n);
        return;
    }
    sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
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
    if (!changed) {
        sv4_destroy_array(g.mon.work, (size_t)g.mon.n);
        return;
    }
    for (int i = 0; i < g.mon.n; i++) sv4_copy(&g.mon.last[i], &g.mon.work[i]);
    llg_print_array(g.mon.fmt, g.mon.work, g.mon.n);
    sv4_destroy_array(g.mon.work, (size_t)g.mon.n);

}

// Print queued $strobe lines after the current time step has settled.
static void flush_strobes(void) {
    while (g.strobes && !g.finish) {
        llg_strobe_t* e = g.strobes;
        g.strobes = e->next;
        if (!g.strobes) g.strobe_tail = NULL;
        if (e->typed) {
            if (e->descriptor) {
                e->typed_eval(e->typed_work, NULL);
                llg_print_typed_to(e->descriptor, e->fmt, e->typed_work, e->n,
                               e->scope, 1);
            }
            llg_fmt_args_destroy(e->typed_work, e->n);
            free(e->typed_work);
            free(e->scope);
        } else {
            e->eval(e->work, NULL);
            llg_print_array(e->fmt, e->work, e->n);
            sv4_destroy_array(e->work, (size_t)e->n);
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
            g.current = process;
            g.process_turn_active = 1;
            llg_co_status_t status = llg_co_run(&process->chain);
            g.process_turn_active = 0;
            g.current = NULL;
            if (status == LLG_CO_DONE) {
                proc_complete(process);
            } else if (status == LLG_CO_EXIT &&
                       process->chain.exiting == LLG_EXIT_COMPLETE) {
                proc_complete(process);
            } else if (status == LLG_CO_CALLED) {
                fprintf(stderr,
                        "llg runtime fatal: coroutine anchor escaped llg_co_run\n");
                abort();
            }
            if (g.deferred_stop) {
                g.deferred_stop = 0;
                g.stop_proc = NULL;
                g.stop_region = region;
                g.suspended = 1;
            }
            reap_retired_procs();
        }
        if (g.suspended) {
            if (g.stop_policy == LLG_STOP_POLICY_RESUME) {
                // The llg CLI is noninteractive. Its default policy resumes
                // the exact coroutine continuation in the same time slot,
                // while retaining all other queued work and state.
                if (!resume_stopped_process()) break;
            } else {
                break;
            }
        }
    }
    return !g.finish && !g.suspended;
}

static void wake_zero_waits(llg_region_t region) {
    llg_wait_queue_t* queue = &g.zero_waits[region];
    llg_wait_t* wait = queue->head;
    queue->head = NULL;
    queue->tail = NULL;
    while (wait) {
        llg_wait_t* next = wait->payload.timer.region_next;
        wait->payload.timer.region_next = NULL;
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

// Observed work enabled after the design set reached its fixed point: an
// assertion clock edge produced by the reactive set, or a queued Observed
// process/callback.
static int observed_pending(void) {
    if (g.assertion_edges_pending) return 1;
    for (llg_region_t region = LLG_REGION_PRE_OBSERVED_PLI;
         region <= LLG_REGION_POST_OBSERVED_PLI; region++) {
        if (region_pending(region)) return 1;
    }
    return 0;
}

static int run_observed_set(void) {
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED_PLI)) return 0;
    if (!run_region_queue(LLG_REGION_PRE_OBSERVED)) return 0;
    if (!run_region_queue(LLG_REGION_OBSERVED)) return 0;
    if (!run_concurrent_assertions()) return 0;
    flush_deferred_assertions();
    if (g.finish) return 0;
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
        // A deferred assertion can itself be evaluated by a Reactive
        // callback. Keep that newly coalesced report in the same Reactive
        // fixed-point pass, after the current queue has drained.
        if (g.deferred_assertions) {
            flush_deferred_assertions();
            if (g.finish) return 0;
            continue;
        }
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
    g.private_evaluation = 1;
    flush_strobes();
    if (!g.finish) check_monitor();
    g.private_evaluation = 0;
    if (g.finish) return 0;
    return run_region_queue(LLG_REGION_POSTPONED_PLI);
}

// ── final blocks (see llg_rt.h) ─────────────────────────────────────────────

void llg_spawn_final(void (*fn)(void), const char* name) {
    if (llg_n_finals == INT_MAX) {
        fprintf(stderr, "llg runtime fatal: final block registry size overflow\n");
        abort();
    }
    finals_reserve(llg_n_finals + 1);
    llg_finals[llg_n_finals].fn = fn;
    llg_finals[llg_n_finals].name = name;
    llg_n_finals++;
}

void llg_rt_run_finals(void) {
    if (llg_n_finals == 0) {
        llg_clear_final_timeformat();
        return;
    }
    // `$stop` is a resumable scheduler suspension, not a simulation exit.
    // Do not run final procedures while an embedding has intentionally
    // returned control to its caller under the EXIT policy.
    if (g.suspended) return;
    if (llg_last_config_error) {
        llg_clear_final_timeformat();
        finals_release();
        llg_n_finals = 0;
        return;
    }
    // Explicit reset, decoupled from the cleanup-memset invariant: a stale
    // $finish flag left by the scheduler exit must never read as
    // "$finish inside a final" after the first final completes.
    g.finish = 0;
    g.running = 0;
    g.current_region = LLG_REGION_POSTPONED;
    llg_restore_final_timeformat();
    g.now = llg_final_time;
    g.zero_loop_limit = llg_configured_zero_loop_limit;
    g.process_step_limit = llg_configured_process_step_limit;
    g.stop_policy = llg_configured_stop_policy;
    g.initialized = 1;
    llg_in_finals = 1;
    for (int i = 0; i < llg_n_finals; i++) {
        llg_proc_t process;
        memset(&process, 0, sizeof(process));
        process.name = llg_finals[i].name;
        process.handle = process_handle_new(&process);
        process.status = LLG_PROCESS_RUNNING;
        process.budget_time = g.now;
        process.region = LLG_REGION_POSTPONED;
        process.chain.owner = &process;
        register_proc(&process);
        g.current = &process;
        llg_finals[i].fn();
        g.current = NULL;
        if (process.wait.kind != W_NONE) {
            fprintf(stderr,
                    "llg: fatal: final block `%s` suspended on a wait "
                    "(timing controls are rejected by codegen)\n",
                    process.name ? process.name : "final");
            abort();
        }
        proc_complete(&process);
        unregister_proc(&process);
        llg_co_arena_release(&process.chain.arena);
        // Finals permit function statements only. Codegen rejects NBAs,
        // deferred output tasks, waits, and forks, so no scheduler region is
        // run between these sequential zero-time calls.
        if (g.finish) break;
    }
    llg_in_finals = 0;
    llg_rt_cleanup();
    finals_release();
    llg_n_finals = 0;
}

void llg_rt_run(void) {
    if (g.config_error) {
        llg_rt_cleanup();
        return;
    }
    // A caller must explicitly acknowledge an EXIT-policy stop before the
    // scheduler can advance. This keeps future queues and the suspended
    // coroutine untouched when an embedding probes the runtime again.
    if (g.suspended) return;
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
            // Iterate the outer loop of SV 4.5 until Active through
            // Post-Re-NBA, including Observed, are all empty.
            if (design_pending() || observed_pending() || reactive_pending())
                continue;
            if (!run_pre_postponed_set()) break;
            if (design_pending() || observed_pending() || reactive_pending())
                continue;
            break;
        }
        if (g.finish) break;
        if (!run_postponed_set()) break;
        if (g.finish) break;
        int have_future_event = g.timed_head || g.delayed_nba_buckets ||
                                g.inertial_pending || g.callbacks;
        uint64_t t = g.timed_head ? g.timed_head->payload.timer.time : UINT64_MAX;
        if (g.delayed_nba_buckets && g.delayed_nba_buckets->time < t)
            t = g.delayed_nba_buckets->time;
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
            clocking_advance_time();
            g.now = t;
            g.region_passes = 0;
        }
        llg_wait_t* wait = g.timed_head;
        while (wait && wait->payload.timer.time == g.now) {
            llg_wait_t* next = wait->payload.timer.next;
            wake_proc(wait->proc);
            wait = next;
        }
        g.current_region = LLG_REGION_PREPONED;
    }
    if (g.suspended) {
        // EXIT-policy suspension is deliberately resumable. Keep all
        // scheduler queues, coroutine frames, activations and output state in
        // place; an embedding can call llg_rt_resume() and llg_rt_run().
        g.running = 0;
        return;
    }
    run_deferred_assertions_now();
    flush_assertion_attempts();
    // Finals ($time inside them) report when the scheduler loop ended.
    llg_final_time = g.now;
    // No pending update or deferred evaluator executes between $finish and
    // final procedures, regardless of whether its issuing process completed.
    g.running = 0;
    llg_file_defer_cleanup = llg_n_finals != 0;
    if (llg_n_finals > 0) llg_save_final_timeformat();
    llg_rt_cleanup();
    llg_file_defer_cleanup = 0;
}

// ── $display / $write ─────────────────────────────────────────────────────────

static void llg_vprint(const char* fmt, va_list ap, int newline) {
    const char* p = fmt;
    while (*p) {
        char c = *p++;
        if (c == '%') {
            const char* spec_start = p - 1;
            int has_width;
            int width;
            int zero;
            p = llg_parse_legacy_spec(p, &has_width, &width, &zero);
            c = *p;
            if (c) ++p;
            if (c == '%') {
                fputc('%', stdout);
            } else if (c == 't') {
                sv4_t v = va_arg(ap, sv4_t);
                size_t tmp_cap = llg_format_scratch_size(llg_sv4_width(v), 0);
                char* tmp = llg_checked_malloc(tmp_cap, 1, "time output");
                size_t len = llg_format_time_integer(v, g.design_precision_fs,
                                                      tmp, tmp_cap);
                if (!has_width && !zero) width = g.time_format.minimum_field_width;
                while (width > 0 && (size_t)width > len) {
                    fputc(' ', stdout);
                    width--;
                }
                fwrite(tmp, 1, len, stdout);
                free(tmp);
            } else if (c == 's') {
                const char* s = va_arg(ap, const char*);
                if (s) {
                    fputs(s, stdout);
                }
            } else if (c == 'd' || c == 'h' || c == 'b' || c == 'o') {
                sv4_t v = va_arg(ap, sv4_t);
                // One complete packed value, including a possible minus sign.
                size_t tmp_cap = (size_t)llg_sv4_width(v) + 3u;
                char* tmp = llg_checked_malloc(tmp_cap, 1, "packed output");
                sv4_format(c, v, tmp, tmp_cap);
                fputs(tmp, stdout);
                free(tmp);
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

static int llg_system_allowed(void) {
    const char* value = getenv("LLG_ALLOW_SYSTEM");
    return value && (!strcmp(value, "1") || !strcmp(value, "true") ||
                     !strcmp(value, "yes") || !strcmp(value, "on"));
}

sv4_t llg_system(llg_string_t command, int has_command) {
    if (has_command != 0 && has_command != 1) {
        fprintf(stderr, "llg: invalid internal `$system` argument marker\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }
    if (has_command && (!command.data && command.len != 0)) {
        fprintf(stderr,
                "llg: `$system` command has a nonzero length without storage\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }
    if (has_command) {
        for (size_t i = 0; i < command.len; ++i) {
            if (command.data[i] == '\0') {
                fprintf(stderr,
                        "llg: `$system` command contains an embedded NUL and "
                        "was not executed\n");
                llg_last_failure = 1;
                g.finish = 1;
                llg_string_destroy(&command);
                return sv4_from_u64(UINT32_MAX, 32, 1);
            }
        }
    }
    if (!llg_system_allowed()) {
        fprintf(stderr,
                "llg: $system is disabled; set LLG_ALLOW_SYSTEM=1 for the "
                "generated simulator process\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }

    // IEEE 1800-2009 §20.18 specifies the NULL argument for the omitted form.
    // Keep it distinct from the explicit empty C command string.
    int status;
    if (!has_command) {
        llg_string_destroy(&command);
        status = system(NULL);
    } else {
        status = system(command.data ? command.data : "");
        llg_string_destroy(&command);
    }
    return sv4_from_u64((uint32_t)status, 32, 1);
}
/* Destination-passing `_to` forms; private fragment, see value/destinations.h. */
void llg_fixed_array_stream_source_to(sv4_t* dst, const llg_fixed_array_t* array, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_fixed_array_stream_source(array, declaration_left, declaration_right, element_width, *fallback, selector_kind, *first, *second));
}
void llg_fixed_array_compare_to(sv4_t* dst, const llg_fixed_array_t* p0, const llg_fixed_array_t* p1, int p2, int p3) {
    sv4_replace(dst, llg_fixed_array_compare(p0, p1, p2, p3));
}
void llg_net_alias_read_to(sv4_t* dst, llg_net_alias_t* alias) {
    sv4_replace(dst, llg_net_alias_read(alias));
}
void llg_q_full_to(sv4_t* dst, const sv4_t* q_id, sv4_t* status) {
    sv4_replace(dst, llg_q_full(*q_id, status));
}
void llg_urandom_to(sv4_t* dst) {
    sv4_replace(dst, llg_urandom());
}
void llg_urandom_seed_to(sv4_t* dst, const sv4_t* seed) {
    sv4_replace(dst, llg_urandom_seed(*seed));
}
void llg_urandom_range_to(sv4_t* dst, const sv4_t* max, const sv4_t* min, int has_min) {
    sv4_replace(dst, llg_urandom_range(*max, *min, has_min));
}
void llg_sequence_local_read_to(sv4_t* dst, void* attempt, uint32_t slot) {
    sv4_replace(dst, llg_sequence_local_read(attempt, slot));
}
void llg_system_to(sv4_t* dst, llg_string_t* command, int has_command) {
    sv4_replace(dst, llg_system(llg_string_take(command), has_command));
}
void llg_frame_read_value_to(sv4_t* dst, const llg_frame_t* frame, size_t slot) {
    sv4_replace(dst, llg_frame_read_value(frame, slot));
}
void llg_sampled_domain_past_to(sv4_t* dst, uint64_t identity, uint64_t ticks) {
    sv4_replace(dst, llg_sampled_domain_past(identity, ticks));
}
void llg_rt_ref_read_to(sv4_t* dst, const llg_ref_t* ref) {
    sv4_replace(dst, llg_rt_ref_read(ref));
}
void llg_process_get_randstate_to(llg_string_t* dst) {
    llg_string_replace(dst, llg_process_get_randstate());
}
void llg_string_format_typed_to(llg_string_t* dst, llg_string_t* format, llg_fmt_arg_t* args, int n, const char* scope) {
    llg_string_replace(dst, llg_string_format_typed(llg_string_take(format), args, n, scope));
}
