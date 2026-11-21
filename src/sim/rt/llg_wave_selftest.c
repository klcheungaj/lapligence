// llg_wave_selftest.c — bounded queue, flush barrier, VCD, and FST smoke test.
#define LLG_WAVEFORM 1
#define LLG_PLATFORM_POSIX_2008 1
#define LLG_PLATFORM_FILE_POSITIONS 1
#include "llg_platform.h"

#include "llg_wave.h"
#include "fstapi.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef LLG_SELFTEST_TRACK_STORAGE
// Counters of the tracking allocator that storage test builds link. Declared
// at file scope: MSVC /W4 rejects block-scope function declarations (C4210).
size_t value_test_live(void);
size_t value_test_bytes(void);
#endif

// Results use the model's message layout with id SIM-032: a failure is an
// ERROR on stderr (located at its selftest line when it has one), the pass
// an INFO on stdout.
LLG_MSG_PRINTF(4, 5)
static void selftest_report(FILE* stream, int32_t severity, int32_t line, const char* fmt, ...) {
    char location[64];
    llg_report_t text;
    llg_report_t out;
    va_list args;
    if (line > 0) snprintf(location, sizeof location, "llg_wave_selftest.c:%" PRId32, line);
    llg_report_init(&text);
    llg_report_init(&out);
    va_start(args, fmt);
    llg_report_vprintf(&text, fmt, args);
    va_end(args);
    llg_msg_append(&out, LLG_MSG_SELFTEST, severity, line > 0 ? location : NULL, NULL, NULL,
                   text.data, text.len, NULL);
    llg_report_free(&text);
    llg_report_emit(&out, stream);
}

#define CHECK(expr) do { \
    if (!(expr)) { \
        selftest_report(stderr, LLG_SEVERITY_ERROR, __LINE__, "wave selftest check failed: %s", \
                        #expr); \
        goto cleanup; \
    } \
} while (0)

static int32_t file_contains(const char* path, const char* needle) {
    FILE* file = fopen(path, "rb");
    if (!file) return 0;
    if (llg_file_seek64(file, 0, SEEK_END) != 0) {
        fclose(file);
        return 0;
    }
    int64_t size = llg_file_tell64(file);
    if (size < 0 || (uint64_t)size >= SIZE_MAX || llg_file_seek64(file, 0, SEEK_SET) != 0) {
        fclose(file);
        return 0;
    }
    char* text = (char*)malloc((size_t)size + 1u);
    if (!text) {
        fclose(file);
        return 0;
    }
    if (fread(text, 1u, (size_t)size, file) != (size_t)size) {
        free(text);
        fclose(file);
        return 0;
    }
    text[size] = 0;
    fclose(file);
    int32_t found = strstr(text, needle) != NULL;
    free(text);
    return found;
}

#ifndef LLG_WAVE_DELTAS
#define ORDERED_COUNT 20u

// End-of-step records (S47-D14): a step's records follow registration order
// whatever the order of the writes (more than the insertion-sorted batch
// size), and a value that ends its step where its last record left it
// writes no record.
static int32_t restored_and_ordered(const char* path) {
    sv4_t values[ORDERED_COUNT];
    double real_value = 0.5;
    char name[32];
    char expected[ORDERED_COUNT * 16u + 8u];
    size_t used = 0;
    int32_t ok = 0;
    for (uint32_t i = 0; i < ORDERED_COUNT; i++) values[i] = SV4_C(0, 8);
    if (llg_wave_model_init(1) != 0) goto done;
    for (uint32_t i = 0; i < ORDERED_COUNT; i++) {
        snprintf(name, sizeof name, "top\037v%02" PRIu32, (uint32_t)i);
        if (llg_wave_register_sv4(name, &values[i], 8) != 0) goto done;
    }
    if (llg_wave_register_real("top\037r", &real_value) != 0) goto done;
    llg_wave_file(path, 0);
    llg_wave_dumpvars(0);
    llg_wave_slot_end(0);
    used += (size_t)snprintf(expected + used, sizeof expected - used, "#1\n");
    for (uint32_t i = ORDERED_COUNT; i-- > 0;) {
        sv4_replace(&values[i], SV4_C(i + 1u, 8));
        llg_wave_changed_sv4(&values[i]);
    }
    for (uint32_t i = 0; i < ORDERED_COUNT; i++) {
        uint32_t v = i + 1u;
        used += (size_t)snprintf(expected + used, sizeof expected - used,
                                 "b%" PRIu32 "%" PRIu32 "%" PRIu32 "%" PRIu32 "%" PRIu32 "%" PRIu32 "%" PRIu32 "%" PRIu32 " %c\n", (v >> 7) & 1u, (v >> 6) & 1u,
                                 (v >> 5) & 1u, (v >> 4) & 1u, (v >> 3) & 1u, (v >> 2) & 1u,
                                 (v >> 1) & 1u, v & 1u, (char)('!' + i));
    }
    llg_wave_slot_end(1);
    sv4_replace(&values[3], SV4_C(99, 8));
    llg_wave_changed_sv4(&values[3]);
    sv4_replace(&values[3], SV4_C(4, 8));
    llg_wave_changed_sv4(&values[3]);
    llg_wave_slot_end(2);
    real_value = 1.5;
    llg_wave_changed_real(&real_value);
    real_value = 0.5;
    llg_wave_changed_real(&real_value);
    llg_wave_slot_end(3);
    sv4_replace(&values[0], SV4_C(0x80, 8));
    llg_wave_changed_sv4(&values[0]);
    llg_wave_slot_end(4);
    if (llg_wave_close(4) != 0) goto done;
    ok = file_contains(path, expected) && !file_contains(path, "#2\n") &&
         !file_contains(path, "#3\n") && file_contains(path, "#4\nb10000000 !\n");
done:
    for (uint32_t i = 0; i < ORDERED_COUNT; i++) sv4_destroy(&values[i]);
    (void)remove(path);
    return ok;
}
#endif

int main(void) {
    const char* vcd_path = "llg_wave_selftest.vcd";
    const char* selected_vcd_path = "llg_wave_selection_selftest.vcd";
    const char* fst_path = "llg_wave_selftest.FST";
    sv4_t packed = SV4_X(8);
    sv4_t punctuated = SV4_C(0, 1);
    sv4_t underscored = SV4_C(1, 1);
    sv4_t escaped_dot = SV4_C(0, 1);
    sv4_t mem_three = SV4_C(3, 8);
    sv4_t mem_two = SV4_C(2, 8);
    sv4_t write_value = SV4_C(0x55, 8);
    void* reader = NULL;
    int32_t result = 1;
    double real_value = 1.25;

    llg_rt_init();
    CHECK(llg_wave_model_init(10) == 0);
    CHECK(llg_wave_register_sv4("top\037unit\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    CHECK(llg_wave_register_real("top\037real_value", &real_value) == 0);
    CHECK(llg_wave_register_sv4("top\037a-b", &punctuated, 1) == 0);
    CHECK(llg_wave_register_sv4("top\037a_b", &underscored, 1) == 0);
    CHECK(llg_wave_register_sv4("top\037a.b", &escaped_dot, 1) == 0);
    llg_wave_file(vcd_path, 0);
    llg_wave_dumpvars(0);
    llg_ba(&packed, write_value);
    llg_ba(&packed, write_value); // equality-suppressed runtime write
    llg_ba_d(&real_value, 2.0);
    llg_ba_d(&real_value, 2.0); // bitwise-equal real write is suppressed
    // The dump starts at the end of the $dumpvars time step.
    llg_wave_slot_end(0);
    // More than two queue capacities forces the producer through the bounded
    // full-ring wait while preserving every committed event. Repeated
    // changes within one step are recorded once.
    for (uint64_t now = 1; now <= 2500; now++) {
        sv4_replace(&packed, SV4_C(now, 8));
        llg_wave_changed_sv4(&packed);
        llg_wave_changed_sv4(&packed);
        llg_wave_slot_end(now);
    }
    real_value = 2.5;
    llg_wave_changed_real(&real_value);
    llg_wave_flush(2500);
    // FLUSH is an acknowledgement barrier: the file is readable through the
    // last prior event before close joins the worker.
    CHECK(file_contains(vcd_path, "$timescale 10fs $end"));
    CHECK(file_contains(vcd_path, "$scope module top $end"));
    CHECK(file_contains(vcd_path, "$scope module unit $end"));
    CHECK(file_contains(vcd_path, "a$2Db $end"));
    CHECK(file_contains(vcd_path, "a_b $end"));
    CHECK(file_contains(vcd_path, "a$2Eb $end"));
    CHECK(!file_contains(vcd_path, "$scope module a $end"));
    CHECK(file_contains(vcd_path, "#2500\n"));
    CHECK(llg_wave_close(2500) == 0);
    CHECK(file_contains(vcd_path, "$enddefinitions $end"));

    // Selection is applied before the lazy header is emitted.  This keeps
    // excluded storage out of both VCD hierarchy and value records while
    // retaining declared array indices and pointer aliases.
    const char* selections[] = {"top\037mem", "top\037alias"};
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037value", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037mem[3]", &mem_three, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037mem[2]", &mem_two, 8) == 0);
    llg_wave_file(selected_vcd_path, 0);
    llg_wave_dumpvars_select(0, 0, selections, 2);
    llg_wave_slot_end(0);
    llg_wave_flush(0);
    CHECK(file_contains(selected_vcd_path, "alias $end"));
    CHECK(!file_contains(selected_vcd_path, "value $end"));
    CHECK(file_contains(selected_vcd_path, "mem$5B3$5D $end"));
    CHECK(file_contains(selected_vcd_path, "mem$5B2$5D $end"));
    CHECK(llg_wave_close(0) == 0);

#ifndef LLG_WAVE_DELTAS
    CHECK(restored_and_ordered("llg_wave_restore_selftest.vcd"));
#endif

    sv4_replace(&packed, SV4_C(0, 8));
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    llg_wave_file(fst_path, 0);
    llg_wave_dumpvars(0);
    llg_wave_slot_end(0);
    sv4_replace(&packed, SV4_C(0xa5, 8));
    llg_wave_changed_sv4(&packed);
    llg_wave_slot_end(7);
    llg_wave_off(8);
    llg_wave_slot_end(8);
    llg_wave_on(9);
    CHECK(llg_wave_close(9) == 0);

    reader = fstReaderOpen(fst_path);
    CHECK(reader != NULL);
    CHECK(fstReaderGetMaxHandle(reader) == 1);
    CHECK(fstReaderGetVarCount(reader) == 2);
    CHECK(fstReaderGetEndTime(reader) == 9);
    result = 0;
cleanup:
    if (reader) fstReaderClose(reader);
    if (llg_wave_close(9) != 0) result = 1;
    llg_rt_cleanup();
    sv4_destroy(&write_value);
    sv4_destroy(&mem_two);
    sv4_destroy(&mem_three);
    sv4_destroy(&escaped_dot);
    sv4_destroy(&underscored);
    sv4_destroy(&punctuated);
    sv4_destroy(&packed);
#ifdef LLG_SELFTEST_TRACK_STORAGE
    if (value_test_live() || value_test_bytes()) {
        selftest_report(stderr, LLG_SEVERITY_ERROR, 0, "wave selftest leaked packed storage");
        result = 1;
    }
#endif

    (void)remove(vcd_path);
    (void)remove(selected_vcd_path);
    (void)remove(fst_path);
    if (!result) selftest_report(stdout, LLG_SEVERITY_INFO, 0, "llg waveform selftest: OK");
    return result;
}
