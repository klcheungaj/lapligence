
static llg_nba_t* new_nba_in_region(uint64_t ticks, llg_region_t region) {
    if (!region_can_mutate("nonblocking scheduling")) return NULL;
    llg_proc_t* owner = g.in_deferred_action ? NULL : llg_current();
    if (ticks > UINT64_MAX - g.now || g.nba_sequence == UINT64_MAX) {
        fprintf(stderr, "llg: fatal: nonblocking assignment time or sequence overflow\n");
        abort();
    }
    llg_nba_t* n = (llg_nba_t*)llg_checked_calloc(1, sizeof(llg_nba_t), "nonblocking assignment");
    n->target = NULL;
    n->net_target = NULL;
    n->net_slot = -1;
    n->event_target = NULL;
    n->is_real = 0;
    n->is_event = 0;
    n->has_mask = 0;
    n->has_range = 0;
    n->real_target = NULL;
    n->real_value = 0.0;
    n->is_string = 0;
    n->is_chandle = 0;
    n->tag_view = NULL;
    n->time = g.now + ticks;
    n->sequence = g.nba_sequence++;
    n->region = region;
    n->owner = owner;
    return n;
}

static llg_nba_t* new_nba(uint64_t ticks) {
    return new_nba_in_region(
        ticks,
        region_is_reactive(g.current_region) ? LLG_REGION_RE_NBA : LLG_REGION_NBA);
}

static llg_nba_t* new_clocking_nba(uint64_t ticks) {
    return new_nba_in_region(ticks, LLG_REGION_RE_NBA);
}

static void nba_queue_append(llg_nba_queue_t* queue, llg_nba_t* n) {
    n->queue_next = NULL;
    n->queue_prev = queue->tail;
    if (queue->tail) queue->tail->queue_next = n;
    else queue->head = n;
    queue->tail = n;
}

static void nba_queue_remove(llg_nba_queue_t* queue, llg_nba_t* n) {
    if (n->queue_prev) n->queue_prev->queue_next = n->queue_next;
    else queue->head = n->queue_next;
    if (n->queue_next) n->queue_next->queue_prev = n->queue_prev;
    else queue->tail = n->queue_prev;
    n->queue_next = NULL;
    n->queue_prev = NULL;
}

static void nba_owner_append(llg_proc_t* owner, llg_nba_t* n) {
    n->owner_next = NULL;
    n->owner_prev = owner->nba_tail;
    if (owner->nba_tail) owner->nba_tail->owner_next = n;
    else owner->nba_head = n;
    owner->nba_tail = n;
}

static void nba_owner_remove(llg_nba_t* n) {
    llg_proc_t* owner = n->owner;
    if (!owner) return;
    if (n->owner_prev) n->owner_prev->owner_next = n->owner_next;
    else owner->nba_head = n->owner_next;
    if (n->owner_next) n->owner_next->owner_prev = n->owner_prev;
    else owner->nba_tail = n->owner_prev;
    n->owner_next = NULL;
    n->owner_prev = NULL;
    n->owner = NULL;
}

static llg_nba_bucket_t* delayed_nba_bucket(uint64_t time) {
    llg_nba_bucket_t** slot = &g.delayed_nba_buckets;
    while (*slot && (*slot)->time < time) slot = &(*slot)->next;
    if (*slot && (*slot)->time == time) return *slot;
    llg_nba_bucket_t* bucket = (llg_nba_bucket_t*)llg_checked_calloc(
        1, sizeof(*bucket), "delayed nonblocking assignment bucket");
    bucket->time = time;
    bucket->next = *slot;
    *slot = bucket;
    return bucket;
}

static void enqueue_nba(llg_nba_t* n) {
    if (!n) return;
    if (n->time == g.now) {
        nba_queue_append(&g.nba_queues[n->region], n);
        if (n->owner) nba_owner_append(n->owner, n);
    } else {
        llg_nba_bucket_t* bucket = delayed_nba_bucket(n->time);
        nba_queue_append(&bucket->queues[n->region], n);
        // Future NBAs retain their destinations and values independently of
        // the issuing process. They intentionally do not join its cancellable
        // current-slot list.
        n->owner = NULL;
    }
}

static sv4_t nba_range_slice(uint32_t target_width, sv4_t value,
                             sv4_select_plan_t plan, int reverse,
                             uint32_t* offset, uint32_t* width) {
    if (plan.storage_width != target_width) {
        fputs("llg runtime fatal: selected NBA storage width mismatch\n", stderr);
        abort();
    }
    *offset = plan.storage_lsb;
    *width = plan.count;
    return sv4_select_plan_slice(value, &plan, reverse);
}

static void nba_capture_range(llg_nba_t* n, uint32_t target_width,
                              sv4_t value, sv4_select_plan_t plan,
                              int reverse) {
    n->value = nba_range_slice(target_width, value, plan, reverse,
                               &n->range_offset, &n->range_width);
    n->has_range = 1;
}

static int nba_mask_contiguous(uint32_t target_width, sv4_t value, sv4_t mask,
                               uint32_t* offset, uint32_t* width) {
    uint32_t limit = target_width;
    if (llg_sv4_width(value) < limit) limit = llg_sv4_width(value);
    if (llg_sv4_width(mask) < limit) limit = llg_sv4_width(mask);
    uint32_t first = limit;
    uint32_t last = 0;
    uint32_t count = 0;
    for (uint32_t bit = 0; bit < limit; ++bit) {
        if (!((llg_sv4_word(mask, bit / 64u, LLG_SV4_BITS) >> (bit % 64u)) & 1u)) continue;
        if (first == limit) first = bit;
        last = bit;
        ++count;
    }
    if (!count) {
        *offset = *width = 0;
        return 1;
    }
    if (count != last - first + 1) return 0;
    *offset = first;
    *width = count;
    return 1;
}

static void nba_capture_masked(llg_nba_t* n, uint32_t target_width,
                               sv4_t value, sv4_t mask) {
    uint32_t offset;
    uint32_t width;
    if (nba_mask_contiguous(target_width, value, mask, &offset, &width)) {
        n->range_offset = offset;
        n->range_width = width;
        n->has_range = 1;
        sv4_select_plan_t plan = {
            target_width, llg_sv4_width(value), offset, offset, width,
        };
        n->value = sv4_select_plan_slice(value, &plan, 0);
        return;
    }
    sv4_copy(&n->value, &value);
    sv4_copy(&n->mask, &mask);
    n->has_mask = 1;
}

static void promote_delayed_nbas(void) {
    while (g.delayed_nba_buckets && g.delayed_nba_buckets->time == g.now) {
        llg_nba_bucket_t* bucket = g.delayed_nba_buckets;
        g.delayed_nba_buckets = bucket->next;
        for (int region = 0; region < LLG_REGION_COUNT; ++region) {
            llg_nba_queue_t* due = &bucket->queues[region];
            llg_nba_queue_t* current = &g.nba_queues[region];
            if (!due->head) continue;
            // Everything in a future bucket was issued before work could run
            // at that time, so it precedes any current-slot entry.
            if (current->head) {
                due->tail->queue_next = current->head;
                current->head->queue_prev = due->tail;
                current->head = due->head;
            } else {
                *current = *due;
            }
        }
        free(bucket);
    }
}

static void cancel_proc_nbas(llg_proc_t* proc) {
    while (proc && proc->nba_head) {
        llg_nba_t* n = proc->nba_head;
        nba_queue_remove(&g.nba_queues[n->region], n);
        nba_owner_remove(n);
        nba_destroy(n);
    }
}

static void free_all_nbas(void) {
    for (int region = 0; region < LLG_REGION_COUNT; ++region) {
        llg_nba_queue_t* queue = &g.nba_queues[region];
        while (queue->head) {
            llg_nba_t* n = queue->head;
            nba_queue_remove(queue, n);
            nba_owner_remove(n);
            nba_destroy(n);
        }
    }
    while (g.delayed_nba_buckets) {
        llg_nba_bucket_t* bucket = g.delayed_nba_buckets;
        g.delayed_nba_buckets = bucket->next;
        for (int region = 0; region < LLG_REGION_COUNT; ++region) {
            llg_nba_queue_t* queue = &bucket->queues[region];
            while (queue->head) {
                llg_nba_t* n = queue->head;
                nba_queue_remove(queue, n);
                nba_destroy(n);
            }
        }
        free(bucket);
    }
}

void llg_nba_after(sv4_t* target, sv4_t value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    sv4_copy(&n->value, &value);
    enqueue_nba(n);
}

void llg_nba_net_after(llg_net_t* net, int slot, sv4_t value, uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    sv4_copy(&n->value, &value);
    enqueue_nba(n);
}

void llg_nba_net_masked_after(llg_net_t* net, int slot, sv4_t value,
                              sv4_t mask, uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    nba_capture_masked(n, llg_sv4_width(*net->drivers[slot]), value, mask);
    enqueue_nba(n);
}

void llg_nba_net_selected_after(llg_net_t* net, int slot, sv4_t value,
                                sv4_select_plan_t plan, int reverse,
                                uint64_t ticks) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->net_target = net;
    n->net_slot = slot;
    nba_capture_range(n, llg_sv4_width(*net->drivers[slot]), value, plan, reverse);
    enqueue_nba(n);
}

static void clocking_drive_schedule(const llg_clocking_drive_t* drive,
                                    const llg_wait_src_t* specs, int n_specs) {
    if (!specs || n_specs <= 0 || !region_can_mutate("clocking drive scheduling"))
        return;
    if (clocking_event_current(specs, n_specs)) {
        clocking_drive_enqueue(drive);
        return;
    }
    llg_clocking_drive_t* pending = (llg_clocking_drive_t*)llg_checked_calloc(
        1, sizeof(*pending), "pending clocking drive");
    pending->specs = (llg_wait_src_t*)llg_checked_malloc(
        (size_t)n_specs, sizeof(*pending->specs), "clocking drive event sources");
    memcpy(pending->specs, specs, (size_t)n_specs * sizeof(*specs));
    pending->n_specs = n_specs;
    pending->target = drive->target;
    pending->target_scope = value_scope_retain_target(drive->target);
    pending->net_target = drive->net_target;
    pending->net_slot = drive->net_slot;
    pending->real_target = drive->real_target;
    sv4_copy(&pending->value, &drive->value);
    sv4_copy(&pending->mask, &drive->mask);
    pending->has_mask = drive->has_mask;
    pending->range_offset = drive->range_offset;
    pending->range_width = drive->range_width;
    pending->has_range = drive->has_range;
    pending->is_real = drive->is_real;
    pending->real_value = drive->real_value;
    pending->ticks = drive->ticks;
    if (g.clocking_drives_tail) g.clocking_drives_tail->next = pending;
    else g.clocking_drives = pending;
    g.clocking_drives_tail = pending;
}

void llg_clocking_nba_sync_after(sv4_t* target, sv4_t value, uint64_t ticks,
                                 const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = value;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_net_sync_after(llg_net_t* net, int slot, sv4_t value,
                                     uint64_t ticks,
                                     const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = value;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_sync_masked_after(
    sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks,
    const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = value;
    drive.mask = mask;
    drive.has_mask = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_net_sync_masked_after(
    llg_net_t* net, int slot, sv4_t value, sv4_t mask, uint64_t ticks,
    const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = value;
    drive.mask = mask;
    drive.has_mask = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_clocking_nba_sync_selected_after(
    sv4_t* target, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, const llg_wait_src_t* specs, int n_specs) {
    if (!target) return;
    llg_clocking_drive_t drive = {0};
    drive.target = target;
    drive.value = nba_range_slice(llg_sv4_width(*target), value, plan, reverse,
                                  &drive.range_offset, &drive.range_width);
    drive.has_range = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
    sv4_destroy(&drive.value);
}

void llg_clocking_nba_net_sync_selected_after(
    llg_net_t* net, int slot, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, const llg_wait_src_t* specs, int n_specs) {
    if (!net || slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) return;
    llg_clocking_drive_t drive = {0};
    drive.net_target = net;
    drive.net_slot = slot;
    drive.value = nba_range_slice(llg_sv4_width(*net->drivers[slot]), value, plan,
                                  reverse, &drive.range_offset,
                                  &drive.range_width);
    drive.has_range = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
    sv4_destroy(&drive.value);
}

void llg_clocking_nba_d_sync_after(double* target, double value, uint64_t ticks,
                                   const llg_wait_src_t* specs, int n_specs) {
    llg_clocking_drive_t drive = {0};
    drive.real_target = target;
    drive.real_value = value;
    drive.is_real = 1;
    drive.ticks = ticks;
    clocking_drive_schedule(&drive, specs, n_specs);
}

void llg_nba_event_after(llg_event_t* ev, uint64_t ticks) {
    if (!ev || !ev->object) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->event_target = ev->object;
    n->is_event = 1;
    enqueue_nba(n);
}

void llg_nba_event(llg_event_t* ev) {
    llg_nba_event_after(ev, 0);
}

void llg_nba(sv4_t* target, sv4_t value) {
    llg_nba_after(target, value, 0);
}

void llg_nba_masked(sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks) {
    if (!target) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_masked(n, llg_sv4_width(*target), value, mask);
    enqueue_nba(n);
}

void llg_nba_selected_after(sv4_t* target, sv4_t value,
                            sv4_select_plan_t plan, int reverse,
                            uint64_t ticks) {
    if (!target) return;
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_range(n, llg_sv4_width(*target), value, plan, reverse);
    enqueue_nba(n);
}

void llg_nba_tagged_selected_after(sv4_t* target, sv4_t value,
                                   sv4_select_plan_t plan, int reverse,
                                   uint64_t ticks,
                                   const llg_ref_tag_check_t* checks,
                                   size_t check_count, const char* location) {
    if (!target) return;
    if (!check_count || !checks) {
        llg_nba_selected_after(target, value, plan, reverse, ticks);
        return;
    }
    if (check_count > (SIZE_MAX - sizeof(llg_ref_view_t)) / sizeof(*checks)) {
        fputs("llg: fatal: tagged nonblocking assignment check count overflow\n", stderr);
        abort();
    }
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    // One allocation holds the view header and its check array; the checks
    // carry only plans, widths and static member-name literals.
    llg_ref_view_t* view = (llg_ref_view_t*)llg_checked_malloc(
        1, sizeof(llg_ref_view_t) + check_count * sizeof(*checks),
        "tagged nonblocking assignment checks");
    llg_ref_tag_check_t* copied = (llg_ref_tag_check_t*)(view + 1);
    memcpy(copied, checks, check_count * sizeof(*checks));
    *view = (llg_ref_view_t){
        .parent = NULL,
        .plan = plan,
        .tag_check_count = check_count,
        .tag_checks = copied,
        .location = location,
    };
    n->tag_view = view;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    nba_capture_range(n, llg_sv4_width(*target), value, plan, reverse);
    enqueue_nba(n);
}

void llg_nba_d_after(double* target, double value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->is_real = 1;
    n->real_target = target;
    n->real_value = value;
    enqueue_nba(n);
}

void llg_string_nba_after(llg_string_t* target, llg_string_t value,
                          uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) {
        llg_string_destroy(&value);
        return;
    }
    n->is_string = 1;
    n->native.string.target = target;
    n->native.string.value = value;
    enqueue_nba(n);
}

void llg_chandle_nba_after(void** target, void* value, uint64_t ticks) {
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->is_chandle = 1;
    n->native.chandle.target = target;
    n->native.chandle.value = value;
    enqueue_nba(n);
}

void llg_ba(sv4_t* target, sv4_t value) {
    // Procedural writes cannot override either a force or a procedural
    // continuous assignment (LRM 10.6.1/10.6.2).
    if (llg_is_forced(target) || pca_active(target)) return;
    sig_write(target, value);
}

void llg_ba_from(sv4_t* target, const sv4_t* value) {
    llg_ba(target, *value);
}
