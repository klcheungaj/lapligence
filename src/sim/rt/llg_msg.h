// llg_msg.h: the runtime message reporter.
//
// Every runtime message is rendered from llg_msg_layout.def (the layout the
// llg host renders from too) and carries an id from llg_msg_ids.def. All
// functions here run on cold paths: a message is assembled in one buffer and
// written with one stdio call, so a message from another thread (or a
// waveform report) never lands inside it. Short messages stay in the inline
// buffer; longer ones grow on the heap, and an allocation failure truncates a
// message rather than aborting a report.
//
// The runtime is several translation units, some of which build standalone
// (value, string, constraint and component probes), so the reporter is
// `static inline` here and each unit counts its own messages. A unit whose
// counts belong in the end-of-run summary exports them with
// LLG_MSG_EXPORT_COUNTS; the scheduler adds them up.
#ifndef LLG_MSG_H
#define LLG_MSG_H

#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {
    LLG_SEVERITY_INFO = 0,
    LLG_SEVERITY_WARNING = 1,
    LLG_SEVERITY_ERROR = 2,
    LLG_SEVERITY_FATAL = 3,
};

typedef enum {
#define LLG_MSG_ID(name, tool, index, severity, meaning, disable) LLG_MSG_##name,
#include "llg_msg_ids.def"
#undef LLG_MSG_ID
    LLG_MSG_ID_COUNT
} llg_msg_id_t;

// ── Whole-message buffer ─────────────────────────────────────────────────────

typedef struct {
    char* data;
    size_t len;
    size_t cap;
    char inline_buf[512];
} llg_report_t;

static inline void llg_report_init(llg_report_t* report) {
    report->data = report->inline_buf;
    report->len = 0;
    report->cap = sizeof(report->inline_buf);
}

// Make room for `extra` more bytes plus a terminator; returns how many of the
// requested bytes fit.
static inline size_t llg_report_reserve(llg_report_t* report, size_t extra) {
    size_t room = report->cap - report->len - 1u;
    if (extra <= room) return extra;
    size_t need = report->len + extra + 1u;
    if (need < report->len) return room;
    size_t cap = report->cap * 2u > need ? report->cap * 2u : need;
    char* grown = report->data == report->inline_buf ? (char*)malloc(cap)
                                                     : (char*)realloc(report->data, cap);
    if (!grown) return room;
    if (report->data == report->inline_buf) memcpy(grown, report->data, report->len);
    report->data = grown;
    report->cap = cap;
    return extra;
}

static inline void llg_report_bytes(llg_report_t* report, const char* bytes, size_t len) {
    size_t fit = llg_report_reserve(report, len);
    if (fit) memcpy(report->data + report->len, bytes, fit);
    report->len += fit;
    report->data[report->len] = '\0';
}

static inline void llg_report_vprintf(llg_report_t* report, const char* fmt, va_list args) {
    va_list again;
    va_copy(again, args);
    size_t room = report->cap - report->len;
    int written = vsnprintf(report->data + report->len, room, fmt, args);
    if (written < 0) {
        va_end(again);
        return;
    }
    if ((size_t)written >= room) {
        size_t fit = llg_report_reserve(report, (size_t)written);
        (void)vsnprintf(report->data + report->len, fit + 1u, fmt, again);
        written = (int)fit;
    }
    va_end(again);
    report->len += (size_t)written;
}

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 3)))
#endif
static inline void llg_report_printf(llg_report_t* report, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    llg_report_vprintf(report, fmt, args);
    va_end(args);
}

static inline void llg_report_free(llg_report_t* report) {
    if (report->data != report->inline_buf) free(report->data);
    llg_report_init(report);
}

// Write the buffer to `stream` in one call and release it.
static inline void llg_report_emit(llg_report_t* report, FILE* stream) {
    if (report->len) (void)fwrite(report->data, 1, report->len, stream);
    fflush(stream);
    llg_report_free(report);
}

// ── Layout ───────────────────────────────────────────────────────────────────

#define LLG_MSG_LAYOUT(name, value) static const char* const LLG_MSG_LAYOUT_##name = value;
#include "llg_msg_layout.def"
#undef LLG_MSG_LAYOUT

// This unit's message counts per severity. A unit that always links with
// the scheduler defines LLG_MSG_COUNT_HOOK (before its first include) to count
// in the scheduler's table instead.
#ifdef LLG_MSG_COUNT_HOOK
void LLG_MSG_COUNT_HOOK(int severity);
#else
static uint64_t llg_msg_counts[4];
#endif

// The units whose counts the end-of-run summary adds up.
uint64_t llg_msg_count_scheduler(int severity);
uint64_t llg_msg_count_container(int severity);
uint64_t llg_msg_count_waveform(int severity);

#define LLG_MSG_EXPORT_COUNTS(unit)                                        \
    uint64_t llg_msg_count_##unit(int severity) {                          \
        return severity >= 0 && severity < 4 ? llg_msg_counts[severity] : 0; \
    }

typedef struct {
    const char* severity;
    const char* location;
    const char* id;
    const char* context;
    const char* text;
    size_t text_len;
} llg_msg_fields_t;

// The value of template field `name` (`len` bytes), or NULL when absent.
static inline const char* llg_msg_field(const llg_msg_fields_t* f, const char* name,
                                        size_t len, size_t* value_len) {
    const char* value = NULL;
    if (len == 8 && memcmp(name, "severity", 8) == 0) value = f->severity;
    else if (len == 8 && memcmp(name, "location", 8) == 0) value = f->location;
    else if (len == 2 && memcmp(name, "id", 2) == 0) value = f->id;
    else if (len == 7 && memcmp(name, "context", 7) == 0) value = f->context;
    else if (len == 4 && memcmp(name, "text", 4) == 0) {
        *value_len = f->text_len;
        return f->text;
    }
    *value_len = value ? strlen(value) : 0;
    return value;
}

// Render `tmpl` into `out`: `{field}` is replaced, `<...>` is dropped when
// one of its fields is empty (src/core/report.rs renders the same way).
static inline void llg_msg_render(llg_report_t* out, const char* tmpl,
                                  const llg_msg_fields_t* f) {
    const char* p = tmpl;
    while (*p) {
        if (*p == '<') {
            const char* end = strchr(p, '>');
            if (!end) return;
            int complete = 1;
            const char* q = p + 1;
            while (q < end) {
                if (*q == '{') {
                    const char* close = strchr(q, '}');
                    size_t value_len = 0;
                    if (!close || close > end) return;
                    if (!llg_msg_field(f, q + 1, (size_t)(close - q - 1), &value_len) ||
                        value_len == 0)
                        complete = 0;
                    q = close + 1;
                } else {
                    q++;
                }
            }
            if (!complete) {
                p = end + 1;
                continue;
            }
            p++;
            while (p < end) {
                if (*p == '{') {
                    const char* close = strchr(p, '}');
                    size_t value_len = 0;
                    const char* value =
                        llg_msg_field(f, p + 1, (size_t)(close - p - 1), &value_len);
                    if (value) llg_report_bytes(out, value, value_len);
                    p = close + 1;
                } else {
                    llg_report_bytes(out, p, 1);
                    p++;
                }
            }
            p = end + 1;
        } else if (*p == '{') {
            const char* close = strchr(p, '}');
            size_t value_len = 0;
            if (!close) return;
            const char* value = llg_msg_field(f, p + 1, (size_t)(close - p - 1), &value_len);
            if (value) llg_report_bytes(out, value, value_len);
            p = close + 1;
        } else {
            const char* next = p;
            while (*next && *next != '{' && *next != '<') next++;
            llg_report_bytes(out, p, (size_t)(next - p));
            p = next;
        }
    }
}

static inline const char* llg_msg_severity_word(int severity) {
    switch (severity) {
        case LLG_SEVERITY_INFO: return LLG_MSG_LAYOUT_SEVERITY_INFO;
        case LLG_SEVERITY_WARNING: return LLG_MSG_LAYOUT_SEVERITY_WARNING;
        case LLG_SEVERITY_ERROR: return LLG_MSG_LAYOUT_SEVERITY_ERROR;
        default: return LLG_MSG_LAYOUT_SEVERITY_FATAL;
    }
}

static inline int llg_msg_default_severity(llg_msg_id_t id) {
    static const unsigned char severities[] = {
#define LLG_MSG_ID(name, tool, index, severity, meaning, disable) (unsigned char)(severity),
#include "llg_msg_ids.def"
#undef LLG_MSG_ID
    };
    return (unsigned)id < (unsigned)LLG_MSG_ID_COUNT ? severities[id] : LLG_SEVERITY_FATAL;
}

// Append message `id` with `severity` to `out` (newline included) and count
// it. `location`, `scope` and `time` may be NULL or empty.
static inline void llg_msg_append(llg_report_t* out, llg_msg_id_t id, int severity,
                                  const char* location, const char* scope,
                                  const char* time, const char* text, size_t text_len) {
    static const char* const tools[] = {
#define LLG_MSG_ID(name, tool, index, severity, meaning, disable) #tool,
#include "llg_msg_ids.def"
#undef LLG_MSG_ID
    };
    static const unsigned short indices[] = {
#define LLG_MSG_ID(name, tool, index, severity, meaning, disable) (unsigned short)(index),
#include "llg_msg_ids.def"
#undef LLG_MSG_ID
    };
    char id_text[32];
    char context_inline[256];
    char* context = context_inline;
    unsigned known = (unsigned)id < (unsigned)LLG_MSG_ID_COUNT;
    snprintf(id_text, sizeof id_text, "%s%s%03u", known ? tools[id] : "SIM",
             LLG_MSG_LAYOUT_ID_JOIN, known ? (unsigned)indices[id] : 999u);
    int has_scope = scope && scope[0];
    int has_time = time && time[0];
    size_t context_len = (has_scope ? strlen(scope) : 0) + (has_time ? strlen(time) : 0) +
                         strlen(LLG_MSG_LAYOUT_CONTEXT_JOIN) + 1u;
    if (context_len > sizeof context_inline) {
        context = (char*)malloc(context_len);
        if (!context) {
            context = context_inline;
            has_scope = 0;
        }
    }
    snprintf(context, has_scope || has_time ? context_len : 1u, "%s%s%s",
             has_scope ? scope : "", has_scope && has_time ? LLG_MSG_LAYOUT_CONTEXT_JOIN : "",
             has_time ? time : "");
    if (!has_scope && !has_time) context[0] = '\0';
    if (severity < LLG_SEVERITY_INFO || severity > LLG_SEVERITY_FATAL) severity = LLG_SEVERITY_FATAL;
    llg_msg_fields_t fields = {llg_msg_severity_word(severity), location, id_text,
                               context, text, text_len};
    llg_msg_render(out, LLG_MSG_LAYOUT_TEMPLATE, &fields);
    llg_report_bytes(out, "\n", 1);
    if (context != context_inline) free(context);
#ifdef LLG_MSG_COUNT_HOOK
    LLG_MSG_COUNT_HOOK(severity);
#else
    if (llg_msg_counts[severity] != UINT64_MAX) llg_msg_counts[severity]++;
#endif
}

// Report message `id` (its catalog severity) with printf-style text.
static inline void llg_msg_vreport(llg_msg_id_t id, const char* location,
                                   const char* fmt, va_list args) {
    llg_report_t text;
    llg_report_t out;
    llg_report_init(&text);
    llg_report_init(&out);
    llg_report_vprintf(&text, fmt, args);
    llg_msg_append(&out, id, llg_msg_default_severity(id), location, NULL, NULL, text.data,
                   text.len);
    llg_report_free(&text);
    llg_report_emit(&out, stderr);
}

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 3, 4)))
#endif
static inline void llg_msg(llg_msg_id_t id, const char* location, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    llg_msg_vreport(id, location, fmt, args);
    va_end(args);
}

// Report a fatal internal message and abort.
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 3)))
#endif
static inline _Noreturn void llg_msg_abort(llg_msg_id_t id, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    llg_msg_vreport(id, NULL, fmt, args);
    va_end(args);
    abort();
}

// The end-of-run summary text for `counts` from the layout's SUMMARY template.
static inline void llg_msg_summary_text(llg_report_t* out, const uint64_t counts[4]) {
    char numbers[4][24];
    const char* names[4] = {"info", "warning", "error", "fatal"};
    int i;
    for (i = 0; i < 4; i++)
        snprintf(numbers[i], sizeof numbers[i], "%llu", (unsigned long long)counts[i]);
    const char* p = LLG_MSG_LAYOUT_SUMMARY;
    while (*p) {
        if (*p == '<') {
            const char* end = strchr(p, '>');
            const char* q;
            int complete = 1;
            if (!end) return;
            for (q = p + 1; q < end; q++) {
                if (*q != '{') continue;
                for (i = 0; i < 4; i++) {
                    size_t n = strlen(names[i]);
                    if (strncmp(q + 1, names[i], n) == 0 && q[n + 1] == '}' &&
                        counts[i] == 0 && (i == LLG_SEVERITY_INFO || i == LLG_SEVERITY_FATAL))
                        complete = 0;
                }
            }
            if (!complete) {
                p = end + 1;
                continue;
            }
            p++;
            while (p < end) {
                int matched = 0;
                if (*p == '{') {
                    for (i = 0; i < 4; i++) {
                        size_t n = strlen(names[i]);
                        if (strncmp(p + 1, names[i], n) == 0 && p[n + 1] == '}') {
                            llg_report_bytes(out, numbers[i], strlen(numbers[i]));
                            p += n + 2;
                            matched = 1;
                            break;
                        }
                    }
                }
                if (!matched) {
                    llg_report_bytes(out, p, 1);
                    p++;
                }
            }
            p = end + 1;
            continue;
        }
        if (*p == '{') {
            int matched = 0;
            for (i = 0; i < 4; i++) {
                size_t n = strlen(names[i]);
                if (strncmp(p + 1, names[i], n) == 0 && p[n + 1] == '}') {
                    llg_report_bytes(out, numbers[i], strlen(numbers[i]));
                    p += n + 2;
                    matched = 1;
                    break;
                }
            }
            if (matched) continue;
        }
        llg_report_bytes(out, p, 1);
        p++;
    }
}

#endif
