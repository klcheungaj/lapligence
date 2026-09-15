
// ── $monitor / $strobe ────────────────────────────────────────────────────────

// Format `fmt` with `n` sv4_t arguments from `args`: the legacy packed path
// handles the original %d/%h/%b/%o/%t family, while typed display calls below
// additionally preserve real/string ownership and the SystemVerilog format
// conversions. `%%` prints '%', and an unknown or missing specifier prints
// verbatim without consuming an argument.
static void llg_format_array(char* out, size_t cap, const char* fmt,
                              const sv4_t* args, int n) {
    size_t len = 0;
    const char* p = fmt;
    int argi = 0;
    while (*p && len + 1 < cap) {
        char c = *p++;
        if (c == '%') {
            int has_width;
            int width;
            int zero;
            p = llg_parse_legacy_spec(p, &has_width, &width, &zero);
            c = *p;
            if (c) ++p;
            if (c == '%') {
                llg_append(out, cap, &len, '%');
            } else if ((c == 'd' || c == 'h' || c == 'b' || c == 'o' || c == 't') &&
                       argi < n) {
                char tmp[LLG_MAX_WIDTH * 2u + 256u];
                size_t tmp_len;
                if (c == 't') {
                    tmp_len = llg_format_time_integer(args[argi++],
                                                      g.design_precision_fs,
                                                      tmp, sizeof(tmp));
                    if (!has_width && !zero) width = g.time_format.minimum_field_width;
                } else {
                    sv4_format(c, args[argi++], tmp, sizeof(tmp));
                    tmp_len = strlen(tmp);
                }
                while (width > 0 && (size_t)width > tmp_len && len + 1 < cap) {
                    out[len++] = ' ';
                    width--;
                }
                for (size_t i = 0; i < tmp_len && len + 1 < cap; i++)
                    out[len++] = tmp[i];
            } else {
                out[len++] = '%';
                if (c && len + 1 < cap) out[len++] = c;
            }
        } else {
            out[len++] = c;
        }
    }
    out[len] = 0;
}

// Print one formatted line to stdout (shared by display/monitor/strobe).
static void llg_print_line(const char* line) {
    fputs(line, stdout);
    fputc('\n', stdout);
    fflush(stdout);
}

static void llg_print_array(const char* fmt, const sv4_t* args, int n) {
    size_t cap = strlen(fmt) + 1;
    for (int i = 0; i < n; ++i) {
        size_t extra = (size_t)args[i].width + 2u;
        if ((size_t)g.time_format.minimum_field_width > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.minimum_field_width;
        if ((size_t)g.time_format.precision > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.precision;
        if (g.time_format.suffix.len > SIZE_MAX - extra)
            llg_fatal_allocation("formatted line", 1, SIZE_MAX);
        extra += g.time_format.suffix.len;
        if (extra > SIZE_MAX - cap) llg_fatal_allocation("formatted line", cap, extra);
        cap += extra;
    }
    char* out = llg_checked_malloc(cap, 1, "formatted line");
    llg_format_array(out, cap, fmt, args, n);
    llg_print_line(out);
    free(out);
}

static void llg_fmt_args_destroy(llg_fmt_arg_t* args, int n) {
    if (!args) return;
    for (int i = 0; i < n; i++) {
        if (args[i].kind == LLG_FMT_STRING) llg_string_destroy(&args[i].value.string);
    }
}

static llg_fmt_arg_t llg_fmt_arg_clone(const llg_fmt_arg_t* value) {
    llg_fmt_arg_t result = *value;
    if (value->kind == LLG_FMT_STRING)
        result.value.string = llg_string_clone(&value->value.string);
    return result;
}

static void llg_append_text(char* out, size_t cap, size_t* len,
                            const char* text, size_t n) {
    for (size_t i = 0; i < n && *len + 1 < cap; i++) out[(*len)++] = text[i];
}

typedef struct {
    int left;
    int plus;
    int space;
    int alternate;
    int zero;
    int width;
    int has_width;
    int precision;
    int has_precision;
} llg_fmt_spec_t;

static const char* llg_parse_typed_spec(const char* start, const char* p,
                                         llg_fmt_spec_t* spec) {
    memset(spec, 0, sizeof(*spec));
    for (;;) {
        if (*p == '-') spec->left = 1;
        else if (*p == '+') spec->plus = 1;
        else if (*p == ' ') spec->space = 1;
        else if (*p == '#') spec->alternate = 1;
        else if (*p == '0') spec->zero = 1;
        else break;
        p++;
    }
    while (*p >= '0' && *p <= '9') {
        spec->has_width = 1;
        if (spec->width <= (INT_MAX - (*p - '0')) / 10)
            spec->width = spec->width * 10 + (*p - '0');
        p++;
    }
    if (*p == '.') {
        spec->has_precision = 1;
        p++;
        while (*p >= '0' && *p <= '9') {
            if (spec->precision <= (INT_MAX - (*p - '0')) / 10)
                spec->precision = spec->precision * 10 + (*p - '0');
            p++;
        }
    }
    (void)start;
    return p;
}

static int llg_decimal_increment(char* digits, size_t* length, size_t cap) {
    for (size_t i = *length; i > 0; --i) {
        if (digits[i - 1] != '9') {
            digits[i - 1]++;
            return 1;
        }
        digits[i - 1] = '0';
    }
    if (*length >= cap) return 0;
    memmove(digits + 1, digits, *length);
    digits[0] = '1';
    (*length)++;
    return 1;
}

static int llg_time_format_exponents(int* source, int* display) {
    int display_exponent = llg_time_unit_exponent(g.time_format.unit_fs);
    if (display_exponent == INT_MIN) return 0;
    int source_exponent = llg_time_unit_exponent(g.design_precision_fs);
    if (source_exponent == INT_MIN) source_exponent = display_exponent;
    *source = source_exponent;
    *display = display_exponent;
    return 1;
}

// Convert an integral time argument from its owning scope's unit to the
// design-wide `$timeformat` unit, rounding the discarded decimal digits half
// up.  The conversion operates on decimal digits so wide four-state values do
// not pass through a host integer or floating-point type.
static size_t llg_format_time_integer(sv4_t value, uint64_t source_unit_fs,
                                      char* raw, size_t cap) {
    char decimal[LLG_MAX_WIDTH * 2u + 256u];
    char scaled[LLG_MAX_WIDTH * 2u + 256u];
    sv4_to_dec_string(value, decimal, sizeof(decimal));
    size_t decimal_len = strlen(decimal);
    size_t len = 0;
    if (decimal_len == 0) return 0;
    if (decimal[0] == 'x') {
        llg_append_text(raw, cap, &len, decimal, decimal_len);
        llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                        g.time_format.suffix.len);
        return len;
    }
    int source_exponent;
    int display_exponent;
    if (!llg_time_format_exponents(&source_exponent, &display_exponent)) {
        source_exponent = display_exponent = 0;
    }
    int metadata_exponent = llg_time_unit_exponent(source_unit_fs);
    if (metadata_exponent != INT_MIN) source_exponent = metadata_exponent;
    int negative = decimal[0] == '-';
    const char* digits = decimal + (negative ? 1 : 0);
    size_t digits_len = decimal_len - (negative ? 1u : 0u);
    int scale = source_exponent - display_exponent + g.time_format.precision;
    size_t scaled_len = 0;
    if (scale >= 0) {
        // Multiplying zero by a power of ten must not manufacture trailing
        // zero digits; keeping its canonical representation also preserves
        // the expected `%0t` spelling at time zero.
        if (digits_len == 1 && digits[0] == '0') {
            scaled[0] = '0';
            scaled_len = 1;
        } else {
            size_t max_scaled = sizeof(scaled) - 1u;
            if (digits_len > max_scaled || (size_t)scale > max_scaled - digits_len)
                llg_fatal_allocation("formatted time", 1,
                                     digits_len + (size_t)scale + 1u);
            memcpy(scaled, digits, digits_len);
            scaled_len = digits_len;
            for (int i = 0; i < scale; ++i)
                scaled[scaled_len++] = '0';
        }
    } else {
        size_t drop = (size_t)(-scale);
        size_t keep = drop < digits_len ? digits_len - drop : 0;
        if (keep) memcpy(scaled, digits, keep);
        scaled_len = keep;
        if (scaled_len == 0) scaled[scaled_len++] = '0';
        // If the value has fewer digits than the discarded scale, the
        // leading discarded decimal digits are zero (for example 7/10^9),
        // so inspect the first actually discarded digit only when it exists.
        int round_up = drop <= digits_len && digits[digits_len - drop] >= '5';
        if (round_up) (void)llg_decimal_increment(scaled, &scaled_len, sizeof(scaled));
    }
    int precision = g.time_format.precision;
    if (negative) llg_append(raw, cap, &len, '-');
    if (scaled_len > (size_t)precision) {
        size_t integer_len = scaled_len - (size_t)precision;
        llg_append_text(raw, cap, &len, scaled, integer_len);
        if (precision) {
            llg_append(raw, cap, &len, '.');
            llg_append_text(raw, cap, &len, scaled + integer_len,
                            (size_t)precision);
        }
    } else {
        llg_append(raw, cap, &len, '0');
        if (precision) {
            llg_append(raw, cap, &len, '.');
            for (size_t i = scaled_len; i < (size_t)precision; ++i)
                llg_append(raw, cap, &len, '0');
            llg_append_text(raw, cap, &len, scaled, scaled_len);
        }
    }
    llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                    g.time_format.suffix.len);
    return len;
}

static size_t llg_format_time_real(double value, uint64_t source_unit_fs,
                                   char* raw, size_t cap) {
    int source_exponent;
    int display_exponent;
    if (!llg_time_format_exponents(&source_exponent, &display_exponent)) {
        source_exponent = display_exponent = 0;
    }
    int metadata_exponent = llg_time_unit_exponent(source_unit_fs);
    if (metadata_exponent != INT_MIN) source_exponent = metadata_exponent;
    double scale = (double)llg_time_unit_from_exponent(source_exponent) /
                   (double)llg_time_unit_from_exponent(display_exponent);
    // C's printf family is allowed to honor the process rounding mode.  The
    // simulator's time formatter instead uses the standard decimal rule for
    // discarded digits (exact halves away from zero), so round in the scaled
    // decimal domain before asking snprintf only to render the fixed digits.
    double scaled = value * scale;
    double factor = 1.0;
    for (int i = 0; i < g.time_format.precision && isfinite(factor); ++i)
        factor *= 10.0;
    if (isfinite(scaled)) {
        double rounded = round(scaled * factor);
        if (isfinite(rounded)) scaled = rounded / factor;
    }
    int written = snprintf(raw, cap, "%.*f", g.time_format.precision,
                           scaled);
    size_t len = written < 0 ? 0 : (size_t)written;
    if (len >= cap) len = cap ? cap - 1 : 0;
    llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                    g.time_format.suffix.len);
    return len;
}

static size_t llg_format_raw2(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (value.width + 63u) / 64u;
    uint32_t last_bits = value.width % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        // SFormat::formatRaw2 flattens X/Z to zero and emits the native
        // little-endian limb bytes, including the complete last 32-bit half
        // for values whose width is between 33 and 64 bits.
        uint64_t bits = value.bits[i] & ~(value.x[i] | value.z[i]);
        size_t bytes = (i == words - 1 && last_bits <= 32) ? sizeof(uint32_t)
                                                            : sizeof(uint64_t);
        for (size_t j = 0; j < bytes && len < cap; j++)
            raw[len++] = (char)(bits >> (j * 8));
    }
    return len;
}

static size_t llg_format_raw4(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (value.width + 63u) / 64u;
    uint32_t last_bits = value.width % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        uint64_t unknown = value.x[i] | value.z[i];
        uint64_t bits = value.bits[i];
        size_t halves = (i == words - 1 && last_bits <= 32) ? 1u : 2u;
        for (size_t half = 0; half < halves; half++) {
            // VPI's four-state encoding uses aval = known bits XOR unknown
            // and bval = unknown, matching Slang's formatRaw4 helper.
            uint32_t aval = (uint32_t)((bits ^ unknown) >> (half * 32));
            uint32_t bval = (uint32_t)(unknown >> (half * 32));
            if (len + sizeof(aval) + sizeof(bval) > cap) {
                size_t remaining = cap - len;
                if (remaining) {
                    size_t aval_bytes = remaining < sizeof(aval) ? remaining : sizeof(aval);
                    memcpy(raw + len, &aval, aval_bytes);
                    len += aval_bytes;
                    remaining -= aval_bytes;
                    if (remaining) {
                        size_t bval_bytes = remaining < sizeof(bval) ? remaining : sizeof(bval);
                        memcpy(raw + len, &bval, bval_bytes);
                        len += bval_bytes;
                    }
                }
                return len;
            }
            memcpy(raw + len, &aval, sizeof(aval));
            len += sizeof(aval);
            memcpy(raw + len, &bval, sizeof(bval));
            len += sizeof(bval);
        }
    }
    return len;
}

static size_t llg_format_strength(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    for (uint32_t bit = value.width; bit > 0; bit--) {
        uint32_t index = bit - 1;
        uint64_t mask = UINT64_C(1) << (index % 64u);
        uint32_t limb = index / 64u;
        const char* text;
        if (value.x[limb] & mask)
            text = "StX";
        else if (value.z[limb] & mask)
            text = "HiZ";
        else
            text = value.bits[limb] & mask ? "St1" : "St0";
        llg_append_text(raw, cap, &len, text, strlen(text));
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

static size_t llg_format_char(sv4_t value, char* raw, size_t cap) {
    if (cap == 0 || value.width == 0) return 0;
    uint64_t unknown = value.x[0] | value.z[0];
    raw[0] = (char)(unknown & 0xffu ? 0xffu : value.bits[0] & 0xffu);
    return 1;
}

static sv4_t llg_string_to_display_packed(const llg_string_t* value) {
    size_t max_bytes = (size_t)LLG_MAX_WIDTH / 8u;
    if (value->len > max_bytes || (value->len == 0 && LLG_MAX_WIDTH < 8u)) {
        fprintf(stderr,
                "llg runtime fatal: string display conversion exceeds packed width\n");
        abort();
    }
    uint32_t width = value->len ? (uint32_t)(value->len * 8u) : 8u;
    return llg_string_to_packed(llg_string_clone(value), width, 0);
}

static size_t llg_format_pattern_packed(sv4_t value, char* raw, size_t cap) {
    char digits[LLG_MAX_WIDTH * 2u + 256u];
    int has_unknown = sv4_is_unknown(value);
    int all_x = has_unknown;
    int all_z = has_unknown;
    for (int i = 0; i < llg_sv4_nlimbs(value.width); i++) {
        uint64_t mask = llg_sv4_limb_mask(value.width, i);
        all_x &= (value.x[i] & mask) == mask;
        all_z &= (value.z[i] & mask) == mask;
    }
    int base;
    if ((value.width < 8u && !value.is_signed) ||
        (has_unknown && value.width <= 64u && !all_x && !all_z)) {
        base = 'b';
    } else if (value.width <= 32u || value.is_signed || all_x || all_z) {
        base = 'd';
    } else {
        base = 'h';
    }
    sv4_format((char)base, value, digits, sizeof(digits));
    size_t digits_len = strlen(digits);
    size_t len = 0;
    const char* digit_text = digits;
    int include_base = !(base == 'd' && value.width == 32u && value.is_signed && !has_unknown);
    if (digits_len && digits[0] == '-') {
        llg_append(raw, cap, &len, '-');
        digit_text++;
        digits_len--;
    }
    if (include_base) {
        char prefix[64];
        int written = snprintf(prefix, sizeof(prefix), "%u'%s%c", value.width,
                               value.is_signed ? "s" : "", base);
        if (written > 0) llg_append_text(raw, cap, &len, prefix, (size_t)written);
    }
    llg_append_text(raw, cap, &len, digit_text, digits_len);
    return len;
}

static void llg_emit_field(char* out, size_t cap, size_t* len,
                            const char* value, size_t value_len,
                            llg_fmt_spec_t spec, char conversion) {
    char field[LLG_MAX_WIDTH * 2u + 256u];
    size_t n = value_len;
    if (n > sizeof(field) - 1) n = sizeof(field) - 1;
    memcpy(field, value, n);
    field[n] = 0;
    if (spec.has_precision && conversion == 's' && n > (size_t)spec.precision)
        n = (size_t)spec.precision;
    if (spec.has_precision && strchr("dhbox", conversion)) {
        size_t sign = n && field[0] == '-' ? 1u : 0u;
        size_t digits = n - sign;
        while (digits < (size_t)spec.precision && n + 1 < sizeof(field)) {
            memmove(field + sign + 1, field + sign, digits + 1);
            field[sign] = '0';
            n++;
            digits++;
        }
    }
    if (spec.alternate && strchr("hbo", conversion) && n > 0 &&
        !(n == 1 && (field[0] == 'x' || field[0] == 'z'))) {
        const char* prefix = conversion == 'h' ? "0x" : conversion == 'o' ? "0" : "0b";
        size_t prefix_len = strlen(prefix);
        if (n + prefix_len < sizeof(field)) {
            memmove(field + prefix_len, field, n + 1);
            memcpy(field, prefix, prefix_len);
            n += prefix_len;
        }
    }
    int numeric = strchr("dhbotxfeg", conversion) != NULL;
    if (numeric && n > 0 && field[0] != '-' && spec.plus) {
        if (n + 1 < sizeof(field)) {
            memmove(field + 1, field, n + 1);
            field[0] = '+';
            n++;
        }
    } else if (numeric && n > 0 && field[0] != '-' && spec.space) {
        if (n + 1 < sizeof(field)) {
            memmove(field + 1, field, n + 1);
            field[0] = ' ';
            n++;
        }
    }
    size_t pad = spec.width > 0 && (size_t)spec.width > n
                   ? (size_t)spec.width - n : 0;
    // Slang's integral formatter pads non-decimal bases with zeroes whenever
    // a width is present; decimal and textual values use spaces.  `%0d`
    // without a width still gets the natural decimal width and no padding.
    char pad_char = ' ';
    if (strchr("hbox", conversion) != NULL) pad_char = '0';
    // The zero flag is meaningful for host floating-point formatting.  The
    // SystemVerilog integer parser consumes it as syntax but formatInt still
    // uses decimal spaces (and non-decimal bases already use zeroes by
    // virtue of their width rule).
    if (!spec.left && spec.zero && strchr("feg", conversion) != NULL) {
        size_t prefix = (n && (field[0] == '-' || field[0] == '+' || field[0] == ' ')) ? 1u : 0u;
        if (prefix && pad) {
            llg_append_text(out, cap, len, field, prefix);
            for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, '0');
            llg_append_text(out, cap, len, field + prefix, n - prefix);
            return;
        }
    }
    if (!spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
    llg_append_text(out, cap, len, field, n);
    if (spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
}

static size_t llg_format_typed(char* out, size_t cap, const char* fmt,
                               const llg_fmt_arg_t* args, int n,
                               const char* scope) {
    size_t len = 0;
    int argi = 0;
    const char* p = fmt;
    while (*p && len + 1 < cap) {
        if (*p != '%') {
            llg_append(out, cap, &len, *p++);
            continue;
        }
        const char* start = p++;
        llg_fmt_spec_t spec;
        p = llg_parse_typed_spec(start, p, &spec);
        char source_conversion = *p ? *p++ : 0;
        char conversion = source_conversion;
        if (conversion >= 'A' && conversion <= 'Z') conversion = (char)(conversion - 'A' + 'a');
        if (conversion == '%') {
            llg_append(out, cap, &len, '%');
            continue;
        }
        if (conversion == 'm') {
            const char* text = scope ? scope : "";
            llg_emit_field(out, cap, &len, text, strlen(text), spec, 's');
            continue;
        }
        if (conversion == 'l') {
            // `%l` has no width-bearing form in the Slang grammar, so append
            // the library-qualified HDL scope directly and avoid allocating a
            // model-width temporary on the coroutine stack.
            llg_append_text(out, cap, &len, "work.", 5);
            if (scope && scope[0])
                llg_append_text(out, cap, &len, scope, strlen(scope));
            else
                llg_append_text(out, cap, &len, "$unit", 5);
            continue;
        }
        if (!conversion || argi >= n) {
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        const llg_fmt_arg_t* arg = &args[argi++];
        char raw[LLG_MAX_WIDTH * 2u + 256u];
        size_t raw_len = 0;
        if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_PACKED) {
            sv4_format(conversion == 'x' ? 'h' : conversion, arg->value.packed,
                       raw, sizeof(raw));
            raw_len = strlen(raw);
        } else if (strchr("dhbox", conversion) && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            sv4_format(conversion == 'x' ? 'h' : conversion, packed, raw, sizeof(raw));
            raw_len = strlen(raw);
        } else if (conversion == 't' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_time_integer(arg->value.packed,
                                              arg->time_unit_fs, raw, sizeof(raw));
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 't' && arg->kind == LLG_FMT_REAL) {
            raw_len = llg_format_time_real(arg->value.real, arg->time_unit_fs,
                                           raw, sizeof(raw));
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 'c' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_char(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'c' && arg->kind == LLG_FMT_STRING) {
            sv4_t packed = llg_string_to_display_packed(&arg->value.string);
            raw_len = llg_format_char(packed, raw, sizeof(raw));
        } else if (conversion == 'u' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw2(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'z' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw4(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'v' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_strength(arg->value.packed, raw, sizeof(raw));
        } else if (conversion == 'p' && arg->kind == LLG_FMT_PACKED) {
            // Aggregate pattern formatting is rejected by lowering until the
            // owned aggregate representation is available.  A packed scalar
            // follows ConstantValue::toString's base-selection and literal
            // prefix rules, which is the scalar case of Slang's pattern
            // visitor.
            raw_len = llg_format_pattern_packed(arg->value.packed, raw, sizeof(raw));
        } else if (strchr("feg", conversion) && arg->kind == LLG_FMT_REAL) {
            char real_fmt[128];
            size_t spec_len = (size_t)(p - start);
            if (spec_len >= sizeof(real_fmt) - 1) spec_len = sizeof(real_fmt) - 2;
            memcpy(real_fmt, start, spec_len);
            real_fmt[spec_len] = 0;
            int written = snprintf(raw, sizeof(raw), real_fmt, arg->value.real);
            raw_len = written < 0 ? 0 : (size_t)written < sizeof(raw)
                                           ? (size_t)written
                                           : sizeof(raw) - 1;
        } else if (conversion == 's' && arg->kind == LLG_FMT_PACKED) {
            llg_string_t value = llg_string_from_packed(arg->value.packed);
            raw_len = value.len;
            if (raw_len > sizeof(raw)) raw_len = sizeof(raw);
            if (raw_len) memcpy(raw, value.data, raw_len);
            llg_string_destroy(&value);
        } else if (conversion == 's' && arg->kind == LLG_FMT_STRING) {
            raw_len = arg->value.string.len;
            if (raw_len > sizeof(raw)) raw_len = sizeof(raw);
            if (raw_len) memcpy(raw, arg->value.string.data, raw_len);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_STRING) {
            // Keep a string pattern visibly distinct from `%s`, matching the
            // quote-delimited form produced by Slang's pattern formatter.
            size_t value_len = arg->value.string.len;
            if (value_len + 2u <= sizeof(raw)) {
                raw[0] = '"';
                if (value_len) memcpy(raw + 1, arg->value.string.data, value_len);
                raw[value_len + 1] = '"';
                raw_len = value_len + 2u;
            } else {
                raw[0] = '"';
                raw_len = sizeof(raw);
                if (raw_len > 1) {
                    size_t copy = raw_len - 2u;
                    memcpy(raw + 1, arg->value.string.data, copy);
                    raw[raw_len - 1] = '"';
                }
            }
        } else {
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        llg_emit_field(out, cap, &len, raw, raw_len, spec, conversion);
    }
    out[len] = 0;
    return len;
}

static void llg_print_typed_to(uint32_t descriptor, const char* fmt,
                               llg_fmt_arg_t* args, int n, const char* scope,
                               int newline);

static void llg_print_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                            const char* scope, int newline) {
    llg_print_typed_to(1u, fmt, args, n, scope, newline);
}
