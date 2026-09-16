
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
    llg_concurrent_assertion_action_fn pass_action;
    llg_concurrent_assertion_action_fn fail_action;
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

// The cycle-delay zero case distinguishes an event that already occurred in
// the current time slot from one that is still in the future. Keep only the
// latest transition timestamp per signal and edge kind; this registry is
// rebuilt with each runtime generation and never crosses the model boundary.
typedef struct llg_clocking_edge {
    struct llg_clocking_edge* next;
    sv4_t* signal;
    uint64_t any_time;
    uint64_t posedge_time;
    uint64_t negedge_time;
    uint64_t posedge_count;
    uint64_t negedge_count;
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
    llg_net_t* net_target;
    int net_slot;
    double* real_target;
    sv4_t value;
    sv4_t mask;
    int has_mask;
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
    uint64_t design_precision_fs;
    llg_timeformat_state_t time_format;
    llg_region_t current_region;
    uint64_t callback_sequence;
    llg_region_callback_t* callbacks; // sorted by time, region, issue order
    llg_sampled_value_t* sampled;
    llg_sampled_domain_t* sampled_domains;
    uint64_t sampled_domain_sequence;
    llg_clocking_edge_t* clocking_edges;
    llg_clocking_drive_t* clocking_drives;
    llg_clocking_drive_t* clocking_drives_tail;
    uint64_t sampled_time;
    int sampled_time_valid;
    llg_concurrent_assertion_t* assertions;
    llg_concurrent_assertion_t* assertion_tail;
    llg_deferred_trigger_t* deferred_triggers;
    llg_deferred_trigger_t* deferred_trigger_tail;
    llg_deferred_assertion_report_t* deferred_assertions;
    llg_deferred_assertion_report_t* deferred_assertion_tail;
    uint64_t next_process_identity;
    int in_deferred_action;
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
    llg_proc_t* all_procs[LLG_MAX_PROCS];
    int n_procs;
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
    llg_force_entry_t force_table[LLG_MAX_FORCE];
    int force_count;
    llg_pca_binding_t pca_table[LLG_MAX_PCA];
    int pca_count;
    llg_pca_real_binding_t pca_real_table[LLG_MAX_PCA];
    int pca_real_count;
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
    llg_process_handle_t** slot = &g.process_handles;
    while (*slot) {
        if (*slot == handle) {
            *slot = handle->next;
            handle->next = NULL;
            handle->linked = 0;
            return;
        }
        slot = &(*slot)->next;
    }
    handle->linked = 0;
    handle->next = NULL;
}

static llg_process_handle_t* process_handle_new(llg_proc_t* proc) {
    llg_process_handle_t* handle = (llg_process_handle_t*)llg_checked_calloc(
        1, sizeof(*handle), "process handle");
    handle->refs = 1; // process ownership
    handle->proc = proc;
    handle->status = LLG_PROCESS_RUNNING;
    handle->linked = 1;
    handle->next = g.process_handles;
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
static uint64_t llg_time_unit_from_exponent(int64_t exponent) {
    static const uint64_t units[] = {
        1ULL, 10ULL, 100ULL, 1000ULL, 10000ULL, 100000ULL,
        1000000ULL, 10000000ULL, 100000000ULL, 1000000000ULL,
        10000000000ULL, 100000000000ULL, 1000000000000ULL,
        10000000000000ULL, 100000000000000ULL, 1000000000000000ULL,
    };
    if (exponent < -15 || exponent > 0) return 0;
    return units[(size_t)(exponent + 15)];
}

static int llg_time_unit_exponent(uint64_t unit_fs) {
    for (int exponent = -15; exponent <= 0; ++exponent) {
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
    sv4_t* target;
    double* real_target;
    sv4_t* dependency;
} llg_dependency_binding_t;

static llg_dependency_binding_t* llg_dependency_bindings;

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
