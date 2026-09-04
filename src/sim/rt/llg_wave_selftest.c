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
        return 1; \
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
    const char* fst_path = "llg_wave_selftest.FST";
    sv4_t packed = SV4_X(8);
    sv4_t punctuated = SV4_C(0, 1);
    sv4_t underscored = SV4_C(1, 1);
    sv4_t escaped_dot = SV4_C(0, 1);
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
    llg_ba(&packed, SV4_C(0x55, 8));
    llg_ba(&packed, SV4_C(0x55, 8)); // equality-suppressed runtime write
    llg_ba_d(&real_value, 2.0);
    llg_ba_d(&real_value, 2.0); // bitwise-equal real write is suppressed
    // More than two queue capacities forces the producer through the bounded
    // full-ring wait while preserving every committed event.
    for (uint64_t now = 1; now <= 2500; now++) {
        packed = SV4_C(now, 8);
        llg_wave_changed_sv4(&packed, &packed, now);
    }
    real_value = 2.5;
    llg_wave_changed_real(&real_value, real_value, 2500);
    llg_wave_flush(2500);
    // FLUSH is an acknowledgement barrier: the file is readable through the
    // last prior event before close joins the worker.
    CHECK(file_contains(vcd_path, "$timescale 10ps $end"));
    CHECK(file_contains(vcd_path, "$scope module top $end"));
    CHECK(file_contains(vcd_path, "$scope module unit $end"));
    CHECK(file_contains(vcd_path, "a$2Db $end"));
    CHECK(file_contains(vcd_path, "a_b $end"));
    CHECK(file_contains(vcd_path, "a$2Eb $end"));
    CHECK(!file_contains(vcd_path, "$scope module a $end"));
    CHECK(file_contains(vcd_path, "#2500\n"));
    CHECK(llg_wave_close(2500) == 0);
    CHECK(file_contains(vcd_path, "$enddefinitions $end"));

    packed = SV4_C(0, 8);
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    llg_wave_file(fst_path, 0);
    llg_wave_dumpvars(0);
    packed = SV4_C(0xa5, 8);
    llg_wave_changed_sv4(&packed, &packed, 7);
    llg_wave_off(8);
    llg_wave_on(9);
    CHECK(llg_wave_close(9) == 0);

    void* reader = fstReaderOpen(fst_path);
    CHECK(reader != NULL);
    CHECK(fstReaderGetMaxHandle(reader) == 1);
    CHECK(fstReaderGetVarCount(reader) == 2);
    CHECK(fstReaderGetEndTime(reader) == 9);
    fstReaderClose(reader);
    llg_rt_cleanup();

    (void)remove(vcd_path);
    (void)remove(fst_path);
    puts("llg waveform selftest: OK");
    return 0;
}
