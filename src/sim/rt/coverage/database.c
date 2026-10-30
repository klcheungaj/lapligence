// ── Coverage database (IEEE 1800-2009 19.9, 40.3.2.4, 40.3.2.5) ─────────────
//
// One simulator-owned file format, version 1, holds functional coverage (the
// covergroup records of a run), assertion coverage (coverage/assertions.c)
// and FSM coverage (coverage/fsm.c).
// Every integer is written little-endian, field by field, so a database is
// independent of the host's byte order and structure layout:
//
//   "LLGCOVDB" | u32 version | u32 sections | u64 functional hash |
//   u64 assertion hash | u64 payload bytes | payload | u64 FNV-1a 64 checksum
//
// The payload holds each section of `sections`, in bit order (functional,
// assertion, then FSM), as a u64 byte length and its body, so a reader
// finds one section without parsing the others. The FSM section carries its
// schema hash at the start of its body; a reader without that section bit
// rejects the file as having unknown sections.
//
// The hashes identify the model's coverage schema: every covergroup
// declaration's static shape (names, widths, bins, transitions, crosses; no
// options or source sites) and the assertion item table. A database is only
// read into a model with the same hash. A load validates the whole file and
// rebuilds every record before it publishes any, so a corrupt or foreign
// database changes nothing. A save writes a temporary file and replaces the
// target only when the file is complete.

#define LLG_COV_DB_VERSION 1u
#define LLG_COV_DB_FUNCTIONAL 1u
#define LLG_COV_DB_ASSERTION 2u
#define LLG_COV_DB_FSM 4u
#define LLG_COV_DB_HEADER_BYTES 40u
// The largest database a load reads, and the longest name or comment.
#ifndef LLG_COV_DB_MAX_BYTES
#define LLG_COV_DB_MAX_BYTES (UINT64_C(1) << 30)
#endif
#define LLG_COV_DB_MAX_TEXT (UINT32_C(1) << 20)

static const char llg_cov_db_magic[8] = {'L', 'L', 'G', 'C', 'O', 'V', 'D', 'B'};

// The file named by $set_coverage_db_name, saved at the end of the run.
static char* llg_cov_db_name;
// Cached schema hashes of the model's declarations, by model order.
static uint64_t* llg_cov_group_hashes;

// Assertion coverage (coverage/assertions.c) contributes its own section.
static int llg_acov_available(void);
static uint64_t llg_acov_schema_hash(void);
// So does FSM coverage (coverage/fsm.c).
static int llg_fcov_available(void);

// ── Hashing ─────────────────────────────────────────────────────────────────

#define LLG_COV_FNV_OFFSET UINT64_C(0xcbf29ce484222325)
#define LLG_COV_FNV_PRIME UINT64_C(0x100000001b3)

static void llg_cov_fnv(uint64_t* hash, const void* data, size_t len) {
    const unsigned char* bytes = (const unsigned char*)data;
    for (size_t i = 0; i < len; ++i) {
        *hash ^= bytes[i];
        *hash *= LLG_COV_FNV_PRIME;
    }
}

static void llg_cov_fnv_u64(uint64_t* hash, uint64_t value) {
    unsigned char bytes[8];
    for (int i = 0; i < 8; ++i) bytes[i] = (unsigned char)(value >> (8 * i));
    llg_cov_fnv(hash, bytes, sizeof bytes);
}

static void llg_cov_fnv_text(uint64_t* hash, const char* text) {
    size_t len = text ? strlen(text) : 0u;
    llg_cov_fnv_u64(hash, text ? (uint64_t)len : UINT64_MAX);
    llg_cov_fnv(hash, text ? text : "", len);
}

static void llg_cov_fnv_words(uint64_t* hash, const uint64_t* words, size_t count) {
    llg_cov_fnv_u64(hash, count);
    for (size_t i = 0; i < count; ++i) llg_cov_fnv_u64(hash, words ? words[i] : 0u);
}

static void llg_cov_fnv_items(uint64_t* hash, const llg_cov_item_t* items, size_t count) {
    llg_cov_fnv_u64(hash, count);
    for (size_t i = 0; i < count; ++i) {
        llg_cov_fnv_u64(hash, items[i].lo);
        llg_cov_fnv_u64(hash, items[i].hi);
        llg_cov_fnv_u64(hash, items[i].lo_kind);
        llg_cov_fnv_u64(hash, items[i].hi_kind);
    }
}

static void llg_cov_hash_trans(uint64_t* hash, const llg_cov_trans_t* trans) {
    llg_cov_fnv_u64(hash, trans != NULL);
    if (!trans) return;
    llg_cov_fnv_words(hash, trans->bounds, trans->n_classes);
    llg_cov_fnv_u64(hash, trans->words);
    llg_cov_fnv_words(hash, trans->accept, (size_t)trans->n_steps * trans->words);
    llg_cov_fnv_u64(hash, trans->n_steps);
    for (uint32_t i = 0; i < trans->n_steps; ++i) {
        const llg_cov_step_t* step = &trans->steps[i];
        llg_cov_fnv_u64(hash, step->repeat);
        llg_cov_fnv_u64(hash, step->last);
        llg_cov_fnv_u64(hash, step->lo);
        llg_cov_fnv_u64(hash, step->hi);
        llg_cov_fnv_u64(hash, step->state);
        llg_cov_fnv_u64(hash, step->set);
    }
    llg_cov_fnv_u64(hash, trans->n_sets);
    for (uint32_t i = 0; i < trans->n_sets; ++i) {
        llg_cov_fnv_u64(hash, trans->sets[i].bin);
        llg_cov_fnv_u64(hash, trans->sets[i].first_step);
        llg_cov_fnv_u64(hash, trans->sets[i].n_steps);
    }
    for (uint32_t i = 0; i <= trans->n_classes; ++i)
        llg_cov_fnv_u64(hash, trans->start_index ? trans->start_index[i] : 0u);
    uint32_t starts = trans->start_index ? trans->start_index[trans->n_classes] : 0u;
    for (uint32_t i = 0; i < starts; ++i) llg_cov_fnv_u64(hash, trans->start_sets[i]);
    llg_cov_fnv_u64(hash, trans->n_states);
    llg_cov_fnv_u64(hash, trans->window);
    llg_cov_fnv_u64(hash, trans->n_bins);
    for (uint32_t i = 0; i < trans->n_bins; ++i) {
        const llg_cov_tbin_t* bin = &trans->bins[i];
        llg_cov_fnv_text(hash, bin->name);
        llg_cov_fnv_u64(hash, bin->kind);
        llg_cov_fnv_u64(hash, bin->array);
        llg_cov_fnv_u64(hash, (uint64_t)(int64_t)bin->guard);
        llg_cov_fnv_u64(hash, bin->coverable);
    }
    llg_cov_fnv_u64(hash, (uint64_t)(int64_t)trans->default_bin);
}

// The schema identity of one declaration: everything that shapes its
// counters, nothing that options or source positions change.
static uint64_t llg_cov_group_hash(const llg_cov_group_t* group) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_text(&hash, group->type->name);
    llg_cov_fnv_text(&hash, group->name);
    llg_cov_fnv_text(&hash, group->path);
    llg_cov_fnv_u64(&hash, group->n_slots);
    llg_cov_fnv_u64(&hash, group->n_points);
    for (uint32_t p = 0; p < group->n_points; ++p) {
        const llg_cov_point_t* point = &group->points[p];
        llg_cov_fnv_text(&hash, point->name);
        llg_cov_fnv_u64(&hash, point->width);
        llg_cov_fnv_u64(&hash, point->is_signed);
        llg_cov_fnv_u64(&hash, point->kind);
        llg_cov_fnv_u64(&hash, point->dynamic);
        llg_cov_fnv_u64(&hash, point->crossed);
        llg_cov_fnv_words(&hash, point->keys, point->n_keys);
        llg_cov_fnv_u64(&hash, point->n_bins);
        size_t items = 0;
        for (uint32_t b = 0; b < point->n_bins; ++b) {
            const llg_cov_bin_t* bin = &point->bins[b];
            llg_cov_fnv_text(&hash, bin->name);
            llg_cov_fnv_u64(&hash, bin->kind);
            llg_cov_fnv_u64(&hash, bin->shape);
            llg_cov_fnv_u64(&hash, bin->count_is_slot);
            llg_cov_fnv_u64(&hash, (uint64_t)(int64_t)bin->guard);
            llg_cov_fnv_u64(&hash, bin->count);
            llg_cov_fnv_u64(&hash, bin->first_item);
            llg_cov_fnv_u64(&hash, bin->n_items);
            if ((size_t)bin->first_item + bin->n_items > items)
                items = (size_t)bin->first_item + bin->n_items;
        }
        llg_cov_fnv_items(&hash, point->items, items);
        llg_cov_hash_trans(&hash, point->trans);
    }
    llg_cov_fnv_u64(&hash, group->n_crosses);
    for (uint32_t c = 0; c < group->n_crosses; ++c) {
        const llg_cov_cross_t* cross = &group->crosses[c];
        llg_cov_fnv_text(&hash, cross->name);
        llg_cov_fnv_u64(&hash, cross->dynamic);
        llg_cov_fnv_u64(&hash, cross->n_dims);
        for (uint32_t d = 0; d < cross->n_dims; ++d) llg_cov_fnv_u64(&hash, cross->dims[d]);
        size_t items = 0;
        llg_cov_fnv_u64(&hash, cross->n_selects);
        for (uint32_t i = 0; i < cross->n_selects; ++i) {
            const llg_cov_select_t* select = &cross->selects[i];
            llg_cov_fnv_u64(&hash, select->dim);
            llg_cov_fnv_u64(&hash, (uint64_t)(int64_t)select->decl);
            llg_cov_fnv_u64(&hash, select->intersect);
            llg_cov_fnv_u64(&hash, select->negate);
            llg_cov_fnv_u64(&hash, select->first_item);
            llg_cov_fnv_u64(&hash, select->n_items);
            if ((size_t)select->first_item + select->n_items > items)
                items = (size_t)select->first_item + select->n_items;
        }
        llg_cov_fnv_items(&hash, cross->items, items);
        size_t box_selects = 0;
        llg_cov_fnv_u64(&hash, cross->n_boxes);
        for (uint32_t i = 0; i < cross->n_boxes; ++i) {
            llg_cov_fnv_u64(&hash, cross->boxes[i].first_select);
            llg_cov_fnv_u64(&hash, cross->boxes[i].n_selects);
            if ((size_t)cross->boxes[i].first_select + cross->boxes[i].n_selects > box_selects)
                box_selects = (size_t)cross->boxes[i].first_select + cross->boxes[i].n_selects;
        }
        for (size_t i = 0; i < box_selects; ++i) llg_cov_fnv_u64(&hash, cross->box_selects[i]);
        llg_cov_fnv_u64(&hash, cross->n_bins);
        for (uint32_t i = 0; i < cross->n_bins; ++i) {
            const llg_cov_xbin_t* bin = &cross->bins[i];
            llg_cov_fnv_text(&hash, bin->name);
            llg_cov_fnv_u64(&hash, bin->kind);
            llg_cov_fnv_u64(&hash, (uint64_t)(int64_t)bin->guard);
            llg_cov_fnv_u64(&hash, bin->first_box);
            llg_cov_fnv_u64(&hash, bin->n_boxes);
        }
    }
    return hash;
}

void llg_cov_model(llg_cov_group_t* const* groups, size_t n_groups, llg_cov_type_t* const* types,
                   size_t n_types) {
    llg_cov_model_groups = groups;
    llg_cov_n_model_groups = n_groups;
    llg_cov_model_types = types;
    llg_cov_n_model_types = n_types;
    free(llg_cov_group_hashes);
    llg_cov_group_hashes = NULL;
}

static uint64_t llg_cov_model_group_hash(size_t index) {
    if (!llg_cov_group_hashes) {
        size_t n = llg_cov_n_model_groups;
        llg_cov_group_hashes =
            (uint64_t*)llg_checked_calloc(n ? n : 1u, sizeof(uint64_t), "coverage database");
        for (size_t i = 0; i < n; ++i)
            llg_cov_group_hashes[i] = llg_cov_group_hash(llg_cov_model_groups[i]);
    }
    return llg_cov_group_hashes[index];
}

// The functional coverage schema of the model: its declarations in order.
static uint64_t llg_cov_functional_hash(void) {
    uint64_t hash = LLG_COV_FNV_OFFSET;
    llg_cov_fnv_u64(&hash, LLG_COV_DB_VERSION);
    llg_cov_fnv_u64(&hash, llg_cov_n_model_groups);
    for (size_t i = 0; i < llg_cov_n_model_groups; ++i)
        llg_cov_fnv_u64(&hash, llg_cov_model_group_hash(i));
    return hash;
}

static long llg_cov_model_group_index(const llg_cov_group_t* group) {
    for (size_t i = 0; i < llg_cov_n_model_groups; ++i)
        if (llg_cov_model_groups[i] == group) return (long)i;
    return -1;
}

// ── Byte buffers ────────────────────────────────────────────────────────────

typedef struct {
    unsigned char* data;
    size_t len, cap;
} llg_cov_buf_t;

static void llg_cov_put(llg_cov_buf_t* buf, const void* data, size_t len) {
    if (len > SIZE_MAX - buf->len) llg_fatal_allocation("coverage database", 2, SIZE_MAX);
    if (buf->len + len > buf->cap) {
        size_t cap = buf->cap ? buf->cap : 4096u;
        while (cap < buf->len + len) {
            if (cap > SIZE_MAX / 2u) llg_fatal_allocation("coverage database", 2, SIZE_MAX);
            cap *= 2u;
        }
        buf->data = (unsigned char*)llg_cov_grow(buf->data, cap, 1, "coverage database");
        buf->cap = cap;
    }
    if (len) memcpy(buf->data + buf->len, data, len);
    buf->len += len;
}

static void llg_cov_put_u8(llg_cov_buf_t* buf, uint8_t value) {
    llg_cov_put(buf, &value, 1);
}

static void llg_cov_put_u32(llg_cov_buf_t* buf, uint32_t value) {
    unsigned char bytes[4];
    for (int i = 0; i < 4; ++i) bytes[i] = (unsigned char)(value >> (8 * i));
    llg_cov_put(buf, bytes, sizeof bytes);
}

static void llg_cov_put_u64(llg_cov_buf_t* buf, uint64_t value) {
    unsigned char bytes[8];
    for (int i = 0; i < 8; ++i) bytes[i] = (unsigned char)(value >> (8 * i));
    llg_cov_put(buf, bytes, sizeof bytes);
}

static void llg_cov_put_text(llg_cov_buf_t* buf, const char* text) {
    size_t len = text ? strlen(text) : 0u;
    if (len > LLG_COV_DB_MAX_TEXT) len = LLG_COV_DB_MAX_TEXT;
    llg_cov_put_u32(buf, (uint32_t)len);
    llg_cov_put(buf, text ? text : "", len);
}

static void llg_cov_patch_u64(llg_cov_buf_t* buf, size_t at, uint64_t value) {
    for (int i = 0; i < 8; ++i) buf->data[at + (size_t)i] = (unsigned char)(value >> (8 * i));
}

typedef struct {
    const unsigned char* data;
    size_t len, pos;
    const char* error;  // the first reason a read failed
} llg_cov_reader_t;

static int llg_cov_fail(llg_cov_reader_t* reader, const char* error) {
    if (!reader->error) reader->error = error;
    return 0;
}

static int llg_cov_get(llg_cov_reader_t* reader, void* out, size_t len) {
    if (reader->error) return 0;
    if (len > reader->len - reader->pos) return llg_cov_fail(reader, "the file is truncated");
    memcpy(out, reader->data + reader->pos, len);
    reader->pos += len;
    return 1;
}

static int llg_cov_get_u8(llg_cov_reader_t* reader, uint8_t* out) {
    return llg_cov_get(reader, out, 1);
}

static int llg_cov_get_u32(llg_cov_reader_t* reader, uint32_t* out) {
    unsigned char bytes[4];
    if (!llg_cov_get(reader, bytes, sizeof bytes)) return 0;
    *out = 0;
    for (int i = 0; i < 4; ++i) *out |= (uint32_t)bytes[i] << (8 * i);
    return 1;
}

static int llg_cov_get_u64(llg_cov_reader_t* reader, uint64_t* out) {
    unsigned char bytes[8];
    if (!llg_cov_get(reader, bytes, sizeof bytes)) return 0;
    *out = 0;
    for (int i = 0; i < 8; ++i) *out |= (uint64_t)bytes[i] << (8 * i);
    return 1;
}

static int llg_cov_get_i64(llg_cov_reader_t* reader, int64_t* out) {
    uint64_t raw;
    if (!llg_cov_get_u64(reader, &raw)) return 0;
    *out = (int64_t)raw;
    return 1;
}

// A borrowed text field: `*text` points into the buffer, `*len` bytes.
static int llg_cov_get_text(llg_cov_reader_t* reader, const char** text, uint32_t* len) {
    if (!llg_cov_get_u32(reader, len)) return 0;
    if (*len > LLG_COV_DB_MAX_TEXT) return llg_cov_fail(reader, "a name or comment is too long");
    if (*len > reader->len - reader->pos) return llg_cov_fail(reader, "the file is truncated");
    *text = (const char*)reader->data + reader->pos;
    if (memchr(*text, '\0', *len)) return llg_cov_fail(reader, "a name or comment holds a NUL byte");
    reader->pos += *len;
    return 1;
}

// An element count that cannot exceed what the remaining bytes can hold.
static int llg_cov_get_count(llg_cov_reader_t* reader, uint64_t* count, size_t element_bytes) {
    if (!llg_cov_get_u64(reader, count)) return 0;
    if (*count > (reader->len - reader->pos) / element_bytes)
        return llg_cov_fail(reader, "an element count exceeds the file");
    return 1;
}

// ── Records ─────────────────────────────────────────────────────────────────

// Records with equal layout keys have identical counter layouts: the same
// declaration, argument slots and automatic bin limits.
static int llg_cov_same_layout_key(const llg_cov_inst_t* a, const llg_cov_inst_t* b) {
    if (a->group != b->group) return 0;
    for (uint32_t i = 0; i < a->group->n_slots; ++i)
        if (a->slots[i].raw != b->slots[i].raw ||
            a->slots[i].signed_value != b->slots[i].signed_value ||
            a->slots[i].unknown != b->slots[i].unknown)
            return 0;
    for (uint32_t i = 0; i < a->group->n_points; ++i)
        if (a->points[i].auto_bin_max != b->points[i].auto_bin_max) return 0;
    return 1;
}

// Write one record: the options of recs[0] and the counters of every
// record, which share one layout key, added together.
static void llg_cov_put_record(llg_cov_buf_t* buf, const llg_cov_inst_t* const* recs, size_t n,
                               uint64_t group_hash) {
    llg_cov_inst_t* lead = (llg_cov_inst_t*)recs[0];
    const llg_cov_group_t* group = lead->group;
    llg_cov_put_u64(buf, group_hash);
    llg_cov_put_text(buf, llg_cov_inst_name(lead));
    llg_cov_put_text(buf, lead->comment);
    llg_cov_put_u64(buf, (uint64_t)lead->weight);
    llg_cov_put_u64(buf, (uint64_t)lead->goal);
    llg_cov_put_u64(buf, (uint64_t)lead->at_least);
    llg_cov_put_u64(buf, (uint64_t)lead->auto_bin_max);
    llg_cov_put_u64(buf, (uint64_t)lead->cross_num_print_missing);
    llg_cov_put_u8(buf, lead->get_inst_coverage);
    llg_cov_put_u8(buf, lead->per_instance);
    llg_cov_put_u32(buf, group->n_slots);
    for (uint32_t i = 0; i < group->n_slots; ++i) {
        llg_cov_put_u64(buf, lead->slots[i].raw);
        llg_cov_put_u8(buf, lead->slots[i].signed_value);
        llg_cov_put_u8(buf, lead->slots[i].unknown);
    }
    uint32_t items = group->n_points + group->n_crosses;
    llg_cov_put_u32(buf, items);
    for (uint32_t i = 0; i < items; ++i) {
        const llg_cov_point_state_t* state = &lead->points[i];
        llg_cov_put_u64(buf, (uint64_t)state->weight);
        llg_cov_put_u64(buf, (uint64_t)state->goal);
        llg_cov_put_u64(buf, (uint64_t)state->at_least);
        llg_cov_put_u8(buf, state->has_at_least);
        llg_cov_put_u64(buf, (uint64_t)(i < group->n_points ? state->auto_bin_max
                                                            : state->cross_num_print_missing));
        llg_cov_put_text(buf, state->comment);
        // Counters by index, nonzero only, summed over the records.
        llg_cov_merge_table_t sums = {0};
        uint64_t limit = 0;
        if (i < group->n_points) limit = state->layout ? state->layout->n_bins : 0u;
        else if (state->cross) limit = state->cross->layout->product;
        for (size_t r = 0; r < n; ++r) {
            const llg_cov_point_state_t* mine = &recs[r]->points[i];
            if (mine->dense) {
                for (uint64_t bin = 0; bin < limit; ++bin)
                    if (mine->dense[bin])
                        llg_cov_merge_add(&sums, (llg_cov_name_t){0u, bin, 0}, mine->dense[bin], 0);
            } else {
                for (size_t slot = 0; slot < mine->sparse.capacity; ++slot)
                    if (mine->sparse.keys[slot])
                        llg_cov_merge_add(&sums,
                                          (llg_cov_name_t){0u, mine->sparse.keys[slot] - 1u, 0},
                                          mine->sparse.counts[slot], 0);
            }
        }
        llg_cov_put_u64(buf, (uint64_t)sums.used);
        for (size_t slot = 0; slot < sums.capacity; ++slot) {
            if (!sums.slots[slot].used) continue;
            llg_cov_put_u64(buf, sums.slots[slot].name.a);
            llg_cov_put_u64(buf, sums.slots[slot].sum);
        }
        free(sums.slots);
        if (i >= group->n_points) {
            const llg_cov_cross_t* cross = &group->crosses[i - group->n_points];
            uint32_t bins = state->cross ? cross->n_bins : 0u;
            llg_cov_put_u32(buf, bins);
            for (uint32_t b = 0; b < bins; ++b) {
                uint64_t sum = 0;
                for (size_t r = 0; r < n; ++r)
                    sum = llg_cov_saturating_add(sum, recs[r]->points[i].cross->counts[b]);
                llg_cov_put_u64(buf, sum);
            }
            continue;
        }
        const llg_cov_trans_t* trans = group->points[i].trans;
        uint32_t tbins = trans && state->history ? trans->n_bins : 0u;
        llg_cov_put_u32(buf, tbins);
        if (!tbins) continue;
        for (uint32_t b = 0; b < tbins; ++b) {
            uint64_t sum = 0;
            for (size_t r = 0; r < n; ++r)
                sum = llg_cov_saturating_add(sum, recs[r]->points[i].history->counts[b]);
            llg_cov_put_u64(buf, sum);
        }
        llg_cov_history_t merged = {0};
        for (size_t r = 0; r < n; ++r) {
            const llg_cov_history_t* history = recs[r]->points[i].history;
            for (size_t k = 0; k < history->sequence_capacity; ++k) {
                const llg_cov_sequence_t* entry = &history->sequences[k];
                if (!entry->length) continue;
                llg_cov_sequence_t* sum = llg_cov_sequence_entry(
                    &merged, entry->bin, history->pool + entry->keys, entry->length);
                sum->count = llg_cov_saturating_add(sum->count, entry->count);
            }
        }
        llg_cov_put_u64(buf, (uint64_t)merged.n_sequences);
        for (size_t k = 0; k < merged.sequence_capacity; ++k) {
            const llg_cov_sequence_t* entry = &merged.sequences[k];
            if (!entry->length) continue;
            llg_cov_put_u32(buf, entry->bin);
            llg_cov_put_u32(buf, entry->length);
            for (uint32_t key = 0; key < entry->length; ++key)
                llg_cov_put_u64(buf, merged.pool[entry->keys + key]);
            llg_cov_put_u64(buf, entry->count);
        }
        free(merged.sequences);
        free(merged.pool);
    }
}

// The functional section: per type with records, its records. A live
// instance is written together with the loaded record of the same layout
// key and name, so the file holds what the run reported (S46-D2).
static void llg_cov_put_functional(llg_cov_buf_t* buf) {
    size_t n_types = 0;
    for (size_t i = 0; i < llg_cov_n_model_types; ++i) n_types += llg_cov_model_types[i]->first != NULL;
    llg_cov_type_t* const* types = llg_cov_model_types;
    llg_cov_put_u32(buf, (uint32_t)n_types);
    for (size_t t = 0; t < llg_cov_n_model_types; ++t) {
        llg_cov_type_t* type = types[t];
        if (!type->first) continue;
        size_t n = (size_t)type->n_instances, k = 0;
        llg_cov_inst_t** all =
            (llg_cov_inst_t**)llg_checked_calloc(n ? n : 1u, sizeof(void*), "coverage database");
        uint8_t* written = (uint8_t*)llg_checked_calloc(n ? n : 1u, 1, "coverage database");
        for (llg_cov_inst_t* inst = type->first; inst; inst = inst->next) all[k++] = inst;
        llg_cov_put_text(buf, type->name);
        size_t count_at = buf->len;
        llg_cov_put_u32(buf, 0);
        uint32_t records = 0;
        for (size_t i = 0; i < n; ++i) {
            if (written[i] || !all[i]->ready) continue;
            long index = llg_cov_model_group_index(all[i]->group);
            if (index < 0) continue;
            const llg_cov_inst_t* recs[2] = {all[i], NULL};
            size_t used = 1;
            written[i] = 1;
            if (!all[i]->loaded) {
                const char* name = llg_cov_inst_name(all[i]);
                for (size_t j = 0; j < n; ++j) {
                    if (written[j] || !all[j]->loaded || !all[j]->ready) continue;
                    if (strcmp(llg_cov_inst_name(all[j]), name) != 0 ||
                        !llg_cov_same_layout_key(all[i], all[j]))
                        continue;
                    recs[used++] = all[j];
                    written[j] = 1;
                    break;
                }
            }
            llg_cov_put_record(buf, recs, used, llg_cov_model_group_hash((size_t)index));
            records++;
        }
        for (int b = 0; b < 4; ++b)
            buf->data[count_at + (size_t)b] = (unsigned char)(records >> (8 * b));
        free(written);
        free(all);
    }
}

static void llg_acov_put_section(llg_cov_buf_t* buf);
static void llg_fcov_put_section(llg_cov_buf_t* buf);

// The complete file image of the selected sections.
static void llg_cov_db_image(llg_cov_buf_t* buf, uint32_t sections) {
    llg_cov_put(buf, llg_cov_db_magic, sizeof llg_cov_db_magic);
    llg_cov_put_u32(buf, LLG_COV_DB_VERSION);
    llg_cov_put_u32(buf, sections);
    llg_cov_put_u64(buf, sections & LLG_COV_DB_FUNCTIONAL ? llg_cov_functional_hash() : 0u);
    llg_cov_put_u64(buf, sections & LLG_COV_DB_ASSERTION ? llg_acov_schema_hash() : 0u);
    size_t length_at = buf->len;
    llg_cov_put_u64(buf, 0);
    if (sections & LLG_COV_DB_FUNCTIONAL) {
        size_t at = buf->len;
        llg_cov_put_u64(buf, 0);
        llg_cov_put_functional(buf);
        llg_cov_patch_u64(buf, at, (uint64_t)(buf->len - at - 8u));
    }
    if (sections & LLG_COV_DB_ASSERTION) {
        size_t at = buf->len;
        llg_cov_put_u64(buf, 0);
        llg_acov_put_section(buf);
        llg_cov_patch_u64(buf, at, (uint64_t)(buf->len - at - 8u));
    }
    if (sections & LLG_COV_DB_FSM) {
        size_t at = buf->len;
        llg_cov_put_u64(buf, 0);
        llg_fcov_put_section(buf);
        llg_cov_patch_u64(buf, at, (uint64_t)(buf->len - at - 8u));
    }
    llg_cov_patch_u64(buf, length_at, (uint64_t)(buf->len - LLG_COV_DB_HEADER_BYTES));
    uint64_t checksum = LLG_COV_FNV_OFFSET;
    llg_cov_fnv(&checksum, buf->data, buf->len);
    llg_cov_put_u64(buf, checksum);
}

// Write the database `name` (a path the simulation writes, so relative to
// LLG_SIM_OUT_DIR). Returns 1 on success; on failure reports why and removes
// both the temporary file and `name` (40.3.2.5).
static int llg_cov_db_save(const char* name, uint32_t sections, const char* site) {
    char* path = llg_output_path(name);
    size_t len = strlen(path);
    char* temporary = (char*)llg_checked_malloc(len + 32u, 1, "coverage database");
    snprintf(temporary, len + 32u, "%s.tmp%lu", path, llg_process_id());
    llg_cov_buf_t buf = {0};
    llg_cov_db_image(&buf, sections);
    const char* error = NULL;
    FILE* file = fopen(temporary, "wb");
    if (!file) {
        error = strerror(errno);
    } else {
        if (fwrite(buf.data, 1, buf.len, file) != buf.len || fflush(file) != 0)
            error = strerror(errno);
        if (fclose(file) != 0 && !error) error = strerror(errno);
        if (!error && llg_replace_file(temporary, path) != 0) error = strerror(errno);
    }
    if (error) {
        fprintf(stderr, "llg: runtime error: %s: coverage database `%s` was not saved: %s\n",
                site ? site : "end of simulation", name, error);
        llg_rt_mark_failed();
        (void)llg_remove_file(temporary);
        (void)llg_remove_file(path);
    }
    free(buf.data);
    free(temporary);
    free(path);
    return error == NULL;
}

// ── Loading ─────────────────────────────────────────────────────────────────

typedef struct {
    unsigned char* data;
    size_t len;
    uint32_t sections;
    uint64_t functional_hash;
    uint64_t assertion_hash;
    size_t payload;  // offset of the payload
    size_t end;      // offset of the checksum
    // [start, end) of the body of each present section.
    size_t functional_at, functional_end;
    size_t assertion_at, assertion_end;
    size_t fsm_at, fsm_end;
} llg_cov_db_file_t;

// Read and check the frame of database `name`: magic, version, lengths and
// checksum. Returns NULL with `*error` set on failure; `*missing` tells a
// file that cannot be opened from one that is not a valid database.
static const char* llg_cov_db_read(const char* name, llg_cov_db_file_t* db, int* missing) {
    memset(db, 0, sizeof *db);
    *missing = 0;
    FILE* file = fopen(name, "rb");
    if (!file) {
        *missing = 1;
        return strerror(errno);
    }
    size_t cap = 0;
    const char* error = NULL;
    for (;;) {
        if (db->len == cap) {
            if ((uint64_t)cap >= LLG_COV_DB_MAX_BYTES) {
                error = "the file is larger than LLG_COV_DB_MAX_BYTES";
                break;
            }
            cap = cap ? cap * 2u : 65536u;
            db->data = (unsigned char*)llg_cov_grow(db->data, cap, 1, "coverage database");
        }
        size_t got = fread(db->data + db->len, 1, cap - db->len, file);
        db->len += got;
        if (got == 0) {
            if (ferror(file)) error = strerror(errno);
            break;
        }
    }
    fclose(file);
    if (error) return error;
    llg_cov_reader_t reader = {db->data, db->len, 0, NULL};
    char magic[8];
    uint32_t version;
    uint64_t length;
    if (db->len < LLG_COV_DB_HEADER_BYTES + 8u) return "the file is not an llg coverage database";
    if (!llg_cov_get(&reader, magic, sizeof magic) ||
        memcmp(magic, llg_cov_db_magic, sizeof magic) != 0)
        return "the file is not an llg coverage database";
    if (!llg_cov_get_u32(&reader, &version) || version != LLG_COV_DB_VERSION)
        return "the database version is not supported";
    if (!llg_cov_get_u32(&reader, &db->sections) ||
        !llg_cov_get_u64(&reader, &db->functional_hash) ||
        !llg_cov_get_u64(&reader, &db->assertion_hash) || !llg_cov_get_u64(&reader, &length) ||
        length != db->len - LLG_COV_DB_HEADER_BYTES - 8u)
        return "the file length does not match its header";
    if (db->sections & ~(LLG_COV_DB_FUNCTIONAL | LLG_COV_DB_ASSERTION | LLG_COV_DB_FSM))
        return "the database has unknown sections";
    db->payload = LLG_COV_DB_HEADER_BYTES;
    db->end = db->len - 8u;
    uint64_t checksum = LLG_COV_FNV_OFFSET, stored = 0;
    llg_cov_fnv(&checksum, db->data, db->end);
    for (int i = 0; i < 8; ++i) stored |= (uint64_t)db->data[db->end + (size_t)i] << (8 * i);
    if (checksum != stored) return "the checksum does not match (the file is corrupt)";
    // Section framing: each present section's length and body, exactly
    // filling the payload.
    reader.pos = db->payload;
    reader.len = db->end;
    for (uint32_t bit = LLG_COV_DB_FUNCTIONAL; bit <= LLG_COV_DB_FSM; bit <<= 1) {
        if (!(db->sections & bit)) continue;
        uint64_t bytes;
        if (!llg_cov_get_u64(&reader, &bytes) || bytes > reader.len - reader.pos)
            return "the database sections are malformed";
        size_t* at = bit == LLG_COV_DB_FUNCTIONAL ? &db->functional_at
                     : bit == LLG_COV_DB_ASSERTION ? &db->assertion_at
                                                   : &db->fsm_at;
        size_t* end = bit == LLG_COV_DB_FUNCTIONAL ? &db->functional_end
                      : bit == LLG_COV_DB_ASSERTION ? &db->assertion_end
                                                    : &db->fsm_end;
        *at = reader.pos;
        *end = reader.pos + (size_t)bytes;
        reader.pos = *end;
    }
    if (reader.pos != reader.len) return "the database sections are malformed";
    return NULL;
}

static void llg_cov_db_file_free(llg_cov_db_file_t* db) {
    free(db->data);
    db->data = NULL;
}

// Add a count to bin `bin` of a counter table.
static void llg_cov_add_count(llg_cov_point_state_t* state, uint64_t bin, uint64_t count) {
    if (!count) return;
    if (state->dense) {
        state->dense[bin] = llg_cov_saturating_add(state->dense[bin], count);
        return;
    }
    llg_cov_sparse_t* table = &state->sparse;
    llg_cov_sparse_add(table, bin);  // inserts the bin with one count
    uint64_t key = bin + 1u;
    size_t slot = (size_t)(key * UINT64_C(0x9E3779B97F4A7C15)) & (table->capacity - 1u);
    while (table->keys[slot] != key) slot = (slot + 1u) & (table->capacity - 1u);
    table->counts[slot] = llg_cov_saturating_add(table->counts[slot], count - 1u);
}

// One record of the functional section. With `target` NULL (validation),
// builds the record's layout in a new unlinked record stored in `*staged`;
// with a target (publication), adds the counters to it. The reader is
// positioned at the record and left after it.
static int llg_cov_db_record(llg_cov_reader_t* reader, const char* type_name,
                             llg_cov_inst_t** staged, llg_cov_inst_t* target) {
    uint64_t hash;
    if (!llg_cov_get_u64(reader, &hash)) return 0;
    llg_cov_group_t* group = NULL;
    for (size_t i = 0; i < llg_cov_n_model_groups && !group; ++i)
        if (llg_cov_model_group_hash(i) == hash) group = llg_cov_model_groups[i];
    if (!group) return llg_cov_fail(reader, "a record names a covergroup this model does not have");
    if (strcmp(group->type->name, type_name) != 0)
        return llg_cov_fail(reader, "a record is filed under another covergroup type");
    const char* name;
    const char* comment;
    uint32_t name_len, comment_len, slots, items;
    int64_t weight, goal, at_least, auto_bin_max, missing;
    uint8_t get_inst, per_instance;
    if (!llg_cov_get_text(reader, &name, &name_len) ||
        !llg_cov_get_text(reader, &comment, &comment_len) || !llg_cov_get_i64(reader, &weight) ||
        !llg_cov_get_i64(reader, &goal) || !llg_cov_get_i64(reader, &at_least) ||
        !llg_cov_get_i64(reader, &auto_bin_max) || !llg_cov_get_i64(reader, &missing) ||
        !llg_cov_get_u8(reader, &get_inst) || !llg_cov_get_u8(reader, &per_instance) ||
        !llg_cov_get_u32(reader, &slots))
        return 0;
    if (slots != group->n_slots) return llg_cov_fail(reader, "a record has the wrong argument count");
    if (weight < 0 || auto_bin_max < 1) return llg_cov_fail(reader, "a record has invalid options");
    llg_cov_inst_t* inst = target;
    if (!inst) {
        inst = llg_cov_alloc(group);
        *staged = inst;
        inst->loaded = 1;
        inst->name = llg_cov_strdup_bytes(name, name_len);
        if (comment_len) inst->comment = llg_cov_strdup_bytes(comment, comment_len);
        inst->weight = weight;
        inst->goal = goal;
        inst->at_least = at_least;
        inst->auto_bin_max = auto_bin_max;
        inst->cross_num_print_missing = missing;
        inst->get_inst_coverage = get_inst != 0;
        inst->per_instance = per_instance != 0;
    }
    for (uint32_t i = 0; i < slots; ++i) {
        uint64_t raw;
        uint8_t is_signed, unknown;
        if (!llg_cov_get_u64(reader, &raw) || !llg_cov_get_u8(reader, &is_signed) ||
            !llg_cov_get_u8(reader, &unknown))
            return 0;
        if (!target) inst->slots[i] = (llg_cov_slot_value_t){raw, is_signed != 0, unknown != 0};
    }
    if (!llg_cov_get_u32(reader, &items)) return 0;
    if (items != group->n_points + group->n_crosses)
        return llg_cov_fail(reader, "a record has the wrong number of coverpoints and crosses");
    // Item options first, from a lookahead: auto_bin_max shapes the layout
    // that the counters are checked against.
    if (!target) {
        llg_cov_reader_t ahead = *reader;
        for (uint32_t i = 0; i < items; ++i) {
            llg_cov_point_state_t* state = &inst->points[i];
            int64_t value;
            uint8_t has_at_least;
            const char* text;
            uint32_t text_len;
            uint64_t entries;
            if (!llg_cov_get_i64(&ahead, &state->weight) || !llg_cov_get_i64(&ahead, &state->goal) ||
                !llg_cov_get_i64(&ahead, &state->at_least) ||
                !llg_cov_get_u8(&ahead, &has_at_least) || !llg_cov_get_i64(&ahead, &value) ||
                !llg_cov_get_text(&ahead, &text, &text_len))
                return llg_cov_fail(reader, ahead.error);
            state->has_at_least = 1;
            if (i < group->n_points) {
                if (value < 1) return llg_cov_fail(reader, "a record has invalid options");
                state->auto_bin_max = value;
                state->has_auto_bin_max = 1;
            } else {
                state->cross_num_print_missing = value;
                state->has_cross_num_print_missing = 1;
            }
            if (text_len) state->comment = llg_cov_strdup_bytes(text, text_len);
            // Skip the counters; they are checked once the layout exists.
            if (!llg_cov_get_count(&ahead, &entries, 16u)) return llg_cov_fail(reader, ahead.error);
            ahead.pos += (size_t)entries * 16u;
            uint32_t bins;
            if (!llg_cov_get_u32(&ahead, &bins)) return llg_cov_fail(reader, ahead.error);
            if (bins > (ahead.len - ahead.pos) / 8u)
                return llg_cov_fail(reader, "an element count exceeds the file");
            ahead.pos += (size_t)bins * 8u;
            if (i < group->n_points && bins) {
                uint64_t sequences;
                if (!llg_cov_get_u64(&ahead, &sequences)) return llg_cov_fail(reader, ahead.error);
                for (uint64_t q = 0; q < sequences; ++q) {
                    uint32_t bin, length;
                    if (!llg_cov_get_u32(&ahead, &bin) || !llg_cov_get_u32(&ahead, &length))
                        return llg_cov_fail(reader, ahead.error);
                    if ((uint64_t)length + 1u > (ahead.len - ahead.pos) / 8u)
                        return llg_cov_fail(reader, "an element count exceeds the file");
                    ahead.pos += ((size_t)length + 1u) * 8u;
                }
            }
        }
        llg_cov_quiet = 1;
        llg_cov_ready(inst);
        llg_cov_quiet = 0;
    }
    for (uint32_t i = 0; i < items; ++i) {
        llg_cov_point_state_t* state = &inst->points[i];
        int64_t ignored;
        uint8_t flag;
        const char* text;
        uint32_t text_len;
        uint64_t entries;
        if (!llg_cov_get_i64(reader, &ignored) || !llg_cov_get_i64(reader, &ignored) ||
            !llg_cov_get_i64(reader, &ignored) || !llg_cov_get_u8(reader, &flag) ||
            !llg_cov_get_i64(reader, &ignored) || !llg_cov_get_text(reader, &text, &text_len) ||
            !llg_cov_get_count(reader, &entries, 16u))
            return 0;
        int is_point = i < group->n_points;
        uint64_t limit = is_point ? state->layout->n_bins
                                  : (state->cross ? state->cross->layout->product : 0u);
        for (uint64_t e = 0; e < entries; ++e) {
            uint64_t bin, count;
            if (!llg_cov_get_u64(reader, &bin) || !llg_cov_get_u64(reader, &count)) return 0;
            if (bin >= limit) return llg_cov_fail(reader, "a counter is outside its bins");
            if (target) llg_cov_add_count(state, bin, count);
        }
        uint32_t bins;
        if (!llg_cov_get_u32(reader, &bins)) return 0;
        if (!is_point) {
            const llg_cov_cross_t* cross = &group->crosses[i - group->n_points];
            if (bins != (state->cross ? cross->n_bins : 0u))
                return llg_cov_fail(reader, "a cross has the wrong number of bins");
            for (uint32_t b = 0; b < bins; ++b) {
                uint64_t count;
                if (!llg_cov_get_u64(reader, &count)) return 0;
                if (target)
                    state->cross->counts[b] = llg_cov_saturating_add(state->cross->counts[b], count);
            }
            continue;
        }
        const llg_cov_trans_t* trans = group->points[i].trans;
        if (bins != (trans ? trans->n_bins : 0u))
            return llg_cov_fail(reader, "a coverpoint has the wrong number of transition bins");
        if (!bins) continue;
        for (uint32_t b = 0; b < bins; ++b) {
            uint64_t count;
            if (!llg_cov_get_u64(reader, &count)) return 0;
            if (target)
                state->history->counts[b] = llg_cov_saturating_add(state->history->counts[b], count);
        }
        uint64_t sequences;
        if (!llg_cov_get_u64(reader, &sequences)) return 0;
        uint64_t mask = llg_cov_mask(&group->points[i]);
        for (uint64_t q = 0; q < sequences; ++q) {
            uint32_t bin, length;
            if (!llg_cov_get_u32(reader, &bin) || !llg_cov_get_u32(reader, &length)) return 0;
            if (bin >= trans->n_bins || !trans->bins[bin].array || length == 0 ||
                length > trans->window)
                return llg_cov_fail(reader, "a transition sequence does not fit its bin");
            uint64_t* keys = state->history->sequence;
            for (uint32_t k = 0; k < length; ++k) {
                uint64_t key;
                if (!llg_cov_get_u64(reader, &key)) return 0;
                if (key > mask) return llg_cov_fail(reader, "a transition value exceeds the coverpoint");
                keys[k] = key;
            }
            uint64_t count;
            if (!llg_cov_get_u64(reader, &count)) return 0;
            if (target) {
                llg_cov_sequence_t* entry = llg_cov_sequence_entry(state->history, bin, keys, length);
                entry->count = llg_cov_saturating_add(entry->count, count);
            }
        }
    }
    return 1;
}

// Parse the functional section in one of two passes: validation stages a
// record per database record into `staged`; publication adds the counters
// of record r to `targets[r]`.
static int llg_cov_db_functional(llg_cov_reader_t* reader, llg_cov_inst_t*** staged,
                                 size_t* n_staged, size_t* staged_cap, llg_cov_inst_t** targets) {
    uint32_t types;
    if (!llg_cov_get_u32(reader, &types)) return 0;
    size_t record = 0;
    for (uint32_t t = 0; t < types; ++t) {
        const char* name;
        uint32_t name_len, records;
        if (!llg_cov_get_text(reader, &name, &name_len) || !llg_cov_get_u32(reader, &records))
            return 0;
        char* type_name = llg_cov_strdup_bytes(name, name_len);
        for (uint32_t r = 0; r < records; ++r, ++record) {
            int ok;
            if (targets) {
                ok = llg_cov_db_record(reader, type_name, NULL, targets[record]);
            } else {
                llg_cov_inst_t* inst = NULL;
                ok = llg_cov_db_record(reader, type_name, &inst, NULL);
                if (inst) LLG_COV_PUSH(*staged, *n_staged, *staged_cap, inst, "coverage database");
            }
            if (!ok) {
                free(type_name);
                return 0;
            }
        }
        free(type_name);
    }
    return 1;
}

// The loaded record a staged record's counters go to: an earlier loaded
// record with the same declaration, name and layout key, else the staged
// record itself, linked to its type.
static llg_cov_inst_t* llg_cov_db_target(llg_cov_inst_t* staged) {
    for (llg_cov_inst_t* inst = staged->group->type->first; inst; inst = inst->next)
        if (inst->loaded && strcmp(inst->name, staged->name) == 0 &&
            llg_cov_same_layout_key(inst, staged))
            return inst;
    llg_cov_link(staged);
    return staged;
}

void llg_cov_load_db(llg_string_t name, const char* site) {
    char* path = llg_cov_strdup_bytes(name.data, name.data ? name.len : 0u);
    llg_string_destroy(&name);
    llg_cov_db_file_t db;
    int missing;
    const char* error = llg_cov_db_read(path, &db, &missing);
    if (!error && !(db.sections & LLG_COV_DB_FUNCTIONAL))
        error = "the database holds no functional coverage";
    if (!error && db.functional_hash != llg_cov_functional_hash())
        error = "it was saved by a model with different covergroups";
    llg_cov_inst_t** staged = NULL;
    size_t n_staged = 0, staged_cap = 0;
    if (!error) {
        llg_cov_reader_t reader = {db.data, db.functional_end, db.functional_at, NULL};
        if (!llg_cov_db_functional(&reader, &staged, &n_staged, &staged_cap, NULL))
            error = reader.error;
        else if (reader.pos != reader.len)
            error = "the functional section has trailing data";
    }
    if (error) {
        fprintf(stderr,
                "llg: runtime error: %s: coverage database `%s` was not loaded: %s; no coverage "
                "changed\n",
                site, path, error);
        llg_rt_mark_failed();
        for (size_t i = 0; i < n_staged; ++i) llg_cov_free_record(staged[i]);
    } else {
        // Everything validated: publish. Records merge by name and layout.
        llg_cov_inst_t** targets = (llg_cov_inst_t**)llg_checked_calloc(
            n_staged ? n_staged : 1u, sizeof(void*), "coverage database");
        for (size_t i = 0; i < n_staged; ++i) {
            targets[i] = llg_cov_db_target(staged[i]);
            if (targets[i] != staged[i]) staged[i]->loaded = 2;  // merged away below
        }
        llg_cov_reader_t reader = {db.data, db.functional_end, db.functional_at, NULL};
        (void)llg_cov_db_functional(&reader, NULL, NULL, NULL, targets);
        for (size_t i = 0; i < n_staged; ++i)
            if (staged[i]->loaded == 2) llg_cov_free_record(staged[i]);
        free(targets);
    }
    free(staged);
    llg_cov_db_file_free(&db);
    free(path);
}

void llg_cov_set_db_name(llg_string_t name) {
    free(llg_cov_db_name);
    llg_cov_db_name = llg_cov_strdup_bytes(name.data, name.data ? name.len : 0u);
    llg_string_destroy(&name);
}

void llg_cov_end_of_run(void) {
    if (!llg_cov_db_name) return;
    uint32_t sections = LLG_COV_DB_FUNCTIONAL;
    if (llg_acov_available()) sections |= LLG_COV_DB_ASSERTION;
    if (llg_fcov_available()) sections |= LLG_COV_DB_FSM;
    (void)llg_cov_db_save(llg_cov_db_name, sections, NULL);
    free(llg_cov_db_name);
    llg_cov_db_name = NULL;
}

static void llg_cov_db_release(void) {
    // The assertion item table stays bound: it is static model storage, and
    // VPI applications read the final counters in cbEndOfSimulation, after
    // this cleanup (40.5.3). The next llg_acov_model rebinds it.
    free(llg_cov_db_name);
    llg_cov_db_name = NULL;
    free(llg_cov_group_hashes);
    llg_cov_group_hashes = NULL;
}
