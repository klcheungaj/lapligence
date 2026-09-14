//! Direct runtime checks for descriptor masks, ordinary-file ownership, and
//! invalid/closed descriptor status.

#[path = "support/sim.rs"]
mod sim_harness;

use std::fs;

use llg::sim;

const FILE_PROBE: &str = r#"
#include "llg_rt.h"
#include <stdio.h>
#include <string.h>

static int fail(const char* what) {
    fprintf(stderr, "file probe: %s\n", what);
    return 1;
}

int main(void) {
    llg_rt_init();
    if (llg_file_descriptor(sv4_from_u64(1, 32, 1)) != 1u) return fail("stdout mask");
    if (llg_file_descriptor(sv4_from_u64(0x80000002u, 32, 1)) != 0x80000002u)
        return fail("signed standard stderr FD");
    if (llg_file_descriptor(sv4_from_u64(0, 32, 1)) != 0u) return fail("zero accepted");
    if (llg_file_descriptor(sv4_fill(2, 32, 1)) != 0u) return fail("unknown accepted");

    uint32_t descriptor = llg_file_open(
        llg_string_bytes("boundary.txt", 12), llg_string_bytes("w+", 2), 1);
    if (!(descriptor & 0x80000000u)) return fail("ordinary FD tag");
    llg_fmt_arg_t arg = { LLG_FMT_PACKED, { .packed = sv4_from_u64(7, 32, 1) } };
    llg_file_display_typed(descriptor, "probe=%0d", &arg, 1, "probe", 1);
    if (llg_file_flush(descriptor, 0) != 0) return fail("flush");
    if (llg_file_tell(descriptor) != 8) return fail("tell");
    if (llg_file_seek(descriptor, sv4_from_u64(0, 32, 1),
                      sv4_from_u64(0, 32, 1)) != 0) return fail("seek");
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
    llg_file_display_typed(descriptors[0] | 1u, "probe=%0d", &arg, 1, "probe", 1);
    for (int i = 0; i < 30; i++) llg_file_close(descriptors[i]);
    llg_file_close(descriptor);
    if (llg_file_error(descriptor, NULL) != 1) return fail("closed descriptor status");
    llg_rt_cleanup();
    return 0;
}
"#;

const FILE_INPUT_PROBE: &str = r#"
#include "llg_rt.h"
#include <stdio.h>
#include <string.h>

static int fail(const char* what) {
    fprintf(stderr, "file input probe: %s\n", what);
    return 1;
}

static llg_ref_t whole(sv4_t* value) {
    llg_ref_t ref = {0};
    ref.base = value;
    ref.width = value->width;
    ref.is_signed = value->is_signed;
    ref.kind = LLG_REF_WHOLE;
    return ref;
}

int main(void) {
    llg_rt_init();
    FILE* host = fopen("input.txt", "wb");
    if (!host) return fail("create input");
    const unsigned char bytes[] = " 123 1x zed\nhello\n\n";
    if (fwrite(bytes, 1, sizeof(bytes) - 1, host) != sizeof(bytes) - 1) return fail("write input");
    fclose(host);
    uint32_t descriptor = llg_file_open(
        llg_string_bytes("input.txt", 9), llg_string_bytes("rb", 2), 1);
    if (!(descriptor & 0x80000000u)) return fail("open input FD tag");

    sv4_t number = sv4_x(32, 1);
    sv4_t mixed = sv4_x(16, 0);
    llg_string_t word = {0};
    llg_ref_t number_ref = whole(&number);
    llg_ref_t mixed_ref = whole(&mixed);
    llg_file_input_target_t targets[] = {
        { LLG_FILE_INPUT_PACKED, &number_ref, NULL, NULL },
        { LLG_FILE_INPUT_PACKED, &mixed_ref, NULL, NULL },
        { LLG_FILE_INPUT_STRING, NULL, NULL, &word },
    };
    int converted = llg_file_scanf(descriptor, "%d %4h %*s %s", targets, 3);
    if (converted != 3 || sv4_to_i64(number) != 123 ||
        !sv4_is_unknown(mixed) || word.len != 5 || memcmp(word.data, "hello", 5) != 0)
        return fail("formatted scan");
    if (llg_file_getc(descriptor) != '\n') return fail("line delimiter");
    if (llg_file_ungetc(descriptor, sv4_from_u64('A', 8, 0)) != 'A' ||
        llg_file_ungetc(descriptor, sv4_from_u64('B', 8, 0)) != 'B' ||
        llg_file_getc(descriptor) != 'B' || llg_file_getc(descriptor) != 'A')
        return fail("repeated ungetc");
    if (llg_file_gets(descriptor, &word) != 1 || word.len != 1 || word.data[0] != '\n')
        return fail("empty line");
    if (llg_file_getc(descriptor) != EOF || llg_file_eof(descriptor) != 1) return fail("eof");
    if (llg_file_scanf(descriptor, "%d", targets, 1) != -1)
        return fail("formatted eof result");
    if (llg_file_ungetc(descriptor, sv4_from_u64('C', 8, 0)) != 'C' ||
        llg_file_eof(descriptor) != 0 || llg_file_getc(descriptor) != 'C' ||
        llg_file_eof(descriptor) != 0)
        return fail("eof cleared by ungetc");

    const unsigned char source[] = { '4', '2', ' ', 'x', 'z' };
    sv4_t source_number = sv4_x(32, 1);
    sv4_t source_unknown = sv4_x(8, 0);
    llg_ref_t source_number_ref = whole(&source_number);
    llg_ref_t source_unknown_ref = whole(&source_unknown);
    llg_file_input_target_t source_targets[] = {
        { LLG_FILE_INPUT_PACKED, &source_number_ref, NULL, NULL },
        { LLG_FILE_INPUT_PACKED, &source_unknown_ref, NULL, NULL },
    };
    if (llg_string_scanf((const char*)source, sizeof(source), "%d %h",
                         source_targets, 2) != 2 || sv4_to_i64(source_number) != 42 ||
        !sv4_is_unknown(source_unknown)) return fail("string scan");
    const unsigned char invalid_source[] = { 'q' };
    if (llg_string_scanf((const char*)invalid_source, sizeof(invalid_source), "%d",
                         source_targets, 1) != 0 ||
        llg_string_scanf("", 0, "%d", source_targets, 1) != -1)
        return fail("formatted conversion eof boundary");
    sv4_t binary_number = sv4_x(8, 0);
    sv4_t octal_number = sv4_x(8, 0);
    sv4_t auto_number = sv4_x(16, 0);
    sv4_t unsigned_number = sv4_x(32, 0);
    double real_number = 0.0;
    llg_ref_t binary_ref = whole(&binary_number);
    llg_ref_t octal_ref = whole(&octal_number);
    llg_ref_t auto_ref = whole(&auto_number);
    llg_ref_t unsigned_ref = whole(&unsigned_number);
    llg_file_input_target_t numeric_targets[] = {
        { LLG_FILE_INPUT_PACKED, &binary_ref, NULL, NULL },
        { LLG_FILE_INPUT_PACKED, &octal_ref, NULL, NULL },
        { LLG_FILE_INPUT_PACKED, &auto_ref, NULL, NULL },
        { LLG_FILE_INPUT_PACKED, &unsigned_ref, NULL, NULL },
        { LLG_FILE_INPUT_REAL, NULL, &real_number, NULL },
    };
    const unsigned char numeric_source[] = "1010 17 0x2a 429 1.25";
    if (llg_string_scanf((const char*)numeric_source, sizeof(numeric_source) - 1,
                         "%b %o %i %u %f", numeric_targets, 5) != 5 ||
        sv4_to_u64(binary_number) != 10u || sv4_to_u64(octal_number) != 15u ||
        sv4_to_u64(auto_number) != 42u || sv4_to_u64(unsigned_number) != 429u ||
        real_number != 1.25)
        return fail("numeric format scan");
    const unsigned char embedded[] = { 'A', 0, 'B' };
    llg_string_t embedded_target = {0};
    llg_file_input_target_t embedded_target_desc = {
        LLG_FILE_INPUT_STRING, NULL, NULL, &embedded_target
    };
    if (llg_string_scanf((const char*)embedded, sizeof(embedded), "%2c",
                         &embedded_target_desc, 1) != 1 || embedded_target.len != 2 ||
        embedded_target.data[0] != 'A' || embedded_target.data[1] != 0)
        return fail("embedded character input");
    llg_string_destroy(&embedded_target);

    uint32_t binary = llg_file_open(
        llg_string_bytes("binary.bin", 10), llg_string_bytes("wb", 2), 1);
    if (binary == 0) return fail("open binary output");
    llg_file_close(binary);
    host = fopen("binary.bin", "wb");
    if (!host) return fail("create binary");
    const unsigned char binary_bytes[] = { 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc };
    if (fwrite(binary_bytes, 1, sizeof(binary_bytes), host) != sizeof(binary_bytes)) return fail("write binary");
    fclose(host);
    binary = llg_file_open(
        llg_string_bytes("binary.bin", 10), llg_string_bytes("rb", 2), 1);
    if (binary == 0) return fail("reopen binary");
    sv4_t wide = sv4_x(16, 0);
    llg_ref_t wide_ref = whole(&wide);
    if (llg_file_read_packed(binary, &wide_ref) != 2 || sv4_to_u64(wide) != 0x1234u)
        return fail("wide fread");
    sv4_t descending[4];
    for (int i = 0; i < 4; i++) descending[i] = sv4_x(8, 0);
    const int32_t descending_dims[] = { 3, 0 };
    if (llg_file_seek(binary, sv4_from_u64(0, 32, 1), sv4_from_u64(0, 32, 1)) != 0)
        return fail("descending seek");
    int descending_result = llg_file_read_array(binary, descending, 8, 0, 0, 4, descending_dims, 1,
                            1, sv4_from_u64(2, 32, 1), 1, sv4_from_u64(2, 32, 1));
    if (descending_result != 2 ||
        sv4_to_u64(descending[1]) != 0x12u || sv4_to_u64(descending[2]) != 0x34u ||
        !sv4_is_unknown(descending[0]) || !sv4_is_unknown(descending[3]))
        return fail("descending memory fread");
    sv4_t ascending[4];
    for (int i = 0; i < 4; i++) ascending[i] = sv4_x(8, 0);
    const int32_t ascending_dims[] = { 0, 3 };
    if (llg_file_seek(binary, sv4_from_u64(0, 32, 1), sv4_from_u64(0, 32, 1)) != 0 ||
        llg_file_read_array(binary, ascending, 8, 0, 0, 4, ascending_dims, 1,
                            0, sv4_from_u64(0, 1, 0), 0, sv4_from_u64(0, 1, 0)) != 4 ||
        sv4_to_u64(ascending[0]) != 0x12u || sv4_to_u64(ascending[3]) != 0x78u)
        return fail("ascending memory fread");

    if (llg_file_getc(0) != EOF) return fail("invalid descriptor input");
    llg_string_destroy(&word);
    llg_file_close(descriptor);
    llg_file_close(binary);
    llg_rt_cleanup();
    return 0;
}
"#;

#[test]
fn descriptor_masks_and_boundaries_are_portable() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("runtime-file-io", |dir| {
        fs::write(dir.join("runtime_file_io_probe.c"), FILE_PROBE)
            .map_err(|error| error.to_string())?;
        let executable =
            sim::build::build_model_cmake(dir, &[("runtime_file_io_probe.c", FILE_PROBE)])
                .map_err(|error| error.to_string())?;
        let output = sim_harness::run_executable_output(&executable)?;
        assert_eq!(output.stdout, b"probe=7\n");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    })
    .expect("runtime file descriptor probe");
}

#[test]
fn formatted_character_line_and_binary_input_are_portable() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("runtime-file-input", |dir| {
        let executable =
            sim::build::build_model_cmake(dir, &[("runtime_file_input_probe.c", FILE_INPUT_PROBE)])
                .map_err(|error| error.to_string())?;
        let output = sim_harness::run_executable_output(&executable)?;
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    })
    .expect("runtime file input probe");
}
