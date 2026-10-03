#include "ranges.h"

/* Selection only needs representability, not a lossy scalar conversion. */
static int g4_selection_index(g4_t v, int64_t* out) {
    if (llg_gmp_sv4_is_unknown(v))
        return 0;
    uint64_t low = v.width ? g4_a(&v)[0] : 0;
    int negative = v.is_signed && v.width && llg_gmp_sv4_state(v, v.width - 1) == 1;
    if (v.width > 64) {
        if ((int)(low >> 63) != negative)
            return 0;
        size_t n = llg_gmp_sv4_words(v);
        const uint64_t* a = g4_a(&v);
        for (size_t i = 1; i < n; ++i) {
            uint64_t mask = i + 1 == n ? g4_topmask(v.width) : UINT64_MAX;
            if (a[i] != (negative ? mask : 0))
                return 0;
        }
    } else if (negative)
        low |= ~g4_mask(v.width);
    else if (low > INT64_MAX)
        return 0;
    *out = low <= INT64_MAX ? (int64_t)low : -1 - (int64_t)~low;
    return 1;
}
g4_t llg_gmp_sv4_part_select_wide(g4_t v, int64_t left, int64_t right) {
    uint32_t w = g4_selection_part_width(left, right);
    g4_t r = llg_gmp_sv4_zero(w, 0);
    if (left >= right) {
        g4_copy_window(&r, 0, v, right, w);
        llg_gmp_sv4_finish(&r);
        return r;
    }
    int64_t step = left > right ? -1 : 1;
    int out = 0;
    for (int64_t i = left;; i += step) {
        int b = i < 0 || i >= (int64_t)v.width ? 2 : g4_range_state(v, (int)i);
        int pos = w - 1 - out; // first index (left) is the MSB
        g4_put_state(&r, pos, b);
        out++;
        if (i == right)
            break;
    }
    llg_gmp_sv4_finish(&r);
    return r;
}

void llg_gmp_sv4_part_select_set_wide(g4_t* tgt, int64_t left, int64_t right, g4_t value) {
    uint32_t width = g4_selection_part_width(left, right);
    g4_t snapshot = LLG_GMP_SV4_EMPTY;
    if (g4_alias(*tgt, value)) {
        snapshot = llg_gmp_sv4_clone(&value);
        value = snapshot;
    }
    if (left >= right) {
        g4_copy_window(tgt, right, value, (int64_t)value.width - width, width);
        llg_gmp_sv4_finish(tgt);
        llg_gmp_sv4_destroy(&snapshot);
        return;
    }
    int64_t step = left > right ? -1 : 1;
    int in = (int)value.width - 1; // value MSB maps to the first target index
    for (int64_t i = left;; i += step) {
        if (i < 0 || i >= (int64_t)tgt->width) {
            in--;
            if (i == right)
                break;
            continue;
        }
        g4_put_state(tgt, (int)i, g4_range_state(value, in));
        in--;
        if (i == right)
            break;
    }
    llg_gmp_sv4_finish(tgt);
    llg_gmp_sv4_destroy(&snapshot);
}

// A negative indexed select with a negative base is entirely out of range.
static int llg_gmp_sv4_indexed_low(int64_t base, uint32_t width, int neg, int64_t* low) {
    if (!width || (neg && base < 0))
        return 0;
    *low = neg ? base - ((int64_t)width - 1) : base;
    return 1;
}

g4_t llg_gmp_sv4_idx_part_select_wide(g4_t v, uint64_t base, uint32_t width, int neg) {
    g4_selection_width(width);
    g4_t result = llg_gmp_sv4_zero(width, 0);
    int64_t low;
    if (base <= INT64_MAX && llg_gmp_sv4_indexed_low((int64_t)base, width, neg, &low))
        g4_copy_window(&result, 0, v, low, width);
    else
        g4_fill_bits(&result, 0, width, 2);
    llg_gmp_sv4_finish(&result);
    return result;
}

void llg_gmp_sv4_idx_part_select_set_wide(g4_t* tgt, uint64_t base, uint32_t width, int neg,
                                          g4_t value) {
    g4_selection_width(width);
    int64_t low;
    if (base > INT64_MAX || !llg_gmp_sv4_indexed_low((int64_t)base, width, neg, &low))
        return;
    g4_t snapshot = LLG_GMP_SV4_EMPTY;
    if (g4_alias(*tgt, value)) {
        snapshot = llg_gmp_sv4_clone(&value);
        value = snapshot;
    }
    g4_copy_window(tgt, low, value, 0, width);
    llg_gmp_sv4_finish(tgt);
    llg_gmp_sv4_destroy(&snapshot);
}

g4_t llg_gmp_sv4_idx_part_select_value_wide(g4_t v, g4_t base, uint32_t width, int neg) {
    g4_selection_width(width);
    int64_t signed_base, low;
    g4_t result = llg_gmp_sv4_zero(width, 0);
    if (g4_selection_index(base, &signed_base) &&
        llg_gmp_sv4_indexed_low(signed_base, width, neg, &low))
        g4_copy_window(&result, 0, v, low, width);
    else
        g4_fill_bits(&result, 0, width, 2);
    llg_gmp_sv4_finish(&result);
    return result;
}

void llg_gmp_sv4_idx_part_select_set_value_wide(g4_t* tgt, g4_t base, uint32_t width, int neg,
                                                g4_t value) {
    g4_selection_width(width);
    int64_t signed_base, low;
    if (!g4_selection_index(base, &signed_base) ||
        !llg_gmp_sv4_indexed_low(signed_base, width, neg, &low))
        return;
    g4_t snapshot = LLG_GMP_SV4_EMPTY;
    if (g4_alias(*tgt, value)) {
        snapshot = llg_gmp_sv4_clone(&value);
        value = snapshot;
    }
    g4_copy_window(tgt, low, value, 0, width);
    llg_gmp_sv4_finish(tgt);
    llg_gmp_sv4_destroy(&snapshot);
}

llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_init(uint32_t storage_width) {
    llg_gmp_sv4_select_plan_t plan = {storage_width, storage_width, 0, 0, storage_width};
    g4_selection_plan_check(&plan);
    return plan;
}

static llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_window(uint32_t storage_width,
                                                                uint32_t width, int64_t low) {
    llg_gmp_sv4_select_plan_t plan = llg_gmp_sv4_select_plan_init(storage_width);
    g4_selection_width(width);
    plan.width = width;
    if (low >= (int64_t)storage_width || low <= -(int64_t)width) {
        plan.storage_lsb = plan.value_lsb = plan.count = 0;
        g4_selection_plan_check(&plan);
        return plan;
    }
    int64_t start = low > 0 ? low : 0;
    int64_t end = low + (int64_t)width;
    if (end > (int64_t)storage_width)
        end = storage_width;
    plan.storage_lsb = (uint32_t)start;
    plan.value_lsb = (uint32_t)(start - low);
    plan.count = (uint32_t)(end - start);
    g4_selection_plan_check(&plan);
    return plan;
}

llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_bit(uint32_t storage_width, uint64_t index) {
    if (index >= storage_width)
        return llg_gmp_sv4_select_plan_window(storage_width, 1, -1);
    return llg_gmp_sv4_select_plan_window(storage_width, 1, (int64_t)index);
}

llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_part(uint32_t storage_width, int64_t left,
                                                       int64_t right) {
    uint32_t width = g4_selection_part_width(left, right);
    int64_t low = left < right ? left : right;
    return llg_gmp_sv4_select_plan_window(storage_width, width, low);
}

llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_indexed(uint32_t storage_width, g4_t base,
                                                          uint32_t width, int negative) {
    if (!width)
        llg_gmp_sv4_fail("packed selection width must be positive");
    g4_selection_width(width);
    int64_t index;
    if (!g4_selection_index(base, &index))
        return llg_gmp_sv4_select_plan_window(storage_width, width, -(int64_t)width);
    if (negative) {
        if (index < 0)
            return llg_gmp_sv4_select_plan_window(storage_width, width, -(int64_t)width);
        index -= (int64_t)width - 1;
    }
    return llg_gmp_sv4_select_plan_window(storage_width, width, index);
}

void llg_gmp_sv4_select_plan_step(llg_gmp_sv4_select_plan_t* plan, g4_t base, uint32_t width) {
    g4_selection_plan_check(plan);
    g4_selection_width(width);
    if (!width) {
        llg_gmp_sv4_fail("packed selection width must be positive");
    }
    int64_t low = 0;
    if (!plan->count || !g4_selection_index(base, &low) || low >= (int64_t)plan->width ||
        low <= -(int64_t)width) {
        plan->width = width;
        plan->storage_lsb = plan->value_lsb = plan->count = 0;
        return;
    }
    // The preceding inequalities bound low before addition, even for an
    // INT64_MIN/MAX input. All following arithmetic is below twice the
    // supported packed width, not proportional to the source index value.
    int64_t high = low + (int64_t)width;
    int64_t valid_low = (int64_t)plan->value_lsb;
    int64_t valid_high = valid_low + (int64_t)plan->count;
    int64_t start = low > valid_low ? low : valid_low;
    int64_t end = high < valid_high ? high : valid_high;
    if (end <= start) {
        plan->storage_lsb = plan->value_lsb = plan->count = 0;
    } else {
        plan->storage_lsb += (uint32_t)(start - valid_low);
        plan->value_lsb = (uint32_t)(start - low);
        plan->count = (uint32_t)(end - start);
    }
    plan->width = width;
    g4_selection_plan_check(plan);
}

g4_t llg_gmp_sv4_select_plan_read_wide(g4_t source, const llg_gmp_sv4_select_plan_t* plan) {
    g4_selection_plan_check(plan);
    if (source.width != plan->storage_width) {
        llg_gmp_sv4_fail("packed selection storage width mismatch");
    }
    g4_t result = llg_gmp_sv4_zero(plan->width, 0);
    g4_fill_bits(&result, 0, plan->value_lsb, 2);
    g4_fill_bits(&result, plan->value_lsb + plan->count,
                 plan->width - plan->value_lsb - plan->count, 2);
    g4_copy_bits(&result, plan->value_lsb, source, plan->storage_lsb, plan->count);
    llg_gmp_sv4_finish(&result);
    return result;
}

g4_t llg_gmp_sv4_select_plan_slice_wide(g4_t source, const llg_gmp_sv4_select_plan_t* plan,
                                        int reverse) {
    g4_selection_plan_check(plan);
    if (source.width != plan->width) {
        llg_gmp_sv4_fail("packed selection source width mismatch");
    }
    g4_t result = llg_gmp_sv4_zero(plan->count, 0);
    if (!reverse) {
        g4_copy_bits(&result, 0, source, plan->value_lsb, plan->count);
        llg_gmp_sv4_finish(&result);
        return result;
    }
    for (uint32_t i = 0; i < plan->count; ++i) {
        uint32_t logical = plan->value_lsb + i;
        uint32_t source_bit = reverse ? plan->width - 1 - logical : logical;
        g4_put_state(&result, (int)i, g4_range_state(source, (int)source_bit));
    }
    llg_gmp_sv4_finish(&result);
    return result;
}

void llg_gmp_sv4_select_plan_set_wide(g4_t* destination, const llg_gmp_sv4_select_plan_t* plan,
                                      g4_t source) {
    g4_selection_plan_check(plan);
    if (!destination || destination->width != plan->storage_width || source.width != plan->width) {
        llg_gmp_sv4_fail("packed selection assignment width mismatch");
    }
    if (!plan->count)
        return;
    g4_t snapshot = LLG_GMP_SV4_EMPTY;
    if (g4_alias(source, *destination)) {
        snapshot = llg_gmp_sv4_clone(&source);
        source = snapshot; // Borrow the snapshot until the update completes.
    }
    g4_copy_bits(destination, plan->storage_lsb, source, plan->value_lsb, plan->count);
    llg_gmp_sv4_finish(destination);
    llg_gmp_sv4_destroy(&snapshot);
}
