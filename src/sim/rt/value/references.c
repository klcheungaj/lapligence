#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>

int llg_ref_view_valid(const llg_ref_view_t* view, const sv4_t* parent,
                       size_t* failed_check) {
    if (failed_check) *failed_check = 0;
    if (!view || !parent || (view->tag_check_count && !view->tag_checks)) return 0;
    for (size_t index = 0; index < view->tag_check_count; ++index) {
        const llg_ref_tag_check_t* check = &view->tag_checks[index];
        sv4_t receiver = sv4_select_plan_read(*parent, &check->receiver_plan);
        if (!check->tag_width || check->tag_width > llg_sv4_width(receiver)) {
            sv4_destroy(&receiver);
            if (failed_check) *failed_check = index;
            return 0;
        }
        int64_t right = (int64_t)llg_sv4_width(receiver) - check->tag_width;
        int64_t left = (int64_t)llg_sv4_width(receiver) - 1;
        sv4_t tag = sv4_part_select(receiver, left, right);
        sv4_t expected = sv4_from_u64(check->member_index, check->tag_width, 0);
        sv4_t matches = sv4_case_eq(tag, expected);
        int valid = sv4_to_bool(matches);
        sv4_destroy(&matches);
        sv4_destroy(&expected);
        sv4_destroy(&tag);
        sv4_destroy(&receiver);
        if (!valid) {
            if (failed_check) *failed_check = index;
            return 0;
        }
    }
    return 1;
}

sv4_t llg_ref_read(const llg_ref_t* ref) {
    if (!ref) return sv4_x(1, 0);
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->retained_read) return ref->retained_read(ref->retained);
        if (!ref->queue_read)
            return ref->two_state
                       ? sv4_from_u64(0, ref->width, ref->is_signed)
                       : sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        sv4_t value = ref->queue_read(ref->queue, ref->queue_identity);
        sv4_replace(&value, sv4_cast(value, ref->width, ref->is_signed));
        if (ref->two_state) sv4_replace(&value, sv4_to_two_state(value));
        return value;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_COMPOSITE) {
        const llg_ref_composite_t* composite = (const llg_ref_composite_t*)ref->retained;
        if (!composite || !composite->parts || !composite->count)
            { fputs("llg runtime fatal: invalid composite reference\n", stderr); abort(); }
        uint32_t remaining = ref->width;
        sv4_t result = sv4_x(ref->width, ref->is_signed);
        for (size_t i = 0; i < composite->count; i++) {
            const llg_ref_t* part = composite->parts[i];
            if (!part || !part->width || part->width > remaining)
                { fputs("llg runtime fatal: invalid composite reference width\n", stderr); abort(); }
            remaining -= part->width;
            sv4_t value = llg_ref_read(part);
            sv4_part_select_set(&result, (int64_t)remaining + part->width - 1,
                               remaining, value);
            sv4_destroy(&value);
        }
        if (remaining) { fputs("llg runtime fatal: incomplete composite reference\n", stderr); abort(); }
        return result;
    }
    if ((llg_ref_kind_t)ref->kind == LLG_REF_VIEW ||
        (llg_ref_kind_t)ref->kind == LLG_REF_TAGGED_VIEW) {
        const llg_ref_view_t* view = (const llg_ref_view_t*)ref->retained;
        if (!view || !view->parent) { fputs("llg runtime fatal: invalid reference view\n", stderr); abort(); }
        sv4_t parent = llg_ref_read(view->parent);
        int valid = (llg_ref_kind_t)ref->kind != LLG_REF_TAGGED_VIEW ||
                    llg_ref_view_valid(view, &parent, NULL);
        sv4_t result = valid ? sv4_select_plan_read(parent, &view->plan)
                             : (ref->two_state ? sv4_zero(ref->width, ref->is_signed)
                                               : sv4_x(ref->width, ref->is_signed));
        sv4_destroy(&parent);
        if (ref->two_state) sv4_replace(&result, sv4_to_two_state(result));
        sv4_replace(&result, sv4_cast(result, ref->width, ref->is_signed));
        return result;
    }
    if (!ref->base) return ref->two_state ? sv4_zero(ref->width, ref->is_signed)
                                        : sv4_x(ref->width ? ref->width : 1, ref->is_signed);
    sv4_t value;
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_WHOLE:
        value = sv4_clone(ref->base);
        break;
    case LLG_REF_BIT:
        value = sv4_bit_select(*ref->base, ref->index);
        break;
    case LLG_REF_PART:
        value = sv4_part_select(*ref->base, ref->left, ref->right);
        break;
    case LLG_REF_INDEXED:
        value = sv4_idx_part_select(*ref->base, ref->index,
                                    ref->indexed_width,
                                    ref->indexed_negative);
        break;
    case LLG_REF_PACKED_PLAN:
        value = sv4_select_plan_read(*ref->base, (const sv4_select_plan_t*)ref->retained);
        break;
    case LLG_REF_ARRAY:
        if (ref->index == UINT64_MAX || ref->index >= ref->array_size)
            return ref->two_state ? sv4_from_u64(0, ref->width, ref->is_signed)
                                  : sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        value = sv4_clone(&ref->base[ref->index]);
        break;
    default:
        return sv4_x(ref->width ? ref->width : 1, ref->is_signed);
    }
    sv4_replace(&value, sv4_cast(value, ref->width, ref->is_signed));
    if (ref->two_state) sv4_replace(&value, sv4_to_two_state(value));
    return value;
}
