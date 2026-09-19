
// ── Collapsed inout nets ──────────────────────────────────────────────────────

static sv4_t llg_net_compute(const llg_net_t* net) {
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
        if (!part->net || part->signal_bit >= owned[0].width ||
            part->group_bit >= part->net->resolved.width)
            continue;
        sv4_t bit = sv4_bit_select(part->net->resolved, part->group_bit);
        sv4_bit_select_set(&owned[0], part->signal_bit, bit);
        sv4_destroy(&bit);
    }
    // The visible cell is a first-class dependency/waveform target. Route
    // updates through the ordinary signal writer so waiters and waveform
    // callbacks observe canonical alias changes.
    sig_write(&alias->visible, owned[0]);
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
    sv4_move(slot, &replacement);
    // Driver slots keep changing underneath a force; release must observe them.
    llg_net_resolve(net);
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
                mapped->signal_bit >= owned[0].width ||
                mapped->group_bit >= owned[1].width)
                continue;
            sv4_t bit = sv4_bit_select(owned[0], mapped->signal_bit);
            sv4_bit_select_set(&owned[1], mapped->group_bit, bit);
            sv4_destroy(&bit);
        }
        llg_net_write(part->net, part->slot, owned[1]);
        sv4_destroy(&owned[1]);
    }
    llg_value_scope_end(scope);
}

static int inertial_bit(const sv4_t* value, uint32_t bit) {
    if (!value || bit >= value->width) return 0;
    uint64_t mask = 1ULL << (bit % 64u);
    uint32_t limb = bit / 64u;
    if (value->x[limb] & mask) return 2;
    if (value->z[limb] & mask) return 3;
    return (value->bits[limb] & mask) ? 1 : 0;
}

static void inertial_set_bit(sv4_t* value, uint32_t bit, int state) {
    uint64_t mask = 1ULL << (bit % 64u);
    uint32_t limb = bit / 64u;
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
}

static int inertial_masked_same(const sv4_t* a, const sv4_t* b,
                                const sv4_t* mask) {
    uint32_t width = a->width < b->width ? a->width : b->width;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (mask && inertial_bit(mask, bit) != 1) continue;
        if (inertial_bit(a, bit) != inertial_bit(b, bit)) return 0;
    }
    return 1;
}

static void inertial_merge(sv4_t* target, const sv4_t* value,
                           const sv4_t* mask) {
    uint32_t width = target->width < value->width ? target->width : value->width;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (inertial_bit(mask, bit) == 1)
            inertial_set_bit(target, bit, inertial_bit(value, bit));
    }
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
    uint32_t width = old_value->width < new_value->width
                         ? old_value->width
                         : new_value->width;
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
    value = sv4_resize(value, target->width, target->is_signed);
    sv4_t selected_mask = SV4_EMPTY;
    if (mask) {
        selected_mask = sv4_resize(*mask, target->width, 0);
    }
    const sv4_t* effective_mask = mask ? &selected_mask : NULL;
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
    uint64_t ticks = inertial_transition_ticks(
        &driver->current, &value, effective_mask, rise, fall, turn_off);
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling an inertial update\n");
        abort();
    }
    sv4_move(&driver->value, &value);
    driver->time = g.now + ticks;
    driver->pending = 1;
    llg_inertial_t** entry = &g.inertial_pending;
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
    sv4_copy(&driver->current, &value);
    if (driver->net) llg_net_write(driver->net, driver->slot, value);
    else llg_ba(driver->target, value);
    if (publication_net) llg_net_alias_refresh_all(publication_net);
    sv4_destroy(&mask);
    sv4_destroy(&value);
}
