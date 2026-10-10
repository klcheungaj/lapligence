/* Property engine (IEEE 1800-2009 16.12-16.13, 16.15.8, Annex F).
 *
 * Each evaluation attempt is a tree of live node instances built from the
 * assertion's static program. On every leading-clock tick the tree is stepped
 * top-down: a node first advances its existing children, then starts the
 * children due on this tick (which are stepped at once), then settles its own
 * result from its children. A node resolves exactly once, to true or false,
 * with the 16.15.8 nonvacuity of that evaluation; resolving frees its subtree.
 * Results are decided as soon as they are determined, so for example an
 * `and` fails when either operand fails. Asynchronous accept_on/reject_on
 * conditions are also checked between ticks, once per time step, because
 * their sampled values cannot change within a step (16.13.14).
 *
 * Every live instance counts against LLG_SEQUENCE_THREAD_LIMIT, like a
 * sequence thread. */

enum { LLG_PROP_PENDING = 0, LLG_PROP_TRUE = 1, LLG_PROP_FALSE = 2 };

typedef struct {
    llg_concurrent_assertion_t* assertion;
    const llg_property_program_t* program;
    llg_assertion_clock_event_t event;
} llg_property_ctx_t;

static void property_tick(llg_property_ctx_t* ctx, llg_property_inst_t* inst);
static void property_settle(llg_property_inst_t* inst);

static llg_property_locals_t* property_locals_retain(llg_property_locals_t* locals) {
    if (locals) {
        if (locals->refs == SIZE_MAX) {
            fprintf(stderr, "llg: property local reference overflow\n");
            abort();
        }
        locals->refs++;
    }
    return locals;
}

static void property_locals_release(llg_property_locals_t* locals) {
    if (!locals || --locals->refs) return;
    if (locals->values) sv4_destroy_array(locals->values, locals->graph->local_count);
    free(locals->values);
    free(locals);
}

static llg_property_locals_t* property_locals_capture(const llg_sequence_graph_t* graph,
                                                      const sv4_t* values) {
    llg_property_locals_t* locals =
        llg_checked_calloc(1, sizeof(*locals), "property local values");
    locals->refs = 1;
    locals->graph = graph;
    locals->values = sequence_locals_clone(graph, values);
    return locals;
}

static llg_property_inst_t* property_inst_new(const llg_property_program_t* program,
                                              uint32_t node, uint8_t delay,
                                              llg_property_locals_t* locals) {
    llg_property_inst_t* inst = g.property_inst_pool;
    if (inst) {
        g.property_inst_pool = inst->next;
        memset(inst, 0, sizeof(*inst));
    } else {
        inst = llg_checked_calloc(1, sizeof(*inst), "property evaluation node");
    }
    sequence_thread_acquire();
    inst->node = &program->nodes[node];
    inst->delay = delay;
    inst->locals = property_locals_retain(locals);
    return inst;
}

static void property_inst_free(llg_property_inst_t* inst) {
    while (inst->children) {
        llg_property_inst_t* child = inst->children;
        inst->children = child->next;
        property_inst_free(child);
    }
    if (inst->sequence) sequence_attempt_discard(inst->sequence);
    property_locals_release(inst->locals);
    inst->sequence = NULL;
    inst->locals = NULL;
    inst->next = g.property_inst_pool;
    g.property_inst_pool = inst;
    sequence_thread_release();
}

static void property_free_list(llg_property_inst_t* list) {
    while (list) {
        llg_property_inst_t* next = list->next;
        property_inst_free(list);
        list = next;
    }
}

static void free_property_pool(void) {
    while (g.property_inst_pool) {
        llg_property_inst_t* next = g.property_inst_pool->next;
        free(g.property_inst_pool);
        g.property_inst_pool = next;
    }
}

static void property_resolve(llg_property_inst_t* inst, int status, int nonvacuous) {
    inst->status = (uint8_t)status;
    inst->nonvacuous = nonvacuous ? 1 : 0;
    property_free_list(inst->children);
    inst->children = inst->children_tail = NULL;
    if (inst->sequence) {
        sequence_attempt_discard(inst->sequence);
        inst->sequence = NULL;
    }
}

/* Unlink and free `child`, whose predecessor link is `*link`. */
static void property_drop_child(llg_property_inst_t* inst, llg_property_inst_t** link,
                                llg_property_inst_t* previous) {
    llg_property_inst_t* child = *link;
    *link = child->next;
    if (inst->children_tail == child) inst->children_tail = previous;
    property_inst_free(child);
}

static int property_atom(llg_property_ctx_t* ctx, uint32_t atom) {
    if (!ctx->program->atom || atom >= ctx->program->atom_count) {
        fprintf(stderr, "llg runtime fatal: invalid property atom %u\n", (unsigned)atom);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    return ctx->program->atom(atom, ctx->assertion->data) != 0;
}

static llg_sequence_attempt_t* property_sequence_new(llg_property_ctx_t* ctx,
                                                     llg_property_inst_t* inst,
                                                     uint32_t index) {
    const llg_sequence_graph_t* graph = ctx->program->sequences[index];
    llg_property_locals_t* locals = inst->locals;
    llg_sequence_attempt_t* attempt = sequence_attempt_new(
        graph, 0, locals ? locals->graph : NULL, locals ? locals->values : NULL);
    attempt->rank = ctx->assertion->property_ranks[index];
    return attempt;
}

static int property_sequence_step(llg_property_ctx_t* ctx, llg_sequence_attempt_t* attempt) {
    const llg_assertion_clock_event_t* event = &ctx->event;
    int accepted = 0;
    return sequence_attempt_step(attempt, ctx->assertion, 0, event->signal, event->edge,
                                 event->time, event->order, event->tick, &accepted);
}

/* Start a child due on this tick and step it immediately. */
static void property_spawn(llg_property_ctx_t* ctx, llg_property_inst_t* inst,
                           uint32_t node, uint8_t delay, llg_property_locals_t* locals) {
    llg_property_inst_t* child = property_inst_new(ctx->program, node, delay, locals);
    if (inst->children_tail) inst->children_tail->next = child;
    else inst->children = child;
    inst->children_tail = child;
    property_tick(ctx, child);
}

static void property_tick_children(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    for (llg_property_inst_t* child = inst->children; child && !g.finish; child = child->next)
        property_tick(ctx, child);
}

/* An abort condition holding in a time step of the evaluation decides it:
 * true for accept_on, false for reject_on; the evaluation is vacuous
 * (16.13.14, 16.15.8 ab/ac). */
static int property_abort_fires(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    const llg_property_node_t* node = inst->node;
    if (!(node->flags & LLG_PROPERTY_SYNC)) inst->abort_checked = g.now + 1;
    if (!property_atom(ctx, node->first)) return 0;
    property_resolve(inst, (node->flags & LLG_PROPERTY_ACCEPT) ? LLG_PROP_TRUE : LLG_PROP_FALSE,
                     0);
    return 1;
}

/* Step an implication or followed-by antecedent and start one consequent per
 * nonempty match, at the match tick (|->, #-#) or the next tick (|=>, #=#).
 * An empty match lies before the start tick, so the nonoverlapped forms
 * start its consequent at the start tick and the overlapped ones ignore it
 * (16.13.6, 16.13.22). */
static void property_antecedent_step(llg_property_ctx_t* ctx, llg_property_inst_t* inst,
                                     uint64_t tick) {
    const llg_property_node_t* node = inst->node;
    if (tick == 0) inst->sequence = property_sequence_new(ctx, inst, node->first);
    int alive = property_sequence_step(ctx, inst->sequence);
    if (g.finish) return;
    int overlapped = (node->flags & LLG_PROPERTY_OVERLAPPED) != 0;
    const llg_sequence_graph_t* graph = inst->sequence->graph;
    for (const llg_sequence_endpoint_t* endpoint = inst->sequence->endpoints;
         endpoint && !g.finish; endpoint = endpoint->next) {
        if (endpoint->empty && overlapped) continue;
        inst->matched = 1;
        // Locals assigned by the antecedent flow into the consequent; with
        // no antecedent locals the attempt's inherited values flow through.
        llg_property_locals_t* locals = graph->local_count
                                            ? property_locals_capture(graph, endpoint->locals)
                                            : property_locals_retain(inst->locals);
        property_spawn(ctx, inst, node->second, (overlapped || endpoint->empty) ? 0 : 1,
                       locals);
        property_locals_release(locals);
    }
    if (!alive && !g.finish) {
        inst->done = 1;
        sequence_attempt_discard(inst->sequence);
        inst->sequence = NULL;
    }
}

static void property_tick(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    if (inst->status || g.finish) return;
    if (inst->delay) {
        inst->delay--;
        return;
    }
    const llg_property_node_t* node = inst->node;
    uint64_t tick = inst->ticks++;
    switch (node->kind) {
    case LLG_PROPERTY_BOOLEAN:
        property_resolve(inst, property_atom(ctx, node->first) ? LLG_PROP_TRUE : LLG_PROP_FALSE,
                         1);
        return;
    case LLG_PROPERTY_SEQUENCE: {
        // A sequence property holds at its first nonempty match (16.13.1) and
        // fails once no thread can match any more.
        if (tick == 0) inst->sequence = property_sequence_new(ctx, inst, node->first);
        int alive = property_sequence_step(ctx, inst->sequence);
        if (g.finish) return;
        if (inst->sequence->matched) property_resolve(inst, LLG_PROP_TRUE, 1);
        else if (!alive) property_resolve(inst, LLG_PROP_FALSE, 1);
        return;
    }
    case LLG_PROPERTY_ABORT:
        // The outer condition is checked before the operand is stepped, so
        // an abort takes precedence over an evaluation ending in the same
        // step and the outermost of nested aborts wins (16.13.14).
        if (property_abort_fires(ctx, inst)) return;
        if (tick == 0) property_spawn(ctx, inst, node->second, 0, inst->locals);
        else property_tick_children(ctx, inst);
        break;
    case LLG_PROPERTY_IMPLICATION:
    case LLG_PROPERTY_FOLLOWED_BY:
        property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) property_antecedent_step(ctx, inst, tick);
        break;
    case LLG_PROPERTY_IF:
        if (tick == 0) {
            if (property_atom(ctx, node->first)) {
                property_spawn(ctx, inst, node->second, 0, inst->locals);
            } else if (node->third != LLG_PROPERTY_NONE) {
                property_spawn(ctx, inst, node->third, 0, inst->locals);
            } else {
                // `if (b) p` with b false holds vacuously (16.13.5, 16.15.8 g).
                property_resolve(inst, LLG_PROP_TRUE, 0);
                return;
            }
        } else {
            property_tick_children(ctx, inst);
        }
        break;
    case LLG_PROPERTY_NEXTTIME:
        property_tick_children(ctx, inst);
        if (tick == node->min) property_spawn(ctx, inst, node->first, 0, inst->locals);
        break;
    case LLG_PROPERTY_ALWAYS:
    case LLG_PROPERTY_EVENTUALLY:
        property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) {
            if (tick >= node->min) property_spawn(ctx, inst, node->first, 0, inst->locals);
            if (node->max != LLG_SEQUENCE_UNBOUNDED && tick >= node->max) inst->done = 1;
        }
        break;
    case LLG_PROPERTY_UNTIL:
        // Stage k evaluates both operands from tick k; stages are children
        // in pairs (left, right).
        property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) {
            property_spawn(ctx, inst, node->first, 0, inst->locals);
            property_spawn(ctx, inst, node->second, 0, inst->locals);
        }
        break;
    default:
        // NOT and the binary connectives start their operands together.
        if (tick == 0) {
            property_spawn(ctx, inst, node->first, 0, inst->locals);
            if (node->kind != LLG_PROPERTY_NOT && !g.finish)
                property_spawn(ctx, inst, node->second, 0, inst->locals);
        } else {
            property_tick_children(ctx, inst);
        }
        break;
    }
    if (!g.finish) property_settle(inst);
}

static int property_and3(int a, int b) {
    if (a == LLG_PROP_FALSE || b == LLG_PROP_FALSE) return LLG_PROP_FALSE;
    if (a == LLG_PROP_TRUE && b == LLG_PROP_TRUE) return LLG_PROP_TRUE;
    return LLG_PROP_PENDING;
}

static void property_settle_binary(llg_property_inst_t* inst) {
    llg_property_inst_t* left = inst->children;
    llg_property_inst_t* right = left ? left->next : NULL;
    if (!left || !right) return;
    int ls = left->status, rs = right->status;
    int lv = ls && left->nonvacuous, rv = rs && right->nonvacuous;
    switch (inst->node->kind) {
    case LLG_PROPERTY_AND:
        if (ls == LLG_PROP_FALSE || rs == LLG_PROP_FALSE)
            property_resolve(inst, LLG_PROP_FALSE, lv || rv);
        else if (ls == LLG_PROP_TRUE && rs == LLG_PROP_TRUE)
            property_resolve(inst, LLG_PROP_TRUE, lv || rv);
        break;
    case LLG_PROPERTY_OR:
        if (ls == LLG_PROP_TRUE || rs == LLG_PROP_TRUE)
            property_resolve(inst, LLG_PROP_TRUE, lv || rv);
        else if (ls == LLG_PROP_FALSE && rs == LLG_PROP_FALSE)
            property_resolve(inst, LLG_PROP_FALSE, lv || rv);
        break;
    case LLG_PROPERTY_IMPLIES:
        // Nonvacuous iff the antecedent property is (16.15.8 z).
        if (ls == LLG_PROP_FALSE || rs == LLG_PROP_TRUE)
            property_resolve(inst, LLG_PROP_TRUE, lv);
        else if (ls == LLG_PROP_TRUE && rs == LLG_PROP_FALSE)
            property_resolve(inst, LLG_PROP_FALSE, lv);
        break;
    case LLG_PROPERTY_IFF:
        if (ls && rs) property_resolve(inst, ls == rs ? LLG_PROP_TRUE : LLG_PROP_FALSE, lv || rv);
        break;
    default:
        break;
    }
}

/* Universal nodes (implication, always) fail with their first failing child
 * and hold once no child can start and every child held; existential nodes
 * (followed-by, eventually) are the duals. Consumed children leave their
 * nonvacuity in `accumulated`. */
static void property_settle_quantifier(llg_property_inst_t* inst, int universal) {
    int decisive = universal ? LLG_PROP_FALSE : LLG_PROP_TRUE;
    llg_property_inst_t** link = &inst->children;
    llg_property_inst_t* previous = NULL;
    while (*link) {
        llg_property_inst_t* child = *link;
        if (child->status == decisive) {
            property_resolve(inst, decisive, child->nonvacuous);
            return;
        }
        if (child->status) {
            inst->accumulated |= child->nonvacuous;
            property_drop_child(inst, link, previous);
            continue;
        }
        previous = child;
        link = &child->next;
    }
    if (inst->done && !inst->children)
        property_resolve(inst, universal ? LLG_PROP_TRUE : LLG_PROP_FALSE, inst->accumulated);
}

/* `p until q` holds iff some stage j has q holding from tick j while p holds
 * from every earlier tick (and from tick j itself for until_with); a weak
 * until also holds when p holds from every tick (16.13.12). */
static void property_settle_until(llg_property_inst_t* inst) {
    int overlapping = (inst->node->flags & LLG_PROPERTY_OVERLAPPED) != 0;
    // Leading stages whose left operand held and right operand failed only
    // extend the prefix; drop them so a long-running until stays bounded.
    while (inst->children && inst->children->next &&
           inst->children->status == LLG_PROP_TRUE &&
           inst->children->next->status == LLG_PROP_FALSE) {
        llg_property_inst_t* left = inst->children;
        llg_property_inst_t* right = left->next;
        inst->accumulated |= left->nonvacuous | (overlapping ? 0 : right->nonvacuous);
        property_drop_child(inst, &inst->children, NULL);
        property_drop_child(inst, &inst->children, NULL);
    }
    int prefix = LLG_PROP_TRUE;
    int pending = 0;
    int nonvacuous = inst->accumulated;
    for (llg_property_inst_t* left = inst->children; left && left->next;
         left = left->next->next) {
        llg_property_inst_t* right = left->next;
        int ls = left->status, rs = right->status;
        int candidate = property_and3(prefix, property_and3(overlapping ? ls : LLG_PROP_TRUE, rs));
        nonvacuous |= (ls && left->nonvacuous) | (!overlapping && rs && right->nonvacuous);
        if (candidate == LLG_PROP_TRUE) {
            property_resolve(inst, LLG_PROP_TRUE, nonvacuous);
            return;
        }
        if (candidate == LLG_PROP_PENDING) pending = 1;
        // A later stage can hold only if this one does: stop starting more.
        if (rs == LLG_PROP_TRUE) inst->done = 1;
        prefix = property_and3(prefix, ls);
        if (prefix == LLG_PROP_FALSE) {
            inst->done = 1;
            break;
        }
    }
    if (prefix == LLG_PROP_FALSE && !pending) property_resolve(inst, LLG_PROP_FALSE, nonvacuous);
}

static void property_settle(llg_property_inst_t* inst) {
    if (inst->status) return;
    llg_property_inst_t* child = inst->children;
    switch (inst->node->kind) {
    case LLG_PROPERTY_NOT:
        if (child && child->status)
            property_resolve(inst,
                             child->status == LLG_PROP_TRUE ? LLG_PROP_FALSE : LLG_PROP_TRUE,
                             child->nonvacuous);
        break;
    case LLG_PROPERTY_ABORT:
    case LLG_PROPERTY_IF:
    case LLG_PROPERTY_NEXTTIME:
        if (child && child->status) property_resolve(inst, child->status, child->nonvacuous);
        break;
    case LLG_PROPERTY_AND:
    case LLG_PROPERTY_OR:
    case LLG_PROPERTY_IMPLIES:
    case LLG_PROPERTY_IFF:
        property_settle_binary(inst);
        break;
    case LLG_PROPERTY_IMPLICATION:
    case LLG_PROPERTY_ALWAYS:
        property_settle_quantifier(inst, 1);
        break;
    case LLG_PROPERTY_FOLLOWED_BY:
    case LLG_PROPERTY_EVENTUALLY:
        property_settle_quantifier(inst, 0);
        break;
    case LLG_PROPERTY_UNTIL:
        property_settle_until(inst);
        break;
    default:
        break;
    }
}

/* Between ticks only asynchronous abort conditions can change a result. */
static void property_async(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    if (inst->status || inst->ticks == 0 || g.finish) return;
    const llg_property_node_t* node = inst->node;
    if (node->kind == LLG_PROPERTY_ABORT && !(node->flags & LLG_PROPERTY_SYNC) &&
        inst->abort_checked != g.now + 1 && property_abort_fires(ctx, inst))
        return;
    for (llg_property_inst_t* child = inst->children; child && !g.finish; child = child->next)
        property_async(ctx, child);
    if (!g.finish) property_settle(inst);
}

/* Verdict of a pending evaluation when the trace ends here (Annex F
 * F.5.3.2): weak obligations hold, strong ones fail, `not` swaps them. */
static int property_end(const llg_property_inst_t* inst) {
    if (inst->status) return inst->status == LLG_PROP_TRUE;
    // A consequent that would start after the last tick has no obligation.
    if (inst->ticks == 0) return 1;
    const llg_property_node_t* node = inst->node;
    int strong = (node->flags & LLG_PROPERTY_STRONG) != 0;
    const llg_property_inst_t* child = inst->children;
    switch (node->kind) {
    case LLG_PROPERTY_SEQUENCE:
        return !strong;
    case LLG_PROPERTY_NOT:
        return child ? !property_end(child) : 1;
    case LLG_PROPERTY_ABORT:
    case LLG_PROPERTY_IF:
        return child ? property_end(child) : 1;
    case LLG_PROPERTY_NEXTTIME:
        return child ? property_end(child) : !strong;
    case LLG_PROPERTY_AND:
        return child && child->next ? property_end(child) && property_end(child->next) : 1;
    case LLG_PROPERTY_OR:
        return child && child->next ? property_end(child) || property_end(child->next) : 1;
    case LLG_PROPERTY_IMPLIES:
        return child && child->next ? !property_end(child) || property_end(child->next) : 1;
    case LLG_PROPERTY_IFF:
        return child && child->next ? property_end(child) == property_end(child->next) : 1;
    case LLG_PROPERTY_IMPLICATION:
        for (; child; child = child->next)
            if (!property_end(child)) return 0;
        return 1;
    case LLG_PROPERTY_FOLLOWED_BY:
        // A consequent that has not started has no match to follow.
        for (; child; child = child->next)
            if (child->ticks != 0 && property_end(child)) return 1;
        return 0;
    case LLG_PROPERTY_ALWAYS:
        for (; child; child = child->next)
            if (!property_end(child)) return 0;
        return !strong || inst->done;
    case LLG_PROPERTY_EVENTUALLY:
        for (; child; child = child->next)
            if (property_end(child)) return 1;
        return !strong && !inst->done;
    case LLG_PROPERTY_UNTIL: {
        int overlapping = (node->flags & LLG_PROPERTY_OVERLAPPED) != 0;
        int prefix = 1;
        for (; child && child->next; child = child->next->next) {
            int left = property_end(child);
            if (prefix && property_end(child->next) && (!overlapping || left)) return 1;
            prefix = prefix && left;
            if (!prefix) return 0;
        }
        return !strong && prefix;
    }
    default:
        return 1;
    }
}

static void property_report(llg_concurrent_assertion_t* assertion, llg_property_inst_t* root) {
    int pass = root->status == LLG_PROP_TRUE;
    int vacuous = pass && !root->nonvacuous;
    property_inst_free(root);
    assertion_result(assertion, pass, vacuous);
}

static void property_unlink_attempt(llg_concurrent_assertion_t* assertion,
                                    llg_property_inst_t** link, llg_property_inst_t* previous) {
    llg_property_inst_t* attempt = *link;
    *link = attempt->next;
    if (assertion->property_attempts_tail == attempt)
        assertion->property_attempts_tail = previous;
}

static void free_property_attempts(llg_concurrent_assertion_t* assertion) {
    property_free_list(assertion->property_attempts);
    assertion->property_attempts = assertion->property_attempts_tail = NULL;
}

/* One leading-clock tick: step live attempts in start order, then begin a
 * new attempt (16.15). Each attempt reports one result (16.15.3). */
static void run_property_assertion(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event, int root_event) {
    llg_property_ctx_t ctx = {assertion, assertion->property, *event};
    g.sequence_current = assertion;
    llg_property_inst_t** link = &assertion->property_attempts;
    llg_property_inst_t* previous = NULL;
    while (*link) {
        llg_property_inst_t* attempt = *link;
        property_tick(&ctx, attempt);
        if (g.finish) return;
        if (!attempt->status) {
            previous = attempt;
            link = &attempt->next;
            continue;
        }
        property_unlink_attempt(assertion, link, previous);
        property_report(assertion, attempt);
        if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active) return;
        if (g.finish) return;
    }
    if (!root_event || !assertion_attempt_starts(assertion)) return;
    llg_property_inst_t* attempt =
        property_inst_new(assertion->property, assertion->property->root, 0, NULL);
    property_tick(&ctx, attempt);
    if (g.finish) {
        property_inst_free(attempt);
        return;
    }
    if (attempt->status) {
        property_report(assertion, attempt);
        return;
    }
    if (assertion->property_attempts_tail) assertion->property_attempts_tail->next = attempt;
    else assertion->property_attempts = attempt;
    assertion->property_attempts_tail = attempt;
}

/* Asynchronous accept_on/reject_on read sampled values, which are fixed for
 * a whole time step, so one check per step and assertion is exact. */
static void property_assertions_async(void) {
    if (!g.property_async_count) return;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion && !g.finish;
         assertion = assertion->next) {
        if (!assertion->property_async || !assertion->property_attempts ||
            assertion->property_async_checked == g.now + 1)
            continue;
        assertion->property_async_checked = g.now + 1;
        llg_property_ctx_t ctx = {assertion, assertion->property, {0}};
        llg_property_inst_t** link = &assertion->property_attempts;
        llg_property_inst_t* previous = NULL;
        while (*link && !g.finish) {
            llg_property_inst_t* attempt = *link;
            property_async(&ctx, attempt);
            if (!attempt->status) {
                previous = attempt;
                link = &attempt->next;
                continue;
            }
            property_unlink_attempt(assertion, link, previous);
            property_report(assertion, attempt);
            if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active) break;
        }
    }
}

/* At the end of simulation an attempt that is still pending is settled by
 * its finite-trace verdict: an unmet strong obligation is a failure, while
 * weak obligations leave the attempt incomplete with no result (Annex F
 * F.5.3.2; llg decision S38-D1). Returns whether any failure was reported. */
static int settle_property_attempts_at_end(void) {
    int reported = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (!assertion->property) continue;
        llg_property_inst_t* attempts = assertion->property_attempts;
        assertion->property_attempts = assertion->property_attempts_tail = NULL;
        while (attempts) {
            llg_property_inst_t* attempt = attempts;
            attempts = attempt->next;
            int verdict = property_end(attempt);
            property_inst_free(attempt);
            // A blocked expect never resumes after the end of simulation.
            if (!verdict && assertion->kind != LLG_ASSERTION_EXPECT &&
                assertion->kind != LLG_ASSERTION_COVER) {
                assertion_result(assertion, 0, 0);
                reported = 1;
            }
        }
    }
    return reported;
}

static int valid_property_program(const llg_property_program_t* program, sv4_t* clock,
                                  int edge) {
    if (!program || program->node_count == 0 || !program->nodes ||
        program->root >= program->node_count ||
        (program->sequence_count != 0 && !program->sequences) ||
        (program->atom_count != 0 && !program->atom))
        return 0;
    for (uint32_t index = 0; index < program->sequence_count; index++) {
        const llg_sequence_graph_t* graph = program->sequences[index];
        if (!valid_sequence_graph(graph, clock, edge)) return 0;
        // Property programs are single-clock (multiclock properties, 16.14,
        // are rejected by lowering): every edge advances on the leading clock.
        if (graph->leading_clock &&
            (graph->leading_clock != clock || graph->leading_edge != edge))
            return 0;
        for (uint32_t t = 0; t < graph->transition_count; t++) {
            const llg_sequence_transition_t* transition = &graph->transitions[t];
            if (transition->clock && (transition->clock != clock || transition->edge != edge))
                return 0;
        }
    }
    for (uint32_t index = 0; index < program->node_count; index++) {
        const llg_property_node_t* node = &program->nodes[index];
        uint32_t operands[3] = {LLG_PROPERTY_NONE, LLG_PROPERTY_NONE, LLG_PROPERTY_NONE};
        uint32_t atom = LLG_PROPERTY_NONE, sequence = LLG_PROPERTY_NONE;
        switch (node->kind) {
        case LLG_PROPERTY_BOOLEAN: atom = node->first; break;
        case LLG_PROPERTY_SEQUENCE: sequence = node->first; break;
        case LLG_PROPERTY_NOT:
        case LLG_PROPERTY_NEXTTIME:
        case LLG_PROPERTY_ALWAYS:
        case LLG_PROPERTY_EVENTUALLY: operands[0] = node->first; break;
        case LLG_PROPERTY_AND:
        case LLG_PROPERTY_OR:
        case LLG_PROPERTY_IMPLIES:
        case LLG_PROPERTY_IFF:
        case LLG_PROPERTY_UNTIL:
            operands[0] = node->first;
            operands[1] = node->second;
            break;
        case LLG_PROPERTY_IMPLICATION:
        case LLG_PROPERTY_FOLLOWED_BY:
            sequence = node->first;
            operands[0] = node->second;
            break;
        case LLG_PROPERTY_IF:
            atom = node->first;
            operands[0] = node->second;
            operands[1] = node->third;
            break;
        case LLG_PROPERTY_ABORT:
            atom = node->first;
            operands[0] = node->second;
            break;
        default: return 0;
        }
        if (node->kind == LLG_PROPERTY_ALWAYS || node->kind == LLG_PROPERTY_EVENTUALLY) {
            if (node->max < node->min) return 0;
        }
        // Operands precede their users, so evaluation cannot recurse forever.
        for (int i = 0; i < 3; i++)
            if (operands[i] != LLG_PROPERTY_NONE && operands[i] >= index) return 0;
        if ((node->kind == LLG_PROPERTY_NOT || node->kind == LLG_PROPERTY_NEXTTIME ||
             node->kind == LLG_PROPERTY_ALWAYS || node->kind == LLG_PROPERTY_EVENTUALLY ||
             node->kind == LLG_PROPERTY_ABORT || node->kind == LLG_PROPERTY_IF ||
             node->kind == LLG_PROPERTY_IMPLICATION || node->kind == LLG_PROPERTY_FOLLOWED_BY) &&
            operands[0] == LLG_PROPERTY_NONE)
            return 0;
        if (atom != LLG_PROPERTY_NONE && atom >= program->atom_count) return 0;
        if ((node->kind == LLG_PROPERTY_BOOLEAN || node->kind == LLG_PROPERTY_IF ||
             node->kind == LLG_PROPERTY_ABORT) && atom == LLG_PROPERTY_NONE)
            return 0;
        if ((node->kind == LLG_PROPERTY_SEQUENCE || node->kind == LLG_PROPERTY_IMPLICATION ||
             node->kind == LLG_PROPERTY_FOLLOWED_BY) &&
            sequence >= program->sequence_count)
            return 0;
        if (node->kind == LLG_PROPERTY_SEQUENCE && program->sequences[sequence]->admits_empty)
            return 0;
    }
    return 1;
}

int llg_assertion_register_property(
    sv4_t* clock, int edge, sv4_t* disable, const llg_property_program_t* program,
    const llg_co_desc_t* pass_desc, const llg_co_desc_t* fail_desc, void* data,
    int kind, uint64_t identity, const char* label, const char* location,
    const char* scope) {
    if (!g.initialized || g.running || g.config_error || !clock ||
        (edge != LLG_EV_POSEDGE && edge != LLG_EV_NEGEDGE) ||
        kind < LLG_ASSERTION_ASSERT || kind > LLG_ASSERTION_EXPECT ||
        !valid_property_program(program, clock, edge) ||
        (pass_desc && !pass_desc->fn) || (fail_desc && !fail_desc->fn)) {
        fprintf(stderr, "llg: invalid concurrent property assertion registration\n");
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    llg_concurrent_assertion_t* assertion =
        (llg_concurrent_assertion_t*)llg_checked_calloc(
            1, sizeof(*assertion), "concurrent property assertion");
    assertion->clock = clock;
    assertion->edge = edge;
    assertion->disable = disable;
    assertion->pass_desc = pass_desc;
    assertion->fail_desc = fail_desc;
    assertion->data = data;
    assertion->kind = kind;
    assertion->overlapped = 1;
    assertion->identity = identity;
    assertion->label = label;
    assertion->location = location;
    assertion->scope = scope;
    assertion->enabled = 1;
    assertion->property = program;
    if (program->sequence_count) {
        assertion->property_ranks = llg_checked_calloc(
            program->sequence_count, sizeof(*assertion->property_ranks), "property ranks");
        for (uint32_t index = 0; index < program->sequence_count; index++)
            assertion->property_ranks[index] = sequence_graph_rank(program->sequences[index]);
    }
    for (uint32_t index = 0; index < program->node_count; index++) {
        const llg_property_node_t* node = &program->nodes[index];
        if (node->kind == LLG_PROPERTY_ABORT && !(node->flags & LLG_PROPERTY_SYNC))
            assertion->property_async = 1;
    }
    if (assertion->property_async) g.property_async_count++;
    // Ticks of the leading clock number the property's clock events.
    clocking_edge_get(clock)->keep_ticks = 1;
    if (g.assertion_tail)
        g.assertion_tail->next = assertion;
    else
        g.assertions = assertion;
    g.assertion_tail = assertion;
    return 1;
}
