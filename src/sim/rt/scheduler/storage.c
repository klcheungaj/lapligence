
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
    int is_chandle;
    int is_values;
    // Persistent native destination and its owned issue-time value;
    // is_string, is_chandle or is_values selects the member.
    union {
        struct {
            llg_string_t* target;
            llg_string_t value;
        } string;
        struct {
            void** target;
            void* value;
        } chandle;
        // Descriptor-backed array write: the whole array (`whole`) or the
        // payload's elements from the storage position held in `value`.
        struct {
            llg_dyn_value_array_t* target;
            llg_dyn_value_array_t* payload;
            int whole;
        } values;
    } native;
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
    if (nba->is_string) llg_string_destroy(&nba->native.string.value);
    if (nba->is_values && nba->native.values.payload) {
        llg_dyn_value_destroy(nba->native.values.payload);
        free(nba->native.values.payload);
    }
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
static void nba_owner_release_all(llg_proc_t* proc);
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
