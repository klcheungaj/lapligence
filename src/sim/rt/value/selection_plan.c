// Composed packed slices. Private fragment of llg_value.c.

static void sv4_select_plan_check(const sv4_select_plan_t* plan) {
    if (!plan || !plan->storage_width || !plan->width ||
        plan->storage_width >= LLG_SUPPORTED_WIDTH_LIMIT ||
        plan->width >= LLG_SUPPORTED_WIDTH_LIMIT ||
        plan->storage_lsb > plan->storage_width ||
        plan->value_lsb > plan->width ||
        plan->count > plan->storage_width - plan->storage_lsb ||
        plan->count > plan->width - plan->value_lsb) {
        fputs("llg runtime fatal: invalid packed selection plan\n", stderr);
        abort();
    }
}

sv4_select_plan_t sv4_select_plan_init(uint32_t storage_width) {
    sv4_select_plan_t plan = {storage_width, storage_width, 0, 0, storage_width};
    sv4_select_plan_check(&plan);
    return plan;
}

static sv4_select_plan_t sv4_select_plan_window(uint32_t storage_width,
                                                uint32_t width, int64_t low) {
    sv4_select_plan_t plan = sv4_select_plan_init(storage_width);
    sv4_require_width(width, "packed selection");
    plan.width = width;
    if (low >= (int64_t)storage_width || low <= -(int64_t)width) {
        plan.storage_lsb = plan.value_lsb = plan.count = 0;
        sv4_select_plan_check(&plan);
        return plan;
    }
    int64_t start = low > 0 ? low : 0;
    int64_t end = low + (int64_t)width;
    if (end > (int64_t)storage_width) end = storage_width;
    plan.storage_lsb = (uint32_t)start;
    plan.value_lsb = (uint32_t)(start - low);
    plan.count = (uint32_t)(end - start);
    sv4_select_plan_check(&plan);
    return plan;
}

sv4_select_plan_t sv4_select_plan_bit(uint32_t storage_width, uint64_t index) {
    if (index >= storage_width) return sv4_select_plan_window(storage_width, 1, -1);
    return sv4_select_plan_window(storage_width, 1, (int64_t)index);
}

sv4_select_plan_t sv4_select_plan_part(uint32_t storage_width,
                                       int64_t left, int64_t right) {
    uint32_t width = llg_part_select_width(left, right);
    int64_t low = left < right ? left : right;
    return sv4_select_plan_window(storage_width, width, low);
}

sv4_select_plan_t sv4_select_plan_indexed(uint32_t storage_width, sv4_t base,
                                          uint32_t width, int negative) {
    sv4_require_width(width, "indexed part-select");
    int64_t index;
    if (!sv4_to_index_i64(base, &index))
        return sv4_select_plan_window(storage_width, width, -(int64_t)width);
    if (negative) {
        if (index < 0)
            return sv4_select_plan_window(storage_width, width, -(int64_t)width);
        index -= (int64_t)width - 1;
    }
    return sv4_select_plan_window(storage_width, width, index);
}

void sv4_select_plan_step(sv4_select_plan_t* plan, sv4_t base, uint32_t width) {
    sv4_select_plan_check(plan);
    sv4_require_width(width, "packed selection");
    if (!width) {
        fputs("llg runtime fatal: packed selection width must be positive\n", stderr);
        abort();
    }
    int64_t low = 0;
    if (!plan->count || !sv4_to_index_i64(base, &low) ||
        low >= (int64_t)plan->width || low <= -(int64_t)width) {
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
    sv4_select_plan_check(plan);
}

sv4_t sv4_select_plan_read(sv4_t source, const sv4_select_plan_t* plan) {
    sv4_select_plan_check(plan);
    if (source.width != plan->storage_width) {
        fputs("llg runtime fatal: packed selection storage width mismatch\n", stderr);
        abort();
    }
    sv4_t result = sv4_x(plan->width, 0);
    sv4_copy_bits(&result, plan->value_lsb, source, plan->storage_lsb, plan->count);
    return result;
}

sv4_t sv4_select_plan_slice(sv4_t source, const sv4_select_plan_t* plan,
                            int reverse) {
    sv4_select_plan_check(plan);
    if (source.width != plan->width) {
        fputs("llg runtime fatal: packed selection source width mismatch\n", stderr);
        abort();
    }
    sv4_t result = sv4_zero(plan->count, 0);
    if (!reverse) {
        sv4_copy_bits(&result, 0, source, plan->value_lsb, plan->count);
        return result;
    }
    for (uint32_t i = 0; i < plan->count; ++i) {
        uint32_t logical = plan->value_lsb + i;
        uint32_t source_bit = reverse ? plan->width - 1 - logical : logical;
        sv4_lsb_bit_set(&result, (int)i, sv4_lsb_bit(source, (int)source_bit));
    }
    return result;
}

void sv4_select_plan_set(sv4_t* destination, const sv4_select_plan_t* plan, sv4_t source) {
    sv4_select_plan_check(plan);
    if (!destination || destination->width != plan->storage_width || source.width != plan->width) {
        fputs("llg runtime fatal: packed selection assignment width mismatch\n", stderr);
        abort();
    }
    if (!plan->count) return;
    sv4_t snapshot = SV4_EMPTY;
    if (source.bits && source.bits == destination->bits) {
        snapshot = sv4_clone(&source);
        source = snapshot; // Borrow the snapshot until the update completes.
    }
    sv4_copy_bits(destination, plan->storage_lsb, source, plan->value_lsb, plan->count);
    sv4_destroy(&snapshot);
}
