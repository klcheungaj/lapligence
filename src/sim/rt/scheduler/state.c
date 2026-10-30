
// ── $monitor / $strobe state ──────────────────────────────────────────────────

// One registered display list that re-prints when a value it reads changes.
// `$monitor` owns the single `primary` entry (a new call replaces it and the
// `$monitoron/$monitoroff` flag governs it); every `$fmonitor` is independent
// and lives until `$fclose` removes its last channel (SV 21.3.1, 21.3.2).
typedef struct llg_monitor_state {
    struct llg_monitor_state* next;
    int primary;         // the `$monitor` display list
    int dead;            // every channel closed; swept at the next safe point
    int poll;            // re-evaluate every slot (handle-selected reads)
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

// Defined with the monitors; the write paths and run teardown call them.
static void llg_monitor_target_changed(const void* target, int kind);
static void llg_monitors_free_all(void);
static void llg_monitors_cancel_slot(unsigned slot);

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

// One pending or matured deferred immediate-assertion report (SV 16.4.1).
// Each execution of `assert #0` that has something to report appends one
// record to the process's deferred assertion report queue. All pending
// records of a time-step pass live in one issue-ordered array; `owner_prev`
// links the records of one process (index + 1, 0 = none) so a flush point
// (SV 16.4.2) clears that process's queue without scanning the others. The
// owner is a stable per-run process identity rather than a process pointer,
// because a completed process can be reclaimed before its reports execute.
typedef struct llg_deferred_assertion_report {
    uint64_t owner;
    uint64_t identity;
    const char* label;
    const char* location;
    const char* scope;
    llg_deferred_assertion_fn action;
    llg_frame_t* frame;
    // Outermost named activation of the issuing process at issue time, so a
    // `disable` of that scope can flush the report even after the process
    // has left the block (SV 16.4.4). Zero when it has none.
    uint32_t scope_declaration;
    uint32_t scope_instance;
    uint32_t owner_prev;
    uint8_t kind;
    uint8_t passed;
    uint8_t live; // cleared when flushed, cancelled or killed before maturing
    // Queued only to count assertion coverage: no report or action runs.
    uint8_t count_only;
    llg_acov_item_t* acov; // assertion coverage item, or NULL
    // A pending procedural concurrent assertion instance (SV 16.15.6, kind
    // LLG_DEFERRED_PROCEDURAL) shares this queue and its flush points; it
    // names the assertion it starts when it matures, NULL otherwise.
    struct llg_concurrent_assertion* procedural;
} llg_deferred_assertion_report_t;

// Growable report array; capacity is retained across time steps so a steady
// stream of reports allocates nothing.
typedef struct llg_deferred_report_queue {
    llg_deferred_assertion_report_t* items;
    uint32_t count;
    uint32_t capacity;
} llg_deferred_report_queue_t;

// Deferred assertions can be controlled before their first execution. Retain
// selector rules, not only entries for reports that happen to exist already.
typedef struct llg_assertion_rule {
    struct llg_assertion_rule* next;
    char* scope; // NULL means the whole design
    uint64_t assertion_type, directive_type;
    /* Which control the rule sets: checking (SV 20.11) or one action class
     * (SV 20.12); see LLG_ASSERTION_RULE_*. */
    int field;
    int enabled;
} llg_assertion_rule_t;
static llg_assertion_rule_t* llg_assertion_rules;
enum {
    LLG_ASSERTION_RULE_CHECK = 0,
    LLG_ASSERTION_RULE_PASS = 1,
    LLG_ASSERTION_RULE_VACUOUS = 2,
    LLG_ASSERTION_RULE_FAIL = 3,
};

typedef struct llg_sampled_value {
    struct llg_sampled_value* next;
    sv4_t* signal;
    sv4_t value;
    struct llg_sampled_history* history; // newest first, one entry per slot
    struct llg_sampled_history* history_tail;
    uint64_t history_ticks; // deepest clocking skew read from this history
} llg_sampled_value_t;

/* Preponed snapshot of one real variable; the value is numeric, never a
 * packed bit image. */
typedef struct llg_sampled_real {
    struct llg_sampled_real* next;
    double* signal;
    double value;
} llg_sampled_real_t;

typedef struct llg_sampled_history {
    struct llg_sampled_history* next; // older
    struct llg_sampled_history* prev; // newer
    uint64_t time;
    sv4_t value;
} llg_sampled_history_t;

/* One sampled history domain (IEEE 1800-2009 16.9.3): an expression sampled
 * on one clock. `samples`/`times` form a ring with one entry per time step in
 * which the clock ticked, newest at `newest`. It grows up to `limit` (the
 * deepest `$past` read plus the current step) and is then reused in place. */
typedef struct llg_sampled_domain {
    struct llg_sampled_clock* clock;
    llg_sampled_domain_eval_fn value;
    void* data;
    sv4_t initial;
    sv4_t* samples;
    uint64_t* times;
    size_t capacity;
    size_t limit;
    size_t count;
    size_t newest;
    // Preponed value of `current_time` for status calls in a step without a
    // tick; Preponed values cannot change within the step.
    sv4_t current;
    uint64_t current_time;
    int current_valid;
} llg_sampled_domain_t;

/* Ticks shared by every domain sampled on one clocking event and gate. Edge
 * clocks are found by the signal write hook; event clocks tick from their
 * generated waiting process. */
typedef struct llg_sampled_clock {
    struct llg_sampled_clock* next_edge;
    sv4_t* signal;
    int edge;
    llg_sampled_gate_fn gate;
    void* data;
    llg_sampled_domain_t** domains;
    size_t n_domains;
    size_t domains_capacity;
    /* Time + 1 of the step in which the clock last ticked (0: never), read
     * by assertions with global clocking future sampled value functions. */
    uint64_t tick_time1;
} llg_sampled_clock_t;

static llg_sampled_value_t* find_sampled_value(const sv4_t* signal);
static void sampled_record_write(sv4_t* signal);
static void sampled_domain_clock_signal_changed(sv4_t* signal, sv4_t old,
                                                sv4_t value);

typedef struct llg_assertion_attempt {
    struct llg_assertion_attempt* next;
    /* 1 means the consequent is due on the next matching clock edge. */
    uint64_t due;
    uint64_t start; /* time the attempt began (39.4.2 attemptStartTime) */
} llg_assertion_attempt_t;

/* One sequence clock event identity (time slot, cumulative tick, clock). */
typedef struct {
    uint64_t time, tick;
    sv4_t* clock;
    int edge;
} llg_sequence_mark_t;

/* Shared state of one forked join invocation. Each side is represented by
 * one frame on its threads' scope chains; `alive[side]` is cleared when the
 * last thread (or nested frame) of that side releases its frame. */
typedef struct llg_sequence_join_instance {
    struct llg_sequence_join_instance* next_free;
    const llg_sequence_join_t* plan;
    size_t refs;
    uint8_t alive[2];
    uint8_t matched[2];
    llg_sequence_mark_t last[2];
    /* Match multiplicity per side: every match so far (`and` pairs a new
     * match with all of them) and the matches at `last` (`intersect`). */
    uint64_t total[2];
    uint64_t at_last[2];
} llg_sequence_join_instance_t;

/* A dynamic frame on a thread's chain: a first_match invocation scope
 * (`join == NULL`) or one side of a join invocation. */
typedef struct llg_sequence_scope {
    struct llg_sequence_scope* parent;
    size_t refs;
    uint32_t identity;
    int matched;
    uint64_t time, tick;
    sv4_t* clock;
    int edge;
    llg_sequence_join_instance_t* join;
    uint8_t side;
} llg_sequence_scope_t;

typedef struct llg_sequence_endpoint {
    uint32_t local_count;
    struct llg_sequence_endpoint* next;
    sv4_t* locals;
    sv4_t* clock;
    int edge;
    uint64_t time, order, tick;
    int empty;
    /* Number of distinct matches this endpoint stands for (16.9.5-16.9.8). */
    uint64_t mult;
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
    /* Number of distinct paths (ways of matching) merged into this thread.
     * Paths that no match item or local variable distinguishes are counted,
     * not enumerated, so `(a or a)[*n]` stays one thread. */
    uint64_t mult;
    /* A sequence thread carries its own local assertion state.  Keeping this
     * on the token prevents `or`/repetition joins from merging distinct
     * match-item histories merely because their automaton state is equal. */
    sv4_t* locals;
} llg_sequence_token_t;

/* One evaluation attempt of an implication `R |-> P` / `R |=> P` (IEEE
 * 1800-2009 16.13.6). The antecedent attempt and every consequent started by
 * one of its matches share it, so the attempt reports exactly one result:
 * a failure as soon as one consequent fails, a success once the antecedent
 * can match no more and every started consequent has succeeded. */
typedef struct llg_assertion_eval {
    struct llg_assertion_eval* next_free;
    size_t pending; /* consequent attempts not yet resolved */
    uint8_t antecedent_live;
    uint8_t decided; /* the attempt's single result has been reported */
} llg_assertion_eval_t;

typedef struct llg_sequence_attempt {
    struct llg_sequence_attempt* next;
    /* Shared evaluation attempt of an implication, NULL otherwise. */
    llg_assertion_eval_t* eval;
    int eval_owner; /* this is the antecedent attempt of `eval` */
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
    /* Zero-delay processing rank of `graph` when it is owned by a property
     * program rather than the assertion's antecedent/consequent slots. */
    const uint32_t* rank;
    /* Time its assertion attempt began (39.4.2 attemptStartTime); a
     * consequent inherits its antecedent's. */
    uint64_t start;
} llg_sequence_attempt_t;

/* Local-variable values flowing from an antecedent match into a property
 * consequent (16.10, 16.13.6), shared by the instances it starts. */
typedef struct llg_property_locals {
    size_t refs;
    const llg_sequence_graph_t* graph;
    sv4_t* values;
} llg_property_locals_t;

/* The point at which a property node starts (16.14.2, F.5.1): the clock
 * event of its parent that started it. A node first evaluates at the
 * nearest tick of its own clock at or after that point, or strictly after
 * it when `strict` (|=>, #=#): the same clock compares tick numbers, other
 * clocks compare simulation time. */
typedef struct llg_property_launch {
    sv4_t* clock;
    int edge;
    uint64_t time;
    uint64_t tick;
    uint64_t order;
} llg_property_launch_t;

/* One live node of a property evaluation attempt. Children are kept in
 * creation order; a resolved child stays until its parent consumes it. */
typedef struct llg_property_inst {
    struct llg_property_inst* next; /* sibling, attempt list or free list */
    struct llg_property_inst* children;
    struct llg_property_inst* children_tail;
    const llg_property_node_t* node;
    /* Leaf sequence of SEQUENCE, antecedent of IMPLICATION/FOLLOWED_BY. */
    llg_sequence_attempt_t* sequence;
    llg_property_locals_t* locals;
    llg_property_launch_t launch;
    /* Own-clock ticks evaluated so far (the next is relative tick `ticks`);
     * for nodes without a clock of their own, 1 once started. */
    uint64_t ticks;
    /* Simulation time + 1 of the last asynchronous abort check. */
    uint64_t abort_checked;
    uint8_t status;     /* 0 pending, 1 true, 2 false */
    uint8_t nonvacuous; /* valid once resolved (16.15.8) */
    uint8_t strict;     /* starts strictly after `launch` */
    uint8_t done;       /* no further children or antecedent matches */
    uint8_t matched;    /* the antecedent matched at least once */
    uint8_t accumulated; /* nonvacuity of consumed children */
    uint8_t begun;      /* its sequence started (SEQUENCE, IMPLICATION) */
    uint8_t phase;      /* IF: 0 condition pending, 1 then, 2 else */
    /* Nonvacuity of operands an operator was collapsed past, added when the
     * node resolves true or false (see property_collapse). */
    uint8_t carry_true;
    uint8_t carry_false;
    /* Valid once resolved: the result is the vacuous outcome of an
     * implication without an antecedent match or of an else-less if whose
     * condition is false, passed on unchanged. A cover counts every other
     * success as a hit (decision S38-D10). */
    uint8_t cover_vacuous;
    /* IMPLICATION: a consumed consequent's result was not cover-vacuous. */
    uint8_t consumed_hit;
    /* Collapsed past an operator whose result is never cover-vacuous. */
    uint8_t carry_hit;
} llg_property_inst_t;

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

/* One entry of the deferred evaluation queue of an assertion with global
 * clocking future sampled value functions (IEEE 1800-2009 16.9.4,
 * future_assertions.c), in time order. A batch holds the clock events one
 * Observed pass took, the clock history of their time step, the operands
 * captured for them and the attempts their leading-clock event begins; a
 * marker discards the attempts running at its point of the queue (disable
 * iff or $assertkill); `disabled_starts` counts the attempts that began
 * while disable iff held, whose callbacks belong to the next global clock
 * tick like every other result of the assertion (A14-D1). */
typedef struct llg_assertion_lag {
    struct llg_assertion_lag* next;
    uint64_t time;
    llg_assertion_clock_event_t* events;
    llg_assertion_clock_event_t* history;
    sv4_t* slots;
    uint64_t starts;
    uint64_t disabled_starts;
    uint8_t marker;
    uint8_t killed;
    uint8_t has_root;
    /* A kill marker of vpiAssertionKill: only the attempts that began at
     * `kill_start` (39.5.2). */
    uint8_t kill_one;
    uint64_t kill_start;
    /* Action controls in force at the events (SV 20.12): their results
     * belong to this time step. */
    uint8_t pass_action_off;
    uint8_t vacuous_action_off;
    uint8_t fail_action_off;
} llg_assertion_lag_t;

/* One destination clock of a sequence endpoint's `.matched` status (SV
 * 16.14.6): a match is visible at destination tick `visible_tick`, the
 * first tick of `clock` after the match's time step (S39-D5); `also_next`
 * keeps a later match visible at the tick after it. */
typedef struct llg_matched_slot {
    sv4_t* clock;
    int edge;
    uint8_t has_match;
    uint8_t also_next;
    uint64_t visible_tick;
} llg_matched_slot_t;

/* Assertion coverage hooks (coverage/assertions.c, IEEE 1800-2009 40.5.3).
 * Every hook accepts a NULL item: a model without assertion coverage
 * tables, an `expect` or a sequence end point. `start` is the time the
 * attempt began, reported to VPI assertion callbacks (39.4.2). */
static llg_acov_item_t* llg_acov_find(uint64_t identity);
static void llg_acov_attempt(llg_acov_item_t* item, uint64_t start);
static void llg_acov_result(llg_acov_item_t* item, int success, int vacuous, uint64_t start);
static void llg_acov_end_attempts(llg_acov_item_t* item, uint64_t count, int killed);
static void llg_acov_disabled_attempt(llg_acov_item_t* item, uint64_t start);
// One attempt of a watched item ended (`event` LLG_ACOV_EVENT_DISABLED or
// _KILLED); the counters are updated separately, per batch.
static void llg_acov_notify(llg_acov_item_t* item, int event, uint64_t start);
// The assertion engines bracket their work so that attempt events raised
// inside reach VPI callbacks only when the engine returns.
static void assertion_events_enter(void);
static void assertion_events_leave(void);
// assertion_events_leave for a clock tick's Observed pass: the queued events
// are delivered in tick order (decision A17-D1).
static void assertion_events_leave_tick(void);
static uint64_t assertion_event_time(void);
static void llg_acov_events_release(void);

/* An attempt event queued for the VPI assertion callbacks. */
typedef struct {
    const llg_acov_item_t* item;
    uint64_t cb_time;
    uint64_t start;
    int event;
    uint32_t seq; /* queue position while a tick's events are ordered */
} llg_acov_event_t;

typedef struct {
    llg_acov_event_t* items;
    size_t head;
    size_t count;
    size_t capacity;
} llg_acov_event_queue_t;

typedef struct llg_concurrent_assertion {
    struct llg_concurrent_assertion* next;
    sv4_t* clock;
    int edge;
    /* `iff` condition of the leading clocking event, or NULL. A gated clock
     * counts its own ticks: an edge with the condition false is no tick, so
     * `##n` delays count only gated edges. */
    llg_sampled_gate_fn clock_gate;
    uint64_t gated_ticks;
    sv4_t* disable;
    llg_concurrent_assertion_predicate_fn antecedent;
    llg_concurrent_assertion_predicate_fn consequent;
    llg_concurrent_assertion_predicate_fn abort_condition;
    const llg_co_desc_t* pass_desc;
    const llg_co_desc_t* fail_desc;
    void* data;
    int kind;
    /* `cover sequence`: report every match, never a failure. */
    int cover_sequence;
    int overlapped;
    int abort_reject;
    int abort_sync;
    int enabled;
    int expect_active;
    /* An armed expect starts exactly one attempt (IEEE 1800-2009 16.18). */
    int expect_started;
    /* An initial-procedure assertion starts one attempt (16.15.6). */
    int single_attempt;
    int single_attempt_started;
    /* A procedural concurrent assertion (16.15.6): a leading clock event
     * begins one attempt per matured instance, never one on its own. */
    int procedural;
    /* Matured instances that begin at the next leading clock event. */
    uint64_t procedural_matured;
    /* g.now + 1 of the time step whose leading clock event the assertion
     * last processed (0: none), with that event and its sequence cycle, so
     * an instance maturing later in the same step begins at that event;
     * and of the step whose leading clock event arrived while disable iff
     * held, so an instance maturing then begins disabled. */
    uint64_t procedural_tick;
    llg_assertion_clock_event_t procedural_event;
    uint64_t procedural_cycle;
    uint64_t procedural_disabled_tick;
    /* Result of the last expect evaluation for its caller's inline action
     * block: 0 none, 1 success, 2 failure. */
    int expect_outcome;
    /* The caller's expect has an else arm, so no default failure report. */
    int expect_has_fail;
    /* Action controls (SV 20.12). Zero, the registration default, executes
     * the action; they gate actions only, never the result counters. */
    uint8_t pass_action_off;
    uint8_t vacuous_action_off;
    uint8_t fail_action_off;
    /* A sequence endpoint (SV 16.9.11, 16.14.5-16.14.6, 9.4.2.4) is a hidden
     * cover of every match of a sequence instance. It is no assertion: it
     * has no counters or actions and assertion controls skip it. Each match
     * triggers `endpoint_event` in Observed (`.triggered`, `@(seq)`) and
     * latches the `.matched` slots. */
    llg_event_t* endpoint_event;
    uint32_t endpoint_slot_count;
    llg_matched_slot_t* endpoint_slots;
    uint64_t identity;
    /* Assertion coverage item bound at registration, or NULL. */
    llg_acov_item_t* acov;
    const char* label;
    const char* location;
    const char* scope;
    int edge_pending;
    llg_assertion_attempt_t* attempts;
    llg_assertion_attempt_t* attempts_tail;
    const llg_sequence_graph_t* antecedent_sequence;
    const llg_sequence_graph_t* consequent_sequence;
    /* Per-state processing rank of each graph (zero-delay topological
     * order), so every path into a state is merged before it expands. */
    uint32_t* antecedent_rank;
    uint32_t* consequent_rank;
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
    /* Clock edges other than the leading one that advance the assertion's
     * sequences or property nodes, collected at registration. */
    uint32_t extra_clock_count;
    llg_property_clock_t* extra_clocks;
    /* Property-engine assertion: its program, the per-sequence ranks, and
     * the live attempts in start order. */
    const llg_property_program_t* property;
    uint32_t** property_ranks;
    llg_property_inst_t* property_attempts;
    llg_property_inst_t* property_attempts_tail;
    /* The program has an asynchronous accept_on/reject_on, and the time + 1
     * of the step whose sampled abort conditions were last checked. */
    int property_async;
    uint64_t property_async_checked;
    /* Global clocking future sampled value functions (16.9.4): the global
     * clock, the capture callback and slot count, the deferred evaluation
     * queue with its recycled entries, and the replay state (the entry's
     * attempt starts while its events are evaluated). NULL `future_clock`
     * for every other assertion. */
    struct llg_sampled_clock* future_clock;
    llg_assertion_capture_fn future_capture;
    uint32_t future_slots;
    uint8_t lag_replaying;
    uint64_t lag_starts;
    llg_assertion_lag_t* lag_head;
    llg_assertion_lag_t* lag_tail;
    llg_assertion_lag_t* lag_free;
} llg_concurrent_assertion_t;

/* Discard the running attempts of an assertion whose disable iff holds or
 * that a control task kills, counting them for assertion coverage
 * (concurrent_assertions.c). */
static void end_assertion_attempts(llg_concurrent_assertion_t* assertion, int killed);

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
    // Matching clocking events still to occur before the drive matures.
    uint64_t events;
} llg_clocking_drive_t;

typedef struct {
    sv4_t* sig;
    size_t refs;
} llg_clocking_drive_source_t;

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
    llg_inertial_t* inertial_pending;      // sorted by time, FIFO per time
    llg_inertial_t* inertial_pending_tail;
    llg_module_path_set_t* module_path_sets; // bound sets, owned path state
    llg_timing_check_set_t* timing_check_sets; // started sets, owned state
    uint64_t nba_sequence;
    llg_wait_t* timed_head;   // sorted ascending by time
    llg_wait_queue_t zero_waits[LLG_REGION_COUNT];
    llg_wait_t* waiters;      // all active waits
    int wait_count;           // armed waits of processes that can still finish
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
    llg_sampled_value_t* sampled_values; // Preponed value only, no history
    llg_sampled_real_t* sampled_reals;
    // Dense identity tables owned by the runtime; entries are owners.
    llg_sampled_clock_t** sampled_clocks;
    size_t sampled_clocks_capacity;
    llg_sampled_clock_t* sampled_edge_clocks;
    llg_sampled_domain_t** sampled_domains;
    size_t sampled_domains_capacity;
    llg_clocking_edge_t* clocking_edges;
    llg_clocking_edge_t** clocking_index;
    size_t clocking_capacity;
    size_t clocking_count;
    size_t clocking_used;
    // Set by llg_rt_skip_clocking_history: only registered sequence clocks
    // keep edge history.
    int clocking_history_skipped;
    llg_clocking_drive_t* clocking_drives;
    llg_clocking_drive_t* clocking_drives_tail;
    // Distinct clock signals named by pending drives, with reference counts,
    // and the number of named-event sources: writes and triggers that cannot
    // mature a pending drive skip the scan of `clocking_drives`.
    llg_clocking_drive_source_t* clocking_drive_sources;
    size_t clocking_drive_source_count;
    size_t clocking_drive_source_capacity;
    size_t clocking_drive_event_refs;
    uint64_t sampled_time;
    int sampled_time_valid;
    llg_concurrent_assertion_t* assertions;
    // Set when a clock edge reaches a concurrent assertion and cleared when
    // the Observed region evaluates them. An edge enabled by the reactive
    // set sends the time slot back around through Observed (SV 24.3.1).
    int assertion_edges_pending;
    llg_concurrent_assertion_t* assertion_tail;
    // Sequence endpoints by their model index, for `.matched` queries.
    llg_concurrent_assertion_t** sequence_endpoints;
    uint32_t sequence_endpoint_count;
    // Sequence thread budget (LLG_SEQUENCE_THREAD_LIMIT) and the live count
    // of pending tokens plus attempts it bounds.
    uint64_t sequence_thread_limit;
    uint64_t sequence_threads;
    // Assertion whose sequences are being stepped, for budget diagnostics.
    const llg_concurrent_assertion_t* sequence_current;
    // Recycled sequence runtime nodes: steady-state stepping reuses them
    // instead of allocating. Released by llg_rt_cleanup.
    llg_sequence_token_t* sequence_token_pool;
    llg_sequence_scope_t* sequence_scope_pool;
    llg_sequence_join_instance_t* sequence_join_pool;
    llg_sequence_endpoint_t* sequence_endpoint_pool;
    llg_sequence_attempt_t* sequence_attempt_pool;
    llg_assertion_eval_t* assertion_eval_pool;
    llg_assertion_clock_event_t* assertion_clock_event_pool;
    llg_property_inst_t* property_inst_pool;
    // Operands captured for the deferred-queue entry being replayed by an
    // assertion with global clocking future sampled value functions
    // (future_assertions.c); NULL outside a replay.
    const sv4_t* assertion_lag_slots;
    uint32_t assertion_lag_slot_count;
    // Time + 1 of the queue entry being replayed by such an assertion, the
    // time its attempt events belong to (39.4.2.1); 0 outside a replay.
    uint64_t assertion_replay_time1;
    // VPI assertion callbacks (coverage/assertions.c): engine nesting depth
    // and the attempt events raised inside it, delivered when it returns.
    uint32_t assertion_event_depth;
    llg_acov_event_queue_t assertion_events;
    // vpiAssertionSysEnd ran: no control enables an assertion again.
    int assertions_ended;
    // Registered property assertions with an asynchronous abort condition.
    uint64_t property_async_count;
    llg_deferred_trigger_t* deferred_triggers;
    llg_deferred_trigger_t* deferred_trigger_tail;
    // Pending deferred assertion reports of the current pass and the
    // matured reports awaiting their Reactive execution (SV 16.4.1).
    llg_deferred_report_queue_t deferred_pending;
    llg_deferred_report_queue_t deferred_matured;
    uint32_t deferred_pending_live;
    // Live pending procedural concurrent assertion instances among
    // `deferred_pending_live`, and the procedural assertions by their
    // model index (llg_assertion_procedural, llg_assertion_queue).
    uint32_t procedural_pending_live;
    uint32_t procedural_assertion_count;
    llg_concurrent_assertion_t** procedural_assertions;
    uint32_t deferred_matured_next;
    int deferred_matured_scheduled;
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
    llg_proc_t* delayed_calls; // pending carriers of llg_schedule_delayed_call
    llg_process_handle_t* process_handles; // stable identities for live/exited procs
    llg_semaphore_t* semaphores; // all semaphore objects owned by this run
    llg_mailbox_t* mailboxes;       // runtime-owned mailbox objects
    struct llg_dynamic_event_t* dynamic_events; // container event objects, owned until cleanup
    llg_fork_group_t* zombie_groups; // completed/killed groups awaiting teardown
    llg_activation_t* activations; // active named block/task invocations
    llg_monitor_state_t* monitors;      // registration order; `$monitor` is one entry
    llg_monitor_state_t* monitors_tail;
    int monitor_off;                    // `$monitoroff` flag (default on, SV 21.2.3)
    int monitors_sweeping;              // a check is walking the list
    llg_strobe_t* strobes;     // pending $strobe lines, last issued first
    // Active procedural forces. Entries own copied target/source descriptors;
    // no pre-force value is retained because release is object-specific.
    llg_force_entry_t* force_table;   // checked-growable live-entry table
    int force_count;
    int force_capacity;
    llg_rng_state_t rng_root;
    // Runtime-owned processes spawned so far: the ordinal that derives each
    // one's random stream from the root without drawing from it.
    uint64_t owned_spawns;
    // Per-instance initialization streams of static processes (18.14.1),
    // an open-addressing table keyed by instance name.
    int rng_instance_streams;  // seed static processes per instance
    struct llg_rng_scope_entry* rng_scopes;
    // Instance stream of the static initializer being run outside any
    // process, or NULL (the root stream).
    llg_rng_state_t* rng_initializer;
    size_t rng_scope_capacity;
    size_t rng_scope_count;
    // Free checker variable sets (free_variables.c), in registration order;
    // the set being solved and its stream, which randomize draws from.
    struct llg_free_set* free_sets;
    struct llg_free_set* free_solving;
    llg_rng_state_t* free_rng;
    int free_sets_resolved;
    int argc;
    char** argv;
    llg_q_queue_t* q_queues;
} llg_rt_ctx_t;

static llg_rt_ctx_t g;

// Defined in free_variables.c, after the assertion engine it reads.
static void free_sets_release(void);

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
    // The path the file was opened with (owned), served by vpi_mcd_name();
    // NULL for the preopened host streams.
    char* name;
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
    /* The bound storage: a packed value or a real cell (`kind`). */
    const void* key;
    sv4_t* dependency;
    uint8_t kind;
} llg_dependency_binding_t;

enum {
    LLG_DEPENDENCY_BINDING_PACKED = 0,
    LLG_DEPENDENCY_BINDING_REAL = 1,
};

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
// A run-ending simulator error that skips the final procedures
// (llg_rt_skip_finals); read by llg_rt_run_finals after the scheduler exit.
static int llg_skip_final_procedures;
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
