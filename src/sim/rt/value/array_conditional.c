// Fixed unpacked-array and record conditional merges. Private fragment of
// llg_value.c. Payload flattening must not change IEEE 1800-2009 11.4.11 leaf
// semantics.

sv4_t sv4_array_conditional_merge(sv4_t a, sv4_t b, sv4_t element_default) {
    const uint32_t stride = element_default.width;
    if (!a.width || a.width >= LLG_SUPPORTED_WIDTH_LIMIT || a.width != b.width ||
        !stride || stride > a.width || a.width % stride != 0) {
        fputs("llg runtime fatal: invalid array conditional merge shape\n", stderr);
        abort();
    }
    sv4_t result = sv4_zero(a.width, 0);
    for (uint32_t offset = 0; offset < a.width; offset += stride) {
        int equal = 1;
        for (uint32_t i = 0; i < stride; ++i) {
            int left = sv4_lsb_bit(a, (int)(offset + i));
            int right = sv4_lsb_bit(b, (int)(offset + i));
            // A comparison containing X/Z is not known true, even when
            // the raw state planes match. Do not substitute case equality.
            if (left >= 2 || right >= 2 || left != right) {
                equal = 0;
                break;
            }
        }
        sv4_copy_bits(&result, offset, equal ? a : element_default,
                      equal ? offset : 0, stride);
    }
    return result;
}

sv4_t sv4_record_conditional_merge(sv4_t a, sv4_t b, sv4_t period_default,
                                   sv4_t leaf_starts) {
    const uint32_t period = period_default.width;
    if (!a.width || a.width >= LLG_SUPPORTED_WIDTH_LIMIT || a.width != b.width ||
        !period || period > a.width || a.width % period != 0 ||
        leaf_starts.width != period || sv4_lsb_bit(leaf_starts, 0) != 1) {
        fputs("llg runtime fatal: invalid record conditional merge shape\n", stderr);
        abort();
    }
    sv4_t result = sv4_zero(a.width, 0);
    for (uint32_t base = 0; base < a.width; base += period) {
        for (uint32_t start = 0; start < period;) {
            uint32_t end = start + 1;
            while (end < period && sv4_lsb_bit(leaf_starts, (int)end) != 1) ++end;
            int equal = 1;
            for (uint32_t i = start; i < end; ++i) {
                int left = sv4_lsb_bit(a, (int)(base + i));
                int right = sv4_lsb_bit(b, (int)(base + i));
                // As for arrays: X/Z is never a known match.
                if (left >= 2 || right >= 2 || left != right) {
                    equal = 0;
                    break;
                }
            }
            sv4_copy_bits(&result, base + start, equal ? a : period_default,
                          equal ? base + start : start, end - start);
            start = end;
        }
    }
    return result;
}
