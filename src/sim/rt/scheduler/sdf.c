// ── SDF back-annotation (IEEE 1800-2009 32) ──────────────────────────────────
//
// The model generator reads every SDF file; an annotation is the valued list
// of operations one `$sdf_annotate` call (or `--sdf` file) applies. Path,
// check and gate tables are the model's writable storage, read whenever an
// event is scheduled or checked, so pending events keep their times and
// later ones use the new values.

// Pulse-limit tables allocated for paths that had none (their limits were
// the global percentages of the delay); released at cleanup.
static struct {
    uint64_t** tables;
    llg_module_path_t** paths;
    uint32_t count;
    uint32_t capacity;
} sdf_limit_tables;

// `value` added to `current` (INCREMENT) or replacing it, never below 0.
static uint64_t sdf_value(uint64_t current, int64_t value, int increment) {
    if (!increment) return value < 0 ? 0u : (uint64_t)value;
    if (value < 0) {
        uint64_t decrease = (uint64_t)(-(value + 1)) + 1u;
        return decrease >= current ? 0u : current - decrease;
    }
    return (uint64_t)value > UINT64_MAX - current ? UINT64_MAX : current + (uint64_t)value;
}

// A truncated percentage of a delay, as the path stage computes it.
static uint64_t sdf_percent(uint64_t delay, uint32_t percent) {
    return delay / 100u * percent + delay % 100u * percent / 100u;
}

// `millionths` millionths of a percent of `delay` (PATHPULSEPERCENT),
// truncated; at most the delay.
static uint64_t sdf_millionths(uint64_t delay, int64_t millionths) {
    if (millionths <= 0) return 0u;
    if (millionths >= 100000000) return delay;
    uint64_t share = (uint64_t)millionths;
    return delay / 100000000u * share + delay % 100000000u * share / 100000000u;
}

// The path's pulse-limit table, created from the global percentages of its
// current delays when it has none.
static uint64_t* sdf_path_limits(llg_module_path_t* path) {
    if (path->limits) return path->limits;
    uint64_t* limits =
        llg_checked_calloc(2u * LLG_PATH_TRANSITIONS, sizeof(*limits), "SDF pulse limits");
    for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
        limits[t] = sdf_percent(path->delays[t], path_pulse.reject_percent);
        limits[LLG_PATH_TRANSITIONS + t] = sdf_percent(path->delays[t], path_pulse.error_percent);
    }
    if (sdf_limit_tables.count == sdf_limit_tables.capacity) {
        uint32_t capacity = sdf_limit_tables.capacity ? sdf_limit_tables.capacity * 2u : 16u;
        uint64_t** tables = realloc(sdf_limit_tables.tables, capacity * sizeof(*tables));
        llg_module_path_t** paths =
            tables ? realloc(sdf_limit_tables.paths, capacity * sizeof(*paths)) : NULL;
        if (!tables || !paths) {
            fprintf(stderr, "llg: fatal: out of memory for SDF pulse limits\n");
            abort();
        }
        sdf_limit_tables.tables = tables;
        sdf_limit_tables.paths = paths;
        sdf_limit_tables.capacity = capacity;
    }
    sdf_limit_tables.tables[sdf_limit_tables.count] = limits;
    sdf_limit_tables.paths[sdf_limit_tables.count] = path;
    sdf_limit_tables.count++;
    path->limits = limits;
    return limits;
}

static void sdf_path(const llg_sdf_op_t* op, const int64_t* values) {
    llg_module_path_t* path = op->path;
    int increment = (op->flags & LLG_SDF_INCREMENT) != 0;
    for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
        if (op->delay >> t & 1u) path->delays[t] = sdf_value(path->delays[t], *values++, increment);
    }
    if (op->reject | op->error) {
        uint64_t* limits = sdf_path_limits(path);
        for (int error = 0; error < 2; error++) {
            uint16_t mask = error ? op->error : op->reject;
            uint64_t* row = limits + (error ? LLG_PATH_TRANSITIONS : 0u);
            for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
                if (!(mask >> t & 1u)) continue;
                int64_t value = *values++;
                row[t] = (op->flags & LLG_SDF_PERCENT)
                             ? sdf_millionths(path->delays[t], value)
                             : sdf_value(row[t], value, increment);
            }
        }
    }
    if (!path->limits) return;
    for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
        if (op->derive >> t & 1u) {
            path->limits[t] = sdf_percent(path->delays[t], path_pulse.reject_percent);
            path->limits[LLG_PATH_TRANSITIONS + t] =
                sdf_percent(path->delays[t], path_pulse.error_percent);
        }
        // Limits above the delay act as the delay (32.7).
        if ((op->delay | op->reject | op->error | op->derive) >> t & 1u) {
            if (path->limits[t] > path->delays[t]) path->limits[t] = path->delays[t];
            if (path->limits[LLG_PATH_TRANSITIONS + t] > path->delays[t])
                path->limits[LLG_PATH_TRANSITIONS + t] = path->delays[t];
        }
    }
}

// An interconnect op: the delays of its source entry (every entry for
// entry 0), then the load path's delays by the op's policy over the
// annotated entries; the op's pulse limits apply to the path as for an
// IOPATH.
static void sdf_load(const llg_sdf_op_t* op, const int64_t* values) {
    llg_sdf_load_t* load = op->load;
    int increment = (op->flags & LLG_SDF_INCREMENT) != 0;
    uint32_t first = op->index ? op->index : 0u;
    uint32_t last = op->index ? op->index : load->n_entries - 1u;
    for (uint32_t entry = first; entry <= last && entry < load->n_entries; entry++) {
        uint64_t* delays = load->entries + (size_t)entry * LLG_PATH_TRANSITIONS;
        const int64_t* value = values;
        for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
            if (op->delay >> t & 1u) delays[t] = sdf_value(delays[t], *value++, increment);
        }
        load->annotated[entry] = 1u;
    }
    unsigned policy = (op->flags >> LLG_SDF_MIPD_SHIFT) & 3u;
    llg_module_path_t* path = load->path;
    for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
        if (!(op->delay >> t & 1u)) continue;
        if (policy == 3u) {
            path->delays[t] = load->entries[(size_t)first * LLG_PATH_TRANSITIONS + t];
            continue;
        }
        uint64_t combined = 0, sum = 0, count = 0;
        for (uint32_t entry = 0; entry < load->n_entries; entry++) {
            if (!load->annotated[entry]) continue;
            uint64_t delay = load->entries[(size_t)entry * LLG_PATH_TRANSITIONS + t];
            if (!count || (policy == 0u && delay > combined) || (policy == 1u && delay < combined))
                combined = delay;
            sum += delay;
            count++;
        }
        path->delays[t] = policy == 2u && count ? sum / count : combined;
    }
    // Limits and derived limits as for a path, without changing its delays.
    llg_sdf_op_t limits = *op;
    limits.delay = 0;
    limits.path = path;
    uint32_t delays = 0;
    for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) delays += op->delay >> t & 1u;
    sdf_path(&limits, values + delays);
    if (path->limits) {
        for (uint32_t t = 0; t < LLG_PATH_TRANSITIONS; t++) {
            if (!(op->delay >> t & 1u)) continue;
            if (path->limits[t] > path->delays[t]) path->limits[t] = path->delays[t];
            if (path->limits[LLG_PATH_TRANSITIONS + t] > path->delays[t])
                path->limits[LLG_PATH_TRANSITIONS + t] = path->delays[t];
        }
    }
}

static void sdf_gate(const llg_sdf_op_t* op, const int64_t* values) {
    int increment = (op->flags & LLG_SDF_INCREMENT) != 0;
    for (uint32_t index = 0; index < 3u; index++) {
        if (op->delay >> index & 1u)
            op->gate[index] = sdf_value(op->gate[index], *values++, increment);
    }
}

// ── Combined checks resettled at run time (31.3.3, 31.3.6, 31.9.1) ─────────
// The same settlement and delay solve lowering applies to the declared
// limits (codegen timing_checks.rs `settle_combined_limits`,
// `solve_signal_delays`), applied to the annotated limits of every pair of
// the set.

typedef struct {
    uint32_t head;
    uint32_t tail;
    int64_t weight;
    uint32_t pair;
} sdf_constraint_t;

// Edge classes an event touches: bit 0 rise (01, 0x, x1), bit 1 fall.
static unsigned sdf_edge_classes(uint8_t edges) {
    unsigned classes = 0;
    if (!edges || (edges & LLG_TIMING_EDGE_RISING)) classes |= 1u;
    if (!edges || (edges & (LLG_TIMING_EDGE_10 | LLG_TIMING_EDGE_1X | LLG_TIMING_EDGE_X0)))
        classes |= 2u;
    return classes;
}

static void sdf_pairs_resolve(llg_timing_check_set_t* set) {
    uint32_t n = set->n_pairs;
    uint32_t nodes = 2u * set->n_terminals;
    int64_t* before = llg_checked_calloc(n, sizeof(*before), "SDF timing pairs");
    int64_t* after = llg_checked_calloc(n, sizeof(*after), "SDF timing pairs");
    sdf_constraint_t* edges =
        llg_checked_calloc((size_t)n * 8u, sizeof(*edges), "SDF timing constraints");
    int64_t* distance = llg_checked_calloc(nodes, sizeof(*distance), "SDF timing delays");
    uint32_t* predecessor = llg_checked_calloc(nodes, sizeof(*predecessor), "SDF timing delays");
    uint8_t* touched = llg_checked_calloc(nodes, sizeof(*touched), "SDF timing delays");
    // A negative sum sets the negative limits to 0.
    for (uint32_t index = 0; index < n; index++) {
        int64_t b = set->pairs[index].before, a = set->pairs[index].after;
        if ((b < 0 || a < 0) && b + a < 0) {
            b = b < 0 ? 0 : b;
            a = a < 0 ? 0 : a;
        }
        before[index] = b;
        after[index] = a;
    }
    uint32_t count = 0;
    int solved = 0;
    for (;;) {
        count = 0;
        for (uint32_t index = 0; index < n; index++) {
            const llg_timing_pair_t* pair = &set->pairs[index];
            int64_t margin = before[index] + after[index] >= 2 ? 1 : 0;
            int64_t a = before[index] != 0 ? margin : 0;
            int64_t b = after[index] != 0 ? margin : 0;
            unsigned references = sdf_edge_classes(pair->reference_edges);
            unsigned datas = sdf_edge_classes(pair->data_edges);
            for (uint32_t rc = 0; rc < 2u; rc++) {
                if (!(references >> rc & 1u)) continue;
                for (uint32_t dc = 0; dc < 2u; dc++) {
                    if (!(datas >> dc & 1u)) continue;
                    uint32_t r = 2u * pair->reference + rc, d = 2u * pair->data + dc;
                    edges[count++] = (sdf_constraint_t){r, d, a - before[index], index};
                    edges[count++] = (sdf_constraint_t){d, r, b - after[index], index};
                }
            }
        }
        memset(distance, 0, nodes * sizeof(*distance));
        for (uint32_t node = 0; node < nodes; node++) predecessor[node] = UINT32_MAX;
        uint32_t relaxed = UINT32_MAX;
        for (uint32_t round = 0; round <= nodes; round++) {
            relaxed = UINT32_MAX;
            for (uint32_t edge = 0; edge < count; edge++) {
                const sdf_constraint_t* c = &edges[edge];
                if (distance[c->tail] + c->weight > distance[c->head]) {
                    distance[c->head] = distance[c->tail] + c->weight;
                    predecessor[c->head] = edge;
                    relaxed = c->head;
                }
            }
            if (relaxed == UINT32_MAX) break;
        }
        if (relaxed == UINT32_MAX) {
            solved = 1;
            break;
        }
        // Walk back onto the positive cycle; its most negative limit
        // becomes 0 and the delays are solved again.
        uint32_t at = relaxed;
        for (uint32_t step = 0; step < nodes; step++) {
            if (predecessor[at] != UINT32_MAX) at = edges[predecessor[at]].tail;
        }
        uint32_t start = at, chosen = UINT32_MAX, steps = 0;
        while (predecessor[at] != UINT32_MAX) {
            uint32_t pair = edges[predecessor[at]].pair;
            if (before[pair] < 0 || after[pair] < 0) {
                int64_t key = before[pair] < after[pair] ? before[pair] : after[pair];
                int64_t best = chosen == UINT32_MAX
                                   ? 0
                                   : (before[chosen] < after[chosen] ? before[chosen]
                                                                     : after[chosen]);
                if (chosen == UINT32_MAX || key < best || (key == best && pair < chosen))
                    chosen = pair;
            }
            at = edges[predecessor[at]].tail;
            if (at == start || ++steps > count) break;
        }
        if (chosen == UINT32_MAX) break;
        if (before[chosen] <= after[chosen])
            before[chosen] = 0;
        else
            after[chosen] = 0;
    }
    for (uint32_t edge = 0; solved && edge < count; edge++) {
        touched[edges[edge].head] = 1;
        touched[edges[edge].tail] = 1;
    }
    for (uint32_t index = 0; index < set->n_lines; index++) {
        llg_timing_line_t* line = &set->lines[index];
        uint32_t rise_node = 2u * line->source, fall_node = rise_node + 1u;
        int64_t rise = 0, fall = 0;
        if (solved && fall_node < nodes) {
            rise = distance[rise_node];
            fall = distance[fall_node];
            if (!touched[rise_node]) rise = fall;
            if (!touched[fall_node]) fall = rise;
        }
        line->rise = rise < 0 ? 0u : (uint64_t)rise;
        line->fall = fall < 0 ? 0u : (uint64_t)fall;
    }
    for (uint32_t index = 0; index < n; index++) {
        const llg_timing_pair_t* pair = &set->pairs[index];
        int64_t b = before[index], a = after[index];
        int negative = b < 0 || a < 0;
        int reports = !negative || b + a > 1;
        for (int part = 0; part < 2; part++) {
            int32_t row = part ? pair->after_row : pair->before_row;
            if (row < 0) continue;
            llg_timing_check_t* check = &set->checks[row];
            check->limit = part ? a : b;
            check->limit2 = part ? b : a;
            check->text = negative ? (part ? pair->text_after : pair->text)
                                   : (part ? pair->after_text : pair->before_text);
            if (reports)
                check->flags = (uint8_t)(check->flags & ~LLG_TIMING_DISABLED);
            else
                check->flags |= LLG_TIMING_DISABLED;
        }
    }
    free(before);
    free(after);
    free(edges);
    free(distance);
    free(predecessor);
    free(touched);
}

void llg_sdf_apply(const llg_sdf_annotation_t* annotation) {
    for (uint32_t index = 0; index < annotation->n_ops; index++) {
        const llg_sdf_op_t* op = &annotation->ops[index];
        const int64_t* values = annotation->values + op->first;
        switch (op->kind) {
        case LLG_SDF_PATH:
            sdf_path(op, values);
            break;
        case LLG_SDF_GATE:
            sdf_gate(op, values);
            break;
        case LLG_SDF_LOAD:
            sdf_load(op, values);
            break;
        case LLG_SDF_CHECK:
            if (op->flags & LLG_SDF_SECOND)
                op->check->limit2 = values[0];
            else
                op->check->limit = values[0];
            break;
        case LLG_SDF_PAIR:
            if (op->flags & LLG_SDF_SECOND)
                op->checks->pairs[op->index].after = values[0];
            else
                op->checks->pairs[op->index].before = values[0];
            break;
        default:
            break;
        }
    }
    // Re-arm the timers of every set whose limits changed; a cell's ops are
    // consecutive, so each set is visited once per run of its ops.
    llg_timing_check_set_t* previous = NULL;
    for (uint32_t index = 0; index < annotation->n_ops; index++) {
        const llg_sdf_op_t* op = &annotation->ops[index];
        if ((op->kind != LLG_SDF_CHECK && op->kind != LLG_SDF_PAIR) || op->checks == previous)
            continue;
        previous = op->checks;
        if (op->checks->n_pairs) sdf_pairs_resolve(op->checks);
        llg_timing_check_limits_changed(op->checks);
    }
}

static void sdf_release(void) {
    for (uint32_t index = 0; index < sdf_limit_tables.count; index++) {
        if (sdf_limit_tables.paths[index]->limits == sdf_limit_tables.tables[index])
            sdf_limit_tables.paths[index]->limits = NULL;
        free(sdf_limit_tables.tables[index]);
    }
    free(sdf_limit_tables.tables);
    free(sdf_limit_tables.paths);
    sdf_limit_tables.tables = NULL;
    sdf_limit_tables.paths = NULL;
    sdf_limit_tables.count = 0;
    sdf_limit_tables.capacity = 0;
}
