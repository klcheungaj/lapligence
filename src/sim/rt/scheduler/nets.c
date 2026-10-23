
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

// The lowest and highest bit of `value` that is not Z; 0 when all bits are
// Z. Z bits add no drive, so a driver is indexed by this extent only.
static int llg_net_driven_extent(const sv4_t* value, uint32_t* low,
                                 uint32_t* high) {
    uint32_t width = llg_sv4_width(*value);
    int limbs = (int)((width + 63u) / 64u);
    int found = 0;
    for (int limb = 0; limb < limbs; limb++) {
        uint32_t remaining = width - (uint32_t)limb * 64u;
        uint64_t mask = remaining >= 64u
            ? UINT64_MAX : (UINT64_C(1) << remaining) - UINT64_C(1);
        uint64_t driven = (llg_sv4_word(*value, limb, LLG_SV4_BITS) |
                           llg_sv4_word(*value, limb, LLG_SV4_X) |
                           ~llg_sv4_word(*value, limb, LLG_SV4_Z)) & mask;
        if (!driven) continue;
        uint32_t first = 0;
        while (!((driven >> first) & 1u)) first++;
        uint32_t last = 63u;
        while (!((driven >> last) & 1u)) last--;
        if (!found) *low = (uint32_t)limb * 64u + first;
        *high = (uint32_t)limb * 64u + last;
        found = 1;
    }
    return found;
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

// ── Start X of undelayed driven nets (T0-D3) ─────────────────────────────────

static size_t llg_net_pending_words(const llg_net_t* net) {
    return ((size_t)net->width + 63u) / 64u;
}

// Each pending bit keeps its start X whatever the written drivers resolve
// to: the drivers that cover it have not evaluated yet.
static void llg_net_pending_overlay(const llg_net_t* net, sv4_t* value) {
    const uint64_t* pending = net->start_pending;
    size_t words = llg_net_pending_words(net);
    for (size_t word = 0; word < words; word++) {
        uint64_t mask = pending[word];
        if (!mask) continue;
        uint32_t base = (uint32_t)(word * 64u);
        if (mask == UINT64_MAX) {
            llg_sv4_range_fill(value, base, 64u, 2);
            continue;
        }
        for (uint32_t bit = 0; bit < 64u; bit++)
            if (mask >> bit & 1u) llg_sv4_range_fill(value, base + bit, 1u, 2);
    }
}

static void llg_net_pending_mark(llg_net_t* net, uint32_t low, uint32_t count,
                                 int pending) {
    if (low >= net->width) return;
    if (count > net->width - low) count = net->width - low;
    while (count) {
        uint32_t chunk = 64u - low % 64u;
        if (chunk > count) chunk = count;
        uint64_t mask = (UINT64_MAX >> (64u - chunk)) << (low % 64u);
        if (pending)
            net->start_pending[low / 64u] |= mask;
        else
            net->start_pending[low / 64u] &= ~mask;
        low += chunk;
        count -= chunk;
    }
}

// A driver write covering [low, low + count) is the first write of those
// bits' drivers. The net settles when no bit is left pending; a net without
// a pending mask settles at its first write.
static void llg_net_settle_cover(llg_net_t* net, uint32_t low, uint32_t count) {
    uint64_t* pending = net->start_pending;
    if (!pending) {
        net->unsettled = 0;
        return;
    }
    llg_net_pending_mark(net, low, count, 0);
    size_t words = llg_net_pending_words(net);
    for (size_t word = 0; word < words; word++)
        if (pending[word]) return;
    llg_net_start_release(net);
}

void llg_net_start_release(llg_net_t* net) {
    if (!net) return;
    free(net->start_pending);
    net->start_pending = NULL;
    net->unsettled = 0;
}

static sv4_t llg_net_compute(llg_net_t* net) {
    sv4_t value = llg_net_has_index(net)
        ? llg_net_compute_range(net, 0, net->width)
        : sv4_resolve_strengths(
              (const sv4_t* const*)net->drivers, net->strength0, net->strength1,
              net->n_drivers, net->width, net->is_signed, net->resolution);
    if (net->start_pending) llg_net_pending_overlay(net, &value);
    return value;
}

static void llg_net_alias_refresh(llg_net_alias_t* alias) {
    if (!alias || !alias->storage) return;
    llg_value_scope_t* scope = value_scope_begin_private(1);
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


// ── Strength views ───────────────────────────────────────────────────────────
//
// Only nets with a strength consumer carry a view, so ordinary resolution
// keeps its value-only word-parallel path. Every driver contributes one
// strength interval per bit on the signed scale (-7 = Su0 .. 7 = Su1): a
// known value is a point, X spans [-strength0, strength1], and a highz
// endpoint leaves only the other side, which models L/H. The resolved range
// is the hull of the outcomes of every level choice inside those intervals
// (IEEE 1364-2001 7.10.1-7.10.4): the strongest level wins, equal opposite
// levels give X on a wire and the wired value on wand/wor. Only the strongest
// definite and possible levels of each value decide that hull.

static int llg_net_top_level(const uint64_t* levels, uint64_t bit) {
    for (int level = LLG_STRENGTH_SUPPLY; level > LLG_STRENGTH_HIGHZ; level--)
        if (levels[level] & bit) return level;
    return -1;
}

static uint8_t llg_net_strength_code(int lo, int hi) {
    return (uint8_t)(((hi + 7) << 4) | (lo + 7));
}

static uint8_t llg_net_strength_hull(int mode, int k0, int k1, int p0, int p1) {
    int hi;
    int lo;
    if (mode == LLG_RESOLVE_WAND) {
        hi = p1 >= 0 && p1 > k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 >= k1 ? -p0 : k1 >= 0 ? k1 : 0;
    } else if (mode == LLG_RESOLVE_WOR) {
        hi = p1 >= 0 && p1 >= k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 > k1 ? -p0 : k1 >= 0 ? k1 : 0;
    } else {
        hi = p1 >= 0 && p1 >= k0 ? p1 : k0 >= 0 ? -k0 : 0;
        lo = p0 >= 0 && p0 >= k1 ? -p0 : k1 >= 0 ? k1 : 0;
    }
    return llg_net_strength_code(lo, hi);
}

static uint8_t llg_net_forced_strength(const sv4_t* resolved, uint64_t bit) {
    switch (llg_sv4_state(*resolved, bit)) {
    case 0: return llg_net_strength_code(-LLG_STRENGTH_STRONG, -LLG_STRENGTH_STRONG);
    case 1: return llg_net_strength_code(LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG);
    case 2: return llg_net_strength_code(-LLG_STRENGTH_STRONG, LLG_STRENGTH_STRONG);
    default: return llg_net_strength_code(0, 0);
    }
}

static sv4_t llg_net_strength_levels(llg_net_t* net) {
    uint32_t width = net->width;
    size_t words = ((size_t)width + 63u) / 64u;
    size_t out_words = ((size_t)width * 8u + 63u) / 64u;
    uint64_t* out = (uint64_t*)llg_checked_calloc(out_words, sizeof(uint64_t),
                                                  "net strength view");
    int forced = llg_is_forced(&net->resolved);
    int mode = net->resolution;
    int implicit = -1;
    int implicit_strength = LLG_STRENGTH_PULL;
    if (mode == LLG_RESOLVE_TRI0 || mode == LLG_RESOLVE_SUPPLY0) implicit = 0;
    if (mode == LLG_RESOLVE_TRI1 || mode == LLG_RESOLVE_SUPPLY1) implicit = 1;
    if (mode == LLG_RESOLVE_SUPPLY0 || mode == LLG_RESOLVE_SUPPLY1)
        implicit_strength = LLG_STRENGTH_SUPPLY;
    for (size_t word = 0; word < words; word++) {
        uint32_t remaining = width - (uint32_t)(word * 64u);
        uint64_t m = remaining >= 64u ? UINT64_MAX
                                      : (UINT64_C(1) << remaining) - UINT64_C(1);
        uint64_t known0[8] = {0};
        uint64_t known1[8] = {0};
        uint64_t possible0[8] = {0};
        uint64_t possible1[8] = {0};
        if (implicit == 0) known0[implicit_strength] = possible0[implicit_strength] = m;
        if (implicit == 1) known1[implicit_strength] = possible1[implicit_strength] = m;
        for (int d = 0; d < net->n_drivers; d++) {
            const sv4_t* v = net->drivers[d];
            if (!v) continue;
            uint8_t s0 = net->strength0 ? net->strength0[d] : LLG_STRENGTH_STRONG;
            uint8_t s1 = net->strength1 ? net->strength1[d] : LLG_STRENGTH_STRONG;
            if (s0 > LLG_STRENGTH_SUPPLY || s1 > LLG_STRENGTH_SUPPLY) {
                fputs("llg runtime fatal: invalid net drive strength\n", stderr);
                abort();
            }
            uint64_t valid = word < llg_sv4_words(*v)
                ? (llg_sv4_width(*v) - (uint32_t)(word * 64u) >= 64u
                       ? UINT64_MAX
                       : (UINT64_C(1) << (llg_sv4_width(*v) - (uint32_t)(word * 64u))) -
                             UINT64_C(1))
                : 0;
            uint64_t x = llg_sv4_word(*v, word, LLG_SV4_X) & valid & m;
            uint64_t z = (llg_sv4_word(*v, word, LLG_SV4_Z) | ~valid) & m;
            uint64_t bits = llg_sv4_word(*v, word, LLG_SV4_BITS);
            uint64_t k0 = ~bits & ~(x | z) & m;
            uint64_t k1 = bits & ~(x | z) & m;
            if (s0 != LLG_STRENGTH_HIGHZ) {
                known0[s0] |= k0;
                possible0[s0] |= k0 | x;
            }
            if (s1 != LLG_STRENGTH_HIGHZ) {
                known1[s1] |= k1;
                possible1[s1] |= k1 | x;
            }
        }
        uint64_t forced_bits = forced ? force_mask_word(&net->resolved, word) : 0;
        for (uint32_t offset = 0; offset < 64u && offset < remaining; offset++) {
            uint64_t bit = UINT64_C(1) << offset;
            uint64_t net_bit = (uint64_t)word * 64u + offset;
            uint8_t code = forced_bits & bit
                ? llg_net_forced_strength(&net->resolved, net_bit)
                : llg_net_strength_hull(mode, llg_net_top_level(known0, bit),
                                        llg_net_top_level(known1, bit),
                                        llg_net_top_level(possible0, bit),
                                        llg_net_top_level(possible1, bit));
            out[net_bit / 8u] |= (uint64_t)code << ((net_bit % 8u) * 8u);
        }
    }
    sv4_t view = sv4_from_limbs(out, NULL, NULL, width * 8u, 0);
    free(out);
    return view;
}

// Publish the strength view after the value. Unchanged strengths are not
// rewritten, so only a real strength change wakes `%v` monitors. A pending
// net propagation delay publishes the view with the delayed value instead.
static int llg_net_propagation_pending(const llg_net_t* net) {
    if (net->propagation && net->propagation->pending) return 1;
    for (uint32_t part = 0; part < net->n_segments; part++) {
        if (net->segment_records[part] && net->segment_records[part]->pending)
            return 1;
    }
    if (!net->propagation_bits) return 0;
    for (uint32_t bit = 0; bit < net->width; bit++) {
        if (net->propagation_bits[bit] && net->propagation_bits[bit]->pending)
            return 1;
    }
    return 0;
}

static void llg_net_strength_publish(llg_net_t* net) {
    if (!net || !net->strength) return;
    if (llg_net_propagation_pending(net)) return;
    llg_value_scope_t* scope = value_scope_begin_private(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(&owned[0], llg_net_strength_levels(net));
    if (net->start_pending) {
        // Pending bits keep the StX view llg_net_start_driven gave them.
        uint64_t code = llg_net_strength_code(-LLG_STRENGTH_STRONG,
                                              LLG_STRENGTH_STRONG);
        for (uint32_t bit = 0; bit < net->width; bit++) {
            if (!(net->start_pending[bit / 64u] >> (bit % 64u) & 1u)) continue;
            sv4_t byte = sv4_from_u64(code, 8, 0);
            sv4_part_select_set(&owned[0], (int64_t)bit * 8 + 7, (int64_t)bit * 8, byte);
            sv4_destroy(&byte);
        }
    }
    sig_write(net->strength, owned[0]);
    llg_value_scope_end(scope);
}

// Startup: initialize the view from the all-Z drivers without waking readers.
void llg_net_strength_reset(llg_net_t* net) {
    if (!net || !net->strength) return;
    sv4_replace(net->strength, llg_net_strength_levels(net));
}

static void llg_net_delay_stage(llg_net_t* net, sv4_t resolved);
static void llg_net_propagate_paths(llg_net_t* net, sv4_t resolved);

static void llg_net_publish(llg_net_t* net, sv4_t resolved) {
    // An undelayed net pays one flag test.
    if (!net->propagation_enabled) {
        sig_write(&net->resolved, resolved);
        llg_net_alias_refresh_all(net);
    } else if (net->path_dest) {
        llg_net_propagate_paths(net, resolved);
    } else {
        llg_net_delay_stage(net, resolved);
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

void llg_net_start_unknown(llg_net_t* net, int resolve) {
    if (!net) return;
    if (net->strength) {
        size_t words = ((size_t)net->width * 8u + 63u) / 64u;
        uint64_t* out = (uint64_t*)llg_checked_calloc(words, sizeof(uint64_t),
                                                      "net strength view");
        uint64_t code = llg_net_strength_code(-LLG_STRENGTH_STRONG,
                                              LLG_STRENGTH_STRONG);
        for (uint32_t bit = 0; bit < net->width; bit++)
            out[bit / 8u] |= code << ((bit % 8u) * 8u);
        sv4_replace(net->strength, sv4_from_limbs(out, NULL, NULL, net->width * 8u, 0));
        free(out);
    }
    if (resolve) llg_net_resolve(net);
}

void llg_net_start_driven(llg_net_t* net, const uint32_t* runs, uint32_t n_runs) {
    if (!net || !runs) return;
    llg_net_start_release(net);
    // One driver covers every driven bit at its first write; several drivers
    // each settle their own bits.
    if (net->n_drivers > 1)
        net->start_pending = (uint64_t*)llg_checked_calloc(
            llg_net_pending_words(net), sizeof(uint64_t), "net start bits");
    for (uint32_t run = 0; run < n_runs; run++) {
        uint32_t low = runs[2u * run];
        uint32_t count = runs[2u * run + 1u];
        if (count == 0 || low >= net->width || count > net->width - low) continue;
        llg_sv4_range_fill(&net->resolved, low, count, 2);
        if (net->start_pending) llg_net_pending_mark(net, low, count, 1);
        if (net->strength) {
            uint64_t code = llg_net_strength_code(-LLG_STRENGTH_STRONG,
                                                  LLG_STRENGTH_STRONG);
            for (uint32_t bit = low; bit < low + count; bit++) {
                sv4_t byte = sv4_from_u64(code, 8, 0);
                sv4_part_select_set(net->strength, (int64_t)bit * 8 + 7,
                                    (int64_t)bit * 8, byte);
                sv4_destroy(&byte);
            }
        }
    }
    net->unsettled = 1;
}

void llg_net_resolve(llg_net_t* net) {
    if (!net || !region_can_mutate("net resolution")) return;
    if (llg_is_forced(&net->resolved)) {
        force_recompute_target(&net->resolved, net);
    } else {
        llg_value_scope_t* scope = value_scope_begin_private(1);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute(net));
        llg_net_publish(net, owned[0]);
        llg_value_scope_end(scope);
    }
    llg_net_strength_publish(net);
}

// ── Deferred net updates ─────────────────────────────────────────────────────
// The zero-delay drivers that one drain of a region queue runs write their
// driver slots at once, but a net with several driver slots is resolved and
// published only when that queue is empty. The drivers' evaluations then all
// precede the net's update, an order IEEE 1800-2009 4.7 leaves open, so one
// tri-state driver turning off before another turns on in the same pass is
// no glitch on the net. The table is empty outside run_region_queue.
typedef struct {
    llg_net_t* net;
    uint32_t low;
    uint32_t high;
    uint8_t whole;
} llg_net_deferred_t;

static llg_net_deferred_t* net_deferred;
static size_t net_deferred_count;
static size_t net_deferred_capacity;
static int net_defer_active;

// Queue the net's resolution of [low, high] (every bit when `whole`).
static int llg_net_defer(llg_net_t* net, int whole, uint32_t low, uint32_t high) {
    if (!net_defer_active || net->n_drivers < 2) return 0;
    if (net->deferred_slot > 0) {
        llg_net_deferred_t* entry = &net_deferred[net->deferred_slot - 1];
        if (whole) entry->whole = 1;
        if (low < entry->low) entry->low = low;
        if (high > entry->high) entry->high = high;
        return 1;
    }
    if (net_deferred_count >= (size_t)INT32_MAX) return 0;
    if (net_deferred_count == net_deferred_capacity) {
        size_t capacity = net_deferred_capacity ? net_deferred_capacity * 2u : 64u;
        llg_net_deferred_t* grown = (llg_net_deferred_t*)llg_checked_calloc(
            capacity, sizeof(*grown), "deferred net table");
        if (net_deferred_count)
            memcpy(grown, net_deferred, net_deferred_count * sizeof(*grown));
        free(net_deferred);
        net_deferred = grown;
        net_deferred_capacity = capacity;
    }
    net_deferred[net_deferred_count] = (llg_net_deferred_t){net, low, high, (uint8_t)(whole != 0)};
    net->deferred_slot = (int32_t)++net_deferred_count;
    return 1;
}

static void llg_net_publish_computed_range(llg_net_t* net, uint32_t low, uint32_t high) {
    llg_value_scope_t* scope = value_scope_begin_private(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_replace(owned, llg_net_compute_range(net, low, high - low + 1u));
    llg_net_publish_range(net, low, owned[0]);
    llg_value_scope_end(scope);
    llg_net_strength_publish(net);
}

static void llg_net_commit_whole(llg_net_t* net) {
    if (llg_net_defer(net, 1, 0, net->width ? net->width - 1u : 0)) return;
    llg_net_resolve(net);
}

static void llg_net_commit_range(llg_net_t* net, uint32_t low, uint32_t high) {
    if (llg_net_defer(net, 0, low, high)) return;
    llg_net_publish_computed_range(net, low, high);
}

// Resolve every queued net in first-write order. Publication wakes waiters
// into their region queues; writes made by callbacks meanwhile publish at
// once.
static int llg_net_flush_deferred(void) {
    if (!net_deferred_count) return 0;
    int outer = net_defer_active;
    net_defer_active = 0;
    for (size_t i = 0; i < net_deferred_count; i++) {
        llg_net_deferred_t entry = net_deferred[i];
        entry.net->deferred_slot = 0;
        if (entry.whole || llg_is_forced(&entry.net->resolved) ||
            !llg_net_has_index(entry.net) || entry.net->propagation_enabled ||
            entry.net->unsettled)
            llg_net_resolve(entry.net);
        else
            llg_net_publish_computed_range(entry.net, entry.low, entry.high);
    }
    net_deferred_count = 0;
    net_defer_active = outer;
    return 1;
}

static void llg_net_write_covering(llg_net_t* net, int idx, sv4_t value,
                                   uint32_t cover_low, uint32_t cover_count);

void llg_net_write(llg_net_t* net, int idx, sv4_t value) {
    llg_net_write_covering(net, idx, value, 0, net ? net->width : 0);
}

void llg_net_write_driven(llg_net_t* net, int idx, sv4_t value) {
    if (!net) return;
    if (net->start_pending) {
        uint32_t width = llg_sv4_width(value);
        if (width > net->width) width = net->width;
        for (uint32_t low = 0; low < width; low += 64u) {
            uint32_t count = width - low < 64u ? width - low : 64u;
            uint64_t all = UINT64_MAX >> (64u - count);
            uint64_t driven = ~llg_sv4_plane_slice(value, low, count, LLG_SV4_Z) & all;
            net->start_pending[low / 64u] &= ~driven;
        }
    }
    llg_net_write_covering(net, idx, value, 0, 0);
}

// A whole-slot write whose driver drives [cover_low, cover_low + cover_count)
// of the net (the whole net except for a port alias).
static void llg_net_write_covering(llg_net_t* net, int idx, sv4_t value,
                                   uint32_t cover_low, uint32_t cover_count) {
    if (!net || !region_can_mutate("net write")) return;
    if (idx < 0 || idx >= net->n_drivers || !net->drivers[idx]) return;
    sv4_t* slot = net->drivers[idx];
    // The first write of a driver of a net that started X resolves it whole,
    // even when the driver's value equals its all-Z slot (T0-D3).
    int settle = net->unsettled;
    if (settle) llg_net_settle_cover(net, cover_low, cover_count);
    // A same-width value needs no resize to compare: an unchanged driver
    // (most gate and assignment wakes) returns without copying it.
    if (LLG_GATE_FAST_PATH && llg_sv4_width(value) == net->width &&
        sv4_same(*slot, value)) {
        if (settle) llg_net_commit_whole(net);
        return;
    }
    sv4_t replacement = sv4_resize(value, net->width, net->is_signed);
    if (sv4_same(*slot, replacement)) {
        sv4_destroy(&replacement);
        if (settle) llg_net_commit_whole(net);
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
    uint32_t new_low = 0;
    uint32_t new_high = net->width - 1u;
    int new_active;
    if (LLG_GATE_FAST_PATH)
        new_active = llg_net_driven_extent(slot, &new_low, &new_high);
    else
        new_active = !llg_net_all_z(slot);
    if (llg_net_has_index(net) && new_active)
        llg_net_index_insert(net, idx, new_low, new_high);
    // A selected force may cover only part of the net. Recompute through the
    // force path so unforced bits still publish underlying driver changes.
    if (llg_is_forced(&net->resolved)) {
        llg_net_commit_whole(net);
        return;
    }
    if (settle || !llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_commit_whole(net);
        return;
    }
    uint32_t low = old_active ? old_low : 0;
    uint32_t high = old_active ? old_high : net->width - 1u;
    if (!old_active && !new_active) return;
    if (new_active) {
        if (!old_active || low > new_low) low = new_low;
        if (!old_active || high < new_high) high = new_high;
    }
    llg_net_commit_range(net, low, high);
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
    int settle = net->unsettled;
    if (settle) llg_net_settle_cover(net, new_low, llg_sv4_width(selected));
    if (old_active == new_active &&
        (!new_active || (old_low == new_low && old_high == new_high &&
                         llg_net_range_same(slot, new_low, &selected)))) {
        if (settle) llg_net_commit_whole(net);
        return;
    }
    // The index orders drivers by their driven range only: a new value over
    // the same range keeps the driver's node and overwrites those bits.
    int same_range = LLG_GATE_FAST_PATH && llg_net_has_index(net) &&
        old_active && new_active && old_low == new_low && old_high == new_high;
    if (same_range) {
        llg_net_range_copy(slot, new_low, &selected);
    } else if (llg_net_has_index(net)) {
        llg_net_index_remove(net, idx);
        if (old_active) llg_net_range_fill_z(slot, old_low, old_high - old_low + 1u);
    } else {
        llg_net_range_fill_z(slot, 0, net->width);
        old_low = 0;
        old_high = net->width - 1u;
    }
    if (new_active && !same_range) {
        llg_net_range_copy(slot, new_low, &selected);
        if (llg_net_has_index(net)) llg_net_index_insert(net, idx, new_low, new_high);
    }
    if (llg_is_forced(&net->resolved)) {
        llg_net_commit_whole(net);
        return;
    }
    if (settle || !llg_net_has_index(net) || net->propagation_enabled) {
        llg_net_commit_whole(net);
        return;
    }
    if (!old_active && !new_active) return;
    if (old_active && new_active &&
        (old_high + 1u < new_low || new_high + 1u < old_low)) {
        if (llg_net_defer(net, 0, old_low < new_low ? old_low : new_low,
                          old_high > new_high ? old_high : new_high))
            return;
        llg_value_scope_t* scope = value_scope_begin_private(2);
        sv4_t* owned = llg_value_scope_values(scope);
        sv4_replace(&owned[0], llg_net_compute_range(
            net, old_low, old_high - old_low + 1u));
        sv4_replace(&owned[1], llg_net_compute_range(
            net, new_low, new_high - new_low + 1u));
        llg_net_publish_ranges(net, old_low, owned[0], new_low, owned[1]);
        llg_value_scope_end(scope);
        llg_net_strength_publish(net);
        return;
    }
    uint32_t low = old_active ? old_low : new_low;
    uint32_t high = old_active ? old_high : new_high;
    if (new_active) {
        if (!old_active || new_low < low) low = new_low;
        if (!old_active || new_high > high) high = new_high;
    }
    llg_net_commit_range(net, low, high);
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
    llg_value_scope_t* scope = value_scope_begin_private(2);
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
        uint32_t cover_low = part->net->width;
        uint32_t cover_high = 0;
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
            if (mapped->group_bit < cover_low) cover_low = mapped->group_bit;
            if (mapped->group_bit + mapped->bit_count - 1u > cover_high)
                cover_high = mapped->group_bit + mapped->bit_count - 1u;
        }
        // The alias drives only its mapped bits of this slot.
        llg_net_write_covering(part->net, part->slot, owned[1], cover_low,
                               cover_low <= cover_high ? cover_high - cover_low + 1u : 0u);
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

// Delay of one driver update. A one-bit target (a scalar net, a bit select,
// a gate or UDP output) follows the per-transition table of IEEE 1364-2001
// 7.14 Table 39 / IEEE 1800-2009 28.16 Table 28-9, so a transition to X takes
// the smallest applicable delay. A wider target is a vector and takes one
// delay for the whole value (1364-2001 6.1.3, 1800-2009 10.3.3): nonzero to
// zero falls, a transition to all-Z turns off, every other change rises.
static uint64_t inertial_transition_ticks(const sv4_t* old_value,
                                           const sv4_t* new_value,
                                           const sv4_t* mask, uint64_t rise,
                                           uint64_t fall, uint64_t turn_off) {
    uint32_t width = llg_sv4_width(*old_value) < llg_sv4_width(*new_value)
                         ? llg_sv4_width(*old_value)
                         : llg_sv4_width(*new_value);
    uint32_t driven = 0;
    uint32_t last = 0;
    int old_zero = 1;
    int new_zero = 1;
    int new_z = 1;
    int changed = 0;
    for (uint32_t bit = 0; bit < width; bit++) {
        if (mask && inertial_bit(mask, bit) != 1) continue;
        int old_state = inertial_bit(old_value, bit);
        int new_state = inertial_bit(new_value, bit);
        driven++;
        last = bit;
        old_zero &= old_state == 0;
        new_zero &= new_state == 0;
        new_z &= new_state == 3;
        changed |= old_state != new_state;
    }
    if (!changed) return 0;
    if (driven == 1) {
        switch (inertial_transition(inertial_bit(old_value, last),
                                    inertial_bit(new_value, last))) {
        case INERTIAL_RISE: return rise;
        case INERTIAL_FALL: return fall;
        case INERTIAL_TURN_OFF: return turn_off;
        case INERTIAL_RISE_OR_FALL: {
            uint64_t ticks = rise < fall ? rise : fall;
            return turn_off < ticks ? turn_off : ticks;
        }
        default: return 0;
        }
    }
    if (new_z) return turn_off;
    if (new_zero && !old_zero) return fall;
    return rise;
}

// Delay of one net-delay update: the new value of the least significant bit
// selects it, 0 falls, 1 rises, Z turns off and X takes the smallest delay,
// for a vector as a whole and for one bit alike (decision S2-D3; for one bit
// this is the destination column of Table 28-9).
static uint64_t net_delay_ticks(const sv4_t* value, uint64_t rise,
                                uint64_t fall, uint64_t turn_off) {
    switch (inertial_bit(value, 0)) {
    case 0: return fall;
    case 1: return rise;
    case 3: return turn_off;
    default: {
        uint64_t ticks = rise < fall ? rise : fall;
        return turn_off < ticks ? turn_off : ticks;
    }
    }
}

// The pending list is sorted by maturity time and FIFO among equal times.
// Back links make removal constant time; insertion searches from the tail,
// where new (latest) maturities land, for the same position a forward search
// past every entry due no later would find.
static void inertial_pending_remove(llg_inertial_t* driver) {
    if (driver->prev_pending) driver->prev_pending->next_pending = driver->next_pending;
    else if (g.inertial_pending == driver) g.inertial_pending = driver->next_pending;
    if (driver->next_pending) driver->next_pending->prev_pending = driver->prev_pending;
    else if (g.inertial_pending_tail == driver) g.inertial_pending_tail = driver->prev_pending;
    driver->next_pending = NULL;
    driver->prev_pending = NULL;
}

static void inertial_pending_insert(llg_inertial_t* driver) {
    llg_inertial_t* before = g.inertial_pending_tail;
    while (before && before->time > driver->time) before = before->prev_pending;
    driver->prev_pending = before;
    driver->next_pending = before ? before->next_pending : g.inertial_pending;
    if (driver->next_pending) driver->next_pending->prev_pending = driver;
    else g.inertial_pending_tail = driver;
    if (before) before->next_pending = driver;
    else g.inertial_pending = driver;
}

static void inertial_unlink_pending(llg_inertial_t* driver) {
    if (!driver->pending) return;
    inertial_pending_remove(driver);
    driver->pending = 0;
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
    // A per-bit net-delay record has no target cell; its width is that of
    // the bit it publishes.
    uint32_t target_width = target ? llg_sv4_width(*target)
                                   : llg_sv4_width(driver->current);
    value = sv4_resize(value, target_width, target && llg_sv4_signed(*target));
    sv4_t selected_mask = SV4_EMPTY;
    if (mask) {
        selected_mask = sv4_resize(*mask, target_width, 0);
    }
    const sv4_t* effective_mask = mask ? &selected_mask : NULL;
    uint64_t ticks;
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
    ticks = driver->lsb_rule
        ? net_delay_ticks(&value, rise, fall, turn_off)
        : inertial_transition_ticks(&driver->current, &value, effective_mask,
                                    rise, fall, turn_off);
    if (ticks > UINT64_MAX - g.now) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling an inertial update\n");
        abort();
    }
    sv4_move(&driver->value, &value);
    driver->time = g.now + ticks;
    driver->pending = 1;
    inertial_pending_insert(driver);
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

// The publication record of bits [lsb, lsb + width) of `net`, created on
// first use with those committed bits. A path-stage record times one bit of
// a module path destination; a constant net-delay record selects its delay
// with `lsb_rule`.
static llg_inertial_t* llg_net_part_record(llg_net_t* net,
                                           llg_inertial_t** handle,
                                           uint32_t lsb, uint32_t width,
                                           int8_t path_stage) {
    if (!*handle) {
        llg_inertial_t* driver =
            llg_checked_calloc(1, sizeof(*driver), "inertial driver");
        driver->handle = handle;
        driver->publication_net = net;
        driver->publication_bit = lsb;
        driver->bit_publication = 1;
        driver->path_stage = path_stage;
        driver->lsb_rule = !path_stage && net->net_delayed != 2;
        driver->current = sv4_part_select(net->resolved, lsb + width - 1u, lsb);
        driver->next_all = g.inertial_drivers;
        g.inertial_drivers = driver;
        *handle = driver;
    }
    return *handle;
}

static llg_inertial_t* llg_net_bit_record(llg_net_t* net, uint32_t bit) {
    return llg_net_part_record(net, &net->propagation_bits[bit], bit, 1u, 1);
}

// Schedule `resolved` through the net delay: the whole net as one vector,
// or each separately driven part on its own (decision S2-D3).
static void llg_net_delay_stage(llg_net_t* net, sv4_t resolved) {
    if (!region_can_mutate("inertial scheduling")) return;
    if (!net->n_segments) {
        if (!net->propagation) {
            llg_inertial_t* driver =
                llg_checked_calloc(1, sizeof(*driver), "inertial driver");
            driver->handle = &net->propagation;
            driver->target = &net->resolved;
            driver->publication_net = net;
            driver->lsb_rule = net->net_delayed != 2;
            sv4_copy(&driver->current, &net->resolved);
            driver->next_all = g.inertial_drivers;
            g.inertial_drivers = driver;
            net->propagation = driver;
        }
        inertial_update(&net->propagation, &net->resolved, NULL, 0, resolved,
                        NULL, net->propagation_rise, net->propagation_fall,
                        net->propagation_turn_off);
        return;
    }
    for (uint32_t part = 0; part < net->n_segments; part++) {
        uint32_t lsb = net->segment_lsb[part];
        uint32_t width = net->segment_lsb[part + 1u] - lsb;
        llg_inertial_t** handle = &net->segment_records[part];
        llg_net_part_record(net, handle, lsb, width, 0);
        sv4_t value = sv4_part_select(resolved, lsb + width - 1u, lsb);
        inertial_update(handle, NULL, NULL, 0, value, NULL,
                        net->propagation_rise, net->propagation_fall,
                        net->propagation_turn_off);
        sv4_destroy(&value);
    }
}

// A path-stage bit matured: the destination's path-delayed value now passes
// through the destination's own net delay (decision A7-D1).
static void llg_net_path_stage_commit(llg_net_t* net) {
    llg_value_scope_t* scope = value_scope_begin_private(1);
    sv4_t* owned = llg_value_scope_values(scope);
    sv4_copy(&owned[0], &net->resolved);
    for (uint32_t bit = 0; bit < net->width; bit++) {
        const llg_inertial_t* driver = net->propagation_bits[bit];
        if (driver) llg_sv4_range_copy(&owned[0], bit, driver->current);
    }
    llg_net_delay_stage(net, owned[0]);
    llg_value_scope_end(scope);
}

// ── Module path delays (IEEE 1800-2009 30.4-30.7) ──────────────────────────

// Global pulse controls (30.7.2, 30.7.4); llg_module_paths_configure sets
// them, cleanup restores the defaults.
static struct {
    uint32_t reject_percent;
    uint32_t error_percent;
    int style;          // -1: declarations decide, 0 on-event, 1 on-detect
    int show_cancelled; // -1: declarations decide, 0 off, 1 on
} path_pulse = {100u, 100u, -1, -1};

void llg_module_paths_configure(uint32_t reject_percent, uint32_t error_percent,
                                int style, int show_cancelled) {
    path_pulse.reject_percent = reject_percent > 100u ? 100u : reject_percent;
    path_pulse.error_percent = error_percent > 100u ? 100u : error_percent;
    if (path_pulse.error_percent < path_pulse.reject_percent)
        path_pulse.error_percent = path_pulse.reject_percent;
    path_pulse.style = style;
    path_pulse.show_cancelled = show_cancelled;
}

// Index of a transition in the path delay table; -1 for no transition.
static int path_transition_index(unsigned from, unsigned to) {
    static const signed char table[4][4] = {
        /* from 0 */ {-1, 0, 6, 2},
        /* from 1 */ {1, -1, 8, 4},
        /* from x */ {9, 7, -1, 10},
        /* from z */ {5, 3, 11, -1},
    };
    if (from > 3u || to > 3u) return -1;
    return table[from][to];
}

// Edge classes of a transition (Table 9-2): bit 0 a posedge, bit 1 a negedge.
static unsigned path_edge_classes(unsigned from, unsigned to) {
    static const unsigned char table[4][4] = {
        /* from 0 */ {0, 1, 1, 1},
        /* from 1 */ {2, 0, 2, 2},
        /* from x */ {2, 1, 0, 0},
        /* from z */ {2, 1, 0, 0},
    };
    return from > 3u || to > 3u ? 0u : table[from][to];
}

static int path_set_ready(const llg_module_path_set_t* set) {
    return set && (set->n_sources == 0 || (set->last && set->stamps));
}

void llg_module_path_stamp(llg_module_path_set_t* set) {
    if (!path_set_ready(set)) return;
    for (uint32_t index = 0; index < set->n_sources; index++) {
        const sv4_t* source = set->sources[index];
        sv4_t* last = &set->last[index];
        if (llg_sv4_width(*source) == llg_sv4_width(*last) &&
            sv4_same(*source, *last))
            continue;
        uint32_t width = llg_sv4_width(*last);
        for (uint32_t bit = 0; bit < width; bit++) {
            unsigned from = llg_sv4_state(*last, bit);
            unsigned to = llg_sv4_state(*source, bit);
            if (from == to) continue;
            set->stamps[index][bit] = g.now;
            if (!set->rise_stamps) continue;
            unsigned classes = path_edge_classes(from, to);
            if (classes & 1u) set->rise_stamps[index][bit] = g.now;
            if (classes & 2u) set->fall_stamps[index][bit] = g.now;
        }
        sv4_copy(last, source);
    }
}

// Latest change among source bits [lsb, lsb + width) of `source`.
static uint64_t path_source_stamp(const llg_module_path_set_t* set,
                                  uint32_t source, uint32_t lsb,
                                  uint32_t width) {
    uint64_t latest = 0;
    const uint64_t* stamps = set->stamps[source];
    for (uint32_t bit = 0; bit < width; bit++) {
        if (stamps[lsb + bit] > latest) latest = stamps[lsb + bit];
    }
    return latest;
}

static int path_covers(const llg_module_path_t* path, uint32_t bit) {
    return bit >= path->dest_lsb && bit - path->dest_lsb < path->dest_width;
}

// When the input of path `index` last started it, for destination bit `bit`:
// an edge-sensitive path at the last edge of its kind on the least
// significant bit of its input terminal (30.4.3), a full path at the latest
// change of its source range, a parallel path at the change of its own bit.
static uint64_t path_input_time(const llg_module_path_set_t* set,
                                const llg_module_path_dest_t* dest,
                                uint32_t index, uint32_t bit) {
    const llg_module_path_t* path = &dest->paths[index];
    if (path->interconnect) return g.now;
    if (path->edge && set->rise_stamps) {
        uint64_t rise = set->rise_stamps[path->source][path->source_lsb];
        uint64_t fall = set->fall_stamps[path->source][path->source_lsb];
        switch (path->edge) {
        case 1: return rise;
        case 2: return fall;
        default: return rise > fall ? rise : fall;
        }
    }
    if (path->full) return dest->path_stamps[index];
    return set->stamps[path->source][path->source_lsb + (bit - path->dest_lsb)];
}

// A conditional path applies while the least significant bit of its
// condition is 1, x or z (30.4.4.1); the generated evaluator reads the
// conditions in the instance's scope when a transition is being scheduled.
static int path_enabled(llg_module_path_set_t* set,
                        const llg_module_path_t* path, int* evaluated) {
    if (!path->condition || path->ifnone) return 1;
    if (!*evaluated) {
        set->conditions(set->condition_values, NULL);
        *evaluated = 1;
    }
    const sv4_t* value = &set->condition_values[path->condition - 1u];
    return llg_sv4_width(*value) == 0 || llg_sv4_state(*value, 0) != 0u;
}

typedef struct {
    uint64_t due;    // absolute time of the transition
    uint64_t reject; // pulse limits of the chosen path for it
    uint64_t error;
} path_choice_t;

// Choose the delay of one transition of destination bit `bit` (30.5.3):
// among the enabled paths into the bit, those whose input started most
// recently are active, an `ifnone` path yielding to another active path of
// its group (30.4.4.4); the smallest delay of the transition wins and its
// path gives the pulse limits. The transition is due at the later of now
// (the distributed delays have elapsed) and that input time plus the
// delay (30.6). Returns 0 when no path is active: the transition then has
// no path delay.
static int path_choose(llg_module_path_set_t* set, llg_module_path_dest_t* dest,
                       uint32_t bit, int transition, int* evaluated,
                       path_choice_t* choice) {
    int found = 0;
    uint64_t latest = 0;
    for (uint32_t index = 0; index < dest->n_paths; index++) {
        const llg_module_path_t* path = &dest->paths[index];
        if (!path_covers(path, bit) || !path_enabled(set, path, evaluated))
            continue;
        uint64_t time = path_input_time(set, dest, index, bit);
        if (!found || time > latest) latest = time;
        found = 1;
    }
    if (!found) return 0;
    const llg_module_path_t* chosen = NULL;
    for (uint32_t index = 0; index < dest->n_paths; index++) {
        const llg_module_path_t* path = &dest->paths[index];
        if (!path_covers(path, bit) || !path_enabled(set, path, evaluated) ||
            path_input_time(set, dest, index, bit) != latest)
            continue;
        if (path->ifnone) {
            int yields = 0;
            for (uint32_t other = 0; other < dest->n_paths && !yields; other++) {
                const llg_module_path_t* peer = &dest->paths[other];
                yields = peer->group == path->group && !peer->ifnone &&
                         path_covers(peer, bit) &&
                         path_enabled(set, peer, evaluated) &&
                         path_input_time(set, dest, other, bit) == latest;
            }
            if (yields) continue;
        }
        if (!chosen || path->delays[transition] < chosen->delays[transition])
            chosen = path;
    }
    if (!chosen) return 0;
    uint64_t delay = chosen->delays[transition];
    if (delay > UINT64_MAX - latest) {
        fprintf(stderr, "llg: fatal: simulation time overflow while scheduling a module path delay\n");
        abort();
    }
    choice->due = latest + delay > g.now ? latest + delay : g.now;
    if (chosen->limits) {
        choice->reject = chosen->limits[transition];
        choice->error = chosen->limits[LLG_PATH_TRANSITIONS + transition];
    } else {
        // Truncated percentages of the delay (30.7.2); a delay this large
        // cannot be scheduled, so the division first keeps them exact enough.
        choice->reject = delay / 100u * path_pulse.reject_percent +
                         delay % 100u * path_pulse.reject_percent / 100u;
        choice->error = delay / 100u * path_pulse.error_percent +
                        delay % 100u * path_pulse.error_percent / 100u;
    }
    return 1;
}

static void path_queue_push(llg_inertial_t* driver, uint64_t time,
                            unsigned state) {
    llg_path_queue_t* queue = driver->queue;
    if (!queue) {
        queue = llg_checked_calloc(1, sizeof(*queue), "module path queue");
        driver->queue = queue;
    }
    if (queue->count == queue->capacity) {
        uint32_t capacity = queue->capacity ? queue->capacity * 2u : 4u;
        uint64_t* times = realloc(queue->times, capacity * sizeof(*times));
        if (!times) {
            fprintf(stderr, "llg: fatal: out of memory for a module path queue\n");
            abort();
        }
        queue->times = times;
        uint8_t* states = realloc(queue->states, capacity * sizeof(*states));
        if (!states) {
            fprintf(stderr, "llg: fatal: out of memory for a module path queue\n");
            abort();
        }
        queue->states = states;
        queue->capacity = capacity;
    }
    queue->times[queue->count] = time;
    queue->states[queue->count] = (uint8_t)state;
    queue->count++;
}

// Make (state, time) the record's pending update (the head).
static void path_head_schedule(llg_inertial_t* driver, uint64_t time,
                               unsigned state) {
    sv4_replace(&driver->value, sv4_fill((uint8_t)state, 1u, 0));
    driver->time = time;
    driver->pending = 1;
    driver->region = region_is_reactive(g.current_region) ? LLG_REGION_REACTIVE
                                                          : LLG_REGION_ACTIVE;
    inertial_pending_insert(driver);
}

// Move the last scheduled update of a destination bit to (state, time).
static void path_last_replace(llg_inertial_t* driver, uint64_t time,
                              unsigned state) {
    llg_path_queue_t* queue = driver->queue;
    if (queue && queue->count) {
        queue->times[queue->count - 1u] = time;
        queue->states[queue->count - 1u] = (uint8_t)state;
        return;
    }
    inertial_pending_remove(driver);
    path_head_schedule(driver, time, state);
}

// The time of the update before the last one; `now` when the last update is
// the pending head.
static uint64_t path_previous_time(const llg_inertial_t* driver) {
    const llg_path_queue_t* queue = driver->queue;
    if (!queue || !queue->count) return g.now;
    return queue->count > 1u ? queue->times[queue->count - 2u] : driver->time;
}

// The pending head committed: the next queued update becomes the head.
static void path_queue_advance(llg_inertial_t* driver) {
    llg_path_queue_t* queue = driver->queue;
    if (!queue || !queue->count || driver->pending) return;
    uint64_t time = queue->times[0];
    unsigned state = queue->states[0];
    queue->count--;
    memmove(queue->times, queue->times + 1, queue->count * sizeof(*queue->times));
    memmove(queue->states, queue->states + 1, queue->count * sizeof(*queue->states));
    path_head_schedule(driver, time, state);
}

// Schedule destination bit `bit` toward `state` (30.7). Two consecutive
// scheduled transitions form a pulse whose width is the difference of their
// times; the limits of the trailing transition's path decide it: below the
// reject limit the pulse vanishes, below the error limit its leading edge
// turns into x (on-detect: at once), otherwise both transitions remain. A
// trailing edge due no later than the leading one cancels the leading
// edge, or with showcancelled turns the interval into x. With the default
// limits (both equal to the delay) every overlap is rejected, which is the
// inertial update: a changed value replaces the pending one and a return
// to the committed value cancels it.
static void path_bit_schedule(llg_module_path_set_t* set,
                              llg_module_path_dest_t* dest, uint32_t bit,
                              llg_inertial_t* driver, unsigned state,
                              int* evaluated) {
    unsigned flags = dest->styles ? dest->styles[bit] : 0u;
    int on_detect = path_pulse.style >= 0
                        ? path_pulse.style == 1
                        : (flags & LLG_PATH_STYLE_ON_DETECT) != 0u;
    int show_cancelled = path_pulse.show_cancelled >= 0
                             ? path_pulse.show_cancelled == 1
                             : (flags & LLG_PATH_STYLE_SHOW_CANCELLED) != 0u;
    for (;;) {
        llg_path_queue_t* queue = driver->queue;
        int queued = queue && queue->count;
        unsigned projected = queued ? queue->states[queue->count - 1u]
                             : driver->pending ? llg_sv4_state(driver->value, 0)
                                               : llg_sv4_state(driver->current, 0);
        if (state == projected) return;
        path_choice_t choice = {g.now, 0, 0};
        int transition = path_transition_index(projected, state);
        if (transition < 0 ||
            !path_choose(set, dest, bit, transition, evaluated, &choice)) {
            choice.due = g.now;
            choice.reject = 0;
            choice.error = 0;
        }
        if (!driver->pending) {
            path_head_schedule(driver, choice.due, state);
            return;
        }
        uint64_t last = queued ? queue->times[queue->count - 1u] : driver->time;
        if (choice.due <= last) {
            if (show_cancelled && (on_detect || choice.due < last)) {
                uint64_t previous = path_previous_time(driver);
                uint64_t start = on_detect ? (g.now > previous ? g.now : previous)
                                           : (choice.due > previous ? choice.due : previous);
                path_last_replace(driver, start, 2u);
                if (state != 2u) path_queue_push(driver, last, state);
                return;
            }
        } else if (choice.due - last >= choice.reject) {
            if (choice.due - last < choice.error) {
                uint64_t previous = path_previous_time(driver);
                uint64_t start = on_detect ? (g.now > previous ? g.now : previous) : last;
                path_last_replace(driver, start, 2u);
            }
            if (state != 2u || choice.due - last >= choice.error)
                path_queue_push(driver, choice.due, state);
            return;
        }
        // The last scheduled update is cancelled or rejected; the new value
        // is then weighed against the update before it.
        if (queued) {
            queue->count--;
        } else {
            inertial_unlink_pending(driver);
        }
    }
}

// A destination bit transition is scheduled from the paths into the bit
// (path_choose) and filtered as a pulse against the bit's earlier scheduled
// transitions (path_bit_schedule). A net delay declared on the destination
// then applies to the path-delayed value (decision A7-D1).
static void llg_net_propagate_paths(llg_net_t* net, sv4_t resolved) {
    if (!region_can_mutate("inertial scheduling")) return;
    llg_module_path_dest_t* dest = net->path_dest;
    llg_module_path_set_t* set = dest->set;
    // A source written earlier in this time slot counts whatever order the
    // destination driver and the stamp process ran in.
    llg_module_path_stamp(set);
    for (uint32_t index = 0; index < dest->n_paths; index++) {
        const llg_module_path_t* path = &dest->paths[index];
        dest->path_stamps[index] =
            path->full ? path_source_stamp(set, path->source, path->source_lsb,
                                           path->source_width)
                       : 0;
    }
    int evaluated = set->conditions == NULL;
    for (uint32_t bit = 0; bit < net->width; bit++) {
        llg_inertial_t* driver = llg_net_bit_record(net, bit);
        path_bit_schedule(set, dest, bit, driver, llg_sv4_state(resolved, bit),
                          &evaluated);
    }
}

void llg_module_paths_start(llg_module_path_set_t* set) {
    if (!set || set->last) return;
    int edges = 0;
    for (uint32_t index = 0; index < set->n_dests; index++) {
        for (uint32_t path = 0; path < set->dests[index].n_paths; path++)
            edges |= set->dests[index].paths[path].edge != 0;
    }
    if (set->n_sources) {
        set->last = llg_checked_calloc(set->n_sources, sizeof(*set->last),
                                       "module path sources");
        set->stamps = llg_checked_calloc(set->n_sources, sizeof(*set->stamps),
                                         "module path stamps");
        if (edges) {
            set->rise_stamps = llg_checked_calloc(
                set->n_sources, sizeof(*set->rise_stamps), "module path stamps");
            set->fall_stamps = llg_checked_calloc(
                set->n_sources, sizeof(*set->fall_stamps), "module path stamps");
        }
        for (uint32_t index = 0; index < set->n_sources; index++) {
            uint32_t width = llg_sv4_width(*set->sources[index]);
            set->last[index] = (sv4_t)SV4_EMPTY;
            sv4_copy(&set->last[index], set->sources[index]);
            set->stamps[index] = llg_checked_calloc(
                width ? width : 1u, sizeof(**set->stamps), "module path stamps");
            if (edges) {
                set->rise_stamps[index] = llg_checked_calloc(
                    width ? width : 1u, sizeof(**set->rise_stamps), "module path stamps");
                set->fall_stamps[index] = llg_checked_calloc(
                    width ? width : 1u, sizeof(**set->fall_stamps), "module path stamps");
            }
        }
    }
    if (set->n_conditions) {
        set->condition_values = llg_checked_calloc(
            set->n_conditions, sizeof(*set->condition_values), "module path conditions");
        for (uint32_t index = 0; index < set->n_conditions; index++)
            set->condition_values[index] = (sv4_t)SV4_EMPTY;
    }
    for (uint32_t index = 0; index < set->n_dests; index++) {
        llg_module_path_dest_t* dest = &set->dests[index];
        dest->set = set;
        dest->path_stamps = llg_checked_calloc(
            dest->n_paths ? dest->n_paths : 1u, sizeof(*dest->path_stamps),
            "module path stamps");
        dest->net->path_dest = dest;
    }
    set->next = g.module_path_sets;
    g.module_path_sets = set;
}

llg_module_path_set_t* llg_module_path_sets(void) { return g.module_path_sets; }

int llg_net_path_pending(const llg_net_t* net, uint32_t bit, uint64_t* time,
                         int* state) {
    return llg_net_path_event(net, bit, 0, time, state);
}

int llg_net_path_event(const llg_net_t* net, uint32_t bit, uint32_t index,
                       uint64_t* time, int* state) {
    if (!net || !net->path_dest || !net->propagation_bits || bit >= net->width)
        return 0;
    const llg_inertial_t* driver = net->propagation_bits[bit];
    if (!driver || !driver->pending) return 0;
    if (index == 0) {
        if (time) *time = driver->time;
        if (state) *state = (int)llg_sv4_state(driver->value, 0);
        return 1;
    }
    const llg_path_queue_t* queue = driver->queue;
    if (!queue || index - 1u >= queue->count) return 0;
    if (time) *time = queue->times[index - 1u];
    if (state) *state = (int)queue->states[index - 1u];
    return 1;
}

// Release every bound set; the generated model owns the tables themselves.
static void module_paths_release(void) {
    while (g.module_path_sets) {
        llg_module_path_set_t* set = g.module_path_sets;
        g.module_path_sets = set->next;
        set->next = NULL;
        for (uint32_t index = 0; index < set->n_dests; index++) {
            llg_module_path_dest_t* dest = &set->dests[index];
            if (dest->net) dest->net->path_dest = NULL;
            free(dest->path_stamps);
            dest->path_stamps = NULL;
            dest->set = NULL;
        }
        if (set->last) {
            for (uint32_t index = 0; index < set->n_sources; index++) {
                sv4_destroy(&set->last[index]);
                free(set->stamps[index]);
                if (set->rise_stamps) {
                    free(set->rise_stamps[index]);
                    free(set->fall_stamps[index]);
                }
            }
        }
        if (set->condition_values) {
            for (uint32_t index = 0; index < set->n_conditions; index++)
                sv4_destroy(&set->condition_values[index]);
        }
        free(set->condition_values);
        free(set->last);
        free(set->stamps);
        free(set->rise_stamps);
        free(set->fall_stamps);
        set->condition_values = NULL;
        set->last = NULL;
        set->stamps = NULL;
        set->rise_stamps = NULL;
        set->fall_stamps = NULL;
    }
    path_pulse.reject_percent = 100u;
    path_pulse.error_percent = 100u;
    path_pulse.style = -1;
    path_pulse.show_cancelled = -1;
}

// A force publishes directly; it drops every pending net-delay update.
static void llg_net_cancel_propagation(llg_net_t* net) {
    if (net->propagation) inertial_unlink_pending(net->propagation);
    for (uint32_t part = 0; part < net->n_segments; part++) {
        if (net->segment_records[part])
            inertial_unlink_pending(net->segment_records[part]);
    }
    if (!net->propagation_bits) return;
    for (uint32_t bit = 0; bit < net->width; bit++) {
        llg_inertial_t* driver = net->propagation_bits[bit];
        if (!driver) continue;
        inertial_unlink_pending(driver);
        if (driver->queue) driver->queue->count = 0;
    }
}

// A force or release publishes the net directly; the delayed-propagation
// records then start their next transition from the published value, not
// from the value they last committed.
static void llg_net_propagation_sync(llg_net_t* net) {
    if (!net->propagation_enabled) return;
    if (net->propagation) sv4_copy(&net->propagation->current, &net->resolved);
    for (uint32_t part = 0; part < net->n_segments; part++) {
        llg_inertial_t* driver = net->segment_records[part];
        if (!driver) continue;
        uint32_t lsb = net->segment_lsb[part];
        sv4_replace(&driver->current,
                    sv4_part_select(net->resolved,
                                    net->segment_lsb[part + 1u] - 1u, lsb));
    }
    if (!net->propagation_bits) return;
    for (uint32_t bit = 0; bit < net->width; bit++) {
        llg_inertial_t* driver = net->propagation_bits[bit];
        if (!driver) continue;
        sv4_replace(&driver->current, sv4_bit_select(net->resolved, bit));
    }
}

// The first pending update due now in `region`. The list is sorted, so the
// scan stops at the first later maturity.
static llg_inertial_t* inertial_due(llg_region_t region) {
    for (llg_inertial_t* driver = g.inertial_pending;
         driver && driver->time <= g.now; driver = driver->next_pending) {
        if (driver->time == g.now && driver->region == region) return driver;
    }
    return NULL;
}

static int inertial_ready(llg_region_t region) {
    return inertial_due(region) != NULL;
}

static void commit_inertial(llg_region_t region) {
    llg_inertial_t* driver = inertial_due(region);
    if (!driver) return;
    inertial_pending_remove(driver);
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
    if (driver->bit_publication) {
        sv4_copy(&driver->current, &value);
        if (driver->path_stage) path_queue_advance(driver);
        if (driver->path_stage && publication_net->net_delayed) {
            llg_net_path_stage_commit(publication_net);
            sv4_destroy(&mask);
            sv4_destroy(&value);
            return;
        }
        // Like the whole-net commit (`llg_ba`), a forced net keeps its value.
        if (!llg_is_forced(&publication_net->resolved) &&
            sig_write_range(&publication_net->resolved, driver->publication_bit,
                            value))
            llg_net_alias_refresh_all(publication_net);
        llg_net_strength_publish(publication_net);
        sv4_destroy(&mask);
        sv4_destroy(&value);
        return;
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
    if (publication_net) {
        llg_net_alias_refresh_all(publication_net);
        llg_net_strength_publish(publication_net);
    }
    sv4_destroy(&mask);
    sv4_destroy(&value);
}
