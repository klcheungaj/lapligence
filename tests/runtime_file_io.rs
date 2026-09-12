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
    if (llg_file_descriptor(sv4_from_u64(2, 32, 1)) != 2u) return fail("stderr mask");
    if (llg_file_descriptor(sv4_from_u64(0, 32, 1)) != 0u) return fail("zero accepted");
    if (llg_file_descriptor(sv4_fill(2, 32, 1)) != 0u) return fail("unknown accepted");

    uint32_t descriptor = llg_file_open(
        llg_string_bytes("boundary.txt", 12), llg_string_bytes("w+", 2), 1);
    if (descriptor != 4u) return fail("first ordinary descriptor");
    llg_fmt_arg_t arg = { LLG_FMT_PACKED, { .packed = sv4_from_u64(7, 32, 1) } };
    llg_file_display_typed(descriptor | 1u, "probe=%0d", &arg, 1, "probe", 1);
    if (llg_file_flush(descriptor, 0) != 0) return fail("flush");
    if (llg_file_tell(descriptor) != 8) return fail("tell");
    if (llg_file_seek(descriptor, sv4_from_u64(0, 32, 1),
                      sv4_from_u64(0, 32, 1)) != 0) return fail("seek");
    if (llg_file_tell(descriptor) != 0) return fail("seek position");
    if (llg_file_eof(descriptor) != 0) return fail("initial eof");
    if (llg_file_error(descriptor, NULL) != 0) return fail("unexpected error");

    uint32_t descriptors[30];
    descriptors[0] = descriptor;
    char path[32];
    for (int i = 1; i < 30; i++) {
        int length = snprintf(path, sizeof(path), "slot-%d.txt", i);
        descriptors[i] = llg_file_open(
            llg_string_bytes(path, (size_t)length), llg_string_bytes("", 0), 0);
        if (descriptors[i] != (1u << (unsigned)(i + 2))) return fail("descriptor boundary");
    }
    if (llg_file_open(llg_string_bytes("full.txt", 8), llg_string_bytes("", 0), 0) != 0u)
        return fail("full table accepted");
    for (int i = 0; i < 30; i++) llg_file_close(descriptors[i]);
    if (llg_file_error(descriptor, NULL) != 1) return fail("closed descriptor status");
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
