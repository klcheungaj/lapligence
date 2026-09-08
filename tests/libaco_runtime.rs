//! Standalone regressions for the vendored coroutine stack implementation.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

const PROBE: &str = r#"
#include "aco.h"
#include <assert.h>
#include <string.h>

static volatile unsigned bad_index = 32;
static int overflow;

static void first(void) {
    volatile unsigned char small[32];
    volatile unsigned char bytes[2048];
    for (unsigned i = 0; i < sizeof(bytes); ++i) bytes[i] = (unsigned char)i;
    small[0] = 17;
    for (unsigned round = 0; round < 3; ++round) {
        aco_yield();
        assert(small[0] == 17);
        for (unsigned i = 0; i < sizeof(bytes); ++i)
            assert(bytes[i] == (unsigned char)i);
    }
    if (overflow) small[bad_index] = 1;
    aco_exit();
}

static void second(void) {
    volatile unsigned char bytes[4096];
    for (unsigned i = 0; i < sizeof(bytes); ++i) bytes[i] = (unsigned char)(i + 7);
    for (unsigned round = 0; round < 3; ++round) {
        aco_yield();
        for (unsigned i = 0; i < sizeof(bytes); ++i)
            assert(bytes[i] == (unsigned char)(i + 7));
    }
    aco_exit();
}

int main(int argc, char** argv) {
    overflow = argc > 1 && strcmp(argv[1], "--overflow") == 0;
    int large = argc > 1 && strcmp(argv[1], "--large") == 0;
    aco_thread_init(NULL);
    aco_t* main_co = aco_create(NULL, NULL, 0, NULL, NULL);
    aco_share_stack_t* stack = aco_share_stack_new(large ? (size_t)1 << 40 : 0);
    if (argc > 1 && strcmp(argv[1], "--guard") == 0)
        *(volatile unsigned char*)stack->real_ptr = 1;
    aco_t* a = aco_create(main_co, stack, 0, first, NULL);
    aco_t* b = aco_create(main_co, stack, 0, second, NULL);
    for (unsigned round = 0; round < 4; ++round) {
        aco_resume(a);
        aco_resume(b);
    }
    assert(a->is_end && b->is_end);
    aco_destroy(a);
    aco_destroy(b);
    aco_share_stack_destroy(stack);
    aco_destroy(main_co);
    return 0;
}
"#;

fn build_probe(dir: &Path, sanitize: bool) -> std::path::PathBuf {
    let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/libaco");
    let source = dir.join("probe.c");
    let executable = dir.join("probe");
    std::fs::write(&source, PROBE).expect("write coroutine probe");
    let mut command = Command::new("gcc");
    command.args(["-std=c11", "-O3", "-g", "-fno-omit-frame-pointer"]);
    if sanitize {
        command.args(["-DACO_USE_ASAN", "-fsanitize=address"]);
    }
    let output = command
        .arg("-I")
        .arg(&vendor)
        .arg(&source)
        .arg(vendor.join("aco.c"))
        .arg(vendor.join("acosw.S"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("gcc is required for the Linux coroutine regression probes");
    assert!(
        output.status.success(),
        "coroutine probe build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

#[test]
fn shared_stack_preserves_values_and_asan_redzones() {
    let dir = sim_harness::TempDir::new("aco-asan").expect("temporary directory");
    let executable = build_probe(dir.path(), true);
    for fake_stack in [0, 1] {
        for overflow in [false, true] {
            let mut command = Command::new(&executable);
            command.env(
                "ASAN_OPTIONS",
                format!("detect_leaks=1:detect_stack_use_after_return={fake_stack}"),
            );
            if overflow {
                command.arg("--overflow");
            }
            let output = sim_harness::run_command(&mut command, Duration::from_secs(10))
                .expect("run coroutine probe");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.success(), !overflow, "{stderr}");
            if overflow {
                assert!(
                    stderr.contains("AddressSanitizer: stack-buffer-overflow"),
                    "{stderr}"
                );
            }
        }
    }
}

#[test]
fn large_stack_reserves_address_space_and_keeps_guard_page() {
    use std::os::unix::process::ExitStatusExt;

    if std::fs::read_to_string("/proc/sys/vm/overcommit_memory")
        .expect("read Linux overcommit policy")
        .trim()
        == "2"
    {
        eprintln!("SKIP: Linux strict overcommit ignores MAP_NORESERVE");
        return;
    }
    let dir = sim_harness::TempDir::new("aco-reserve").expect("temporary directory");
    let executable = build_probe(dir.path(), false);
    let output = sim_harness::run_command(
        Command::new(&executable).arg("--large"),
        Duration::from_secs(10),
    )
    .expect("run sparse stack probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = sim_harness::run_command(
        Command::new(&executable).arg("--guard"),
        Duration::from_secs(10),
    )
    .expect("run guard page probe");
    assert_eq!(
        output.status.signal(),
        Some(libc::SIGSEGV),
        "guard page must fault"
    );
}
