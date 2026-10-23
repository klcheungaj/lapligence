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

static void sdf_gate(const llg_sdf_op_t* op, const int64_t* values) {
    int increment = (op->flags & LLG_SDF_INCREMENT) != 0;
    for (uint32_t index = 0; index < 3u; index++) {
        if (op->delay >> index & 1u)
            op->gate[index] = sdf_value(op->gate[index], *values++, increment);
    }
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
        case LLG_SDF_CHECK:
            if (op->flags & LLG_SDF_SECOND)
                op->check->limit2 = values[0];
            else
                op->check->limit = values[0];
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
        if (op->kind != LLG_SDF_CHECK || op->checks == previous) continue;
        previous = op->checks;
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
