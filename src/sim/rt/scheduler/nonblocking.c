
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
    n->real_target = NULL;
    n->real_value = 0.0;
    n->is_string = 0;
    n->string_target = NULL;
    n->string_value = (llg_string_t){0};
    n->time = g.now + ticks;
    n->sequence = g.nba_sequence++;
    n->region = region;
    n->owner = owner;
    n->next = NULL;
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

static void enqueue_nba(llg_nba_t* n) {
    if (!n) return;
    if (n->time == g.now && n->owner) {
        llg_proc_t* p = n->owner;
        if (p->nba_tail) p->nba_tail->next = n;
        else p->nba_head = n;
        p->nba_tail = n;
    } else {
        llg_nba_t** slot = &g.delayed_nbas;
        while (*slot && (*slot)->time <= n->time) slot = &(*slot)->next;
        n->next = *slot;
        *slot = n;
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
    sv4_copy(&n->value, &value);
    sv4_copy(&n->mask, &mask);
    n->has_mask = 1;
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
    llg_nba_t* n = new_nba(ticks);
    if (!n) return;
    n->target = target;
    n->target_scope = value_scope_retain_target(target);
    sv4_copy(&n->value, &value);
    sv4_copy(&n->mask, &mask);
    n->has_mask = 1;
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
    n->string_target = target;
    n->string_value = value;
    enqueue_nba(n);
}

void llg_ba(sv4_t* target, sv4_t value) {
    // Procedural writes cannot override either a force or a procedural
    // continuous assignment (LRM 10.6.1/10.6.2).
    if (llg_is_forced(target) || pca_active(target)) return;
    sig_write(target, value);
}
