
static int nba_due(llg_region_t region) {
    for (int i = 0; i < g.n_procs; i++) {
        llg_proc_t* p = g.all_procs[i];
        for (llg_nba_t* n = p ? p->nba_head : NULL; n; n = n->next)
            if (n->time == g.now && n->region == region) return 1;
    }
    for (llg_nba_t* n = g.delayed_nbas; n; n = n->next)
        if (n->time == g.now && n->region == region) return 1;
    return 0;
}

static void apply_nba(llg_nba_t* next) {
    if (next->is_event) event_trigger_object(next->event_target);
    else if (next->is_string) {
        if (next->string_target)
            llg_string_move(next->string_target, next->string_value);
        else
            llg_string_destroy(&next->string_value);
    }
    else if (next->is_real) {
        if (!llg_is_real_forced(next->real_target) && !pca_real_active(next->real_target))
            real_write(next->real_target, next->real_value);
    } else if (next->net_target) {
        llg_net_t* net = next->net_target;
        if (next->net_slot < 0 || next->net_slot >= net->n_drivers) return;
        sv4_t value = next->value;
        sv4_t* current = net->drivers[next->net_slot];
        if (!current) return;
        if (next->has_mask) {
            value = *current;
            for (uint32_t i = 0; i < (value.width + 63u) / 64u; i++) {
                uint64_t mask = next->mask.bits[i];
                value.bits[i] = (value.bits[i] & ~mask) | (next->value.bits[i] & mask);
                value.x[i] = (value.x[i] & ~mask) | (next->value.x[i] & mask);
                value.z[i] = (value.z[i] & ~mask) | (next->value.z[i] & mask);
            }
        }
        llg_net_write(net, next->net_slot, value);
    } else if (!llg_is_forced(next->target) && !pca_active(next->target)) {
        sv4_t value = next->value;
        if (next->has_mask) {
            value = *next->target;
            for (uint32_t i = 0; i < (value.width + 63u) / 64u; i++) {
                uint64_t mask = next->mask.bits[i];
                value.bits[i] = (value.bits[i] & ~mask) | (next->value.bits[i] & mask);
                value.x[i] = (value.x[i] & ~mask) | (next->value.x[i] & mask);
                value.z[i] = (value.z[i] & ~mask) | (next->value.z[i] & mask);
            }
        }
        sig_write(next->target, value);
    }
}

static void commit_nbas(llg_region_t region) {
    for (;;) {
        llg_nba_t* next = NULL;
        llg_nba_t** next_slot = NULL;
        llg_proc_t* owner = NULL;
        for (llg_nba_t** delayed_slot = &g.delayed_nbas; *delayed_slot;
             delayed_slot = &(*delayed_slot)->next) {
            llg_nba_t* n = *delayed_slot;
            if (n->time != g.now || n->region != region) continue;
            if (!next || n->sequence < next->sequence) {
                next = n;
                next_slot = delayed_slot;
                owner = NULL;
            }
        }
        for (int i = 0; i < g.n_procs; i++) {
            llg_proc_t* p = g.all_procs[i];
            if (!p) continue;
            for (llg_nba_t** slot = &p->nba_head; *slot;
                 slot = &(*slot)->next) {
                llg_nba_t* n = *slot;
                if (n->time != g.now || n->region != region) continue;
                if (!next || n->sequence < next->sequence) {
                    next = n;
                    next_slot = slot;
                    owner = p;
                }
            }
        }
        if (!next) break;
        *next_slot = next->next;
        if (owner && owner->nba_tail == next) {
            owner->nba_tail = NULL;
            for (llg_nba_t* n = owner->nba_head; n; n = n->next)
                owner->nba_tail = n;
        }
        apply_nba(next);
        free(next);
    }
}
