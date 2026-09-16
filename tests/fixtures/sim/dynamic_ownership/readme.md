# Dynamic-owner HDL acceptance

These are positive P07 acceptance fixtures, not evidence that the partial P05
emitter already supports them. `tests/sim_dynamic_ownership.rs` launches each
file through the public `llg` executable in optimized and `--no-opt` modes,
requires CMake, isolates working directories, bounds child execution, and
compares independent exact output. A migration rejection is a failure, not a
skip or an expected-success substitute.

| Fixture | Required result / ownership boundary |
| --- | --- |
| `numeric_loop.sv` | 1,000 owned function results; `1007` |
| `branch_side_effects.sv` | Skipped logical/conditional arms do not call the function; equal Z arms merge without losing Z; `23 0`, then `1` |
| `wide_intermediate.sv` | A 7-bit source produces a 7,168-bit expression containing 4,096 ones |
| `mixed_width_loop.sv` | Repeated narrow temporaries alongside one 65,537-bit cell; `8192 1` |
| `selected_nba_capture.sv` | A pending selected write retains the original RHS despite source mutation; `0`, then `42` |
| `recursive_return.sv` | Independent recursive results and early-return cleanup; `2176` |
| `yielding_task.sv` | Automatic task locals and output copy-out across a wait; `47` |
| `finish_cleanup.sv` | Nonreturning finish with another live process; `finish` |

These output checks alone do not establish zero leaks. Pair them with allocation
measurement and supported sanitizer configurations. Real coroutine ASan coverage
remains a separate gate; do not infer it from the sanitizer-safe component suite.

Run: `cargo test --locked --no-default-features --test sim_dynamic_ownership`.
The host-lifecycle IR test is separately opted in with
`cargo test --locked --lib --no-default-features structured_owned_model_ -- --ignored`.
