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

The review-correction fixtures additionally cover:

| Fixture | Required result / ownership boundary |
| --- | --- |
| `function_numeric_input.sv` | Runtime numeric input conversion; `42` |
| `numeric_default_argument.sv` | Default referring to an earlier stable numeric argument; `42` |
| `numeric_inout_argument.sv` | Numeric inout copy-in and output copy-out across a delay; `42 43` |
| `event_array_owners.sv` | Indexed named-event wait; an X-index waiter stays dormant; `event` |
| `evaluated_event_owners.sv` | Evaluated arithmetic event and `iff` qualification; `qualified` |
| `captured_fork_owners.sv` | Detached numeric automatic captures preserve `0`, `1`, `2` |

These output checks alone do not establish zero leaks. Pair them with allocation
measurement and supported sanitizer configurations. Real coroutine ASan coverage
remains a separate gate; do not infer it from the sanitizer-safe component suite.

Run: `cargo test --locked --no-default-features --test sim_dynamic_ownership`.
These fixtures terminate with `$finish(0)` because their exact stderr contract
excludes informational termination messages; runtime diagnostics are not filtered.
The active host-lifecycle IR tests can also be selected directly with
`cargo test --locked --lib --no-default-features structured_owned_model_`.
