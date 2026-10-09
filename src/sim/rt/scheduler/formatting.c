// Checked size arithmetic for per-conversion formatting scratch.
static size_t llg_format_size_add(size_t a, size_t b) {
    if (b > SIZE_MAX - a) llg_fatal_allocation("format scratch", a, b);
    return a + b;
}

static size_t llg_format_scratch_size(size_t payload, size_t precision) {
    size_t cap = llg_format_size_add(payload, precision);
    cap = llg_format_size_add(cap, (size_t)g.time_format.precision);
    cap = llg_format_size_add(cap, g.time_format.suffix.len);
    return llg_format_size_add(cap, 512u);
}


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
                size_t tmp_cap = llg_format_scratch_size(llg_sv4_width(args[argi]), 0);
                char* tmp = llg_checked_malloc(tmp_cap, 1, "packed format");
                size_t tmp_len;
                if (c == 't') {
                    tmp_len = llg_format_time_integer(args[argi++],
                                                      g.design_precision_fs,
                                                      tmp, tmp_cap);
                    if (!has_width && !zero) width = g.time_format.minimum_field_width;
                } else {
                    sv4_format(c, args[argi++], tmp, tmp_cap);
                    tmp_len = strlen(tmp);
                }
                while (width > 0 && (size_t)width > tmp_len && len + 1 < cap) {
                    out[len++] = ' ';
                    width--;
                }
                for (size_t i = 0; i < tmp_len && len + 1 < cap; i++)
                    out[len++] = tmp[i];
                free(tmp);
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
        size_t extra = (size_t)llg_sv4_width(args[i]) + 2u;
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
        if (args[i].kind == LLG_FMT_STRING || args[i].kind == LLG_FMT_TEXT)
            llg_string_destroy(&args[i].value.string);
        else if (args[i].kind == LLG_FMT_PACKED || args[i].kind == LLG_FMT_STRENGTH)
            sv4_destroy(&args[i].value.packed);
        memset(&args[i], 0, sizeof(args[i]));
    }
}

static llg_fmt_arg_t llg_fmt_arg_clone(const llg_fmt_arg_t* value) {
    llg_fmt_arg_t result = *value;
    if (value->kind == LLG_FMT_STRING || value->kind == LLG_FMT_TEXT)
        result.value.string = llg_string_clone(&value->value.string);
    else if (value->kind == LLG_FMT_PACKED || value->kind == LLG_FMT_STRENGTH)
        result.value.packed = sv4_clone(&value->value.packed);
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
    size_t decimal_cap = (size_t)llg_sv4_width(value) + 3u;
    size_t scaled_cap = llg_format_scratch_size(llg_sv4_width(value), 0);
    char* decimal = llg_checked_malloc(decimal_cap, 1, "time digits");
    char* scaled = llg_checked_malloc(scaled_cap, 1, "scaled time digits");
    sv4_to_dec_string(value, decimal, decimal_cap);
    size_t decimal_len = strlen(decimal);
    size_t len = 0;
    int source_exponent;
    int display_exponent;
    int metadata_exponent;
    int negative;
    const char* digits;
    size_t digits_len;
    int scale;
    size_t scaled_len;
    int precision;
    if (decimal_len == 0) goto cleanup;
    if (strchr("xXzZ", decimal[0])) {
        llg_append_text(raw, cap, &len, decimal, decimal_len);
        llg_append_text(raw, cap, &len, g.time_format.suffix.data,
                        g.time_format.suffix.len);
        goto cleanup;
    }
    if (!llg_time_format_exponents(&source_exponent, &display_exponent)) {
        source_exponent = display_exponent = 0;
    }
    metadata_exponent = llg_time_unit_exponent(source_unit_fs);
    if (metadata_exponent != INT_MIN) source_exponent = metadata_exponent;
    negative = decimal[0] == '-';
    digits = decimal + (negative ? 1 : 0);
    digits_len = decimal_len - (negative ? 1u : 0u);
    scale = source_exponent - display_exponent + g.time_format.precision;
    scaled_len = 0;
    if (scale >= 0) {
        // Multiplying zero by a power of ten must not manufacture trailing
        // zero digits; keeping its canonical representation also preserves
        // the expected `%0t` spelling at time zero.
        if (digits_len == 1 && digits[0] == '0') {
            scaled[0] = '0';
            scaled_len = 1;
        } else {
            size_t max_scaled = scaled_cap - 1u;
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
        if (round_up) (void)llg_decimal_increment(scaled, &scaled_len, scaled_cap);
    }
    precision = g.time_format.precision;
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
cleanup:
    free(scaled);
    free(decimal);
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
    uint32_t words = (llg_sv4_width(value) + 63u) / 64u;
    uint32_t last_bits = llg_sv4_width(value) % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        // SFormat::formatRaw2 flattens X/Z to zero and emits the native
        // little-endian limb bytes, including the complete last 32-bit half
        // for values whose width is between 33 and 64 bits.
        uint64_t bits = llg_sv4_word(value, i, LLG_SV4_BITS) & ~(llg_sv4_word(value, i, LLG_SV4_X) | llg_sv4_word(value, i, LLG_SV4_Z));
        size_t bytes = (i == words - 1 && last_bits <= 32) ? sizeof(uint32_t)
                                                            : sizeof(uint64_t);
        for (size_t j = 0; j < bytes && len < cap; j++)
            raw[len++] = (char)(bits >> (j * 8));
    }
    return len;
}

static size_t llg_format_raw4(sv4_t value, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t words = (llg_sv4_width(value) + 63u) / 64u;
    uint32_t last_bits = llg_sv4_width(value) % 64u;
    if (last_bits == 0) last_bits = 64;
    for (uint32_t i = 0; i < words; i++) {
        uint64_t unknown = llg_sv4_word(value, i, LLG_SV4_X) | llg_sv4_word(value, i, LLG_SV4_Z);
        uint64_t bits = llg_sv4_word(value, i, LLG_SV4_BITS);
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
    for (uint32_t bit = llg_sv4_width(value); bit > 0; bit--) {
        uint32_t index = bit - 1;
        uint64_t mask = UINT64_C(1) << (index % 64u);
        uint32_t limb = index / 64u;
        const char* text;
        if (llg_sv4_word(value, limb, LLG_SV4_X) & mask)
            text = "StX";
        else if (llg_sv4_word(value, limb, LLG_SV4_Z) & mask)
            text = "HiZ";
        else
            text = llg_sv4_word(value, limb, LLG_SV4_BITS) & mask ? "St1" : "St0";
        llg_append_text(raw, cap, &len, text, strlen(text));
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

// Format a net strength view (llg_net_t.strength) with IEEE 1364-2001
// 17.1.1.5 / Tables 69-71: a mnemonic for one level, otherwise two digits
// (max then min strength for 0/1; strength0 then strength1 for X). L and H
// always use the mnemonic of their driven level.
static void llg_format_strength_byte(uint8_t code, char text[4]) {
    static const char* const names[8] = {"Hi", "Sm", "Me", "We", "La", "Pu", "St", "Su"};
    int lo = (int)(code & 0x0fu) - 7;
    int hi = (int)(code >> 4) - 7;
    if (lo < -7 || hi > 7 || lo > hi) {
        lo = -LLG_STRENGTH_STRONG;
        hi = LLG_STRENGTH_STRONG;
    }
    char value;
    int first;
    int second;
    if (hi < 0) {
        value = '0';
        first = -lo;
        second = -hi;
    } else if (lo > 0) {
        value = '1';
        first = hi;
        second = lo;
    } else if (lo == 0 && hi == 0) {
        memcpy(text, "HiZ", 4);
        return;
    } else if (hi == 0) {
        value = 'L';
        first = second = -lo;
    } else if (lo == 0) {
        value = 'H';
        first = second = hi;
    } else {
        value = 'X';
        first = -lo;
        second = hi;
    }
    if (first == second) {
        memcpy(text, names[first], 2);
    } else {
        text[0] = (char)('0' + first);
        text[1] = (char)('0' + second);
    }
    text[2] = value;
    text[3] = 0;
}

static size_t llg_format_strength_view(sv4_t view, char* raw, size_t cap) {
    size_t len = 0;
    uint32_t bits = llg_sv4_width(view) / 8u;
    for (uint32_t bit = bits; bit > 0; bit--) {
        uint32_t index = (bit - 1u) * 8u;
        uint64_t word = llg_sv4_word(view, index / 64u, LLG_SV4_BITS);
        char text[4];
        llg_format_strength_byte((uint8_t)(word >> (index % 64u)), text);
        llg_append_text(raw, cap, &len, text, 3);
        if (bit != 1) llg_append(raw, cap, &len, ' ');
    }
    return len;
}

static size_t llg_format_char(sv4_t value, char* raw, size_t cap) {
    if (cap == 0 || llg_sv4_width(value) == 0) return 0;
    uint64_t unknown = llg_sv4_word(value, 0, LLG_SV4_X) | llg_sv4_word(value, 0, LLG_SV4_Z);
    raw[0] = (char)(unknown & 0xffu ? 0xffu : llg_sv4_word(value, 0, LLG_SV4_BITS) & 0xffu);
    return 1;
}

static sv4_t llg_string_to_display_packed(const llg_string_t* value) {
    size_t max_bytes = (size_t)(LLG_SUPPORTED_WIDTH_LIMIT - 1u) / 8u;
    if (value->len > max_bytes) {
        fprintf(stderr,
                "llg runtime fatal: string display conversion exceeds packed width\n");
        abort();
    }
    uint32_t width = value->len ? (uint32_t)(value->len * 8u) : 8u;
    return llg_string_to_packed(llg_string_clone(value), width, 0);
}

static size_t llg_format_integral(char conversion, sv4_t value,
                                  const llg_fmt_spec_t* spec, char* raw,
                                  size_t cap);

// `%p` of an integral singular value prints it "as it would unformatted"
// (SV 21.2.1.7): the default decimal conversion with its X/Z digit rules
// (21.2.1.3, 21.2.1.4). Pattern white space is implementation dependent, so
// the automatic field's leading spaces are dropped.
static size_t llg_format_pattern_packed(sv4_t value, char* raw, size_t cap) {
    llg_fmt_spec_t spec;
    memset(&spec, 0, sizeof(spec));
    return llg_format_integral('d', value, &spec, raw, cap);
}

// Frame scratch for one conversion's text; larger results allocate.
#define LLG_FORMAT_INLINE_SCRATCH 1024u
#define LLG_FORMAT_INLINE_FIELD 256u

// Slang SFormat::formatInt: an unsized decimal field holds the largest value
// of `width` bits, ceil(width / log2(10)) digits, plus one for a signed sign.
static int llg_decimal_field_width(uint32_t width, int is_signed) {
    if (width == 0) return 0;
    int digits = (int)ceil((double)width / 3.32192809488736234787);
    return digits + (is_signed ? 1 : 0);
}

// The packed operand of an integral conversion: the argument itself, or an
// owner in `converted` for a string (8 bits per byte) or a real (rounded to a
// signed 64-bit value, as Slang's evaluator converts it).
static const sv4_t* llg_fmt_integral_operand(const llg_fmt_arg_t* arg,
                                             sv4_t* converted) {
    if (arg->kind == LLG_FMT_STRING) {
        *converted = llg_string_to_display_packed(&arg->value.string);
        return converted;
    }
    if (arg->kind == LLG_FMT_REAL) {
        *converted = sv4_from_real(arg->value.real, 64u, 1);
        return converted;
    }
    return &arg->value.packed;
}

// Digits of an integral conversion. Non-decimal results keep every digit of
// the value's width (automatic size); an explicit width, including `%0`,
// first drops leading zero digits and the field is then padded back to the
// width with zeroes (SV 21.2.1.3).
// Digits of a known value of at most 64 bits, written without the value
// backend's general (allocating) formatter. Returns 0 when not applicable.
static size_t llg_format_word(char conversion, sv4_t value, char* raw,
                              size_t cap) {
    uint32_t width = llg_sv4_width(value);
    if (width == 0 || width > 64u || cap < 72u) return 0;
    uint64_t mask = width == 64u ? UINT64_MAX : (UINT64_C(1) << width) - 1u;
    if ((llg_sv4_word(value, 0, LLG_SV4_X) | llg_sv4_word(value, 0, LLG_SV4_Z)) & mask)
        return 0;
    uint64_t bits = llg_sv4_word(value, 0, LLG_SV4_BITS) & mask;
    int written;
    switch (conversion) {
    case 'd':
        if (llg_sv4_signed(value) && ((bits >> (width - 1u)) & 1u))
            written = snprintf(raw, cap, "-%llu",
                               (unsigned long long)((~bits + 1u) & mask));
        else
            written = snprintf(raw, cap, "%llu", (unsigned long long)bits);
        break;
    case 'h':
        written = snprintf(raw, cap, "%0*llx", (int)((width + 3u) / 4u),
                           (unsigned long long)bits);
        break;
    case 'o':
        written = snprintf(raw, cap, "%0*llo", (int)((width + 2u) / 3u),
                           (unsigned long long)bits);
        break;
    case 'b':
        for (uint32_t bit = width; bit > 0; --bit)
            raw[width - bit] = (char)('0' + ((bits >> (bit - 1u)) & 1u));
        raw[width] = 0;
        written = (int)width;
        break;
    default:
        return 0;
    }
    return written > 0 ? (size_t)written : 0;
}

static size_t llg_format_integral(char conversion, sv4_t value,
                                  const llg_fmt_spec_t* spec, char* raw,
                                  size_t cap) {
    size_t len = llg_format_word(conversion, value, raw, cap);
    if (!len) {
        sv4_format(conversion, value, raw, cap);
        len = strlen(raw);
    }
    if (conversion != 'd' && (spec->has_width || spec->zero)) {
        size_t skip = 0;
        while (skip + 1 < len && raw[skip] == '0') skip++;
        if (skip) {
            memmove(raw, raw + skip, len - skip + 1);
            len -= skip;
        }
    }
    return len;
}

// SV string literal text of a string value: printable bytes verbatim, `"`
// and `\` escaped, and every other byte as a three-digit octal escape, so
// the pattern stays a legal literal (SV 5.9, 21.2.1.7).
static size_t llg_format_pattern_string(const char* data, size_t length,
                                        char* raw, size_t cap) {
    size_t len = 0;
    llg_append(raw, cap, &len, '"');
    for (size_t i = 0; i < length; ++i) {
        unsigned char byte = (unsigned char)data[i];
        if (byte == '"' || byte == '\\') {
            llg_append(raw, cap, &len, '\\');
            llg_append(raw, cap, &len, (char)byte);
        } else if (byte == '\n') {
            llg_append_text(raw, cap, &len, "\\n", 2);
        } else if (byte == '\t') {
            llg_append_text(raw, cap, &len, "\\t", 2);
        } else if (byte >= 0x20u && byte < 0x7fu) {
            llg_append(raw, cap, &len, (char)byte);
        } else {
            char escape[5] = {'\\', (char)('0' + (byte >> 6)),
                              (char)('0' + ((byte >> 3) & 7u)),
                              (char)('0' + (byte & 7u)), 0};
            llg_append_text(raw, cap, &len, escape, 4);
        }
    }
    llg_append(raw, cap, &len, '"');
    raw[len < cap ? len : cap - 1] = 0;
    return len;
}

// `%p` of a real prints it as an unformatted real argument displays: the
// default `%f` conversion (SV 21.2.1.7).
static size_t llg_format_pattern_real(double value, char* raw, size_t cap) {
    int written = snprintf(raw, cap, "%f", value);
    if (written < 0) return 0;
    return (size_t)written < cap ? (size_t)written : cap - 1;
}

static void llg_emit_field(char* out, size_t cap, size_t* len,
                            const char* value, size_t value_len,
                            llg_fmt_spec_t spec, char conversion) {
    size_t field_cap = llg_format_size_add(value_len, (size_t)spec.precision);
    field_cap = llg_format_size_add(field_cap, 8u);
    char inline_field[LLG_FORMAT_INLINE_FIELD];
    char* field = field_cap <= sizeof(inline_field)
                      ? inline_field
                      : llg_checked_malloc(field_cap, 1, "formatted field");
    size_t n = value_len;
    if (n > field_cap - 1) n = field_cap - 1;
    memcpy(field, value, n);
    field[n] = 0;
    if (spec.has_precision && conversion == 's' && n > (size_t)spec.precision)
        n = (size_t)spec.precision;
    if (spec.has_precision && strchr("dhbox", conversion)) {
        size_t sign = n && field[0] == '-' ? 1u : 0u;
        size_t digits = n - sign;
        while (digits < (size_t)spec.precision && n + 1 < field_cap) {
            memmove(field + sign + 1, field + sign, digits + 1);
            field[sign] = '0';
            n++;
            digits++;
        }
    }
    if (spec.alternate && strchr("hbo", conversion) && n > 0 &&
        !(n == 1 && strchr("xXzZ", field[0]))) {
        const char* prefix = conversion == 'h' ? "0x" : conversion == 'o' ? "0" : "0b";
        size_t prefix_len = strlen(prefix);
        if (n + prefix_len < field_cap) {
            memmove(field + prefix_len, field, n + 1);
            memcpy(field, prefix, prefix_len);
            n += prefix_len;
        }
    }
    int numeric = strchr("dhbotxfeg", conversion) != NULL;
    if (numeric && n > 0 && field[0] != '-' && spec.plus) {
        if (n + 1 < field_cap) {
            memmove(field + 1, field, n + 1);
            field[0] = '+';
            n++;
        }
    } else if (numeric && n > 0 && field[0] != '-' && spec.space) {
        if (n + 1 < field_cap) {
            memmove(field + 1, field, n + 1);
            field[0] = ' ';
            n++;
        }
    }
    size_t pad = spec.width > 0 && (size_t)spec.width > n
                   ? (size_t)spec.width - n : 0;
    // Non-decimal bases pad with zeroes and decimal and textual values with
    // spaces (SV 21.2.1.3). A `%0` field has width zero and no padding.
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
            if (field != inline_field) free(field);
            return;
        }
    }
    if (!spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, pad_char);
    llg_append_text(out, cap, len, field, n);
    // Trailing zeroes would read as more digits, so a left-justified field
    // is always completed with spaces.
    if (spec.left) for (size_t i = 0; i < pad; i++) llg_append(out, cap, len, ' ');
    if (field != inline_field) free(field);

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
            // Only run-time format strings reach here: literal formats are
            // checked against their arguments during lowering. The
            // specification is printed as written and reported once.
            static int reported_missing_argument;
            if (conversion && !reported_missing_argument) {
                reported_missing_argument = 1;
                fprintf(stderr,
                        "llg: warning: format specification `%.*s` has no "
                        "argument; printed as written\n",
                        (int)(p - start), start);
            }
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        const llg_fmt_arg_t* arg = &args[argi++];
        size_t payload = 0;
        if (arg->kind == LLG_FMT_PACKED || arg->kind == LLG_FMT_STRENGTH)
            payload = (size_t)llg_sv4_width(arg->value.packed) * 4u;
        else if (arg->kind == LLG_FMT_STRING || arg->kind == LLG_FMT_TEXT) {
            if (arg->value.string.len > SIZE_MAX / 8u)
                llg_fatal_allocation("string display", arg->value.string.len, 8u);
            payload = arg->value.string.len * 8u;
        }
        size_t raw_cap = llg_format_scratch_size(payload, (size_t)spec.precision);
        raw_cap = llg_format_size_add(raw_cap, (size_t)spec.width);
        // Ordinary packed scalars format in frame scratch; only wide values,
        // long strings or large widths/precisions allocate.
        char inline_raw[LLG_FORMAT_INLINE_SCRATCH];
        char* raw = raw_cap <= sizeof(inline_raw)
                        ? inline_raw
                        : llg_checked_malloc(raw_cap, 1, "typed format");
        size_t raw_len = 0;
        if (strchr("dhbox", conversion) &&
            (arg->kind == LLG_FMT_PACKED || arg->kind == LLG_FMT_STRING ||
             arg->kind == LLG_FMT_REAL)) {
            sv4_t converted = SV4_EMPTY;
            const sv4_t* value = llg_fmt_integral_operand(arg, &converted);
            raw_len = llg_format_integral(conversion == 'x' ? 'h' : conversion,
                                          *value, &spec, raw, raw_cap);
            if (conversion == 'd' && !spec.has_width && !spec.zero) {
                // An unsized `%d` field is as wide as the largest value of
                // the argument's width (SV 21.2.1.3).
                spec.width = llg_decimal_field_width(llg_sv4_width(*value),
                                                     llg_sv4_signed(*value));
                spec.has_width = 1;
            }
            sv4_destroy(&converted);
        } else if (conversion == 't' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_time_integer(arg->value.packed,
                                              arg->time_unit_fs, raw, raw_cap);
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 't' && arg->kind == LLG_FMT_REAL) {
            raw_len = llg_format_time_real(arg->value.real, arg->time_unit_fs,
                                           raw, raw_cap);
            if (!spec.has_width && !spec.zero) {
                spec.width = g.time_format.minimum_field_width;
                spec.has_width = spec.width > 0;
            }
        } else if (conversion == 'c' &&
                   (arg->kind == LLG_FMT_PACKED || arg->kind == LLG_FMT_STRING ||
                    arg->kind == LLG_FMT_REAL)) {
            sv4_t converted = SV4_EMPTY;
            const sv4_t* value = llg_fmt_integral_operand(arg, &converted);
            raw_len = llg_format_char(*value, raw, raw_cap);
            sv4_destroy(&converted);
        } else if (conversion == 'u' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw2(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'z' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_raw4(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'v' && arg->kind == LLG_FMT_PACKED) {
            raw_len = llg_format_strength(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'v' && arg->kind == LLG_FMT_STRENGTH) {
            raw_len = llg_format_strength_view(arg->value.packed, raw, raw_cap);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_PACKED) {
            // A packed scalar prints as it would unformatted (SV 21.2.1.7);
            // aggregates arrive as LLG_FMT_TEXT from the pattern walker.
            raw_len = llg_format_pattern_packed(arg->value.packed, raw, raw_cap);
        } else if (strchr("feg", conversion) &&
                   (arg->kind == LLG_FMT_REAL || arg->kind == LLG_FMT_PACKED)) {
            // An integral argument converts to real, X/Z bits as zero.
            double real = arg->kind == LLG_FMT_REAL ? arg->value.real
                                                    : sv4_to_real(arg->value.packed);
            char real_fmt[128];
            size_t spec_len = (size_t)(p - start);
            if (spec_len >= sizeof(real_fmt) - 1) spec_len = sizeof(real_fmt) - 2;
            memcpy(real_fmt, start, spec_len);
            real_fmt[spec_len] = 0;
            int written = snprintf(raw, raw_cap, real_fmt, real);
            raw_len = written < 0 ? 0 : (size_t)written < raw_cap
                                           ? (size_t)written
                                           : raw_cap - 1;
        } else if (conversion == 's' && arg->kind == LLG_FMT_PACKED) {
            llg_string_t value = llg_string_from_packed(arg->value.packed);
            raw_len = value.len;
            if (raw_len > raw_cap) raw_len = raw_cap;
            if (raw_len) memcpy(raw, value.data, raw_len);
            llg_string_destroy(&value);
        } else if (conversion == 's' && arg->kind == LLG_FMT_STRING) {
            raw_len = arg->value.string.len;
            if (raw_len > raw_cap) raw_len = raw_cap;
            if (raw_len) memcpy(raw, arg->value.string.data, raw_len);
        } else if ((conversion == 'p' || conversion == 's') && arg->kind == LLG_FMT_TEXT) {
            raw_len = arg->value.string.len;
            if (raw_len > raw_cap) raw_len = raw_cap;
            if (raw_len) memcpy(raw, arg->value.string.data, raw_len);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_STRING) {
            // A quoted string literal with escapes (SV 21.2.1.7, 5.9).
            raw_len = llg_format_pattern_string(arg->value.string.data,
                                                arg->value.string.len, raw, raw_cap);
        } else if (conversion == 'p' && arg->kind == LLG_FMT_REAL) {
            raw_len = llg_format_pattern_real(arg->value.real, raw, raw_cap);
        } else {
            if (raw != inline_raw) free(raw);
            llg_append_text(out, cap, &len, start, (size_t)(p - start));
            continue;
        }
        llg_emit_field(out, cap, &len, raw, raw_len, spec, conversion);
        if (raw != inline_raw) free(raw);
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
