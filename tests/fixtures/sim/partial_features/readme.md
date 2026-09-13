# Simulator partial-feature regressions

These designs exercise corrected port, event, timing, packed-selection, math,
immediate assertion, and gated host-command behavior through the simulator executable. Every
positive case runs with optimization enabled and disabled; expected results and
test names live in
[the Rust suite](../../../sim_partial_features.rs). `event_effectful_*.sv` are
intentional rejection cases.

From the repository root:

```sh
cargo test --test sim_partial_features -- --test-threads=1
cargo run --bin llg -- --top tb tests/fixtures/sim/partial_features/delayed_nba.sv
```

Use any fixture filename from this directory in the last command; add
`--no-opt` to disable optimization. CMake and a C compiler are required.
See the [remaining feature inventory](../../../../docs/sim_features.md)
for the boundaries these tests do not cover.

## Cases

| Files | Behavior checked |
|---|---|
| `port_*.sv` | Constants, omitted/default inputs, expression dependencies, conversion, selected outputs, and live string value links |
| `reference_*.sv` | Nested packed aliases, selected lvalues, recursive aggregate/object identity, initialization and edge visibility |
| `reference_resizable_rejected.sv` | Explicit rejection of detached/resizable reference storage |
| `event_*.sv`, `mixed_iff_events.sv`, `nonblocking_event_triggers.sv`, `nonblocking_event_repeat_dynamic.sv` | Trigger-time qualification, expression changes, LSB/four-state edges, waiter cleanup, event-handle identity, same-slot `.triggered`, ordered waits, deferred named-event triggers and source-located detached-count rejection |
| `intra_assignment_events.sv` | Blocking and nonblocking event-controlled assignments, RHS capture with blocking update-time versus NBA issue-time selectors, repeat controls and zero/X/Z/negative repeat counts |
| `intra_assignment_event_sources.sv` | Event-list edges, `iff` qualification and automatic function dependencies in an intra-assignment event control |
| `intra_assignment_event_real_repeat.sv` | Single-fault rejection of unsupported real-valued repeat counts |
| `event_activation_capture.sv`, `event_pure_functions.sv` | Per-activation locals/formals, trigger-time `iff`, legal input/const-ref function calls and expression-only dependencies |
| `wait_constant_false.sv` | False and unknown waits suspend without preventing time advancement |
| `delayed_nba*.sv`, `nba_*.sv` | Value/index capture, future commit, issue ordering, disjoint selections and state conversion |
| `activation_frames.sv` | Reentrant automatic subroutines, per-iteration loop captures, shadowed declarations and retained fork activations |
| `real_activation_capture.sv` | Typed retained-frame capture of automatic real locals after the declaring block continues |
| `inertial_*.sv` | Pulse cancellation, captured driver values, unchanged deadlines, scalar/net strengths, region settling, module precision, lifetime, overflow, vector transition selection and per-element array cancellation |
| `dynamic_delay_*.sv`, `blocking_real_delay.sv`, `unknown_delay_zero.sv`, `negative_*delay*.sv` | Runtime delay capture, module precision, real blocking captures, X/Z and negative packed delays, overflow diagnostics |
| `*select_ranges.sv` | Ascending/nonzero ranges, array-element selection and invalid indices |
| `array_indexed_*.sv` | Multidimensional element indexed selections, two-state/real conversion, limb boundaries and captured NBA masks |
| `math_*.sv` | Runtime real math, numeric conversion, one-time argument evaluation and C domain results |
| `system_*.sv` | `$system` task/function command ownership, exact-once argument evaluation, omitted versus explicit-empty commands, permission denial, and malformed argument diagnostics |
| `real_sensitivity.sv` | Typed real/shortreal wait and event changes, combinational propagation through real ports, unchanged-write suppression, signed zero and NaN policy |
| `realtime_*.sv`, `time_query_*.sv` | Fractional time, rounded integer queries, mixed scopes, half-unit boundaries and `$stime` wrap |
| `clocking_h13.sv`, `clocking_h13_interface.sv`, `clocking_h13_virtual.sv` | Clocking input `#1step`, `#0` and positive skews, preponed/observed/history samples, independent/default/global blocks, event controls, aliases, concrete interface members and statically initialized virtual-interface handles |
| `timeformat_*.sv` | Design-wide `$timeformat` defaults and runtime arguments, `%t` precision/suffix/minimum-width conversion across display/write/strobe/monitor, and mixed timescale scopes |
| `clocking_h14*.sv` | Clocking output/inout synchronous drives, constant output skews and signal edge qualifiers, cycle-event waits on irregular clocks, selected targets, captured RHS/selectors, NBA/Re-NBA ordering and resolved inout conflicts |
| `time_literal_exact_2009.sv` | SystemVerilog 2009 local `timeunit`/`timeprecision`, signed/sub-femtosecond unit-suffixed literals, and exact femtosecond delay rounding |
| `finish_*.sv` | Nonreturning termination, pending-work discard, final-block boundary, forked-coroutine exit, diagnostic levels, and source-bearing argument rejection |
| `severity_*.sv` | Typed `$info/$warning/$error` diagnostics, `$fatal` continuation/termination/final behavior, stable counts, and finish-number validation |
| `assertions*.sv`, `deferred_assertions*.sv` | Immediate and deferred assert/assume/cover four-state truth, issue-time value and action-time reference captures, Reactive reports, same-slot glitch coalescing, module-level actions, defaults, labels and optimizer parity |
| `stop_*.sv` | Resumable nested-call suspension, retained future work/finals, explicit batch exit policy, and diagnostic levels |
