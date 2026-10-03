// Typed combinational UDP rows; generated IR validates their shapes.
sv4_t sv4_udp_eval(const uint8_t* rows, size_t row_count, size_t input_count,
                   const sv4_t* const* inputs) {
    size_t stride = input_count + 1;
    for (size_t row = 0; row < row_count; ++row) {
        const uint8_t* fields = rows + row * stride;
        size_t input = 0;
        for (; input < input_count; ++input) {
            const sv4_t* value = inputs[input];
            unsigned state = ((value->x[0] | value->z[0]) & 1u) ? 2u
                             : (unsigned)(value->bits[0] & 1u);
            if (!(fields[input] & (1u << state))) break;
        }
        if (input == input_count) return sv4_fill(fields[input_count], 1, 0);
    }
    return sv4_fill(2, 1, 0);
}
