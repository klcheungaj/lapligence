// Fixed unpacked-array conditional merge. Private fragment of llg_value.c.
// Payload flattening must not change IEEE 1800-2009 11.4.11 element semantics.

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
        for (uint32_t i = 0; i < stride; ++i) {
            int digit = equal ? sv4_lsb_bit(a, (int)(offset + i))
                              : sv4_lsb_bit(element_default, (int)i);
            sv4_lsb_bit_set(&result, (int)(offset + i), digit);
        }
    }
    return result;
}
