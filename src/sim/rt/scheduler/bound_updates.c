// ── Updates that end with their issuer (S09-D8) ───────────────────────────────

// An update retains its target and value and needs no process to commit, so
// most updates outlive theirs. A timed event trigger, and an update a binding
// statement issued, end with it: kill, disable fork, disable and $exit drop
// them like descendants, also after their issuer has finished. They hang in
// its `held_groups` ("Holding descendants", process_control.c), so a process
// that issues none has nothing to keep.

// The process an update issued now ends with, or NULL. An action of a deferred
// report or event control has none, and no control reaches a killed process or
// an unregistered one.
static LLG_OUTLYING llg_proc_t* bound_issuer(void) {
    llg_proc_t* proc = g.in_deferred_action ? NULL : llg_current();
    return proc && !proc->killed && proc->registry_slot >= 0 ? proc : NULL;
}

static LLG_OUTLYING void bound_group_init(llg_fork_group_t* grp, llg_proc_t* issuer,
                                          int32_t kind) {
    grp->join_kind = kind;
    grp->started = 1;
    grp->parent = issuer;
    grp->child_region = region_is_reactive(g.current_region)
                           ? LLG_REGION_REACTIVE
                           : LLG_REGION_ACTIVE;
    grp->owner_activation = issuer->activation_top;
    if (grp->owner_activation) activation_retain(grp->owner_activation);
    held_group_link(grp);
}

static LLG_OUTLYING llg_bound_update_t* bound_update_new(llg_proc_t* issuer) {
    llg_bound_update_t* bound = (llg_bound_update_t*)llg_checked_calloc(
        1, sizeof(*bound), "update of a process");
    bound_group_init(&bound->group, issuer, LLG_JOIN_UPDATE);
    return bound;
}

static void bound_update_free(llg_bound_update_t* bound) {
    activation_release(bound->group.owner_activation);
    free(bound);
}

// The update of `bound` is in an NBA queue.
static LLG_OUTLYING void bound_update_queued(llg_bound_update_t* bound, llg_nba_t* n) {
    bound->trigger = NULL;
    bound->nba = n;
    if (!n->locator)
        n->locator = (llg_nba_locator_t*)llg_checked_calloc(
            1, sizeof(*n->locator), "update of a process");
    n->locator->bound = bound;
}

// Bind an update of an NBA queue to its issuer.
static LLG_OUTLYING void nba_bind(llg_nba_t* n) {
    llg_proc_t* issuer = bound_issuer();
    if (issuer && !g.nba_hold_open) bound_update_queued(bound_update_new(issuer), n);
}

static LLG_OUTLYING void deferred_trigger_bind(llg_deferred_trigger_t* trigger) {
    llg_proc_t* issuer = bound_issuer();
    if (!issuer) return;
    trigger->bound = bound_update_new(issuer);
    trigger->bound->trigger = trigger;
}

// The update commits: its issuer no longer holds it.
static LLG_OUTLYING void bound_update_end(llg_bound_update_t* bound) {
    llg_proc_t* issuer = bound->group.parent;
    held_group_unlink(&bound->group);
    bound_update_free(bound);
    proc_tree_released(issuer);
}

static LLG_OUTLYING void nba_cancel(llg_nba_t* n) {
    promote_delayed_nbas();
    if (n->time == g.now) {
        nba_queue_remove(&g.nba_queues[n->region], n);
    } else {
        llg_nba_bucket_t** slot = &g.delayed_nba_buckets;
        while ((*slot)->time != n->time) slot = &(*slot)->next;
        llg_nba_bucket_t* bucket = *slot;
        nba_queue_remove(&bucket->queues[n->region], n);
        // An emptied time must not be advanced to.
        int32_t pending = 0;
        for (int32_t region = 0; region < LLG_REGION_COUNT; ++region)
            pending |= bucket->queues[region].head != NULL;
        if (!pending) {
            *slot = bucket->next;
            free(bucket);
        }
    }
    nba_destroy(n);
}

// The issuer ends first: the update never happens. The group is already off
// its issuer's list.
static LLG_OUTLYING void bound_update_drop(llg_fork_group_t* grp) {
    llg_bound_update_t* bound = (llg_bound_update_t*)grp;
    if (bound->trigger) {
        deferred_trigger_unlink(bound->trigger);
        free_deferred_trigger(bound->trigger);
    } else {
        nba_cancel(bound->nba);
    }
    bound_update_free(bound);
}

LLG_OUTLYING void llg_nba_bind_begin(void) {
    g.nba_bind_mark = g.nba_sequence;
}

// Only updates of this time slot are bound: with a delay they outlive their
// issuer like any other.
LLG_OUTLYING void llg_nba_bind_end(void) {
    for (llg_nba_t* n = g.nba_queues[nba_region()].tail;
         n && n->sequence >= g.nba_bind_mark; n = n->queue_prev)
        nba_bind(n);
}

LLG_OUTLYING llg_proc_t* llg_spawn_bound_with_frame(const llg_co_desc_t* desc,
                                                    const char* name,
                                                    llg_frame_t* frame) {
    llg_proc_t* issuer = bound_issuer();
    if (!issuer) return llg_spawn_detached_with_frame(desc, name, frame);
    if (!region_can_mutate("bound process scheduling")) return NULL;
    llg_fork_group_t* grp = (llg_fork_group_t*)llg_checked_calloc(
        1, sizeof(*grp), "bound process group");
    bound_group_init(grp, issuer, LLG_JOIN_BOUND);
    return llg_fork_with_frame(desc, name, grp, frame);
}
