#include "llg_rt.h"
#include "test_value_temporaries.h"
#include <stdio.h>
#include <string.h>

static int fail(const char* what) {
    fprintf(stderr, "file probe: %s\n", what);
    return 1;
}

static int check_file_io(void) {
    llg_rt_init();
    if (llg_file_descriptor(test_value(sv4_from_u64(1, 32, 1))) != 1u) return fail("stdout mask");
    if (llg_file_descriptor(test_value(sv4_from_u64(0x80000002u, 32, 1))) != 0x80000002u)
        return fail("signed standard stderr FD");
    if (llg_file_descriptor(test_value(sv4_from_u64(0, 32, 1))) != 0u) return fail("zero accepted");
    if (llg_file_descriptor(test_value(sv4_fill(2, 32, 1))) != 0u) return fail("unknown accepted");

    uint32_t descriptor = llg_file_open(
        llg_string_bytes("boundary.txt", 12), llg_string_bytes("w+", 2), 1);
    if (!(descriptor & 0x80000000u)) return fail("ordinary FD tag");
    llg_fmt_arg_t arg = { LLG_FMT_PACKED, 0, { .packed = sv4_from_u64(7, 32, 1) } };
    llg_file_display_typed(descriptor, "probe=%0d", &arg, 1, "probe", 1);
    if (llg_file_flush(descriptor, 0) != 0) return fail("flush");
    if (llg_file_tell(descriptor) != 8) return fail("tell");
    if (llg_file_seek(descriptor, test_value(sv4_from_u64(0, 32, 1)),
                      test_value(sv4_from_u64(0, 32, 1))) != 0) return fail("seek");
    if (llg_file_tell(descriptor) != 0) return fail("seek position");
    if (llg_file_eof(descriptor) != 0) return fail("initial eof");
    if (llg_file_error(descriptor, NULL) != 0) return fail("unexpected error");

    uint32_t descriptors[30];
    char path[32];
    for (int i = 0; i < 30; i++) {
        int length = snprintf(path, sizeof(path), "slot-%d.txt", i);
        descriptors[i] = llg_file_open(
            llg_string_bytes(path, (size_t)length), llg_string_bytes("", 0), 0);
        if (descriptors[i] != (1u << (unsigned)(i + 1))) return fail("descriptor boundary");
    }
    if (llg_file_open(llg_string_bytes("full.txt", 8), llg_string_bytes("", 0), 0) != 0u)
        return fail("full table accepted");
    if (arg.value.packed.bits != NULL) return fail("formatter did not consume packed argument");
    arg.value.packed = sv4_from_u64(7, 32, 1);
    llg_file_display_typed(descriptors[0] | 1u, "probe=%0d", &arg, 1, "probe", 1);
    for (int i = 0; i < 30; i++) llg_file_close(descriptors[i]);
    llg_file_close(descriptor);
    if (llg_file_error(descriptor, NULL) != 1) return fail("closed descriptor status");
    llg_rt_cleanup();
    return 0;
}

int main(void) {
    if (atexit(test_values_clear) != 0) return 2;
    return test_values_run(check_file_io);
}
