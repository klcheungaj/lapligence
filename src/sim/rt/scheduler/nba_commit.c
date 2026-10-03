
static int nba_due(llg_region_t region) {
    promote_delayed_nbas();
    return g.nba_queues[region].head != NULL;
}

// SV 11.9 checks a member assignment against the tag current when it is
// performed. A member NBA is performed here, so a blocking or other-process
// retag after issue makes the queued member write a runtime error; the
// write is dropped rather than storing one member's payload under another
// member's tag (SV 7.3.2).
static int nba_tag_commit_valid(const llg_ref_view_t* view, const sv4_t* target) {
    size_t failed = 0;
    if (llg_ref_view_valid(view, target, &failed)) return 1;
    const char* member = "<unknown>";
    if (failed < view->tag_check_count && view->tag_checks[failed].member_name)
        member = view->tag_checks[failed].member_name;
    llg_rt_mark_failed();
    fprintf(stderr,
            "llg: runtime error: nonblocking write to tagged-union member %s at %s "
            "found an inactive tag at commit\n",
            member, view->location ? view->location : "<unknown>");
    fflush(stderr);
    return 0;
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
        if (next->tag_view && !nba_tag_commit_valid(next->tag_view, target)) return;
        sv4_t value = (next->has_mask || next->has_range)
                          ? sv4_clone(target)
                          : sv4_clone(&next->value);
        if (next->has_mask) {
            llg_sv4_masked_merge(&value, next->value, next->mask);
        } else if (next->has_range) {
            if (next->range_width) {
                sv4_select_plan_t plan = {
                    llg_sv4_width(value), next->range_width, next->range_offset, 0,
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
