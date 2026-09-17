// llg_wave_selftest.c — bounded queue, flush barrier, VCD, and FST smoke test.
#define LLG_WAVEFORM 1

#include "llg_wave.h"
#include "fstapi.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(expr) do { \
    if (!(expr)) { \
        fprintf(stderr, "wave selftest failed at line %d: %s\n", __LINE__, #expr); \
        goto cleanup; \
    } \
} while (0)

static int file_contains(const char* path, const char* needle) {
    FILE* file = fopen(path, "rb");
    if (!file) return 0;
    if (fseek(file, 0, SEEK_END) != 0) {
        fclose(file);
        return 0;
    }
    long size = ftell(file);
    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {
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
    int found = strstr(text, needle) != NULL;
    free(text);
    return found;
}

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
    int result = 1;
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
    // More than two queue capacities forces the producer through the bounded
    // full-ring wait while preserving every committed event.
    for (uint64_t now = 1; now <= 2500; now++) {
        sv4_replace(&packed, SV4_C(now, 8));
        llg_wave_changed_sv4(&packed, &packed, now);
    }
    real_value = 2.5;
    llg_wave_changed_real(&real_value, real_value, 2500);
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
    llg_wave_flush(0);
    CHECK(file_contains(selected_vcd_path, "alias $end"));
    CHECK(!file_contains(selected_vcd_path, "value $end"));
    CHECK(file_contains(selected_vcd_path, "mem$5B3$5D $end"));
    CHECK(file_contains(selected_vcd_path, "mem$5B2$5D $end"));
    CHECK(llg_wave_close(0) == 0);

    sv4_replace(&packed, SV4_C(0, 8));
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    llg_wave_file(fst_path, 0);
    llg_wave_dumpvars(0);
    sv4_replace(&packed, SV4_C(0xa5, 8));
    llg_wave_changed_sv4(&packed, &packed, 7);
    llg_wave_off(8);
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
    extern size_t value_test_live(void);
    extern size_t value_test_bytes(void);
    if (value_test_live() || value_test_bytes()) {
        fprintf(stderr, "wave selftest leaked packed storage\n");
        result = 1;
    }
#endif

    (void)remove(vcd_path);
    (void)remove(selected_vcd_path);
    (void)remove(fst_path);
    if (!result) puts("llg waveform selftest: OK");
    return result;
}
