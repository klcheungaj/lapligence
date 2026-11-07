
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
    llg_msg(LLG_MSG_RUNTIME_FAILURE, view->location, "nonblocking write to tagged-union member %s found an inactive tag at commit", member);
    fflush(stderr);
    llg_rt_note_error(0, "nonblocking write to an inactive tagged-union member", view->location);
    return 0;
}

static void apply_nba_target(llg_nba_t* next) {
    if (next->fixed_target) {
        fixed_array_apply(next->fixed_target, next->fixed_value);
    } else if (next->is_event) {
        event_trigger_object(next->event_target);
    } else if (next->is_string) {
        if (next->native.string.target) {
            llg_string_move(next->native.string.target, next->native.string.value);
            next->native.string.value = (llg_string_t){0};
        }
    } else if (next->is_values) {
        llg_dyn_value_array_t* payload = next->native.values.payload;
        if (next->native.values.whole) {
            llg_dyn_value_copy(next->native.values.target, payload);
        } else {
            sv4_t zero = sv4_from_u64(0, 64, 1);
            llg_dyn_value_copy_range(next->native.values.target, next->value,
                                     payload, zero, payload->size);
            sv4_destroy(&zero);
        }
    } else if (next->is_container) {
        void* target = next->native.container.target;
        void* payload = next->native.container.payload;
        switch (next->native.container.kind) {
        case LLG_NBA_CONTAINER_DYN: llg_dyn_copy(target, payload); break;
        case LLG_NBA_CONTAINER_QUEUE: llg_queue_copy(target, payload); break;
        case LLG_NBA_CONTAINER_ASSOC: llg_assoc_copy(target, payload); break;
        case LLG_NBA_CONTAINER_DYN_VALUE: llg_dyn_value_copy(target, payload); break;
        case LLG_NBA_CONTAINER_QUEUE_VALUE: llg_queue_value_copy(target, payload); break;
        default: llg_assoc_value_copy(target, payload); break;
        }
    } else if (next->is_chandle) {
        // Handle storage publishes its change marker on a changed value,
        // matching a blocking handle write.
        void** target = next->native.chandle.target;
        if (target && *target != next->native.chandle.value) {
            *target = next->native.chandle.value;
            llg_dependency_changed(next->native.chandle.dependency);
        }
    } else if (next->is_real) {
        if (!llg_is_real_forced(next->real_target))
            real_write(next->real_target, next->real_value);
    } else {
        sv4_t* target = next->target;
        if (next->net_target) {
            if (next->net_slot < 0 || next->net_slot >= next->net_target->n_drivers)
                return;
            target = next->net_target->drivers[next->net_slot];
        } else if (llg_is_forced(target)) return;
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

// A write to an item of a native value finds the item now: a whole write
// since issue may have replaced the value tree. The element position was
// fixed at issue; one out of range writes nothing (SV 7.4.6).
static void apply_located_nba(llg_nba_t* next) {
    const llg_nba_locator_t* locator = next->locator;
    llg_value_t* item = locator->root;
    if (locator->array)
        item = llg_dyn_value_element(locator->array, locator->indices,
                                     locator->index_count);
    if (!item) return;
    for (size_t step = 0; step < locator->item_count; ++step) {
        if (locator->items[step] == LLG_NATIVE_ITEM_INVALID) return;
        item = &item->value.items[locator->items[step]];
    }
    if (next->is_container) {
        const void* payload = next->native.container.payload;
        switch (next->native.container.kind) {
        case LLG_NBA_CONTAINER_DYN: llg_value_item_from_dyn(item, payload); break;
        case LLG_NBA_CONTAINER_QUEUE: llg_value_item_from_queue(item, payload); break;
        case LLG_NBA_CONTAINER_DYN_VALUE: llg_value_item_from_dyn_value(item, payload); break;
        case LLG_NBA_CONTAINER_QUEUE_VALUE: llg_value_item_from_queue_value(item, payload); break;
        default: break;
        }
        if (locator->array) llg_dyn_value_touch(locator->array);
        return;
    }
    if (next->is_string) next->native.string.target = &item->value.string;
    else if (next->is_chandle) next->native.chandle.target = &item->value.handle;
    else if (next->is_real) next->real_target = &item->value.real;
    else next->target = &item->value.packed;
    apply_nba_target(next);
    if (locator->array) llg_dyn_value_touch(locator->array);
}

static void apply_nba(llg_nba_t* next) {
    if (next->locator) apply_located_nba(next);
    else apply_nba_target(next);
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
