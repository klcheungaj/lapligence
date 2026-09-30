# Task lowering regressions

`tests/sim_task_lowering.rs` runs these fixtures through the public CLI in both
optimizer modes. The oracles follow SystemVerilog-2009 generate elaboration
(§27), task lifetime and argument passing (§13), parallel blocks (§9.3.2), and
named-event identity and persistent `triggered` state (§15.5).

| Fixture | Expected behavior |
| --- | --- |
| `generated_instance_timing_task.sv` | Four independent workers add their IDs twice through timed automatic/static tasks and untimed task/function chains. Nested generate instances and hierarchical calls keep their own parameters and state. |
| `fork_event_formal.sv` | The event waiter wakes at time 2; the joined delay branch already completed at time 1. |
| `fork_event_variants.sv` | Weights 1, 10, 100, 1000 and 10000 contribute once to waits, triggered checks and forwarded calls. Nested forks contribute once more for the two specialized calls, giving 22111. `join_any` returns at time 1, `join_none` at time 0; rebinding the actual afterwards preserves input-formal event identity. Synchronously joined siblings share a rebinding of their formal handle. |
