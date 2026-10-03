
static int nba_due(llg_region_t region) {
    promote_delayed_nbas();
    return g.nba_queues[region].head != NULL;
}

static void apply_nba(llg_nba_t* next) {
    if (next->fixed_target) {
        fixed_array_apply(next->fixed_target, next->fixed_value);
    } else if (next->is_event) {
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
        sv4_t value = (next->has_mask || next->has_range)
                          ? sv4_clone(target)
                          : sv4_clone(&next->value);
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
        } else if (next->has_range) {
            if (next->range_width) {
                sv4_select_plan_t plan = {
                    value.width, next->range_width, next->range_offset, 0,
                    next->range_width,
                };
                sv4_select_plan_set(&value, &plan, next->value);
            }
        }
        if (next->net_target) llg_net_write(next->net_target, next->net_slot, value);
        else sig_write(target, value);
        sv4_destroy(&value);
    }
}

static void commit_nbas(llg_region_t region) {
    promote_delayed_nbas();
    llg_nba_queue_t* queue = &g.nba_queues[region];
    while (queue->head) {
        llg_nba_t* next = queue->head;
        nba_queue_remove(queue, next);
        nba_owner_remove(next);
        apply_nba(next);
        nba_destroy(next);
    }
}
