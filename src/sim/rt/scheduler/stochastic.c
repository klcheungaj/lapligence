
// ── IEEE stochastic analysis queues ─────────────────────────────────────────

static void llg_q_set_status(sv4_t* status, int code) {
    if (!status) return;
    llg_ba(status, sv4_from_i64((int64_t)code, status->width));
}

static int llg_q_read_integer(sv4_t value, int64_t* result,
                              const char* operation, const char* argument) {
    if (value.width == 0 || sv4_is_unknown(value) || !sv4_fits_i64(value)) {
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
    llg_ba(target, sv4_from_i64((int64_t)value, target->width));
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
    if (job_id) llg_ba(job_id, sv4_from_i64(entry->job_id, job_id->width));
    if (inform_id)
        llg_ba(inform_id, sv4_from_i64(entry->inform_id, inform_id->width));
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
