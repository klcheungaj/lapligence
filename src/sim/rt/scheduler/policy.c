
const char* llg_region_name(llg_region_t region) {
    if (region < 0 || region >= LLG_REGION_COUNT) return "invalid";
    return llg_region_names[region];
}

static int region_valid(llg_region_t region) {
    return region >= 0 && region < LLG_REGION_COUNT;
}

static int region_is_reactive(llg_region_t region) {
    return region >= LLG_REGION_REACTIVE && region <= LLG_REGION_POST_RE_NBA_PLI;
}

static int region_is_design(llg_region_t region) {
    return (region >= LLG_REGION_ACTIVE && region <= LLG_REGION_POST_NBA_PLI) ||
           region_is_reactive(region);
}

static int region_is_read_only_now(llg_region_t region) {
    if (!g.running) return 0;
    return region == LLG_REGION_PREPONED || region == LLG_REGION_PREPONED_PLI ||
           (region >= LLG_REGION_PRE_OBSERVED_PLI &&
            region <= LLG_REGION_POST_OBSERVED_PLI) ||
           region == LLG_REGION_POSTPONED || region == LLG_REGION_POSTPONED_PLI;
}

int llg_region_is_read_only(void) {
    return region_is_read_only_now(g.current_region);
}

llg_region_t llg_current_region(void) { return g.current_region; }

static void region_violation(const char* action, llg_region_t region) {
    fprintf(stderr, "llg: illegal %s in read-only or completed %s region\n",
            action, llg_region_name(region));
    llg_last_failure = 1;
    g.finish = 1;
}

static int region_can_mutate(const char* action) {
    if (region_is_read_only_now(g.current_region)) {
        region_violation(action, g.current_region);
        return 0;
    }
    return 1;
}

static int callback_region_allowed(llg_region_t region, uint64_t ticks) {
    if (!region_valid(region)) {
        fprintf(stderr, "llg: invalid execution region %d for callback\n", (int)region);
        llg_last_failure = 1;
        g.finish = 1;
        return 0;
    }
    if (!g.running || ticks != 0) return 1;
    if (region_is_read_only_now(g.current_region)) {
        if (g.current_region == LLG_REGION_OBSERVED &&
            region == LLG_REGION_REACTIVE)
            return 1;
        region_violation("callback scheduling", g.current_region);
        return 0;
    }
    if (region >= g.current_region) return 1;
    // A writable iterative region can return work to the design/reactive set;
    // the scheduler will drain that set again before reaching Postponed.
    if (region_is_design(region)) return 1;
    fprintf(stderr, "llg: illegal callback scheduling from %s to %s at time %llu\n",
            llg_region_name(g.current_region), llg_region_name(region),
            (unsigned long long)g.now);
    llg_last_failure = 1;
    g.finish = 1;
    return 0;
}

static int parse_positive_u64(const char* text, uint64_t* value) {
    if (!text || text[0] == '\0') return 0;
    uint64_t parsed = 0;
    for (const unsigned char* p = (const unsigned char*)text; *p; ++p) {
        if (*p < '0' || *p > '9') return 0;
        uint64_t digit = (uint64_t)(*p - '0');
        if (parsed > (UINT64_MAX - digit) / 10u) return 0;
        parsed = parsed * 10u + digit;
    }
    if (parsed == 0) return 0;
    *value = parsed;
    return 1;
}

static int load_limit(const char* name, uint64_t fallback, uint64_t* value,
                      int* present) {
    const char* text = getenv(name);
    *present = text != NULL;
    if (!text) {
        if (fallback == 0) {
            fprintf(stderr,
                    "llg: invalid %s default (must be a positive decimal uint64)\n",
                    name);
            return 0;
        }
        *value = fallback;
        return 1;
    }
    if (!parse_positive_u64(text, value)) {
        fprintf(stderr,
                "llg: invalid %s (must be a positive decimal uint64)\n", name);
        return 0;
    }
    return 1;
}

static int configure_limits(void) {
    int zero_present = 0;
    if (!load_limit("LLG_ZERO_LOOP_LIMIT", (uint64_t)LLG_ZERO_LOOP_LIMIT,
                    &g.zero_loop_limit, &zero_present)) {
        return 0;
    }

    const char* process_name = "LLG_PROCESS_STEP_LIMIT";
    const char* process_text = getenv(process_name);
    if (!process_text) {
        process_name = "LLG_NONCONVERGENCE_LIMIT";
        process_text = getenv(process_name);
    }
    if (process_text) {
        if (!parse_positive_u64(process_text, &g.process_step_limit)) {
            fprintf(stderr,
                    "llg: invalid %s (must be a positive decimal uint64)\n",
                    process_name);
            return 0;
        }
    } else if (zero_present) {
        // A single zero-time limit is convenient for callers that only need
        // to tighten the guard; the dedicated process setting still wins.
        g.process_step_limit = g.zero_loop_limit;
    } else {
        int process_present = 0;
        if (!load_limit("LLG_PROCESS_STEP_LIMIT",
                        (uint64_t)LLG_PROCESS_STEP_LIMIT,
                        &g.process_step_limit, &process_present)) {
            return 0;
        }
    }
    return 1;
}

static int configure_stop_policy(void) {
    if (llg_stop_policy_override) {
        g.stop_policy = llg_configured_stop_policy;
        return 1;
    }
    const char* text = getenv("LLG_STOP_POLICY");
    if (!text || strcmp(text, "resume") == 0) {
        g.stop_policy = LLG_STOP_POLICY_RESUME;
        return 1;
    }
    if (strcmp(text, "exit") == 0) {
        g.stop_policy = LLG_STOP_POLICY_EXIT;
        return 1;
    }
    fprintf(stderr,
            "llg: invalid LLG_STOP_POLICY `%s` (expected resume or exit)\n",
            text);
    return 0;
}

static int consume_limit(uint64_t* counter, uint64_t limit) {
    if (*counter >= limit) return 0;
    *counter += 1;
    return 1;
}
