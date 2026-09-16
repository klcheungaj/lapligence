
uint64_t llg_time_precision_fs(void) { return g.design_precision_fs; }

uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs) {
    if (unit_fs == 0) {
        fprintf(stderr, "llg runtime fatal: zero time unit\n");
        abort();
    }
    // Portable 64x64 -> 128 multiplication using 32-bit halves.
    uint64_t a0 = (uint32_t)g.now, a1 = g.now >> 32;
    uint64_t b0 = (uint32_t)precision_fs, b1 = precision_fs >> 32;
    uint64_t p0 = a0 * b0;
    uint64_t t = a1 * b0 + (p0 >> 32);
    uint64_t middle = (uint32_t)t;
    uint64_t high = a1 * b1 + (t >> 32);
    t = a0 * b1 + middle;
    high += t >> 32;
    uint64_t low = (t << 32) | (uint32_t)p0;
    if (high >= unit_fs) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    uint64_t quotient = 0, remainder = high;
    for (unsigned i = 64; i > 0; --i) {
        int overflow = (remainder >> 63) != 0;
        remainder = (remainder << 1) | ((low >> (i - 1)) & 1u);
        quotient <<= 1;
        if (overflow || remainder >= unit_fs) {
            remainder -= unit_fs;
            quotient |= 1u;
        }
    }
    if (remainder >= unit_fs - remainder) {
        if (quotient == UINT64_MAX) {
            fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
            abort();
        }
        ++quotient;
    }
    return quotient;
}

static void llg_timeformat_error(const char* message, llg_string_t* suffix) {
    fprintf(stderr, "llg: $timeformat %s\n", message);
    if (suffix) llg_string_destroy(suffix);
    llg_last_failure = 1;
    g.finish = 1;
}

void llg_timeformat(sv4_t units, sv4_t precision, llg_string_t suffix,
                    sv4_t minimum_field_width) {
    if (!region_can_mutate("$timeformat state update")) {
        llg_string_destroy(&suffix);
        return;
    }
    if (sv4_is_unknown(units) || !sv4_fits_i64(units)) {
        llg_timeformat_error("units must be a known signed integer", &suffix);
        return;
    }
    if (sv4_is_unknown(precision) || !sv4_fits_i64(precision)) {
        llg_timeformat_error("precision must be a known signed integer", &suffix);
        return;
    }
    if (sv4_is_unknown(minimum_field_width) ||
        !sv4_fits_i64(minimum_field_width)) {
        llg_timeformat_error("minimum field width must be a known signed integer",
                             &suffix);
        return;
    }
    int64_t units_value = sv4_to_i64(units);
    int64_t precision_value = sv4_to_i64(precision);
    int64_t width_value = sv4_to_i64(minimum_field_width);
    uint64_t unit_fs = llg_time_unit_from_exponent(units_value);
    if (!unit_fs) {
        llg_timeformat_error("units must be between -15 and 0", &suffix);
        return;
    }
    if (precision_value < 0 ||
        precision_value > (int64_t)LLG_TIMEFORMAT_MAX_PRECISION) {
        llg_timeformat_error("precision is out of range for the formatter", &suffix);
        return;
    }
    if (width_value < 0 || width_value > (int64_t)(LLG_SUPPORTED_WIDTH_LIMIT * 2u + 256u)) {
        llg_timeformat_error("minimum field width is out of range", &suffix);
        return;
    }
    llg_string_move(&g.time_format.suffix, suffix);
    g.time_format.unit_fs = unit_fs;
    g.time_format.precision = (int)precision_value;
    g.time_format.minimum_field_width = (int)width_value;
}

int llg_rt_process_count(void) {
    int count = 0;
    for (int i = 0; i < g.n_procs; i++)
        if (g.all_procs[i]) count++;
    return count;
}
