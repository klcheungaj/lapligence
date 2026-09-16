
// ── $display / $write ─────────────────────────────────────────────────────────

static void llg_vprint(const char* fmt, va_list ap, int newline) {
    const char* p = fmt;
    while (*p) {
        char c = *p++;
        if (c == '%') {
            const char* spec_start = p - 1;
            int has_width;
            int width;
            int zero;
            p = llg_parse_legacy_spec(p, &has_width, &width, &zero);
            c = *p;
            if (c) ++p;
            if (c == '%') {
                fputc('%', stdout);
            } else if (c == 't') {
                sv4_t v = va_arg(ap, sv4_t);
                size_t tmp_cap = llg_format_scratch_size(v.width, 0);
                char* tmp = llg_checked_malloc(tmp_cap, 1, "time output");
                size_t len = llg_format_time_integer(v, g.design_precision_fs,
                                                      tmp, tmp_cap);
                if (!has_width && !zero) width = g.time_format.minimum_field_width;
                while (width > 0 && (size_t)width > len) {
                    fputc(' ', stdout);
                    width--;
                }
                fwrite(tmp, 1, len, stdout);
                free(tmp);
            } else if (c == 's') {
                const char* s = va_arg(ap, const char*);
                if (s) {
                    fputs(s, stdout);
                }
            } else if (c == 'd' || c == 'h' || c == 'b' || c == 'o') {
                sv4_t v = va_arg(ap, sv4_t);
                // One complete packed value, including a possible minus sign.
                size_t tmp_cap = (size_t)v.width + 3u;
                char* tmp = llg_checked_malloc(tmp_cap, 1, "packed output");
                sv4_format(c, v, tmp, tmp_cap);
                fputs(tmp, stdout);
                free(tmp);
            } else if (c == 'f' || c == 'e' || c == 'g') {
                double v = va_arg(ap, double);
                char real_fmt[128];
                size_t spec_len = (size_t)(p - spec_start);
                if (spec_len >= sizeof(real_fmt)) spec_len = sizeof(real_fmt) - 1;
                memcpy(real_fmt, spec_start, spec_len);
                real_fmt[spec_len] = 0;
                fprintf(stdout, real_fmt, v);
            } else {
                // Unknown specifier: print it verbatim.
                fputc('%', stdout);
                if (c) fputc(c, stdout);
            }
        } else {
            fputc(c, stdout);
        }
    }
    if (newline) fputc('\n', stdout);
    fflush(stdout);
}

void llg_display(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 1);
    va_end(ap);
}

void llg_write(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    llg_vprint(fmt, ap, 0);
    va_end(ap);
}

void llg_display_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                       const char* scope) {
    llg_print_typed(fmt, args, n, scope, 1);
    llg_fmt_args_destroy(args, n);
}

void llg_write_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                     const char* scope) {
    llg_print_typed(fmt, args, n, scope, 0);
    llg_fmt_args_destroy(args, n);
}

static int llg_system_allowed(void) {
    const char* value = getenv("LLG_ALLOW_SYSTEM");
    return value && (!strcmp(value, "1") || !strcmp(value, "true") ||
                     !strcmp(value, "yes") || !strcmp(value, "on"));
}

sv4_t llg_system(llg_string_t command, int has_command) {
    if (has_command != 0 && has_command != 1) {
        fprintf(stderr, "llg: invalid internal `$system` argument marker\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }
    if (has_command && (!command.data && command.len != 0)) {
        fprintf(stderr,
                "llg: `$system` command has a nonzero length without storage\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }
    if (has_command) {
        for (size_t i = 0; i < command.len; ++i) {
            if (command.data[i] == '\0') {
                fprintf(stderr,
                        "llg: `$system` command contains an embedded NUL and "
                        "was not executed\n");
                llg_last_failure = 1;
                g.finish = 1;
                llg_string_destroy(&command);
                return sv4_from_u64(UINT32_MAX, 32, 1);
            }
        }
    }
    if (!llg_system_allowed()) {
        fprintf(stderr,
                "llg: $system is disabled; set LLG_ALLOW_SYSTEM=1 for the "
                "generated simulator process\n");
        llg_last_failure = 1;
        g.finish = 1;
        llg_string_destroy(&command);
        return sv4_from_u64(UINT32_MAX, 32, 1);
    }

    // IEEE 1800-2009 §20.18 specifies the NULL argument for the omitted form.
    // Keep it distinct from the explicit empty C command string.
    int status;
    if (!has_command) {
        llg_string_destroy(&command);
        status = system(NULL);
    } else {
        status = system(command.data ? command.data : "");
        llg_string_destroy(&command);
    }
    return sv4_from_u64((uint32_t)status, 32, 1);
}
