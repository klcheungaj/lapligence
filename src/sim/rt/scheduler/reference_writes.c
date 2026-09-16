
void llg_ref_write(llg_ref_t* ref, sv4_t value) {
    if (!ref) return;
    sv4_t converted = sv4_cast(value, ref->width, ref->is_signed);
    sv4_t updated = SV4_EMPTY;
    if (ref->two_state) sv4_replace(&converted, sv4_to_two_state(converted));
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->retained_write)
            (void)ref->retained_write(ref->retained, converted);
        else if (ref->queue_write)
            (void)ref->queue_write(ref->queue, ref->queue_identity, converted);
        goto cleanup;
    }
    if (!ref->base) goto cleanup;
    if ((llg_ref_kind_t)ref->kind == LLG_REF_WHOLE) {
        llg_ba(ref->base, converted);
        goto cleanup;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_ARRAY) {
        if (ref->index != UINT64_MAX && ref->index < ref->array_size)
            llg_ba(&ref->base[ref->index], converted);
        goto cleanup;
    }
    sv4_copy(&updated, ref->base);
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_BIT:
        sv4_bit_select_set(&updated, ref->index, converted);
        break;
    case LLG_REF_PART:
        sv4_part_select_set(&updated, ref->left, ref->right, converted);
        break;
    case LLG_REF_INDEXED:
        sv4_idx_part_select_set(&updated, ref->index, ref->indexed_width,
                                ref->indexed_negative, converted);
        break;
    default: goto cleanup;
    }
    llg_ba(ref->base, updated);
cleanup:
    sv4_destroy(&updated);
    sv4_destroy(&converted);
}

void llg_ref_write_bit(llg_ref_t* ref, uint64_t index, sv4_t value) {
    if (!ref || index >= ref->width) return;
    sv4_t updated = llg_ref_read(ref);
    sv4_bit_select_set(&updated, index, value);
    llg_ref_write(ref, updated);
    sv4_destroy(&updated);

}

void llg_nba_d(double* target, double value) {
    llg_nba_d_after(target, value, 0);
}

void llg_ba_d(double* target, double value) {
    if (llg_is_real_forced(target) || pca_real_active(target)) return;
    real_write(target, value);
}
