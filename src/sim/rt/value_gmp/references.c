#include "internal.h"
int llg_gmp_ref_view_valid(const llg_gmp_ref_view_t* view, const g4_t* parent,
                           size_t* failed_check) {
    if (failed_check)
        *failed_check = 0;
    if (!view || !parent || (view->tag_check_count && !view->tag_checks))
        return 0;
    for (size_t index = 0; index < view->tag_check_count; ++index) {
        const llg_gmp_ref_tag_check_t* check = &view->tag_checks[index];
        g4_t receiver = llg_gmp_sv4_select_plan_read(*parent, &check->receiver_plan);
        if (!check->tag_width || check->tag_width > receiver.width) {
            llg_gmp_sv4_destroy(&receiver);
            if (failed_check)
                *failed_check = index;
            return 0;
        }
        int64_t right = (int64_t)receiver.width - check->tag_width;
        int64_t left = (int64_t)receiver.width - 1;
        g4_t tag = llg_gmp_sv4_part_select(receiver, left, right);
        g4_t expected = llg_gmp_sv4_from_u64(check->member_index, check->tag_width, 0);
        g4_t matches = llg_gmp_sv4_case_eq(tag, expected);
        int valid = llg_gmp_sv4_to_bool(matches);
        llg_gmp_sv4_destroy(&matches);
        llg_gmp_sv4_destroy(&expected);
        llg_gmp_sv4_destroy(&tag);
        llg_gmp_sv4_destroy(&receiver);
        if (!valid) {
            if (failed_check)
                *failed_check = index;
            return 0;
        }
    }
    return 1;
}

g4_t llg_gmp_ref_read(const llg_gmp_ref_t* ref) {
    if (!ref)
        return llg_gmp_sv4_x(1, 0);
    if ((llg_gmp_ref_kind_t)ref->kind == LLG_GMP_REF_QUEUE) {
        if (ref->retained_read)
            return ref->retained_read(ref->retained);
        if (!ref->queue_read)
            return ref->two_state ? llg_gmp_sv4_from_u64(0, ref->width, ref->is_signed)
                                  : llg_gmp_sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        g4_t value = ref->queue_read(ref->queue, ref->queue_identity);
        llg_gmp_sv4_replace(&value, llg_gmp_sv4_cast(value, ref->width, ref->is_signed));
        if (ref->two_state)
            llg_gmp_sv4_replace(&value, llg_gmp_sv4_to_two_state(value));
        return value;
    }
    if ((llg_gmp_ref_kind_t)ref->kind == LLG_GMP_REF_COMPOSITE) {
        const llg_gmp_ref_composite_t* composite = (const llg_gmp_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts || !composite->count) {
            llg_gmp_sv4_fail("invalid composite reference");
        }
        uint32_t remaining = ref->width;
        g4_t result = llg_gmp_sv4_x(ref->width, ref->is_signed);
        for (size_t i = 0; i < composite->count; i++) {
            const llg_gmp_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining) {
                llg_gmp_sv4_fail("invalid composite reference width");
            }
            remaining -= part->width;
            g4_t value = llg_gmp_ref_read(part);
            llg_gmp_sv4_part_select_set(&result, (int64_t)remaining + part->width - 1, remaining,
                                        value);
            llg_gmp_sv4_destroy(&value);
        }
        if (remaining) {
            llg_gmp_sv4_fail("incomplete composite reference");
        }
        return result;
    }
    if ((llg_gmp_ref_kind_t)ref->kind == LLG_GMP_REF_VIEW ||
        (llg_gmp_ref_kind_t)ref->kind == LLG_GMP_REF_TAGGED_VIEW) {
        const llg_gmp_ref_view_t* view = (const llg_gmp_ref_view_t*)ref->retained;
        if (!view || !view->parent) {
            llg_gmp_sv4_fail("invalid reference view");
        }
        g4_t parent = llg_gmp_ref_read(view->parent);
        int valid = (llg_gmp_ref_kind_t)ref->kind != LLG_GMP_REF_TAGGED_VIEW ||
                    llg_gmp_ref_view_valid(view, &parent, NULL);
        g4_t result = valid ? llg_gmp_sv4_select_plan_read(parent, &view->plan)
                            : (ref->two_state ? llg_gmp_sv4_zero(ref->width, ref->is_signed)
                                              : llg_gmp_sv4_x(ref->width, ref->is_signed));
        llg_gmp_sv4_destroy(&parent);
        if (ref->two_state)
            llg_gmp_sv4_replace(&result, llg_gmp_sv4_to_two_state(result));
        llg_gmp_sv4_replace(&result, llg_gmp_sv4_cast(result, ref->width, ref->is_signed));
        return result;
    }
    if (!ref->base)
        return ref->two_state ? llg_gmp_sv4_zero(ref->width, ref->is_signed)
                              : llg_gmp_sv4_x(ref->width ? ref->width : 1, ref->is_signed);
    g4_t value;
    switch ((llg_gmp_ref_kind_t)ref->kind) {
    case LLG_GMP_REF_WHOLE:
        value = llg_gmp_sv4_clone(ref->base);
        break;
    case LLG_GMP_REF_BIT:
        value = llg_gmp_sv4_bit_select(*ref->base, ref->index);
        break;
    case LLG_GMP_REF_PART:
        value = llg_gmp_sv4_part_select(*ref->base, ref->left, ref->right);
        break;
    case LLG_GMP_REF_INDEXED:
        value = llg_gmp_sv4_idx_part_select(*ref->base, ref->index, ref->indexed_width,
                                            ref->indexed_negative);
        break;
    case LLG_GMP_REF_PACKED_PLAN:
        value = llg_gmp_sv4_select_plan_read(*ref->base,
                                             (const llg_gmp_sv4_select_plan_t*)ref->retained);
        break;
    case LLG_GMP_REF_ARRAY:
        if (ref->index == UINT64_MAX || ref->index >= ref->array_size)
            return ref->two_state ? llg_gmp_sv4_from_u64(0, ref->width, ref->is_signed)
                                  : llg_gmp_sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        value = llg_gmp_sv4_clone(&ref->base[ref->index]);
        break;
    default:
        return llg_gmp_sv4_x(ref->width ? ref->width : 1, ref->is_signed);
    }
    llg_gmp_sv4_replace(&value, llg_gmp_sv4_cast(value, ref->width, ref->is_signed));
    if (ref->two_state)
        llg_gmp_sv4_replace(&value, llg_gmp_sv4_to_two_state(value));
    return value;
}
