// ── randomize() glue (IEEE 1800-2009 18.6, 18.12) ───────────────────────────
//
// Converts packed values to the solver's 2-state words, chooses the random
// stream and reports outcomes (decisions S29-D1, D3, D5, D6). The solver
// itself (llg_constraint.c) never touches HDL storage; generated code commits
// the solution only after LLG_CR_OK.

static uint32_t randomize_stream_next(void* stream) {
    return llg_rng_state_next((llg_rng_state_t*)stream);
}

/* Copy a packed value's known bits into `words` (CR_WORDS(width) words).
 * Returns LLG_CR_INPUT_XZ when any bit is X or Z. */
static int randomize_words(sv4_t value, uint64_t* words, uint32_t width) {
    uint32_t count = width ? (width + 63u) / 64u : 1u;
    size_t available = llg_sv4_words(value);
    for (uint32_t i = 0; i < count; ++i)
        words[i] = i < available ? llg_sv4_word(value, i, LLG_SV4_BITS) : 0u;
    if (width % 64u) words[count - 1u] &= (UINT64_C(1) << (width % 64u)) - 1u;
    return sv4_is_unknown(value) ? LLG_CR_INPUT_XZ : LLG_CR_INPUT_KNOWN;
}

/* Scratch sized for one value; widths stay below LLG_SUPPORTED_WIDTH_LIMIT. */
static uint64_t* randomize_scratch(uint32_t width) {
    static uint64_t* buffer;
    static size_t capacity;
    size_t need = width ? (width + 63u) / 64u : 1u;
    if (need > capacity) {
        uint64_t* grown = (uint64_t*)realloc(buffer, need * sizeof(uint64_t));
        if (!grown) {
            fprintf(stderr, "llg: randomize: out of memory\n");
            abort();
        }
        buffer = grown;
        capacity = need;
    }
    return buffer;
}

void llg_cr_set_var_sv4(llg_cr_session_t* s, uint32_t var, sv4_t value, uint32_t width,
                        int active) {
    sv4_t sized = sv4_resize(value, width, 0);
    uint64_t* words = randomize_scratch(width);
    int input = randomize_words(sized, words, width);
    llg_cr_set_var(s, var, words, active, input);
    sv4_destroy(&sized);
}

void llg_cr_set_leaf_sv4(llg_cr_session_t* s, uint32_t leaf, sv4_t value, uint32_t width,
                         int null_dereference) {
    if (null_dereference) {
        llg_cr_set_leaf(s, leaf, NULL, LLG_CR_INPUT_NULL);
        return;
    }
    sv4_t sized = sv4_resize(value, width, 0);
    uint64_t* words = randomize_scratch(width);
    int input = randomize_words(sized, words, width);
    llg_cr_set_leaf(s, leaf, words, input);
    sv4_destroy(&sized);
}

sv4_t llg_cr_value_sv4(const llg_cr_session_t* s, uint32_t var, uint32_t width,
                       int8_t is_signed) {
    return sv4_from_limbs(llg_cr_value(s, var), NULL, NULL, width, is_signed);
}

sv4_t llg_cr_arg_sv4(const uint64_t* words, uint32_t width, int8_t is_signed) {
    return sv4_from_limbs(words, NULL, NULL, width, is_signed);
}

int llg_cr_result_sv4(sv4_t value, uint64_t* result, uint32_t width) {
    sv4_t sized = sv4_resize(value, width, 0);
    int input = randomize_words(sized, result, width);
    sv4_destroy(&sized);
    return input;
}

static const char* randomize_reason_text(int reason) {
    switch (reason) {
    case LLG_CR_REASON_NULL: return "null object handle in a constraint";
    case LLG_CR_REASON_XZ_LEAF: return "X or Z value in a constraint";
    case LLG_CR_REASON_XZ_VAR: return "X or Z value of a random variable that is not randomized";
    case LLG_CR_REASON_XZ_CALL: return "X or Z result of a constraint function";
    case LLG_CR_REASON_DIVZERO: return "division by zero in a constraint";
    case LLG_CR_REASON_INDEX: return "select index out of range in a constraint";
    case LLG_CR_REASON_ORDER: return "circular function-argument order";
    default: return "evaluation error";
    }
}

/* Solve, report and return the randomize() result (1 on success). UNSAT
 * warns and the run continues; RESOURCE and ERROR are run-time errors that
 * make the run exit with status 1 (S29-D5). `site` names the call. */
static int randomize_run(const llg_cr_problem_t* problem, llg_cr_session_t* s,
                         llg_rng_state_t* stream, llg_cr_call_fn calls, void* context,
                         const char* site) {
    llg_cr_rng_t rng = {randomize_stream_next, stream};
    llg_cr_report_t report;
    llg_cr_status_t status = llg_cr_solve(s, &rng, calls, context, &report);
    switch (status) {
    case LLG_CR_OK:
        return 1;
    case LLG_CR_UNSAT:
        fprintf(stderr, "llg: warning: randomize at %s: the constraints cannot be satisfied; randomize returns 0\n",
                site);
        return 0;
    case LLG_CR_RESOURCE:
        fprintf(stderr,
                "llg: error: randomize at %s: solver budget %s exhausted before a solution "
                "or proof of unsatisfiability was found (raise %s); randomize returns 0\n",
                site, report.budget, report.budget);
        llg_last_failure = 1;
        return 0;
    default: {
        const char* name = NULL;
        switch (report.reason) {
        case LLG_CR_REASON_NULL:
        case LLG_CR_REASON_XZ_LEAF:
            if (report.item < problem->leaf_count) name = problem->leaves[report.item].name;
            break;
        case LLG_CR_REASON_XZ_VAR:
        case LLG_CR_REASON_ORDER:
            if (report.item < problem->var_count) name = problem->vars[report.item].name;
            break;
        case LLG_CR_REASON_XZ_CALL:
            if (report.item < problem->call_count) name = problem->calls[report.item].name;
            break;
        default:
            break;
        }
        fprintf(stderr, "llg: error: randomize at %s: %s%s%s%s; randomize returns 0\n", site,
                randomize_reason_text(report.reason), name ? " (`" : "", name ? name : "",
                name ? "`)" : "");
        llg_last_failure = 1;
        return 0;
    }
    }
}

int llg_randomize_object(const llg_cr_problem_t* problem, llg_cr_session_t* s,
                         llg_rng_state_t* object_stream, llg_cr_call_fn calls, void* context,
                         const char* site) {
    /* A model that never inspects object streams keeps no per-object state;
     * the thread stream then stands in (the emitter keeps object streams in
     * every model that randomizes objects). */
    return randomize_run(problem, s, object_stream ? object_stream : llg_process_rng(), calls,
                         context, site);
}

/* std::randomize draws from the calling thread's stream (S29-D3). */
int llg_randomize_scope(const llg_cr_problem_t* problem, llg_cr_session_t* s,
                        llg_cr_call_fn calls, void* context, const char* site) {
    return randomize_run(problem, s, llg_process_rng(), calls, context, site);
}
