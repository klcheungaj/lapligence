// ── Assertion coverage and coverage access (IEEE 1800-2009 40.2-40.3) ───────
//
// The generated model owns one llg_acov_item_t per assertion statement
// instance (concurrent assert/assume/cover property and cover sequence,
// immediate and deferred immediate assertions) and the instance hierarchy
// that $coverage_* scope arguments select. Models without a coverage access
// call or VPI catalog have no table: every assertion then runs with a NULL
// item and pays one pointer test per attempt and result.
//
// Counters follow 40.5.3. An attempt is a started evaluation; it concludes
// as a nonvacuous success, a vacuous success, a failure (assert and assume
// only; a cover that does not match concludes without a failure), or it is
// disabled (disable iff) or killed ($assertkill). A stopped item counts
// nothing; RESET zeroes the counters but leaves running attempts running.
//
// $coverage_* status values (40.3.1). Statement, toggle and FSM coverage
// are not implemented: CHECK and START report SV_COV_NOCOV, STOP and RESET
// succeed without effect (S46-D5), get and get_max return 0 (NOCOV) and
// save reports SV_COV_NOCOV.

#define LLG_SV_COV_OVERFLOW (-2)
#define LLG_SV_COV_ERROR (-1)
#define LLG_SV_COV_NOCOV 0
#define LLG_SV_COV_OK 1

#define LLG_SV_COV_START 0u
#define LLG_SV_COV_STOP 1u
#define LLG_SV_COV_RESET 2u
#define LLG_SV_COV_CHECK 3u
#define LLG_SV_COV_MODULE 10u
#define LLG_SV_COV_HIER 11u
#define LLG_SV_COV_ASSERTION 20u
#define LLG_SV_COV_TOGGLE 23u

#define LLG_ACOV_COUNTERS 6u

static llg_acov_item_t* llg_acov_items;
static size_t llg_acov_n_items;
static const llg_acov_instance_t* llg_acov_instances;
static size_t llg_acov_n_instances;
// The model has assertion coverage tables (possibly with no item).
static int llg_acov_present;

static void llg_acov_zero(llg_acov_item_t* item) {
    item->attempts = item->successes = item->vacuous = 0;
    item->failures = item->disabled = item->killed = 0;
}

void llg_acov_model(llg_acov_item_t* items, size_t n_items, const llg_acov_instance_t* instances,
                    size_t n_instances) {
    llg_acov_items = items;
    llg_acov_n_items = items ? n_items : 0u;
    llg_acov_instances = instances;
    llg_acov_n_instances = instances ? n_instances : 0u;
    llg_acov_present = 1;
    for (size_t i = 0; i < llg_acov_n_items; ++i) {
        llg_acov_zero(&llg_acov_items[i]);
        llg_acov_items[i].stopped = 0;
        llg_acov_items[i].control = 0;
        llg_acov_items[i].watches = 0;
    }
}

// The item of the assertion statement `identity`, or NULL (no table, or
// not an item: an `expect` or a sequence end point). Items are sorted by
// identity.
static llg_acov_item_t* llg_acov_find(uint64_t identity) {
    size_t lo = 0, hi = llg_acov_n_items;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2u;
        uint64_t key = llg_acov_items[mid].identity;
        if (key == identity) return &llg_acov_items[mid];
        if (key < identity) lo = mid + 1u;
        else hi = mid;
    }
    return NULL;
}

static void llg_acov_add(uint64_t* counter, uint64_t count) {
    *counter = count > UINT64_MAX - *counter ? UINT64_MAX : *counter + count;
}

// ── VPI assertion callbacks (39.4.2) ────────────────────────────────────────
//
// An item's attempt events go to the VPI hook only while it is watched.
// Inside the assertion engines (assertion_events_enter/leave) they are
// queued and delivered in order when the outermost engine call returns.

static llg_assertion_watch_fn llg_assertion_watch;

void llg_assertion_watch_hook(llg_assertion_watch_fn hook) { llg_assertion_watch = hook; }

void llg_acov_watch(size_t index, int delta) {
    if (index >= llg_acov_n_items) return;
    llg_acov_item_t* item = &llg_acov_items[index];
    if (delta > 0) item->watches++;
    else if (item->watches) item->watches--;
}

// The time an attempt event belongs to: the replayed time step of an
// assertion with global clocking future sampled value functions, else now.
static uint64_t assertion_event_time(void) {
    return g.assertion_replay_time1 ? g.assertion_replay_time1 - 1u : g.now;
}

static void assertion_events_flush(void) {
    llg_acov_event_queue_t* queue = &g.assertion_events;
    // A callback may raise further events; FIFO order holds across nesting.
    while (queue->count) {
        llg_acov_event_t event = queue->items[queue->head];
        queue->head = (queue->head + 1u) % queue->capacity;
        queue->count--;
        if (llg_assertion_watch && event.item->watches)
            llg_assertion_watch(event.item, event.event, event.cb_time, event.start);
    }
    queue->head = 0;
}

static void assertion_events_enter(void) { g.assertion_event_depth++; }

static void assertion_events_leave(void) {
    if (--g.assertion_event_depth == 0 && LLG_CO_UNLIKELY(g.assertion_events.count))
        assertion_events_flush();
}

static void assertion_event_push(const llg_acov_item_t* item, int event, uint64_t cb_time,
                                 uint64_t start) {
    llg_acov_event_queue_t* queue = &g.assertion_events;
    if (queue->count == queue->capacity) {
        size_t capacity = queue->capacity ? queue->capacity * 2u : 16u;
        llg_acov_event_t* items =
            (llg_acov_event_t*)llg_checked_malloc(capacity, sizeof(*items), "assertion events");
        for (size_t i = 0; i < queue->count; ++i)
            items[i] = queue->items[(queue->head + i) % queue->capacity];
        free(queue->items);
        queue->items = items;
        queue->capacity = capacity;
        queue->head = 0;
    }
    queue->items[(queue->head + queue->count) % queue->capacity] =
        (llg_acov_event_t){item, cb_time, start, event};
    queue->count++;
}

static void llg_acov_event(llg_acov_item_t* item, int event, uint64_t start) {
    if (!llg_assertion_watch) return;
    uint64_t cb_time = event == LLG_ACOV_EVENT_START ? start : assertion_event_time();
    if (g.assertion_event_depth || g.assertion_events.count)
        assertion_event_push(item, event, cb_time, start);
    else
        llg_assertion_watch(item, event, cb_time, start);
}

static void llg_acov_notify(llg_acov_item_t* item, int event, uint64_t start) {
    if (LLG_CO_UNLIKELY(item != NULL && item->watches != 0)) llg_acov_event(item, event, start);
}

// The hook stays: final procedures run after the runtime cleanup and their
// immediate assertions still report (VPI removes the hook at shutdown).
static void llg_acov_events_release(void) {
    free(g.assertion_events.items);
    g.assertion_events = (llg_acov_event_queue_t){0};
    g.assertion_event_depth = 0;
}

static void llg_acov_attempt(llg_acov_item_t* item, uint64_t start) {
    if (!item) return;
    if (!item->stopped) llg_acov_add(&item->attempts, 1u);
    llg_acov_notify(item, LLG_ACOV_EVENT_START, start);
}

static void llg_acov_result(llg_acov_item_t* item, int success, int vacuous, uint64_t start) {
    if (!item) return;
    if (!item->stopped) {
        if (success) llg_acov_add(vacuous ? &item->vacuous : &item->successes, 1u);
        else if (item->kind != LLG_ASSERTION_COVER) llg_acov_add(&item->failures, 1u);
    }
    llg_acov_notify(item,
                    !success  ? LLG_ACOV_EVENT_FAILURE
                    : vacuous ? LLG_ACOV_EVENT_VACUOUS
                              : LLG_ACOV_EVENT_SUCCESS,
                    start);
}

// `count` running attempts ended disabled (disable iff) or killed. The
// callers raise one event per attempt themselves (llg_acov_notify).
static void llg_acov_end_attempts(llg_acov_item_t* item, uint64_t count, int killed) {
    if (!item || item->stopped || !count) return;
    llg_acov_add(killed ? &item->killed : &item->disabled, count);
}

// An attempt that starts while its disable iff condition holds (40.5.3).
static void llg_acov_disabled_attempt(llg_acov_item_t* item, uint64_t start) {
    llg_acov_attempt(item, start);
    llg_acov_end_attempts(item, 1u, 0);
    llg_acov_notify(item, LLG_ACOV_EVENT_DISABLED, start);
}

void llg_acov_check(llg_acov_item_t* item, int passed) {
    llg_acov_attempt(item, g.now);
    llg_acov_result(item, passed != 0, 0, g.now);
}

// ── VPI assertion control (39.5) ────────────────────────────────────────────

void llg_assertions_end(void) { g.assertions_ended = 1; }

// Discard the pending deferred reports of `item` (39.5.3: a control that
// discards attempts also flushes their unmatured reports), each an attempt
// killed (S46-D8). Every pending report belongs to the current time step.
static void llg_acov_flush_deferred(llg_acov_item_t* item) {
    for (uint32_t i = 0; g.deferred_pending_live && i < g.deferred_pending.count; i++) {
        llg_deferred_assertion_report_t* report = &g.deferred_pending.items[i];
        if (!report->live || report->kind == LLG_DEFERRED_PROCEDURAL || report->acov != item)
            continue;
        llg_acov_attempt(item, g.now);
        llg_acov_end_attempts(item, 1u, 1);
        llg_acov_notify(item, LLG_ACOV_EVENT_KILLED, g.now);
        deferred_report_drop(report);
    }
}

// Discard every attempt of a concurrent assertion record, counted as
// killed, and its pending procedural instances (39.5.3).
static void llg_acov_discard_record(llg_concurrent_assertion_t* assertion) {
    end_assertion_attempts(assertion, 1);
    assertion->edge_pending = 0;
    if (assertion->procedural) {
        procedural_cancel_pending(assertion);
        assertion->procedural_matured = 0;
    }
}

static void llg_acov_record_control(llg_concurrent_assertion_t* assertion, int operation,
                                    uint64_t start) {
    switch (operation) {
        case LLG_ACOV_OP_DISABLE: assertion->enabled = 0; break;
        case LLG_ACOV_OP_ENABLE: assertion->enabled = !g.assertions_ended; break;
        case LLG_ACOV_OP_KILL:
            if (assertion->future_clock) future_assertion_kill_at(assertion, start);
            else assertion_kill_attempts_at(assertion, start);
            break;
        case LLG_ACOV_OP_KILL_ALL:
            assertion->enabled = 0;
            llg_acov_discard_record(assertion);
            break;
        case LLG_ACOV_OP_RESET:
            llg_acov_discard_record(assertion);
            assertion->enabled = !g.assertions_ended;
            assertion->pass_action_off = 0;
            assertion->vacuous_action_off = 0;
            assertion->fail_action_off = 0;
            break;
        case LLG_ACOV_OP_PASS_OFF:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_PASS_OFF);
            break;
        case LLG_ACOV_OP_PASS_ON:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_PASS_ON);
            break;
        case LLG_ACOV_OP_FAIL_OFF:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_FAIL_OFF);
            break;
        case LLG_ACOV_OP_FAIL_ON:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_FAIL_ON);
            break;
        case LLG_ACOV_OP_VACUOUS_OFF:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_VACUOUS_OFF);
            break;
        case LLG_ACOV_OP_NONVACUOUS_ON:
            assertion_apply_action_control(assertion, LLG_ASSERTION_CONTROL_NONVACUOUS_ON);
            break;
        default: break;
    }
}

// Immediate and deferred items keep their controls on the item: the
// generated code of a model with a VPI catalog reads them per check.
static void llg_acov_item_control(llg_acov_item_t* item, int operation, uint64_t start) {
    uint8_t off = g.assertions_ended ? LLG_ACOV_CTL_OFF : 0u;
    switch (operation) {
        case LLG_ACOV_OP_DISABLE: item->control |= LLG_ACOV_CTL_OFF; break;
        case LLG_ACOV_OP_ENABLE: item->control = (uint8_t)((item->control & ~LLG_ACOV_CTL_OFF) | off); break;
        case LLG_ACOV_OP_KILL:
            if (start == g.now) llg_acov_flush_deferred(item);
            break;
        case LLG_ACOV_OP_KILL_ALL:
            item->control |= LLG_ACOV_CTL_OFF;
            llg_acov_flush_deferred(item);
            break;
        case LLG_ACOV_OP_RESET:
            llg_acov_flush_deferred(item);
            item->control = off;
            break;
        case LLG_ACOV_OP_PASS_OFF: item->control |= LLG_ACOV_CTL_PASS_OFF | LLG_ACOV_CTL_VACUOUS_OFF; break;
        case LLG_ACOV_OP_PASS_ON:
            item->control = (uint8_t)(item->control & ~(LLG_ACOV_CTL_PASS_OFF | LLG_ACOV_CTL_VACUOUS_OFF));
            break;
        case LLG_ACOV_OP_FAIL_OFF: item->control |= LLG_ACOV_CTL_FAIL_OFF; break;
        case LLG_ACOV_OP_FAIL_ON: item->control = (uint8_t)(item->control & ~LLG_ACOV_CTL_FAIL_OFF); break;
        case LLG_ACOV_OP_VACUOUS_OFF: item->control |= LLG_ACOV_CTL_VACUOUS_OFF; break;
        case LLG_ACOV_OP_NONVACUOUS_ON: item->control = (uint8_t)(item->control & ~LLG_ACOV_CTL_PASS_OFF); break;
        default: break;
    }
}

int llg_acov_control(size_t index, int operation, uint64_t start) {
    if (index >= llg_acov_n_items || operation < LLG_ACOV_OP_DISABLE ||
        operation > LLG_ACOV_OP_NONVACUOUS_ON)
        return 0;
    llg_acov_item_t* item = &llg_acov_items[index];
    assertion_events_enter();
    if (item->form == LLG_ACOV_IMMEDIATE || item->form == LLG_ACOV_DEFERRED) {
        llg_acov_item_control(item, operation, start);
    } else {
        for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
             assertion = assertion->next)
            if (assertion->acov == item) llg_acov_record_control(assertion, operation, start);
    }
    assertion_events_leave();
    return 1;
}

// ── Database section ────────────────────────────────────────────────────────

static int llg_acov_available(void) { return llg_acov_present && llg_acov_n_items != 0; }

// Identity of the item table: per item its kind, form, owning instance path
// and hierarchical name (the label of an unnamed one is empty), in table
// order.
static uint64_t llg_acov_schema_hash(void) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_u64(&hash, (uint64_t)llg_acov_n_items);
    for (size_t i = 0; i < llg_acov_n_items; ++i) {
        const llg_acov_item_t* item = &llg_acov_items[i];
        llg_cov_fnv_u64(&hash, item->kind);
        llg_cov_fnv_u64(&hash, item->form);
        llg_cov_fnv_text(&hash, item->instance >= 0 &&
                                        (size_t)item->instance < llg_acov_n_instances
                                    ? llg_acov_instances[item->instance].path
                                    : "$root");
        llg_cov_fnv_text(&hash, item->full_name);
    }
    return hash;
}

// u32 items, then per item: attempts, successes, vacuous, failures,
// disabled, killed (u64 each).
static void llg_acov_put_section(llg_cov_buf_t* buf) {
    llg_cov_put_u32(buf, (uint32_t)llg_acov_n_items);
    for (size_t i = 0; i < llg_acov_n_items; ++i) {
        const llg_acov_item_t* item = &llg_acov_items[i];
        llg_cov_put_u64(buf, item->attempts);
        llg_cov_put_u64(buf, item->successes);
        llg_cov_put_u64(buf, item->vacuous);
        llg_cov_put_u64(buf, item->failures);
        llg_cov_put_u64(buf, item->disabled);
        llg_cov_put_u64(buf, item->killed);
    }
}

// Validate the assertion section of `db`, then add its counters. Returns
// NULL on success, else the reason; nothing changes on failure.
static const char* llg_acov_merge_section(const llg_cov_db_file_t* db) {
    if (db->assertion_hash != llg_acov_schema_hash())
        return "it was saved by a model with different assertions";
    llg_cov_reader_t reader = {db->data, db->assertion_end, db->assertion_at, NULL};
    uint32_t n;
    if (!llg_cov_get_u32(&reader, &n)) return reader.error;
    if ((size_t)n != llg_acov_n_items ||
        reader.len - reader.pos != (size_t)n * LLG_ACOV_COUNTERS * 8u)
        return "the assertion section does not match its item table";
    for (size_t i = 0; i < llg_acov_n_items; ++i) {
        llg_acov_item_t* item = &llg_acov_items[i];
        uint64_t* counters[LLG_ACOV_COUNTERS] = {&item->attempts, &item->successes,
                                                 &item->vacuous,  &item->failures,
                                                 &item->disabled, &item->killed};
        for (size_t c = 0; c < LLG_ACOV_COUNTERS; ++c) {
            uint64_t value = 0;
            (void)llg_cov_get_u64(&reader, &value);
            llg_acov_add(counters[c], value);
        }
    }
    return NULL;
}

// ── $coverage_* (40.3.2) ────────────────────────────────────────────────────

// A known integral argument of at most 64 bits.
static int llg_acov_arg(const sv4_t* value, uint64_t* result) {
    if (!value || llg_sv4_width(*value) == 0 || llg_sv4_width(*value) > 64 ||
        sv4_is_unknown(*value))
        return 0;
    *result = sv4_to_u64(*value);
    return 1;
}

static int llg_acov_definition_known(const char* definition) {
    for (size_t i = 0; i < llg_acov_n_instances; ++i)
        if (strcmp(llg_acov_instances[i].definition, definition) == 0) return 1;
    return 0;
}

// Whether instance `instance` (-1: owned by $root) lies in the scope `scope`
// (an instance index, -1 $root, -2 the module definition `definition`) with
// MODULE (`hier` 0) or HIER semantics (Table 40-2).
static int llg_acov_in_scope(int32_t instance, int hier, int32_t scope, const char* definition) {
    if (scope == -1) return hier || instance < 0;
    for (int32_t at = instance; at >= 0 && (size_t)at < llg_acov_n_instances;
         at = llg_acov_instances[at].parent) {
        if (scope >= 0 ? at == scope
                       : definition && strcmp(llg_acov_instances[at].definition, definition) == 0)
            return 1;
        if (!hier) return 0;
    }
    return 0;
}

typedef struct {
    size_t items;    // items in scope
    size_t covered;  // of those, items with a nonvacuous success (S46-D4)
} llg_acov_tally_t;

// Visit the items in scope; `operation` is START, STOP or RESET, or
// UINT32_MAX to only tally them.
static llg_acov_tally_t llg_acov_scope(int hier, int32_t scope, const char* definition,
                                       uint32_t operation) {
    llg_acov_tally_t tally = {0, 0};
    for (size_t i = 0; i < llg_acov_n_items; ++i) {
        llg_acov_item_t* item = &llg_acov_items[i];
        if (!llg_acov_in_scope(item->instance, hier, scope, definition)) continue;
        tally.items++;
        tally.covered += item->successes != 0;
        if (operation == LLG_SV_COV_START) item->stopped = 0;
        else if (operation == LLG_SV_COV_STOP) item->stopped = 1;
        else if (operation == LLG_SV_COV_RESET) llg_acov_zero(item);
    }
    return tally;
}

static int32_t llg_acov_count(size_t count) {
    return count > (size_t)INT32_MAX ? LLG_SV_COV_OVERFLOW : (int32_t)count;
}

// `what` names the call in messages ("$coverage_merge at <site>").
static int32_t llg_coverage_merge(uint64_t type, const char* name, const char* what) {
    llg_cov_db_file_t db;
    int missing;
    const char* error = llg_cov_db_read(name, &db, &missing);
    int32_t status = LLG_SV_COV_NOCOV;
    if (!error && type == LLG_SV_COV_ASSERTION && llg_acov_present &&
        (db.sections & LLG_COV_DB_ASSERTION)) {
        error = llg_acov_merge_section(&db);
        if (!error) status = LLG_SV_COV_OK;
    }
    if (error) {
        fprintf(stderr,
                "llg: runtime error: %s: coverage database `%s` was not merged: %s; no coverage "
                "changed\n",
                what, name, error);
        llg_rt_mark_failed();
        status = LLG_SV_COV_ERROR;
    }
    llg_cov_db_file_free(&db);
    return status;
}

// `what` names the call in run-time errors.
static int32_t llg_coverage_dispatch(int function, const uint64_t* v, int32_t scope,
                                     const char* text, const char* what) {
    uint64_t control = 0, type, scope_def = LLG_SV_COV_MODULE;
    switch (function) {
    case LLG_COVERAGE_CONTROL:
        control = v[0];
        type = v[1];
        scope_def = v[2];
        if (control > LLG_SV_COV_CHECK) return LLG_SV_COV_ERROR;
        break;
    case LLG_COVERAGE_GET_MAX:
    case LLG_COVERAGE_GET:
        type = v[0];
        scope_def = v[1];
        break;
    case LLG_COVERAGE_MERGE:
    case LLG_COVERAGE_SAVE:
        type = v[0];
        break;
    default:
        return LLG_SV_COV_ERROR;
    }
    if (type < LLG_SV_COV_ASSERTION || type > LLG_SV_COV_TOGGLE) return LLG_SV_COV_ERROR;
    if (function == LLG_COVERAGE_MERGE) return llg_coverage_merge(type, text, what);
    if (function == LLG_COVERAGE_SAVE) {
        if (type != LLG_SV_COV_ASSERTION || !llg_acov_available()) return LLG_SV_COV_NOCOV;
        return llg_cov_db_save(text, LLG_COV_DB_ASSERTION, what) ? LLG_SV_COV_OK
                                                                 : LLG_SV_COV_ERROR;
    }
    // A bad scope is an error for every kind (Table 40-1 "Bad args").
    if (scope_def != LLG_SV_COV_MODULE && scope_def != LLG_SV_COV_HIER) return LLG_SV_COV_ERROR;
    if (scope == -2 ? !llg_acov_definition_known(text)
                    : scope < -1 || (scope >= 0 && (size_t)scope >= llg_acov_n_instances))
        return LLG_SV_COV_ERROR;
    int hier = scope_def == LLG_SV_COV_HIER;
    if (type != LLG_SV_COV_ASSERTION || !llg_acov_present) {
        // Statement, toggle and FSM coverage are not available (S46-D5).
        if (function == LLG_COVERAGE_CONTROL &&
            (control == LLG_SV_COV_STOP || control == LLG_SV_COV_RESET))
            return LLG_SV_COV_OK;
        return LLG_SV_COV_NOCOV;
    }
    llg_acov_tally_t tally = llg_acov_scope(
        hier, scope, text, function == LLG_COVERAGE_CONTROL ? (uint32_t)control : UINT32_MAX);
    switch (function) {
    case LLG_COVERAGE_GET_MAX:
        return llg_acov_count(tally.items);
    case LLG_COVERAGE_GET:
        return llg_acov_count(tally.covered);
    default:
        if (control == LLG_SV_COV_STOP || control == LLG_SV_COV_RESET) return LLG_SV_COV_OK;
        // CHECK and START: every item in scope collects coverage, so the
        // scope is fully covered or has no assertion coverage at all.
        return tally.items ? LLG_SV_COV_OK : LLG_SV_COV_NOCOV;
    }
}

int32_t llg_coverage_call(int function, const sv4_t* args, int n_args, int32_t instance,
                          llg_string_t text, int has_text, const char* site) {
    char* name = has_text ? llg_cov_strdup_bytes(text.data, text.data ? text.len : 0u) : NULL;
    llg_string_destroy(&text);
    uint64_t values[3] = {0, 0, 0};
    int32_t status = LLG_SV_COV_ERROR;
    int known = n_args >= 1 && n_args <= 3 && args;
    for (int i = 0; known && i < n_args; ++i) known = llg_acov_arg(&args[i], &values[i]);
    if (known && (function == LLG_COVERAGE_MERGE || function == LLG_COVERAGE_SAVE ||
                  instance == -2) && !name)
        known = 0;
    if (known) {
        static const char* const names[] = {"$coverage_control", "$coverage_get_max",
                                            "$coverage_get", "$coverage_merge", "$coverage_save"};
        char what[512];
        snprintf(what, sizeof what, "%s at %s",
                 function >= 0 && function <= LLG_COVERAGE_SAVE ? names[function] : "?",
                 site ? site : "");
        status = llg_coverage_dispatch(function, values, instance, name, what);
    }
    free(name);
    return status;
}

// ── VPI bridge (39.3, 40.5) ─────────────────────────────────────────────────

size_t llg_acov_item_count(void) { return llg_acov_n_items; }

const llg_acov_item_t* llg_acov_item_at(size_t index) {
    return index < llg_acov_n_items ? &llg_acov_items[index] : NULL;
}

size_t llg_acov_instance_count(void) { return llg_acov_n_instances; }

const llg_acov_instance_t* llg_acov_instance_at(size_t index) {
    return index < llg_acov_n_instances ? &llg_acov_instances[index] : NULL;
}

int32_t llg_acov_instance_named(const char* path) {
    if (!path) return -1;
    for (size_t i = 0; i < llg_acov_n_instances; ++i)
        if (strcmp(llg_acov_instances[i].path, path) == 0) return (int32_t)i;
    return -1;
}

size_t llg_acov_covered_in(int32_t instance) {
    return llg_acov_scope(0, instance, NULL, UINT32_MAX).covered;
}

int32_t llg_coverage_control_vpi(int control, int type, int32_t instance, int64_t item) {
    if (control < (int)LLG_SV_COV_START || control > (int)LLG_SV_COV_CHECK) return LLG_SV_COV_ERROR;
    if (item < 0) {
        uint64_t values[3] = {(uint64_t)control, (uint64_t)type, LLG_SV_COV_MODULE};
        if (instance < 0) return LLG_SV_COV_ERROR;
        return llg_coverage_dispatch(LLG_COVERAGE_CONTROL, values, instance, NULL,
                                     "vpi_control");
    }
    if ((size_t)item >= llg_acov_n_items || type < (int)LLG_SV_COV_ASSERTION ||
        type > (int)LLG_SV_COV_TOGGLE)
        return LLG_SV_COV_ERROR;
    // An assertion handle names assertion coverage only.
    if (type != (int)LLG_SV_COV_ASSERTION)
        return control == (int)LLG_SV_COV_STOP || control == (int)LLG_SV_COV_RESET
                   ? LLG_SV_COV_OK
                   : LLG_SV_COV_NOCOV;
    llg_acov_item_t* target = &llg_acov_items[item];
    if (control == (int)LLG_SV_COV_START) target->stopped = 0;
    else if (control == (int)LLG_SV_COV_STOP) target->stopped = 1;
    else if (control == (int)LLG_SV_COV_RESET) llg_acov_zero(target);
    return LLG_SV_COV_OK;
}

int32_t llg_coverage_file_vpi(int merge, int type, const char* name) {
    uint64_t values[1] = {(uint64_t)(int64_t)type};
    if (!name || type < 0) return LLG_SV_COV_ERROR;
    return llg_coverage_dispatch(merge ? LLG_COVERAGE_MERGE : LLG_COVERAGE_SAVE, values, -1, name,
                                 merge ? "vpi_control(vpiCoverageMerge)"
                                       : "vpi_control(vpiCoverageSave)");
}
