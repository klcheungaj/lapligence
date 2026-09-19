
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
    if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW) {
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
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        sv4_replace(&values[2], sv4_zero(view->plan.storage_width, 0));
        sv4_replace(&values[3], sv4_zero(view->plan.storage_width, 0));
        sv4_select_plan_set(&values[2], &view->plan, values[0]);
        sv4_select_plan_set(&values[3], &view->plan, values[1]);
        llg_ref_write_masked(view->parent, values[2], values[3]);
    } else if (sv4_to_bool(values[1])) {
        sv4_replace(&values[2], llg_ref_read(ref));
        if (values[2].width != ref->width) abort();
        for (uint32_t i = 0; i < (ref->width + 63u) / 64u; i++) {
            const uint64_t bits = values[1].bits[i];
            values[2].bits[i] = (values[2].bits[i] & ~bits) | (values[0].bits[i] & bits);
            values[2].x[i] = (values[2].x[i] & ~bits) | (values[0].x[i] & bits);
            values[2].z[i] = (values[2].z[i] & ~bits) | (values[0].z[i] & bits);
        }
        llg_ref_write(ref, values[2]);
    }
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
    } else if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) abort();
        values[2] = sv4_zero(view->plan.storage_width, 0);
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
            values[2] = sv4_zero(target->width, target->is_signed);
            values[3] = sv4_zero(target->width, 0);
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
    if (llg_is_real_forced(target) || pca_real_active(target)) return;
    real_write(target, value);
}
