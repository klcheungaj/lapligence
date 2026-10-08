//! Standalone tests for the IEEE Annex N legacy random runtime.

use crate::{c_compiler, sim_harness};

use std::time::Duration;

const RANDOM_PROBE: &str = r#"
#include "llg_random.h"

#include <stdint.h>
#include <stdio.h>

static int check_vector(void) {
    int32_t seed = 1;
    int32_t r0 = llg_random_next(&seed);
    int32_t r1 = llg_random_next(&seed);
    int32_t r2 = llg_random_next(&seed);
    if (r0 != -2147414528 || r1 != -1671855048 || r2 != 1129920902 ||
        seed != -1017563188)
        return 1;

    seed = 1;
    if (llg_dist_uniform(&seed, -2, 2) != -2 ||
        llg_dist_uniform(&seed, -2, 2) != -2 ||
        llg_dist_uniform(&seed, -2, 2) != 1 ||
        seed != -1017563188)
        return 2;

    seed = 10;
    if (llg_dist_normal(&seed, 10, 2) != 10 ||
        llg_dist_normal(&seed, 10, 2) != 9 || seed != -977101388)
        return 3;
    seed = 10;
    if (llg_dist_exponential(&seed, 5) != 44 ||
        llg_dist_exponential(&seed, 5) != 11 || seed != 460696424)
        return 4;
    seed = 10;
    if (llg_dist_poisson(&seed, 10) != 1 ||
        llg_dist_poisson(&seed, 10) != 13 || seed != 849187386)
        return 5;
    seed = 10;
    if (llg_dist_chi_square(&seed, 5) != 2 ||
        llg_dist_chi_square(&seed, 5) != 2 || seed != -351915328)
        return 6;
    seed = 10;
    if (llg_dist_t(&seed, 5) != 1 || llg_dist_t(&seed, 5) != 1 ||
        seed != 849187386)
        return 7;
    seed = 10;
    if (llg_dist_erlang(&seed, 2, 10) != 55 ||
        llg_dist_erlang(&seed, 2, 10) != 3 || seed != -291802762)
        return 8;
    return 0;
}

static int check_boundaries(void) {
    int32_t seed = 7;
    if (llg_dist_uniform(NULL, 1, 2) != 0) return 1;
    if (llg_dist_uniform(&seed, 3, 3) != 3 || seed != 7) return 2;
    if (llg_dist_uniform(&seed, 3, -3) != 3 || seed != 7) return 3;
    seed = 1;
    if (llg_dist_uniform(&seed, INT32_MAX - 1, INT32_MAX) != INT32_MAX - 1 ||
        seed != 69070)
        return 4;
    seed = 1;
    if (llg_dist_uniform(&seed, INT32_MIN, INT32_MIN + 1) != INT32_MIN ||
        seed != 69070)
        return 5;
    seed = 0;
    if (llg_random_next(&seed) != 303379748 || seed != -1844104698) return 6;
    seed = 7;
    if (llg_dist_exponential(&seed, 0) != 0 || seed != 7) return 7;
    if (llg_dist_poisson(&seed, -1) != 0 || seed != 7) return 8;
    if (llg_dist_chi_square(&seed, 0) != 0 || seed != 7) return 9;
    if (llg_dist_t(&seed, -1) != 0 || seed != 7) return 10;
    if (llg_dist_erlang(&seed, 0, 1) != 0 || seed != 7) return 11;
    return 0;
}

int main(void) {
    int vector = check_vector();
    int boundaries = check_boundaries();
    printf("vector=%d boundaries=%d\n", vector, boundaries);
    return vector || boundaries;
}
"#;

#[test]
fn random_runtime_vectors_and_boundaries_are_stable_at_both_optimization_levels() {
    let compiler = c_compiler::host_c_compiler();
    if !c_compiler::c_compiler_available(&compiler) {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let (header, implementation) = llg::sim::rt::random_sources();
    for (optimization, level) in [
        ("-O2", c_compiler::Optimize::Speed),
        ("-O0", c_compiler::Optimize::None),
    ] {
        let dir = sim_harness::TempDir::new(&format!("runtime-random-{optimization}"))
            .expect("create random runtime directory");
        std::fs::write(dir.path().join("llg_random.h"), header).expect("write random header");
        std::fs::write(dir.path().join("llg_random.c"), implementation)
            .expect("write random implementation");
        std::fs::write(dir.path().join("random_probe.c"), RANDOM_PROBE)
            .expect("write random probe");
        let (mut command, executable) = c_compiler::strict_c11_executable(
            &compiler,
            dir.path(),
            level,
            &["llg_random.c", "random_probe.c"],
            "random_probe",
        );
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .unwrap_or_else(|error| panic!("compile random runtime at {optimization}: {error}"));
        assert!(
            output.status.success(),
            "random runtime must compile at {optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = sim_harness::run_executable(&executable).expect("run random probe");
        assert_eq!(output, "vector=0 boundaries=0\n", "{optimization}");
    }
}
