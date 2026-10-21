/* Property engine (IEEE 1800-2009 16.12-16.14, 16.15.8, Annex F).
 *
 * Each evaluation attempt is a tree of live node instances built from the
 * assertion's static program. Every clock event of the assertion is
 * delivered to the tree top-down: a node first advances its existing
 * children, then starts the children due at this event (which see it at
 * once), then settles its own result from its children. A node resolves
 * exactly once, to true or false, with the 16.15.8 nonvacuity of that
 * evaluation; resolving frees its subtree. Results are decided as soon as
 * they are determined, so for example an `and` fails when either operand
 * fails. Asynchronous accept_on/reject_on conditions are also checked between
 * ticks, once per time step, because their sampled values cannot change
 * within a step (16.13.14).
 *
 * Every node starts at a point, the event of its parent that started it
 * (llg_property_launch_t). Nodes with a clock of their own (Booleans,
 * nexttime, always, eventually, until and the if condition) first evaluate
 * at the nearest tick of that clock at or after the point, or strictly after
 * it after |=> (16.14.2, F.5.1 `T_s(b, c) = !c[*0:$] ##1 c & b`); a tick of
 * another clock in the same time step that was delivered earlier counts as
 * that nearest tick. The other operators start their operands at their own
 * point, and sequences align themselves through the sequence engine's
 * launch boundary. A recursive instance (CALL) starts its target node when
 * it is reached, so recursion unfolds one time advance at a time (16.13.17,
 * F.7), and operators whose remaining result is one operand's collapse into
 * that operand, so a recursion such as `p and (1 |=> r)` keeps a bounded
 * tree.
 *
 * Every live instance counts against LLG_SEQUENCE_THREAD_LIMIT, like a
 * sequence thread. */

enum { LLG_PROP_PENDING = 0, LLG_PROP_TRUE = 1, LLG_PROP_FALSE = 2 };

typedef struct {
    llg_concurrent_assertion_t* assertion;
    const llg_property_program_t* program;
    /* The clock event this step evaluates at. */
    llg_assertion_clock_event_t event;
    /* Order of the event being delivered: events of this time step with a
     * smaller order have been delivered already. */
    uint64_t bound;
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

static llg_property_launch_t property_launch_at(const llg_assertion_clock_event_t* event) {
    return (llg_property_launch_t){event->signal, event->edge, event->time, event->tick,
                                   event->order};
}

/* A recursive instance evaluates its target node (validated to reach a
 * node that is no CALL). */
static uint32_t property_call_target(const llg_property_program_t* program, uint32_t node) {
    while (program->nodes[node].kind == LLG_PROPERTY_CALL) node = program->nodes[node].first;
    return node;
}

static llg_property_inst_t* property_inst_new(const llg_property_program_t* program,
                                              uint32_t node, llg_property_launch_t launch,
                                              int strict, llg_property_locals_t* locals) {
    llg_property_inst_t* inst = g.property_inst_pool;
    if (inst) {
        g.property_inst_pool = inst->next;
        memset(inst, 0, sizeof(*inst));
    } else {
        inst = llg_checked_calloc(1, sizeof(*inst), "property evaluation node");
    }
    sequence_thread_acquire();
    inst->node = &program->nodes[property_call_target(program, node)];
    inst->launch = launch;
    inst->strict = strict ? 1 : 0;
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
    inst->nonvacuous = nonvacuous ||
                       (status == LLG_PROP_TRUE ? inst->carry_true : inst->carry_false);
    inst->cover_vacuous = 0;
    property_free_list(inst->children);
    inst->children = inst->children_tail = NULL;
    if (inst->sequence) {
        sequence_attempt_discard(inst->sequence);
        inst->sequence = NULL;
    }
}

/* Resolve `inst` with an outcome that keeps the cover vacuity of the operand
 * or condition it comes from (see llg_property_inst_t::cover_vacuous). */
static void property_pass_on(llg_property_inst_t* inst, int status, int nonvacuous,
                             int cover_vacuous) {
    property_resolve(inst, status, nonvacuous);
    inst->cover_vacuous = (uint8_t)(cover_vacuous && !inst->carry_hit);
}

/* Unlink and free `child`, whose predecessor link is `*link`. */
static void property_drop_child(llg_property_inst_t* inst, llg_property_inst_t** link,
                                llg_property_inst_t* previous) {
    llg_property_inst_t* child = *link;
    *link = child->next;
    if (inst->children_tail == child) inst->children_tail = previous;
    property_inst_free(child);
}

/* Replace `inst` by its pending operand `child` once the operator's result
 * is that operand's: `true and p`, `false or p`, an implication whose
 * antecedent is exhausted with one consequent left, a nexttime or if whose
 * operand has started. The operator's other operands only add nonvacuity
 * (16.15.8), kept per outcome; `hit` marks an operator whose result is
 * never cover-vacuous. The instance keeps its place among its siblings, so
 * no parent needs to know. */
static void property_collapse(llg_property_inst_t* inst, llg_property_inst_t* child,
                              int carry_true, int carry_false, int hit) {
    llg_property_inst_t** link = &inst->children;
    while (*link != child) link = &(*link)->next;
    *link = child->next;
    child->next = NULL;
    property_free_list(inst->children);
    if (inst->sequence) sequence_attempt_discard(inst->sequence);
    property_locals_release(inst->locals);
    llg_property_inst_t* next = inst->next;
    uint8_t keep_true = (uint8_t)(inst->carry_true | (carry_true ? 1 : 0));
    uint8_t keep_false = (uint8_t)(inst->carry_false | (carry_false ? 1 : 0));
    uint8_t keep_hit = (uint8_t)(inst->carry_hit | (hit ? 1 : 0));
    *inst = *child;
    inst->next = next;
    inst->carry_true |= keep_true;
    inst->carry_false |= keep_false;
    inst->carry_hit |= keep_hit;
    memset(child, 0, sizeof(*child));
    child->next = g.property_inst_pool;
    g.property_inst_pool = child;
    sequence_thread_release();
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

/* The clock of a node: the leading clock of a singly clocked program. */
static void property_node_clock(const llg_property_ctx_t* ctx, const llg_property_node_t* node,
                                sv4_t** clock, int* edge) {
    if (ctx->program->clock_count) {
        const llg_property_clock_t* entry = &ctx->program->clocks[node->clock];
        *clock = entry->signal;
        *edge = entry->edge;
    } else {
        *clock = ctx->assertion->clock;
        *edge = ctx->assertion->edge;
    }
}

/* Whether a tick of the node's own clock is at or after (strictly after)
 * its start point. */
static int property_reached(const llg_property_inst_t* inst, sv4_t* clock, int edge,
                            const llg_assertion_clock_event_t* event) {
    if (inst->launch.clock == clock && inst->launch.edge == edge)
        return inst->strict ? event->tick > inst->launch.tick : event->tick >= inst->launch.tick;
    return inst->strict ? event->time > inst->launch.time : event->time >= inst->launch.time;
}

/* The first tick of a node's own clock at or after its start point, if it
 * has occurred: the event being delivered, or a coincident tick of the own
 * clock already delivered in this time step for a node started by another
 * clock's event (one time step is one letter of Annex F). */
static const llg_assertion_clock_event_t* property_first_tick(const llg_property_ctx_t* ctx,
                                                              const llg_property_inst_t* inst,
                                                              sv4_t* clock, int edge) {
    const llg_assertion_clock_event_t* event = &ctx->event;
    if (event->signal == clock && event->edge == edge)
        return property_reached(inst, clock, edge, event) ? event : NULL;
    if (inst->strict || (inst->launch.clock == clock && inst->launch.edge == edge)) return NULL;
    for (const llg_assertion_clock_event_t* past = ctx->assertion->clock_history; past;
         past = past->next)
        if (past->signal == clock && past->edge == edge && past->time == inst->launch.time &&
            past->order < ctx->bound)
            return past;
    return NULL;
}

static llg_sequence_attempt_t* property_sequence_new(llg_property_ctx_t* ctx,
                                                     llg_property_inst_t* inst,
                                                     uint32_t index) {
    const llg_sequence_graph_t* graph = ctx->program->sequences[index];
    llg_property_locals_t* locals = inst->locals;
    llg_sequence_attempt_t* attempt = sequence_attempt_new(
        graph, 0, locals ? locals->graph : NULL, locals ? locals->values : NULL);
    attempt->rank = ctx->assertion->property_ranks[index];
    // The sequence starts at the node's start point: at its leading clock's
    // nearest tick at or after it, or strictly after it (16.14.1).
    attempt->launch_pending = 1;
    attempt->launch_strict = inst->strict;
    attempt->launch = (llg_sequence_endpoint_t){
        .clock = inst->launch.clock, .edge = inst->launch.edge, .time = inst->launch.time,
        .order = inst->launch.order, .tick = inst->launch.tick, .mult = 1};
    return attempt;
}

static int property_sequence_step(llg_property_ctx_t* ctx, llg_sequence_attempt_t* attempt) {
    const llg_assertion_clock_event_t* event = &ctx->event;
    int accepted = 0;
    return sequence_attempt_step(attempt, ctx->assertion, 0, event->signal, event->edge,
                                 event->time, event->order, event->tick, &accepted);
}

/* Start a child at `launch` and deliver the current event to it. */
static void property_spawn(llg_property_ctx_t* ctx, llg_property_inst_t* inst, uint32_t node,
                           llg_property_launch_t launch, int strict,
                           llg_property_locals_t* locals) {
    llg_property_inst_t* child = property_inst_new(ctx->program, node, launch, strict, locals);
    if (inst->children_tail) inst->children_tail->next = child;
    else inst->children = child;
    inst->children_tail = child;
    property_tick(ctx, child);
}

/* Start a child at the tick being evaluated. */
static void property_spawn_here(llg_property_ctx_t* ctx, llg_property_inst_t* inst,
                                uint32_t node) {
    property_spawn(ctx, inst, node, property_launch_at(&ctx->event), 0, inst->locals);
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

/* Whether an abort condition is checked at the event being delivered:
 * accept_on/reject_on in every time step from its start point (the next
 * time step after |=>), sync_accept_on/sync_reject_on at the ticks of its
 * clock from that point (F.3.4.3.7 `accept_on(b && c)`). */
static int property_abort_due(const llg_property_ctx_t* ctx, const llg_property_inst_t* inst) {
    const llg_property_node_t* node = inst->node;
    if (node->flags & LLG_PROPERTY_SYNC) {
        sv4_t* clock;
        int edge;
        property_node_clock(ctx, node, &clock, &edge);
        return ctx->event.signal == clock && ctx->event.edge == edge &&
               property_reached(inst, clock, edge, &ctx->event);
    }
    return inst->abort_checked != g.now + 1 && (!inst->strict || g.now > inst->launch.time);
}

/* Step an implication or followed-by antecedent and start one consequent per
 * nonempty match, at the match (|->, #-#) or strictly after it (|=>, #=#).
 * An empty match lies before the start point, so the nonoverlapped forms
 * start its consequent there and the overlapped ones ignore it (16.13.6,
 * 16.13.22). */
static void property_antecedent_step(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    const llg_property_node_t* node = inst->node;
    int alive = property_sequence_step(ctx, inst->sequence);
    if (g.finish) return;
    if (inst->sequence->started) inst->begun = 1;
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
        llg_property_launch_t at = {endpoint->clock, endpoint->edge, endpoint->time,
                                    endpoint->tick, endpoint->order};
        property_spawn(ctx, inst, node->second, at, !(overlapped || endpoint->empty), locals);
        property_locals_release(locals);
    }
    if (!alive && !g.finish) {
        inst->done = 1;
        sequence_attempt_discard(inst->sequence);
        inst->sequence = NULL;
    }
}

/* One tick of a node's own clock. */
static void property_clock_tick(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    const llg_property_node_t* node = inst->node;
    uint64_t tick = inst->ticks++;
    switch (node->kind) {
    case LLG_PROPERTY_BOOLEAN:
        property_resolve(inst, property_atom(ctx, node->first) ? LLG_PROP_TRUE : LLG_PROP_FALSE,
                         1);
        return;
    case LLG_PROPERTY_NEXTTIME:
        property_tick_children(ctx, inst);
        if (tick == node->min && !g.finish) property_spawn_here(ctx, inst, node->first);
        break;
    case LLG_PROPERTY_ALWAYS:
    case LLG_PROPERTY_EVENTUALLY:
        property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) {
            if (tick >= node->min) property_spawn_here(ctx, inst, node->first);
            if (node->max != LLG_SEQUENCE_UNBOUNDED && tick >= node->max) inst->done = 1;
        }
        break;
    case LLG_PROPERTY_UNTIL:
        // Stage k evaluates both operands from tick k; stages are children
        // in pairs (left, right).
        property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) {
            property_spawn_here(ctx, inst, node->first);
            if (!g.finish) property_spawn_here(ctx, inst, node->second);
        }
        break;
    default:
        break;
    }
    if (!g.finish) property_settle(inst);
}

static int property_clocked_kind(uint32_t kind) {
    return kind == LLG_PROPERTY_BOOLEAN || kind == LLG_PROPERTY_NEXTTIME ||
           kind == LLG_PROPERTY_ALWAYS || kind == LLG_PROPERTY_EVENTUALLY ||
           kind == LLG_PROPERTY_UNTIL;
}

/* `if (b) p else q`: b is sampled at the if's first tick, and the branch
 * it selects starts at that tick; a differently clocked branch first
 * evaluates at its own clock's nearest tick at or after it (decision
 * A13-D3, which follows the industry-standard behaviour rather than the
 * F.3.4.3.4 rewrite `(b |-> p) and (weak(b) or q)`, whose q would start
 * with the if). */
static void property_if_step(llg_property_ctx_t* ctx, llg_property_inst_t* inst, int start) {
    const llg_property_node_t* node = inst->node;
    if (!start) property_tick_children(ctx, inst);
    if (inst->phase || inst->status || g.finish) return;
    sv4_t* clock;
    int edge;
    property_node_clock(ctx, node, &clock, &edge);
    const llg_assertion_clock_event_t* first = property_first_tick(ctx, inst, clock, edge);
    if (!first) return;
    llg_property_ctx_t at = *ctx;
    at.event = *first;
    uint32_t branch;
    if (property_atom(&at, node->first)) {
        inst->phase = 1;
        branch = node->second;
    } else if (node->third != LLG_PROPERTY_NONE) {
        inst->phase = 2;
        branch = node->third;
    } else {
        // `if (b) p` with b false holds vacuously (16.13.5, 16.15.8 g).
        property_pass_on(inst, LLG_PROP_TRUE, 0, 1);
        return;
    }
    if (g.finish) return;
    property_spawn(&at, inst, branch, property_launch_at(first), 0, inst->locals);
    // A branch started at an earlier coincident tick also sees this event.
    if (first != &ctx->event && inst->children && !g.finish) property_tick(ctx, inst->children);
}

static void property_tick(llg_property_ctx_t* ctx, llg_property_inst_t* inst) {
    if (inst->status || g.finish) return;
    const llg_property_node_t* node = inst->node;
    if (property_clocked_kind(node->kind)) {
        sv4_t* clock;
        int edge;
        property_node_clock(ctx, node, &clock, &edge);
        if (!inst->ticks) {
            const llg_assertion_clock_event_t* first = property_first_tick(ctx, inst, clock, edge);
            if (!first) return;
            if (first != &ctx->event) {
                // The first tick was an earlier event of this time step:
                // evaluate it there, then deliver this event as well.
                llg_property_ctx_t at = *ctx;
                at.event = *first;
                property_clock_tick(&at, inst);
                if (!g.finish) property_tick(ctx, inst);
                return;
            }
        } else if (ctx->event.signal != clock || ctx->event.edge != edge) {
            property_tick_children(ctx, inst);
            if (!g.finish) property_settle(inst);
            return;
        }
        property_clock_tick(ctx, inst);
        return;
    }
    int start = inst->ticks == 0;
    inst->ticks = 1;
    switch (node->kind) {
    case LLG_PROPERTY_SEQUENCE: {
        // A sequence property holds at its first nonempty match (16.13.1) and
        // fails once no thread can match any more.
        if (start) inst->sequence = property_sequence_new(ctx, inst, node->first);
        int alive = property_sequence_step(ctx, inst->sequence);
        if (g.finish) return;
        if (inst->sequence->started) inst->begun = 1;
        if (inst->sequence->matched) property_resolve(inst, LLG_PROP_TRUE, 1);
        else if (!alive) property_resolve(inst, LLG_PROP_FALSE, 1);
        return;
    }
    case LLG_PROPERTY_ABORT:
        // The outer condition is checked before the operand is stepped, so
        // an abort takes precedence over an evaluation ending in the same
        // step and the outermost of nested aborts wins (16.13.14).
        if (property_abort_due(ctx, inst) && property_abort_fires(ctx, inst)) return;
        if (start) property_spawn(ctx, inst, node->second, inst->launch, inst->strict, inst->locals);
        else property_tick_children(ctx, inst);
        break;
    case LLG_PROPERTY_IMPLICATION:
    case LLG_PROPERTY_FOLLOWED_BY:
        if (start) inst->sequence = property_sequence_new(ctx, inst, node->first);
        else property_tick_children(ctx, inst);
        if (!inst->done && !g.finish) property_antecedent_step(ctx, inst);
        break;
    case LLG_PROPERTY_IF:
        property_if_step(ctx, inst, start);
        if (inst->status) return;
        break;
    default:
        // NOT and the binary connectives start their operands together.
        if (start) {
            property_spawn(ctx, inst, node->first, inst->launch, inst->strict, inst->locals);
            if (node->kind != LLG_PROPERTY_NOT && !g.finish)
                property_spawn(ctx, inst, node->second, inst->launch, inst->strict, inst->locals);
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
        else if (ls == LLG_PROP_TRUE)
            property_collapse(inst, right, lv, lv, 1);
        else if (rs == LLG_PROP_TRUE)
            property_collapse(inst, left, rv, rv, 1);
        break;
    case LLG_PROPERTY_OR:
        if (ls == LLG_PROP_TRUE || rs == LLG_PROP_TRUE)
            property_resolve(inst, LLG_PROP_TRUE, lv || rv);
        else if (ls == LLG_PROP_FALSE && rs == LLG_PROP_FALSE)
            property_resolve(inst, LLG_PROP_FALSE, lv || rv);
        else if (ls == LLG_PROP_FALSE)
            property_collapse(inst, right, lv, lv, 1);
        else if (rs == LLG_PROP_FALSE)
            property_collapse(inst, left, rv, rv, 1);
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
    // An implication passes on the cover vacuity of its consequents (none:
    // no antecedent match); the other quantifiers never yield one.
    int implication = inst->node->kind == LLG_PROPERTY_IMPLICATION;
    while (*link) {
        llg_property_inst_t* child = *link;
        if (child->status == decisive) {
            property_pass_on(inst, decisive, child->nonvacuous,
                             implication && child->cover_vacuous);
            return;
        }
        if (child->status) {
            inst->accumulated |= child->nonvacuous;
            inst->consumed_hit |= !child->cover_vacuous;
            property_drop_child(inst, link, previous);
            continue;
        }
        previous = child;
        link = &child->next;
    }
    if (inst->done && !inst->children)
        property_pass_on(inst, universal ? LLG_PROP_TRUE : LLG_PROP_FALSE, inst->accumulated,
                         implication && !inst->consumed_hit);
    else if (inst->done && implication && inst->children == inst->children_tail)
        property_collapse(inst, inst->children, inst->accumulated, 0, inst->consumed_hit);
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
            property_pass_on(inst,
                             child->status == LLG_PROP_TRUE ? LLG_PROP_FALSE : LLG_PROP_TRUE,
                             child->nonvacuous, child->cover_vacuous);
        break;
    case LLG_PROPERTY_ABORT:
        if (child && child->status)
            property_pass_on(inst, child->status, child->nonvacuous, child->cover_vacuous);
        break;
    case LLG_PROPERTY_IF: {
        if (!inst->phase || !child) break;
        // An if-else is a choice between two properties, so its result is
        // never cover-vacuous; an else-less if passes its branch's on.
        int has_else = inst->node->third != LLG_PROPERTY_NONE;
        if (child->status)
            property_pass_on(inst, child->status, child->nonvacuous,
                             !has_else && child->cover_vacuous);
        else
            property_collapse(inst, child, 0, 0, has_else);
        break;
    }
    case LLG_PROPERTY_NEXTTIME:
        if (!child) break;
        if (child->status)
            property_pass_on(inst, child->status, child->nonvacuous, child->cover_vacuous);
        else
            property_collapse(inst, child, 0, 0, 0);
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
        property_abort_due(ctx, inst) && property_abort_fires(ctx, inst))
        return;
    for (llg_property_inst_t* child = inst->children; child && !g.finish; child = child->next)
        property_async(ctx, child);
    if (!g.finish) property_settle(inst);
}

/* Whether an evaluation has begun: a node with a clock of its own has had
 * its first tick, a sequence has started, an if has sampled its condition,
 * and an operator has an operand that began. */
static int property_begun(const llg_property_inst_t* inst) {
    switch (inst->node->kind) {
    case LLG_PROPERTY_BOOLEAN:
    case LLG_PROPERTY_NEXTTIME:
    case LLG_PROPERTY_ALWAYS:
    case LLG_PROPERTY_EVENTUALLY:
    case LLG_PROPERTY_UNTIL:
        return inst->ticks != 0;
    case LLG_PROPERTY_SEQUENCE:
    case LLG_PROPERTY_IMPLICATION:
    case LLG_PROPERTY_FOLLOWED_BY:
        return inst->begun;
    case LLG_PROPERTY_IF:
        return inst->phase != 0;
    default:
        for (const llg_property_inst_t* child = inst->children; child; child = child->next)
            if (property_begun(child)) return 1;
        return 0;
    }
}

/* Which nodes of a program contain an `s_eventually` obligation, through
 * any operand or recursive instance (operands precede their users, so one
 * forward pass is exact unless a CALL names a later node). */
static uint8_t* property_eventuality_table(const llg_property_program_t* program) {
    uint8_t* has = llg_checked_calloc(program->node_count, 1, "property eventuality table");
    for (int changed = 1; changed;) {
        changed = 0;
        for (uint32_t index = 0; index < program->node_count; index++) {
            const llg_property_node_t* node = &program->nodes[index];
            uint32_t operands[2] = {LLG_PROPERTY_NONE, LLG_PROPERTY_NONE};
            switch (node->kind) {
            case LLG_PROPERTY_NOT:
            case LLG_PROPERTY_NEXTTIME:
            case LLG_PROPERTY_ALWAYS:
            case LLG_PROPERTY_EVENTUALLY:
            case LLG_PROPERTY_CALL: operands[0] = node->first; break;
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
            case LLG_PROPERTY_ABORT: operands[0] = node->second; break;
            case LLG_PROPERTY_IF:
                operands[0] = node->second;
                operands[1] = node->third;
                break;
            default: break;
            }
            uint8_t value = node->kind == LLG_PROPERTY_EVENTUALLY &&
                            (node->flags & LLG_PROPERTY_STRONG) != 0;
            for (int i = 0; i < 2 && !value; i++)
                value = operands[i] != LLG_PROPERTY_NONE && has[operands[i]];
            if (value && !has[index]) {
                has[index] = 1;
                changed = 1;
            }
        }
    }
    return has;
}

/* Whether a pending evaluation fails when the simulation ends here: it
 * waits on an `s_eventually` obligation, one already started or one its
 * started operators will still start (a nexttime, always, eventually or
 * until operand, or a node that has not begun). Every other pending
 * obligation, strong ones included, ends without a result, and an
 * implication or followed-by whose antecedent has not matched imposes
 * nothing (decisions S38-D1, A13-D7). */
static int property_end_fails(const llg_property_inst_t* inst,
                              const llg_property_node_t* nodes, const uint8_t* eventuality) {
    if (inst->status) return 0;
    const llg_property_node_t* node = inst->node;
    size_t index = (size_t)(node - nodes);
    if (!property_begun(inst)) return eventuality[index];
    if (node->kind == LLG_PROPERTY_EVENTUALLY && (node->flags & LLG_PROPERTY_STRONG)) return 1;
    for (const llg_property_inst_t* child = inst->children; child; child = child->next)
        if (property_end_fails(child, nodes, eventuality)) return 1;
    switch (node->kind) {
    case LLG_PROPERTY_NEXTTIME:
        return !inst->children && eventuality[node->first];
    case LLG_PROPERTY_ALWAYS:
    case LLG_PROPERTY_EVENTUALLY:
        return !inst->done && eventuality[node->first];
    case LLG_PROPERTY_UNTIL:
        return !inst->done && (eventuality[node->first] || eventuality[node->second]);
    default:
        return 0;
    }
}

/* A cover counts a success as a hit unless it is cover-vacuous (decision
 * S38-D10); assert and assume keep the 16.15.8 vacuity. */
static void property_report(llg_concurrent_assertion_t* assertion, llg_property_inst_t* root) {
    int pass = root->status == LLG_PROP_TRUE;
    int vacuous = pass && (assertion->kind == LLG_ASSERTION_COVER ? root->cover_vacuous
                                                                  : !root->nonvacuous);
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

/* One clock event: step live attempts in start order, then, at a leading
 * clock tick, begin a new attempt (16.15). Each attempt reports one result
 * (16.15.3). */
static void run_property_assertion(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event, int root_event) {
    llg_property_ctx_t ctx = {assertion, assertion->property, *event, event->order};
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
    for (uint64_t starts = root_event ? assertion_root_starts(assertion) : 0; starts;
         starts--) {
        property_attempt_begin(assertion, event);
        if (g.finish) return;
        if (assertion->kind == LLG_ASSERTION_EXPECT && !assertion->expect_active) return;
    }
}

/* Begin one attempt of a property assertion at leading-clock `event`. */
static void property_attempt_begin(llg_concurrent_assertion_t* assertion,
                                   const llg_assertion_clock_event_t* event) {
    llg_property_ctx_t ctx = {assertion, assertion->property, *event, event->order};
    g.sequence_current = assertion;
    llg_property_inst_t* attempt = property_inst_new(
        assertion->property, assertion->property->root, property_launch_at(event), 0, NULL);
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
        llg_property_ctx_t ctx = {assertion, assertion->property, {0}, 0};
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

/* At the end of simulation an attempt that is still pending fails if it
 * waits on an `s_eventually` obligation and otherwise ends without a result
 * (16.13.21 calls both "Pending"; decisions S38-D1, A13-D7). Covers and
 * expects are not settled. Returns whether any failure was reported. */
static int settle_property_attempts_at_end(void) {
    int reported = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (!assertion->property) continue;
        llg_property_inst_t* attempts = assertion->property_attempts;
        assertion->property_attempts = assertion->property_attempts_tail = NULL;
        // A blocked expect never resumes after the end of simulation.
        int settled = attempts && assertion->kind != LLG_ASSERTION_EXPECT &&
                      assertion->kind != LLG_ASSERTION_COVER;
        uint8_t* eventuality = settled ? property_eventuality_table(assertion->property) : NULL;
        while (attempts) {
            llg_property_inst_t* attempt = attempts;
            attempts = attempt->next;
            int fails = settled &&
                        property_end_fails(attempt, assertion->property->nodes, eventuality);
            property_inst_free(attempt);
            if (fails) {
                assertion_result(assertion, 0, 0);
                reported = 1;
            }
        }
        free(eventuality);
    }
    return reported;
}

static int valid_property_clock(const llg_property_program_t* program, sv4_t* signal, int edge) {
    for (uint32_t index = 0; index < program->clock_count; index++)
        if (program->clocks[index].signal == signal && program->clocks[index].edge == edge)
            return 1;
    return 0;
}

static int valid_property_program(const llg_property_program_t* program, sv4_t* clock,
                                  int edge) {
    if (!program || program->node_count == 0 || !program->nodes ||
        program->root >= program->node_count ||
        (program->sequence_count != 0 && !program->sequences) ||
        (program->atom_count != 0 && !program->atom) ||
        (program->clock_count != 0 && !program->clocks))
        return 0;
    // A multiclocked program lists every clock its nodes and sequences use,
    // the leading clock first; a singly clocked one uses the leading clock.
    if (program->clock_count &&
        (program->clocks[0].signal != clock || program->clocks[0].edge != edge))
        return 0;
    for (uint32_t index = 0; index < program->clock_count; index++) {
        const llg_property_clock_t* entry = &program->clocks[index];
        if (!entry->signal || (entry->edge != LLG_EV_POSEDGE && entry->edge != LLG_EV_NEGEDGE))
            return 0;
    }
    for (uint32_t index = 0; index < program->sequence_count; index++) {
        const llg_sequence_graph_t* graph = program->sequences[index];
        if (!valid_sequence_graph(graph, clock, edge)) return 0;
        if (graph->leading_clock &&
            (program->clock_count
                 ? !valid_property_clock(program, graph->leading_clock, graph->leading_edge)
                 : graph->leading_clock != clock || graph->leading_edge != edge))
            return 0;
        for (uint32_t t = 0; t < graph->transition_count; t++) {
            const llg_sequence_transition_t* transition = &graph->transitions[t];
            if (transition->clock &&
                (program->clock_count
                     ? !valid_property_clock(program, transition->clock, transition->edge)
                     : transition->clock != clock || transition->edge != edge))
                return 0;
        }
    }
    for (uint32_t index = 0; index < program->node_count; index++) {
        const llg_property_node_t* node = &program->nodes[index];
        uint32_t operands[3] = {LLG_PROPERTY_NONE, LLG_PROPERTY_NONE, LLG_PROPERTY_NONE};
        uint32_t atom = LLG_PROPERTY_NONE, sequence = LLG_PROPERTY_NONE;
        if (program->clock_count ? node->clock >= program->clock_count : node->clock != 0)
            return 0;
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
        case LLG_PROPERTY_CALL: {
            // A recursive instance may name any node, but a chain of CALLs
            // must reach an operator, so starting one cannot loop forever.
            uint32_t target = node->first;
            for (uint32_t step = 0;; step++) {
                if (target >= program->node_count || step >= program->node_count) return 0;
                if (program->nodes[target].kind != LLG_PROPERTY_CALL) break;
                target = program->nodes[target].first;
            }
            continue;
        }
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
    assertion->acov =
        assertion->kind == LLG_ASSERTION_EXPECT ? NULL : llg_acov_find(identity);
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
    // Ticks of every clock number the property's clock events.
    clocking_edge_get(clock)->keep_ticks = 1;
    for (uint32_t index = 1; index < program->clock_count; index++)
        assertion_add_extra_clock(assertion, program->clocks[index].signal,
                                  program->clocks[index].edge);
    if (g.assertion_tail)
        g.assertion_tail->next = assertion;
    else
        g.assertions = assertion;
    g.assertion_tail = assertion;
    return 1;
}
