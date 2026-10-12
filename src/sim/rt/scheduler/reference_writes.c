
static int llg_ref_report_tag_failure(const llg_ref_view_t* view,
                                      const sv4_t* parent) {
    size_t failed = 0;
    if (llg_ref_view_valid(view, parent, &failed)) return 1;
    const char* member = "<unknown>";
    const char* location = "<unknown>";
    if (view && failed < view->tag_check_count && view->tag_checks) {
        if (view->tag_checks[failed].member_name)
            member = view->tag_checks[failed].member_name;
        if (view->location) location = view->location;
    }
    llg_rt_mark_failed();
    fprintf(stderr,
            "llg: runtime error: access to inactive tagged-union member %s at %s\n",
            member, location);
    fflush(stderr);
    return 0;
}

sv4_t llg_rt_ref_read(const llg_ref_t* ref) {
    if (!ref || (llg_ref_kind_t)ref->kind != LLG_REF_TAGGED_VIEW)
        return llg_ref_read(ref);
    const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
    if (!view || !view->parent) {
        fputs("llg runtime fatal: invalid tagged reference view\n", stderr);
        abort();
    }
    sv4_t parent = llg_rt_ref_read(view->parent);
    int valid = llg_ref_report_tag_failure(view, &parent);
    sv4_t result = valid ? sv4_select_plan_read(parent, &view->plan)
                         : (ref->two_state ? sv4_zero(ref->width, ref->is_signed)
                                           : sv4_x(ref->width, ref->is_signed));
    sv4_destroy(&parent);
    if (ref->two_state) sv4_replace(&result, sv4_to_two_state(result));
    sv4_replace(&result, sv4_cast(result, ref->width, ref->is_signed));
    return result;
}

void llg_ref_write(llg_ref_t* ref, sv4_t value) {
    if (!ref) return;
    /* Signal publication can terminate the current coroutine. Both snapshots
     * belong to its unwind stack, not merely to this C stack frame. */
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    values[0] = sv4_cast(value, ref->width, ref->is_signed);
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->retained_write)
            (void)ref->retained_write(ref->retained, values[0]);
        else if (ref->queue_write)
            (void)ref->queue_write(ref->queue, ref->queue_identity, values[0]);
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        llg_ref_composite_t* composite = (llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; i++) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[1], sv4_part_select(values[0],
                (int64_t)remaining + part->width - 1, remaining));
            llg_ref_write(part, values[1]);
        }
        if (remaining) abort();
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
        (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        sv4_replace(&values[1], sv4_fill(1, ref->width, 0));
        llg_ref_write_masked(ref, values[0], values[1]);
        goto cleanup;
    }
    if (!ref->base) goto cleanup;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_WHOLE) {
        llg_ba(ref->base, values[0]);
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
        if (ref->index != UINT64_MAX && ref->index < ref->array_size)
            llg_ba(&ref->base[ref->index], values[0]);
        goto cleanup;
    }
    sv4_copy(&values[1], ref->base);
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_BIT:
        sv4_bit_select_set(&values[1], ref->index, values[0]);
        break;
    case LLG_REF_PART:
        sv4_part_select_set(&values[1], ref->left, ref->right, values[0]);
        break;
    case LLG_REF_PACKED_PLAN:
        sv4_select_plan_set(&values[1], (const sv4_select_plan_t*)ref->retained, values[0]);
        break;
    case LLG_REF_INDEXED:
        sv4_idx_part_select_set(&values[1], ref->index, ref->indexed_width,
                                ref->indexed_negative, values[0]);
        break;
    default: goto cleanup;
    }
    llg_ba(ref->base, values[1]);
cleanup:
    llg_value_scope_end(scope);
}

void llg_ref_write_masked(llg_ref_t* ref, sv4_t value, sv4_t mask) {
    if (!ref || !ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(4);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_cast(value, ref->width, ref->is_signed));
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    sv4_replace(&values[1], sv4_cast(mask, ref->width, 0));
    sv4_replace(&values[1], sv4_to_two_state(values[1]));
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; i++) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[2], sv4_part_select(values[0], (int64_t)remaining + part->width - 1, remaining));
            sv4_replace(&values[3], sv4_part_select(values[1], (int64_t)remaining + part->width - 1, remaining));
            if (sv4_to_bool(values[3])) llg_ref_write_masked(part, values[2], values[3]);
        }
        if (remaining) abort();
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
               (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        if ((llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
            sv4_replace(&values[2], llg_rt_ref_read(view->parent));
            if (!llg_ref_report_tag_failure(view, &values[2])) goto cleanup;
        }
        sv4_replace(&values[2], sv4_zero(view->plan.storage_width, 0));
        sv4_replace(&values[3], sv4_zero(view->plan.storage_width, 0));
        sv4_select_plan_set(&values[2], &view->plan, values[0]);
        sv4_select_plan_set(&values[3], &view->plan, values[1]);
        llg_ref_write_masked(view->parent, values[2], values[3]);
    } else if (sv4_to_bool(values[1])) {
        sv4_replace(&values[2], llg_ref_read(ref));
        if (llg_sv4_width(values[2]) != ref->width) abort();
        llg_sv4_masked_merge(&values[2], values[0], values[1]);
        llg_ref_write(ref, values[2]);
    }
cleanup:
    llg_value_scope_end(scope);
}

void llg_ref_nba_masked(llg_ref_t* ref, sv4_t value, sv4_t mask, uint64_t ticks) {
    if (!ref || !ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(4);
    sv4_t* values = llg_value_scope_values(scope);
    values[0] = sv4_cast(value, ref->width, ref->is_signed);
    if (ref->two_state) sv4_replace(&values[0], sv4_to_two_state(values[0]));
    values[1] = sv4_cast(mask, ref->width, 0);
    sv4_replace(&values[1], sv4_to_two_state(values[1]));
    if (!sv4_to_bool(values[1])) goto cleanup;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts) abort();
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; ++i) {
            llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) abort();
            remaining -= part->width;
            sv4_replace(&values[2], sv4_part_select(values[0], (int64_t)remaining + part->width - 1, remaining));
            sv4_replace(&values[3], sv4_part_select(values[1], (int64_t)remaining + part->width - 1, remaining));
            llg_ref_nba_masked(part, values[2], values[3], ticks);
        }
        if (remaining) abort();
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
               (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        if ((llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
            sv4_replace(&values[2], llg_rt_ref_read(view->parent));
            if (!llg_ref_report_tag_failure(view, &values[2])) goto cleanup;
        }
        sv4_replace(&values[2], sv4_zero(view->plan.storage_width, 0));
        values[3] = sv4_zero(view->plan.storage_width, 0);
        sv4_select_plan_set(&values[2], &view->plan, values[0]);
        sv4_select_plan_set(&values[3], &view->plan, values[1]);
        llg_ref_nba_masked(view->parent, values[2], values[3], ticks);
    } else if (ref->base) {
        sv4_t* target = ref->base;
        if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
            if (ref->index == UINT64_MAX || ref->index >= ref->array_size) goto cleanup;
            target = &ref->base[ref->index];
        } else if ((llg_ref_kind_t)ref->kind != LLG_REF_WHOLE) {
            values[2] = sv4_zero(llg_sv4_width(*target), llg_sv4_signed(*target));
            values[3] = sv4_zero(llg_sv4_width(*target), 0);
            switch ((llg_ref_kind_t)ref->kind) {
            case LLG_REF_BIT:
                sv4_bit_select_set(&values[2], ref->index, values[0]);
                sv4_bit_select_set(&values[3], ref->index, values[1]);
                break;
            case LLG_REF_PART:
                sv4_part_select_set(&values[2], ref->left, ref->right, values[0]);
                sv4_part_select_set(&values[3], ref->left, ref->right, values[1]);
                break;
            case LLG_REF_INDEXED:
                sv4_idx_part_select_set(&values[2], ref->index, ref->indexed_width, ref->indexed_negative, values[0]);
                sv4_idx_part_select_set(&values[3], ref->index, ref->indexed_width, ref->indexed_negative, values[1]);
                break;
            case LLG_REF_PACKED_PLAN:
                sv4_select_plan_set(&values[2], (const sv4_select_plan_t*)ref->retained, values[0]);
                sv4_select_plan_set(&values[3], (const sv4_select_plan_t*)ref->retained, values[1]);
                break;
            default: abort();
            }
            llg_nba_masked(target, values[2], values[3], ticks);
            goto cleanup;
        }
        llg_nba_masked(target, values[0], values[1], ticks);
    }
cleanup:
    llg_value_scope_end(scope);
}

/* Consumes a freshly produced input value before invoking any notification.
 * The caller must not destroy its shallow descriptor after this transfer. */
static void llg_ref_write_owned(llg_ref_t* ref, sv4_t value) {
    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* owned = llg_value_scope_values(scope);
    owned[0] = value;
    llg_ref_write(ref, owned[0]);
    llg_value_scope_end(scope);
}

void llg_ref_write_bit(llg_ref_t* ref, uint64_t index, sv4_t value) {
    if (!ref || index >= ref->width) return;
    llg_value_scope_t* scope = llg_value_scope_begin(3);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_zero(ref->width, 0));
    sv4_replace(&values[1], sv4_zero(ref->width, 0));
    sv4_replace(&values[2], sv4_from_u64(1, 1, 0));
    sv4_bit_select_set(&values[0], index, value);
    sv4_bit_select_set(&values[1], index, values[2]);
    llg_ref_write_masked(ref, values[0], values[1]);
    llg_value_scope_end(scope);
}

void llg_nba_d(double* target, double value) {
    llg_nba_d_after(target, value, 0);
}

void llg_ba_d(double* target, double value) {
    if (llg_is_real_forced(target)) return;
    real_write(target, value);
}

/* Wait sources of `ref` descriptors. Every store publishes at its variable,
 * element cell or class-property address, and container element stores
 * publish their container's contents marker. A view selects bits of its
 * parent; a composite (an unpacked aggregate) concatenates its parts, the
 * first part most significant. A dependency walk names the storage behind
 * bits [lsb, lsb + count) of a descriptor's value: its depth is the nesting
 * of the actual's aggregate type. */
static sv4_t* ref_leaf_dependency(const llg_ref_t* ref) {
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_WHOLE:
    case LLG_REF_BIT:
    case LLG_REF_PART:
    case LLG_REF_INDEXED:
        return llg_dependency_or_never(ref->base);
    case LLG_REF_QUEUE:
        if (ref->retained_read == llg_queue_cell_read)
            return llg_dependency_or_never(llg_queue_cell_dependency(ref->retained));
        if (ref->retained_read == llg_element_cell_read)
            return llg_dependency_or_never(llg_element_cell_dependency(ref->retained));
        return llg_dependency_or_never(ref->queue ? ref->queue->contents_dependency : NULL);
    default:
        return llg_dependency_or_never(NULL);
    }
}

static size_t ref_dependency_range(const llg_ref_t* ref, uint32_t lsb, uint32_t count,
                                   llg_wait_dependency_t* out) {
    if (!ref || !count) return 0;
    llg_ref_kind_t kind = (llg_ref_kind_t)ref->kind;
    if (kind == LLG_REF_COMPOSITE && ref->retained) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        size_t found = 0;
        uint32_t remaining = ref->width;
        for (size_t i = 0; i < composite->count; i++) {
            const llg_ref_t* part = composite->parts[i];
            uint32_t width = part ? part->width : 0;
            if (width > remaining) break;
            remaining -= width;
            uint64_t low = lsb > remaining ? lsb : remaining;
            uint64_t high = (uint64_t)lsb + count < (uint64_t)remaining + width
                                ? (uint64_t)lsb + count : (uint64_t)remaining + width;
            if (low < high)
                found += ref_dependency_range(part, (uint32_t)(low - remaining),
                                              (uint32_t)(high - low),
                                              out ? out + found : NULL);
        }
        return found;
    }
    if ((kind == LLG_REF_VIEW || kind == LLG_REF_TAGGED_VIEW) && ref->retained) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view->parent) return 0;
        // A tagged view's validity also depends on its tag bits.
        if (kind == LLG_REF_TAGGED_VIEW)
            return ref_dependency_range(view->parent, 0, view->parent->width, out);
        uint64_t low = lsb > view->plan.value_lsb ? lsb : view->plan.value_lsb;
        uint64_t high = (uint64_t)lsb + count;
        uint64_t end = (uint64_t)view->plan.value_lsb + view->plan.count;
        if (end < high) high = end;
        if (low >= high) return 0; // only bits the selection leaves undefined
        return ref_dependency_range(
            view->parent, (uint32_t)(view->plan.storage_lsb + (low - view->plan.value_lsb)),
            (uint32_t)(high - low), out);
    }
    if (out) *out = (llg_wait_dependency_t){ .sig = ref_leaf_dependency(ref) };
    return 1;
}

static size_t ref_dependencies(const llg_ref_t* ref, llg_wait_dependency_t* out) {
    return ref ? ref_dependency_range(ref, 0, ref->width, out) : 0;
}

static void ref_wait_eval(sv4_t* out, void* context) {
    const llg_ref_t* ref = *llg_frame_opaque_address((llg_frame_t*)context, 0);
    llg_rt_ref_read_to(out, ref);
}

llg_expr_event_spec_t llg_ref_wait_spec(const llg_ref_t* ref, int kind) {
    llg_expr_event_spec_t spec = { .kind = kind };
    if (ref && (llg_ref_kind_t)ref->kind == LLG_REF_WHOLE) {
        spec.sig = llg_dependency_or_never(ref->base);
        return spec;
    }
    size_t count = ref_dependencies(ref, NULL);
    if (count > INT_MAX) {
        fprintf(stderr, "llg: `ref` wait names too many storage locations\n");
        abort();
    }
    llg_frame_t* frame = llg_frame_new(2);
    llg_frame_capture_opaque(frame, 0, (void*)ref);
    llg_wait_dependency_t* dependencies = (llg_wait_dependency_t*)llg_frame_capture_object(
        frame, 1, (count ? count : 1) * sizeof(*dependencies), NULL);
    ref_dependencies(ref, dependencies);
    spec.eval = ref_wait_eval;
    spec.eval_context = frame;
    spec.dependencies = dependencies;
    spec.n_dependencies = (int)count;
    // Nothing can change a value whose selection is out of range.
    if (!count) spec.sig = llg_dependency_or_never(NULL);
    return spec;
}

enum { REF_WAIT_INLINE_DEPENDENCIES = 8 };

llg_co_arm_t llg_arm_ref_dependencies(llg_proc_t* self, const llg_wait_dependency_t* deps,
                                      int n, const llg_ref_t* const* refs, int n_refs,
                                      int event) {
    if (n < 0 || n_refs < 0) abort();
    size_t total = (size_t)n;
    for (int i = 0; i < n_refs; i++) total += ref_dependencies(refs[i], NULL);
    if (total > INT_MAX) {
        fprintf(stderr, "llg: `ref` wait names too many storage locations\n");
        abort();
    }
    // The arm copies the list; whole descriptors fit the inline buffer.
    llg_wait_dependency_t inline_buffer[REF_WAIT_INLINE_DEPENDENCIES];
    llg_wait_dependency_t* all = total <= REF_WAIT_INLINE_DEPENDENCIES
        ? inline_buffer
        : (llg_wait_dependency_t*)llg_checked_malloc(total, sizeof(*all),
                                                     "reference wait dependencies");
    if (n) memcpy(all, deps, (size_t)n * sizeof(*all));
    size_t used = (size_t)n;
    for (int i = 0; i < n_refs; i++) used += ref_dependencies(refs[i], all + used);
    llg_co_arm_t arm = event ? llg_arm_event_dependencies(self, all, (int)total)
                             : llg_arm_any_dependencies(self, all, (int)total);
    if (all != inline_buffer) free(all);
    return arm;
}
