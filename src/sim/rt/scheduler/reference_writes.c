
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
    sv4_t updated = llg_ref_read(ref);
    sv4_bit_select_set(&updated, index, value);
    llg_ref_write_owned(ref, updated);

}

void llg_nba_d(double* target, double value) {
    llg_nba_d_after(target, value, 0);
}

void llg_ba_d(double* target, double value) {
    if (llg_is_real_forced(target) || pca_real_active(target)) return;
    real_write(target, value);
}
