
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
    if (next->is_event) {
        event_trigger_object(next->event_target);
    } else if (next->is_string) {
        if (next->string_target) {
            llg_string_move(next->string_target, next->string_value);
            next->string_value = (llg_string_t){0};
        }
    } else if (next->is_real) {
        if (!llg_is_real_forced(next->real_target) && !pca_real_active(next->real_target))
            real_write(next->real_target, next->real_value);
    } else {
        sv4_t* target = next->target;
        if (next->net_target) {
            if (next->net_slot < 0 || next->net_slot >= next->net_target->n_drivers)
                return;
            target = next->net_target->drivers[next->net_slot];
        } else if (llg_is_forced(target) || pca_active(target)) return;
        if (!target) return;
        sv4_t value = next->has_mask ? sv4_clone(target) : sv4_clone(&next->value);
        if (next->has_mask) {
            uint32_t n = (value.width + 63u) / 64u;
            uint32_t mn = (next->mask.width + 63u) / 64u;
            uint32_t vn = (next->value.width + 63u) / 64u;
            for (uint32_t i = 0; i < n; ++i) {
                uint64_t mask = i < mn && i < vn ? next->mask.bits[i] : 0;
                if (!mask) continue;
                value.bits[i] = (value.bits[i] & ~mask) | (next->value.bits[i] & mask);
                value.x[i] = (value.x[i] & ~mask) | (next->value.x[i] & mask);
                value.z[i] = (value.z[i] & ~mask) | (next->value.z[i] & mask);
            }
        }
        if (next->net_target) llg_net_write(next->net_target, next->net_slot, value);
        else sig_write(target, value);
        sv4_destroy(&value);
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
        nba_destroy(next);
    }
}
