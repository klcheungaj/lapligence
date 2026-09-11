# Simulator partial-feature regressions

These designs exercise corrected port, event, timing, packed-selection and
math behavior through the simulator executable. Every positive case runs with
optimization enabled and disabled; expected results and test names live in
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
| `port_*.sv` | Constants, omitted/default inputs, expression dependencies, conversion and selected outputs |
| `reference_*.sv` | Nested packed reference aliases, initialization and edge visibility |
| `event_*.sv`, `mixed_iff_events.sv` | Trigger-time qualification, expression changes, LSB/four-state edges and waiter cleanup |
| `wait_constant_false.sv` | False and unknown waits suspend without preventing time advancement |
| `delayed_nba*.sv`, `nba_*.sv` | Value/index capture, future commit, issue ordering, disjoint selections and state conversion |
| `inertial_*.sv` | Pulse cancellation, captured driver values, unchanged deadlines, net strengths, region settling, module precision, lifetime, overflow and explicit multiple-delay rejection |
| `dynamic_delay_*.sv`, `blocking_real_delay.sv`, `unknown_delay_zero.sv`, `negative_*delay*.sv` | Runtime delay capture, module precision, real blocking captures, X/Z and negative packed delays, overflow diagnostics |
| `*select_ranges.sv` | Ascending/nonzero ranges, array-element selection and invalid indices |
| `array_indexed_*.sv` | Multidimensional element indexed selections, two-state/real conversion, limb boundaries and captured NBA masks |
| `math_*.sv` | Runtime real math, numeric conversion, one-time argument evaluation and C domain results |
| `realtime_*.sv` | Fractional time in each module's time unit |
