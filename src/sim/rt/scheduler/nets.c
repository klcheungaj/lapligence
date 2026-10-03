
// ── Collapsed inout nets ──────────────────────────────────────────────────────

static uint32_t llg_net_index_priority(int index) {
    uint32_t value = (uint32_t)index + UINT32_C(0x9e3779b9);
    value ^= value >> 16;
    value *= UINT32_C(0x7feb352d);
    value ^= value >> 15;
    value *= UINT32_C(0x846ca68b);
    value ^= value >> 16;
    return value;
}

static uint32_t llg_net_index_max(const llg_net_t* net, int index) {
    return index < 0 ? 0 : net->driver_index[index].max_high;
}

static void llg_net_index_recompute(llg_net_t* net, int index) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    node->max_high = node->high;
    uint32_t left = llg_net_index_max(net, node->left);
    uint32_t right = llg_net_index_max(net, node->right);
    if (left > node->max_high) node->max_high = left;
    if (right > node->max_high) node->max_high = right;
}

static int llg_net_index_before(const llg_net_t* net, int left, int right) {
    const llg_net_driver_index_t* a = &net->driver_index[left];
    const llg_net_driver_index_t* b = &net->driver_index[right];
    return a->low < b->low || (a->low == b->low && left < right);
}

static int llg_net_index_rotate_left(llg_net_t* net, int root) {
    int next = net->driver_index[root].right;
    net->driver_index[root].right = net->driver_index[next].left;
    net->driver_index[next].left = root;
    llg_net_index_recompute(net, root);
    llg_net_index_recompute(net, next);
    return next;
}

static int llg_net_index_rotate_right(llg_net_t* net, int root) {
    int next = net->driver_index[root].left;
    net->driver_index[root].left = net->driver_index[next].right;
    net->driver_index[next].right = root;
    llg_net_index_recompute(net, root);
    llg_net_index_recompute(net, next);
    return next;
}

static int llg_net_index_insert_at(llg_net_t* net, int root, int index) {
    if (root < 0) return index;
    if (llg_net_index_before(net, index, root)) {
        net->driver_index[root].left =
            llg_net_index_insert_at(net, net->driver_index[root].left, index);
        if (net->driver_index[net->driver_index[root].left].priority <
            net->driver_index[root].priority)
            root = llg_net_index_rotate_right(net, root);
    } else {
        net->driver_index[root].right =
            llg_net_index_insert_at(net, net->driver_index[root].right, index);
        if (net->driver_index[net->driver_index[root].right].priority <
            net->driver_index[root].priority)
            root = llg_net_index_rotate_left(net, root);
    }
    llg_net_index_recompute(net, root);
    return root;
}

static int llg_net_index_merge(llg_net_t* net, int left, int right) {
    if (left < 0) return right;
    if (right < 0) return left;
    if (net->driver_index[left].priority < net->driver_index[right].priority) {
        net->driver_index[left].right =
            llg_net_index_merge(net, net->driver_index[left].right, right);
        llg_net_index_recompute(net, left);
        return left;
    }
    net->driver_index[right].left =
        llg_net_index_merge(net, left, net->driver_index[right].left);
    llg_net_index_recompute(net, right);
    return right;
}

static int llg_net_index_remove_at(llg_net_t* net, int root, int index) {
    if (root < 0) return -1;
    if (root == index)
        return llg_net_index_merge(net, net->driver_index[root].left,
                                   net->driver_index[root].right);
    if (llg_net_index_before(net, index, root))
        net->driver_index[root].left =
            llg_net_index_remove_at(net, net->driver_index[root].left, index);
    else
        net->driver_index[root].right =
            llg_net_index_remove_at(net, net->driver_index[root].right, index);
    llg_net_index_recompute(net, root);
    return root;
}

static int llg_net_has_index(const llg_net_t* net) {
    return net->driver_index && net->overlap_scratch && net->n_drivers > 0;
}

void llg_net_index_reset(llg_net_t* net) {
    if (!net || !net->driver_index) return;
    net->index_root = -1;
    for (int index = 0; index < net->n_drivers; index++) {
        llg_net_driver_index_t* node = &net->driver_index[index];
        node->low = 0;
        node->high = 0;
        node->max_high = 0;
        node->priority = llg_net_index_priority(index);
        node->left = -1;
        node->right = -1;
        node->active = 0;
    }
}

static void llg_net_index_remove(llg_net_t* net, int index) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    if (!node->active) return;
    net->index_root = llg_net_index_remove_at(net, net->index_root, index);
    node->left = -1;
    node->right = -1;
    node->active = 0;
}

static void llg_net_index_insert(llg_net_t* net, int index,
                                 uint32_t low, uint32_t high) {
    llg_net_driver_index_t* node = &net->driver_index[index];
    node->low = low;
    node->high = high;
    node->max_high = high;
    node->left = -1;
    node->right = -1;
    node->active = 1;
    net->index_root = llg_net_index_insert_at(net, net->index_root, index);
}

static void llg_net_index_collect(const llg_net_t* net, int root,
                                  uint32_t low, uint32_t high, int* count) {
    if (root < 0) return;
    const llg_net_driver_index_t* node = &net->driver_index[root];
    if (node->left >= 0 &&
        net->driver_index[node->left].max_high >= low)
        llg_net_index_collect(net, node->left, low, high, count);
    if (node->low <= high && node->high >= low)
        net->overlap_scratch[(*count)++] = root;
    if (node->low <= high)
        llg_net_index_collect(net, node->right, low, high, count);
}

static int llg_net_all_z(const sv4_t* value) {
    int limbs = (int)((llg_sv4_width(*value) + 63u) / 64u);
    for (int limb = 0; limb < limbs; limb++) {
        uint32_t remaining = llg_sv4_width(*value) - (uint32_t)limb * 64u;
        uint64_t mask = remaining >= 64u
            ? UINT64_MAX : (UINT64_C(1) << remaining) - UINT64_C(1);
        if ((llg_sv4_word(*value, limb, LLG_SV4_BITS) & mask) || (llg_sv4_word(*value, limb, LLG_SV4_X) & mask) ||
            (llg_sv4_word(*value, limb, LLG_SV4_Z) & mask) != mask)
            return 0;
    }
    return 1;
}

static sv4_t llg_net_compute_range(llg_net_t* net, uint32_t low,
                                   uint32_t width) {
    int count = 0;
    llg_net_index_collect(net, net->index_root, low, low + width - 1u, &count);
    return sv4_resolve_strengths_range(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->overlap_scratch, count, net->width, low, width,
        net->is_signed, net->resolution);
}

static sv4_t llg_net_compute(llg_net_t* net) {
    if (llg_net_has_index(net))
        return llg_net_compute_range(net, 0, net->width);
    return sv4_resolve_strengths(
        (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
        net->n_drivers, net->width, net->is_signed, net->resolution);
}

static void llg_net_alias_refresh(llg_net_alias_t* alias) {
    if (!alias || !alias->storage) return;
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_copy(&owned[0], alias->storage);
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        const llg_net_alias_part_t* part = &alias->parts[i];
        if (!part->net || part->bit_count == 0 ||
            (uint64_t)part->signal_bit + part->bit_count > llg_sv4_width(owned[0]) ||
            (uint64_t)part->group_bit + part->bit_count > llg_sv4_width(part->net->resolved))
            continue;
        sv4_t bits = sv4_part_select(part->net->resolved,
                                     (int64_t)part->group_bit + part->bit_count - 1,
                                     part->group_bit);
        sv4_part_select_set(&owned[0], (int64_t)part->signal_bit + part->bit_count - 1,
                            part->signal_bit, bits);
        sv4_destroy(&bits);
    }
    // The visible cell is a first-class dependency/waveform target. Route
    // updates through the ordinary signal writer so waiters and waveform
    // callbacks observe canonical alias changes.
    sig_write(&alias->visible, owned[0]);
    if (alias->publication_target) sig_write(alias->publication_target, owned[0]);
    llg_value_scope_end(scope);

}

static void llg_net_alias_refresh_all(llg_net_t* net) {
    if (!net) return;
    for (int i = 0; i < net->n_aliases; i++)
        llg_net_alias_refresh(net->aliases[i]);
}

static void llg_net_publish(llg_net_t* net, sv4_t resolved) {
    if (net->propagation_enabled) {
        llg_inertial_assign(&net->propagation, &net->resolved, resolved,
                            net->propagation_rise, net->propagation_fall,
                            net->propagation_turn_off);
        if (net->propagation) net->propagation->publication_net = net;
    } else {
        sig_write(&net->resolved, resolved);
        llg_net_alias_refresh_all(net);
    }
}

static void llg_net_publish_range(llg_net_t* net, uint32_t low,
                                  sv4_t resolved) {
    if (sig_write_range(&net->resolved, low, resolved))
        llg_net_alias_refresh_all(net);
}

static void llg_net_publish_ranges(llg_net_t* net,
                                   uint32_t first_low, sv4_t first,
                                   uint32_t second_low, sv4_t second) {
    if (sig_write_ranges(&net->resolved, first_low, first,
                         second_low, second, 1))
        llg_net_alias_refresh_all(net);
}

void llg_net_resolve(llg_net_t* net) {
    if (!net || !region_can_mutate("net resolution")) return;
    if (llg_is_forced(&net->resolved)) {
        force_recompute_target(&net->resolved, net);
    } else {
        llg_value_scope_t* scope = llg_value_scope_begin(1);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute(net));
        llg_net_publish(net, owned[0]);
        llg_value_scope_end(scope);
    }
}

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    if (!net || !region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers || !net->drivers[idx]) return;
    sv4_t replacement = sv4_resize(value, net->width, net->is_signed);
    sv4_t* slot = net->drivers[idx];
    if (sv4_same(*slot, replacement)) {
        sv4_destroy(&replacement);
        return;
    }
    uint32_t old_low = 0;
    uint32_t old_high = 0;
    int old_active = 0;
    if (llg_net_has_index(net)) {
        llg_net_driver_index_t* node = &net->driver_index[idx];
        old_low = node->low;
        old_high = node->high;
        old_active = node->active;
        llg_net_index_remove(net, idx);
    }
    sv4_move(slot, &replacement);
    int new_active = !llg_net_all_z(slot);
    if (llg_net_has_index(net) && new_active)
        llg_net_index_insert(net, idx, 0, net->width - 1u);
    // A selected force may cover only part of the net. Recompute through the
    // force path so unforced bits still publish underlying driver changes.
    if (llg_is_forced(&net->resolved)) {
        llg_net_resolve(net);
        return;
    }
    if (!llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_resolve(net);
        return;
    }
    uint32_t low = old_active ? old_low : 0;
    uint32_t high = old_active ? old_high : net->width - 1u;
    if (!old_active && !new_active) return;
    if (new_active) {
        if (!old_active || low > 0) low = 0;
        if (!old_active || high < net->width - 1u) high = net->width - 1u;
    }
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(owned, llg_net_compute_range(net, low, high - low + 1u));
    llg_net_publish_range(net, low, owned[0]);
    llg_value_scope_end(scope);
}

static int llg_net_range_same(const sv4_t* target, uint32_t offset,
                              const sv4_t* value) {
    return llg_sv4_range_same(*target, offset, *value);
}

static void llg_net_range_fill_z(sv4_t* target, uint32_t offset,
                                 uint32_t width) {
    llg_sv4_range_fill(target, offset, width, 3);
}

static void llg_net_range_copy(sv4_t* target, uint32_t offset,
                               const sv4_t* value) {
    llg_sv4_range_copy(target, offset, *value);
}

static void llg_net_write_slice(llg_net_t* net, int idx, sv4_t selected,
                                uint32_t new_low) {
    sv4_t* slot = net->drivers[idx];
    uint32_t old_low = 0;
    uint32_t old_high = net->width - 1u;
    int old_active = 1;
    if (llg_net_has_index(net)) {
        llg_net_driver_index_t* node = &net->driver_index[idx];
        old_low = node->low;
        old_high = node->high;
        old_active = node->active;
    }
    int new_active = llg_sv4_width(selected) && !llg_net_all_z(&selected);
    uint32_t new_high = new_active ? new_low + llg_sv4_width(selected) - 1u : new_low;
    if (old_active == new_active &&
        (!new_active || (old_low == new_low && old_high == new_high &&
                         llg_net_range_same(slot, new_low, &selected)))) {
        return;
    }
    if (llg_net_has_index(net)) llg_net_index_remove(net, idx);
    if (llg_net_has_index(net)) {
        if (old_active) llg_net_range_fill_z(slot, old_low, old_high - old_low + 1u);
    } else {
        llg_net_range_fill_z(slot, 0, net->width);
        old_low = 0;
        old_high = net->width - 1u;
    }
    if (new_active) {
        llg_net_range_copy(slot, new_low, &selected);
        if (llg_net_has_index(net)) llg_net_index_insert(net, idx, new_low, new_high);
    }
    if (llg_is_forced(&net->resolved)) {
        llg_net_resolve(net);
        return;
    }
    if (!llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_resolve(net);
        return;
    }
    if (!old_active && !new_active) return;
    if (old_active && new_active &&
        (old_high + 1u < new_low || new_high + 1u < old_low)) {
        llg_value_scope_t* scope = llg_value_scope_begin(2);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute_range(
            net, old_low, old_high - old_low + 1u));
        sv4_replace(&owned[1], llg_net_compute_range(
            net, new_low, new_high - new_low + 1u));
        llg_net_publish_ranges(net, old_low, owned[0], new_low, owned[1]);
        llg_value_scope_end(scope);
        return;
    }
    uint32_t low = old_active ? old_low : new_low;
    uint32_t high = old_active ? old_high : new_high;
    if (new_active) {
        if (!old_active || new_low < low) low = new_low;
        if (!old_active || new_high > high) high = new_high;
    }
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(owned, llg_net_compute_range(net, low, high - low + 1u));
    llg_net_publish_range(net, low, owned[0]);
    llg_value_scope_end(scope);
}

void llg_net_write_selected(llg_net_t* net, int idx, sv4_t value,
                            sv4_select_plan_t plan, int reverse) {
    if (!net || !region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers || !net->drivers[idx] ||
        plan.storage_width != net->width) {
        fputs("llg: fatal: invalid selected net driver\n", stderr);
        abort();
    }
    sv4_t selected = sv4_select_plan_slice(value, &plan, reverse);
    llg_net_write_slice(net, idx, selected, plan.storage_lsb);
    sv4_destroy(&selected);
}

/* Grow one net's alias list to hold at least one more entry. The old table
 * stays live until the grown copy is complete, so an allocation failure aborts
 * without leaving the net partially rebound. */
static void llg_net_alias_reserve(llg_net_t* net) {
    if (net->n_aliases < net->alias_capacity) return;
    if (net->alias_capacity < 0) {
        fputs("llg: fatal: net alias capacity is invalid\n", stderr);
        abort();
    }
    int capacity = net->alias_capacity ? net->alias_capacity : 4;
    while (capacity <= net->n_aliases) {
        if (capacity > INT_MAX / 2) {
            fputs("llg: fatal: net alias capacity overflow\n", stderr);
            abort();
        }
        capacity *= 2;
    }
    llg_net_alias_t** grown = (llg_net_alias_t**)llg_checked_malloc(
        (size_t)capacity, sizeof(*grown), "net alias table");
    for (int i = 0; i < net->n_aliases; i++) grown[i] = net->aliases[i];
    free(net->aliases);
    net->aliases = grown;
    net->alias_capacity = capacity;
}

void llg_net_alias_bind(llg_net_alias_t* alias) {
    if (!alias || !alias->parts || alias->n_parts == 0) return;
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        llg_net_t* net = alias->parts[i].net;
        if (!net) continue;
        int seen = 0;
        for (int j = 0; j < net->n_aliases; j++)
            if (net->aliases[j] == alias) seen = 1;
        if (seen) continue;
        llg_net_alias_reserve(net);
        net->aliases[net->n_aliases++] = alias;
    }
    llg_net_alias_refresh(alias);
}

void llg_net_alias_clear(llg_net_t* net) {
    if (!net) return;
    free(net->aliases);
    net->aliases = NULL;
    net->n_aliases = 0;
    net->alias_capacity = 0;
}

sv4_t llg_net_alias_read(llg_net_alias_t* alias) {
    // Driver/force/propagation commits publish this view before readers run.
    // Observing it, including from Postponed, must never perform a write.
    return alias ? sv4_clone(&alias->visible) : sv4_from_u64(0, 1, 0);
}

void llg_net_alias_write(llg_net_alias_t* alias, sv4_t value) {
    if (!alias || !alias->parts || !region_can_mutate("net alias write")) return;
    /* A publication callback may reenter or terminate the current process.
     * Both the input snapshot and each contribution must outlive that edge. */
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_copy(&owned[0], &value);
    for (uint32_t i = 0; i < alias->n_parts; i++) {
        const llg_net_alias_part_t* part = &alias->parts[i];
        if (!part->net) continue;
        int seen = 0;
        for (uint32_t j = 0; j < i; j++) {
            const llg_net_alias_part_t* prior = &alias->parts[j];
            if (prior->net == part->net && prior->slot == part->slot) seen = 1;
        }
        if (seen) continue;
        sv4_replace(&owned[1], sv4_fill(3, part->net->width, part->net->is_signed));
        for (uint32_t j = i; j < alias->n_parts; j++) {
            const llg_net_alias_part_t* mapped = &alias->parts[j];
            if (mapped->net != part->net || mapped->slot != part->slot ||
                mapped->bit_count == 0 ||
                (uint64_t)mapped->signal_bit + mapped->bit_count > llg_sv4_width(owned[0]) ||
                (uint64_t)mapped->group_bit + mapped->bit_count > llg_sv4_width(owned[1]))
                continue;
            sv4_t bits = sv4_part_select(owned[0],
                                         (int64_t)mapped->signal_bit + mapped->bit_count - 1,
                                         mapped->signal_bit);
            sv4_part_select_set(&owned[1], (int64_t)mapped->group_bit + mapped->bit_count - 1,
                                mapped->group_bit, bits);
            sv4_destroy(&bits);
        }
        llg_net_write(part->net, part->slot, owned[1]);
        sv4_destroy(&owned[1]);
    }
    llg_value_scope_end(scope);
}

static int inertial_bit(const sv4_t* value, uint32_t bit) {
    if (!value || bit >= llg_sv4_width(*value)) return 0;
    return (int)llg_sv4_state(*value, bit);
}

static int inertial_masked_same(const sv4_t* a, const sv4_t* b,
                                const sv4_t* mask) {
    return llg_sv4_masked_same(*a, *b, mask);
}

static void inertial_merge(sv4_t* target, const sv4_t* value,
                           const sv4_t* mask) {
    llg_sv4_masked_copy(target, *value, *mask);
}

enum {
    INERTIAL_NO_TRANSITION = 0,
    INERTIAL_RISE = 1,
    INERTIAL_FALL = 2,
    INERTIAL_TURN_OFF = 3,
    INERTIAL_RISE_OR_FALL = 4,
};

static int inertial_transition(int old_state, int new_state) {
    if (old_state == new_state) return INERTIAL_NO_TRANSITION;
    if (new_state == 3) return INERTIAL_TURN_OFF;
    if (new_state == 1) {
        return old_state == 0 || old_state == 2 || old_state == 3
                   ? INERTIAL_RISE
                   : INERTIAL_NO_TRANSITION;
    }
    if (new_state == 0) {
        return old_state == 1 || old_state == 2 || old_state == 3
                   ? INERTIAL_FALL
                   : INERTIAL_NO_TRANSITION;
    }
    // A transition to X is ambiguous: its delay is selected from every
    // possible stable destination (0, 1, or Z). The known endpoint remains
    // directional when the transition is X -> 0/1.
    if (old_state == 0 || old_state == 1 || old_state == 3)
        return INERTIAL_RISE_OR_FALL;
    return INERTIAL_NO_TRANSITION;
}

static uint64_t inertial_transition_ticks(const sv4_t* old_value,
                                           const sv4_t* new_value,
                                           const sv4_t* mask, uint64_t rise,
                                           uint64_t fall, uint64_t turn_off) {
    uint64_t selected = UINT64_MAX;
    int has_transition = 0;
    uint32_t width = llg_sv4_width(*old_value) < llg_sv4_width(*new_value)
                         ? llg_sv4_width(*old_value)
                         : llg_sv4_width(*new_value);
    for (uint32_t bit = 0; bit < width; bit++) {
        if (mask && inertial_bit(mask, bit) != 1) continue;
        int transition = inertial_transition(
            inertial_bit(old_value, bit), inertial_bit(new_value, bit));
        uint64_t ticks;
        switch (transition) {
        case INERTIAL_RISE: ticks = rise; break;
        case INERTIAL_FALL: ticks = fall; break;
        case INERTIAL_TURN_OFF: ticks = turn_off; break;
        case INERTIAL_RISE_OR_FALL:
            ticks = rise < fall ? rise : fall;
            if (turn_off < ticks) ticks = turn_off;
            break;
        default: continue;
        }
        // UINT64_MAX is a valid delay. Track whether a transition was found
        // separately so the maximum delay is not mistaken for "no transition"
        // and silently converted to zero.
        if (!has_transition || ticks < selected) selected = ticks;
        has_transition = 1;
    }
    return has_transition ? selected : 0;
}

static void inertial_unlink_pending(llg_inertial_t* driver) {
    if (!driver->pending) return;
    llg_inertial_t** entry = &g.inertial_pending;
    while (*entry && *entry != driver) entry = &(*entry)->next_pending;
    if (*entry) *entry = driver->next_pending;
    driver->pending = 0;
    driver->next_pending = NULL;
    sv4_destroy(&driver->value);
    sv4_destroy(&driver->mask);
    driver->has_mask = 0;

}

static void inertial_update(llg_inertial_t** handle, sv4_t* target,
                            llg_net_t* net, int slot, sv4_t value,
                            const sv4_t* mask, uint64_t rise, uint64_t fall,
                            uint64_t turn_off) {
    if (!region_can_mutate("inertial scheduling")) return;
    llg_inertial_t* driver = *handle;
    if (!driver) {
        driver = llg_checked_calloc(1, sizeof(*driver), "inertial driver");
        driver->handle = handle;
        driver->target = target;
        driver->net = net;
        driver->slot = slot;
        sv4_copy(&driver->current, target);
        driver->region = region_is_reactive(g.current_region)
                             ? LLG_REGION_REACTIVE
                             : LLG_REGION_ACTIVE;
        driver->next_all = g.inertial_drivers;
        g.inertial_drivers = driver;
        *handle = driver;
    } else if (driver->target != target || driver->net != net || driver->slot != slot) {
        inertial_unlink_pending(driver);
        driver->target = target;
        driver->net = net;
        driver->publication_net = NULL;
        driver->slot = slot;
        sv4_copy(&driver->current, target);
    }
    driver->region = region_is_reactive(g.current_region)
                         ? LLG_REGION_REACTIVE
                         : LLG_REGION_ACTIVE;
    value = sv4_resize(value, llg_sv4_width(*target), llg_sv4_signed(*target));
    sv4_t selected_mask = SV4_EMPTY;
    if (mask) {
        selected_mask = sv4_resize(*mask, llg_sv4_width(*target), 0);
    }
    const sv4_t* effective_mask = mask ? &selected_mask : NULL;
    uint64_t ticks;
    llg_inertial_t** entry;
    if (driver->pending) {
        // Unchanged expression values keep the original propagation time.
        if (driver->has_mask == (effective_mask != NULL) &&
            (!effective_mask || sv4_same(driver->mask, *effective_mask)) &&
            inertial_masked_same(&driver->value, &value, effective_mask))
            goto cleanup;
        inertial_unlink_pending(driver);
    }
    if (effective_mask ? inertial_masked_same(&driver->current, &value, effective_mask)
                       : sv4_same(driver->current, value))
        goto cleanup;
    driver->has_mask = effective_mask != NULL;
    if (effective_mask) sv4_copy(&driver->mask, effective_mask);
    driver->rise = rise;
    driver->fall = fall;
    driver->turn_off = turn_off;
    ticks = inertial_transition_ticks(
        &driver->current, &value, effective_mask, rise, fall, turn_off);
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling an inertial update\n");
        abort();
    }
    sv4_move(&driver->value, &value);
    driver->time = g.now + ticks;
    driver->pending = 1;
    entry = &g.inertial_pending;
    while (*entry && (*entry)->time <= driver->time) entry = &(*entry)->next_pending;
    driver->next_pending = *entry;
    *entry = driver;
cleanup:
    sv4_destroy(&value);
    sv4_destroy(&selected_mask);

}

void llg_inertial_assign(llg_inertial_t** handle, sv4_t* target,
                         sv4_t value, uint64_t rise, uint64_t fall,
                         uint64_t turn_off) {
    inertial_update(handle, target, NULL, 0, value, NULL, rise, fall, turn_off);
}

void llg_inertial_net(llg_inertial_t** handle, llg_net_t* net, int slot,
                      sv4_t value, uint64_t rise, uint64_t fall,
                      uint64_t turn_off) {
    if (slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) {
        fprintf(stderr, "llg: fatal: invalid inertial net driver slot\n");
        abort();
    }
    inertial_update(handle, net->drivers[slot], net, slot, value, NULL, rise,
                    fall, turn_off);
}

void llg_inertial_selected_assign(llg_inertial_t** handle, sv4_t* target,
                                  sv4_t value, sv4_t mask, uint64_t rise,
                                  uint64_t fall, uint64_t turn_off) {
    inertial_update(handle, target, NULL, 0, value, &mask, rise, fall, turn_off);
}

void llg_inertial_selected_net(llg_inertial_t** handle, llg_net_t* net,
                               int slot, sv4_t value, sv4_t mask,
                               uint64_t rise, uint64_t fall,
                               uint64_t turn_off) {
    if (slot < 0 || slot >= net->n_drivers || !net->drivers[slot]) {
        fprintf(stderr, "llg: fatal: invalid inertial net driver slot\n");
        abort();
    }
    inertial_update(handle, net->drivers[slot], net, slot, value, &mask, rise,
                    fall, turn_off);
}

static int inertial_ready(llg_region_t region) {
    for (llg_inertial_t* driver = g.inertial_pending; driver;
         driver = driver->next_pending) {
        if (driver->time == g.now && driver->region == region) return 1;
    }
    return 0;
}

static void commit_inertial(llg_region_t region) {
    llg_inertial_t** slot = &g.inertial_pending;
    while (*slot && ((*slot)->time != g.now || (*slot)->region != region))
        slot = &(*slot)->next_pending;
    llg_inertial_t* driver = *slot;
    if (!driver) return;
    *slot = driver->next_pending;
    driver->next_pending = NULL;
    driver->pending = 0;

    // Detach before publishing: callbacks may schedule another update on driver.
    sv4_t value = SV4_EMPTY;
    sv4_t mask = SV4_EMPTY;
    sv4_move(&value, &driver->value);
    sv4_move(&mask, &driver->mask);
    int has_mask = driver->has_mask;
    driver->has_mask = 0;
    llg_net_t* publication_net = driver->publication_net;
    if (has_mask) {
        sv4_t merged = sv4_clone(driver->target);
        inertial_merge(&merged, &value, &mask);
        sv4_move(&value, &merged);
    }
    uint32_t range_offset = 0;
    uint32_t range_width = 0;
    int selected_net = driver->net && has_mask &&
        nba_mask_contiguous(driver->net->width, value, mask,
                            &range_offset, &range_width);
    if (selected_net) {
        sv4_t selected = range_width
            ? sv4_part_select(value, range_offset + range_width - 1u,
                              range_offset)
            : (sv4_t)SV4_EMPTY;
        sv4_t current = sv4_fill(3, driver->net->width,
                                 driver->net->is_signed);
        if (range_width)
            llg_net_range_copy(&current, range_offset, &selected);
        sv4_copy(&driver->current, &current);
        sv4_destroy(&current);
        llg_net_write_slice(driver->net, driver->slot, selected, range_offset);
        sv4_destroy(&selected);
    } else if (driver->net) {
        sv4_copy(&driver->current, &value);
        llg_net_write(driver->net, driver->slot, value);
    } else {
        sv4_copy(&driver->current, &value);
        llg_ba(driver->target, value);
    }
    if (publication_net) llg_net_alias_refresh_all(publication_net);
    sv4_destroy(&mask);
    sv4_destroy(&value);
}
