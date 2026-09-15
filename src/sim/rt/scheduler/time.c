
uint64_t llg_time_precision_fs(void) { return g.design_precision_fs; }

uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs) {
    if (unit_fs == 0) {
        fprintf(stderr, "llg runtime fatal: zero time unit\n");
        abort();
    }
#if defined(__SIZEOF_INT128__)
    __uint128_t physical = (__uint128_t)g.now * precision_fs;
    __uint128_t scaled = physical / unit_fs;
    __uint128_t remainder = physical % unit_fs;
    if (remainder >= (__uint128_t)unit_fs - remainder) {
        scaled++;
    }
    if (scaled > UINT64_MAX) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    return (uint64_t)scaled;
#else
    if (precision_fs != 0 && g.now > UINT64_MAX / precision_fs) {
        fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
        abort();
    }
    uint64_t physical = g.now * precision_fs;
    uint64_t scaled = physical / unit_fs;
    uint64_t remainder = physical % unit_fs;
    if (remainder >= unit_fs - remainder) {
        if (scaled == UINT64_MAX) {
            fprintf(stderr, "llg runtime fatal: scaled simulation time overflow\n");
            abort();
        }
        scaled++;
    }
    return scaled;
#endif
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
    if (width_value < 0 || width_value > (int64_t)(LLG_MAX_WIDTH * 2u + 256u)) {
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
