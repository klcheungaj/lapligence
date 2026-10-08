/* Sequence thread budget. Every pending token and every attempt counts as
 * one live thread; crossing LLG_SEQUENCE_THREAD_LIMIT reports the assertion
 * and stops the run, so no obligation is ever dropped silently. */
static void sequence_thread_acquire(void) {
    if (++g.sequence_threads == g.sequence_thread_limit + 1u) {
        const llg_concurrent_assertion_t* assertion = g.sequence_current;
        fprintf(stderr,
                "llg: sequence thread budget exhausted: more than %llu live sequence threads"
                " at time %llu (concurrent assertion %s at %s); raise LLG_SEQUENCE_THREAD_LIMIT\n",
                (unsigned long long)g.sequence_thread_limit, (unsigned long long)g.now,
                assertion && assertion->label && assertion->label[0] ? assertion->label : "<unnamed>",
                assertion && assertion->location ? assertion->location : "<unknown>");
        llg_last_failure = 1;
        g.finish = 1;
    }
}

static void sequence_thread_release(void) {
    if (g.sequence_threads) g.sequence_threads--;
}

static llg_sequence_scope_t* sequence_scope_alloc(void) {
    llg_sequence_scope_t* scope = g.sequence_scope_pool;
    if (!scope) return llg_checked_calloc(1, sizeof(*scope), "sequence frame");
    g.sequence_scope_pool = scope->parent;
    memset(scope, 0, sizeof(*scope));
    return scope;
}

static llg_sequence_join_instance_t* sequence_join_alloc(void) {
    llg_sequence_join_instance_t* join = g.sequence_join_pool;
    if (!join) return llg_checked_calloc(1, sizeof(*join), "sequence join");
    g.sequence_join_pool = join->next_free;
    memset(join, 0, sizeof(*join));
    return join;
}

static llg_sequence_token_t* sequence_token_alloc(void) {
    llg_sequence_token_t* token = g.sequence_token_pool;
    if (token) {
        g.sequence_token_pool = token->next;
        memset(token, 0, sizeof(*token));
    } else {
        token = llg_checked_calloc(1, sizeof(*token), "sequence token");
    }
    sequence_thread_acquire();
    return token;
}

static llg_sequence_endpoint_t* sequence_endpoint_alloc(void) {
    llg_sequence_endpoint_t* endpoint = g.sequence_endpoint_pool;
    if (!endpoint) return llg_checked_calloc(1, sizeof(*endpoint), "sequence endpoint");
    g.sequence_endpoint_pool = endpoint->next;
    memset(endpoint, 0, sizeof(*endpoint));
    return endpoint;
}

static void sequence_endpoint_recycle(llg_sequence_endpoint_t* endpoint) {
    sv4_destroy_array(endpoint->locals, endpoint->local_count);
    free(endpoint->locals);
    endpoint->locals = NULL;
    endpoint->next = g.sequence_endpoint_pool;
    g.sequence_endpoint_pool = endpoint;
}

static void free_sequence_pools(void) {
    while (g.sequence_token_pool) {
        llg_sequence_token_t* next = g.sequence_token_pool->next;
        free(g.sequence_token_pool);
        g.sequence_token_pool = next;
    }
    while (g.sequence_scope_pool) {
        llg_sequence_scope_t* next = g.sequence_scope_pool->parent;
        free(g.sequence_scope_pool);
        g.sequence_scope_pool = next;
    }
    while (g.sequence_join_pool) {
        llg_sequence_join_instance_t* next = g.sequence_join_pool->next_free;
        free(g.sequence_join_pool);
        g.sequence_join_pool = next;
    }
    while (g.sequence_endpoint_pool) {
        llg_sequence_endpoint_t* next = g.sequence_endpoint_pool->next;
        free(g.sequence_endpoint_pool);
        g.sequence_endpoint_pool = next;
    }
    while (g.sequence_attempt_pool) {
        llg_sequence_attempt_t* next = g.sequence_attempt_pool->next;
        free(g.sequence_attempt_pool);
        g.sequence_attempt_pool = next;
    }
    while (g.assertion_clock_event_pool) {
        llg_assertion_clock_event_t* next = g.assertion_clock_event_pool->next;
        free(g.assertion_clock_event_pool);
        g.assertion_clock_event_pool = next;
    }
    g.sequence_threads = 0;
    g.sequence_current = NULL;
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
        llg_sequence_join_instance_t* join = scope->join;
        if (join) {
            // The last thread of this side is gone: it can match no more.
            join->alive[scope->side] = 0;
            if (--join->refs == 0) {
                join->next_free = g.sequence_join_pool;
                g.sequence_join_pool = join;
            }
        }
        scope->parent = g.sequence_scope_pool;
        g.sequence_scope_pool = scope;
        scope = parent;
    }
}

static int sequence_mark_same(const llg_sequence_mark_t* a, const llg_sequence_mark_t* b) {
    return a->time == b->time && a->tick == b->tick && a->clock == b->clock &&
           a->edge == b->edge;
}

/* A pending thread is dead when an enclosing first_match invocation has
 * closed, or when an enclosing join can no longer pair its endpoint: the
 * other side has no live thread and, for AND, never matched. */
static int sequence_scope_doomed(const llg_sequence_scope_t* scope) {
    for (; scope; scope = scope->parent) {
        if (scope->matched) return 1;
        const llg_sequence_join_instance_t* join = scope->join;
        if (!join) continue;
        int other = !scope->side;
        if (join->alive[other]) continue;
        if (join->plan->kind == LLG_SEQUENCE_JOIN_INTERSECT || !join->matched[other]) return 1;
    }
    return 0;
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
    token->locals = NULL;
    token->scope = NULL;
    token->next = g.sequence_token_pool;
    g.sequence_token_pool = token;
    sequence_thread_release();
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
    llg_sequence_token_t* token = sequence_token_alloc();
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
        sequence_endpoint_recycle(endpoint);
        endpoint = next;
    }
}

static void sequence_endpoint_add(llg_sequence_attempt_t* attempt,
                                   const llg_sequence_token_t* token, int empty) {
    llg_sequence_endpoint_t* endpoint = sequence_endpoint_alloc();
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
    llg_sequence_attempt_t* attempt = g.sequence_attempt_pool;
    if (attempt) {
        g.sequence_attempt_pool = attempt->next;
        memset(attempt, 0, sizeof(*attempt));
    } else {
        attempt = llg_checked_calloc(1, sizeof(*attempt), "sequence attempt");
    }
    sequence_thread_acquire();
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
        if (matches && edge->exit_join) {
            llg_sequence_scope_t* frame = destination->scope;
            const llg_sequence_join_t* plan = edge->exit_join <= graph->join_count
                ? &graph->joins[edge->exit_join - 1] : NULL;
            if (!plan || !frame || !frame->join || frame->join->plan != plan) {
                fprintf(stderr, "llg: unbalanced sequence join\n");
                llg_last_failure = 1; g.finish = 1; matches = 0;
            } else {
                llg_sequence_join_instance_t* join = frame->join;
                int side = frame->side;
                int other = !side;
                llg_sequence_mark_t mark = { .time = event.time, .tick = event.tick,
                    .clock = event.signal, .edge = event.edge };
                join->matched[side] = 1;
                join->last[side] = mark;
                int pair = plan->kind == LLG_SEQUENCE_JOIN_AND
                    ? join->matched[other]
                    : join->matched[other] && sequence_mark_same(&join->last[other], &mark);
                // One continuation per join endpoint tick: the other side's
                // threads ending on the same tick would only duplicate it.
                if (!pair || (join->emitted && sequence_mark_same(&join->emitted_at, &mark))) {
                    matches = 0;
                } else {
                    join->emitted = 1;
                    join->emitted_at = mark;
                    destination->scope = frame->parent;
                    sequence_scope_retain(destination->scope);
                    sequence_scope_release(frame);
                }
            }
        }
        llg_sequence_token_t* fork = NULL;
        if (matches && edge->enter_join) {
            const llg_sequence_join_t* plan = edge->enter_join <= graph->join_count
                ? &graph->joins[edge->enter_join - 1] : NULL;
            if (!plan) {
                fprintf(stderr, "llg: invalid sequence join\n");
                llg_last_failure = 1; g.finish = 1; matches = 0;
            } else {
                // Both operands start on this tick, each in its own thread
                // with its own side frame and local-variable copy.
                llg_sequence_join_instance_t* join = sequence_join_alloc();
                join->plan = plan;
                join->refs = 2;
                join->alive[0] = join->alive[1] = 1;
                if (plan->kind == LLG_SEQUENCE_JOIN_AND) {
                    join->matched[0] = plan->left_empty;
                    join->matched[1] = plan->right_empty;
                }
                fork = sequence_token_copy(graph, destination);
                llg_sequence_scope_t* left = sequence_scope_alloc();
                left->refs = 1;
                left->join = join;
                left->side = 0;
                left->parent = destination->scope; // transfer the token's parent reference
                destination->scope = left;
                llg_sequence_scope_t* right = sequence_scope_alloc();
                right->refs = 1;
                right->join = join;
                right->side = 1;
                right->parent = fork->scope;
                fork->scope = right;
                fork->state = plan->right_start;
            }
        }
        if (matches && edge->enter_scope) {
            llg_sequence_scope_t* scope = sequence_scope_alloc();
            scope->refs = 1;
            scope->identity = edge->enter_scope;
            scope->parent = destination->scope; // transfer the token's parent reference
            destination->scope = scope;
        }
        if (matches) sequence_match_items(graph, attempt, edge->match_start, edge->match_count);
        attempt->locals = NULL;
        if (matches && !g.finish) sequence_token_push(graph, &work, destination);
        else sequence_token_free(destination);
        if (fork) {
            if (!g.finish) sequence_token_push(graph, &work, fork);
            else sequence_token_free(fork);
        }
    }
    sequence_tokens_free(work);
    sequence_tokens_free(processed);
    // Dropping a doomed thread can end a join side and doom further
    // threads, so prune to a fixed point (bounded by the frame depth).
    int pruned;
    do {
        pruned = 0;
        llg_sequence_token_t** link = &next;
        while (*link) {
            llg_sequence_token_t* token = *link;
            if (sequence_scope_doomed(token->scope) || (graph->first_match && attempt->endpoints)) {
                *link = token->next;
                sequence_token_free(token);
                pruned = 1;
            } else link = &token->next;
        }
    } while (pruned);
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
    attempt->locals = NULL;
    attempt->inherited = NULL;
    attempt->next = g.sequence_attempt_pool;
    g.sequence_attempt_pool = attempt;
    sequence_thread_release();
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
        sequence_endpoint_recycle(endpoint);
        endpoint = next;
    }
    return !g.finish;
}
