# Coroutine semantic oracles

These SystemVerilog-2009 fixtures pin observable behavior that must remain
unchanged when generated models move from stackful to stackless coroutines.
`tests/sim_coroutine_semantics.rs` runs each case through the public `llg` CLI
with default optimization and `--no-opt`, owns the exact stdout/stderr/status
expectation, and treats any frontend or lowering warning as a failure.

| Fixture | Contract |
| --- | --- |
| `scheduler_wake_order.sv` | Preserves llg reverse-registration signal wake order and the resulting Inactive FIFO order; a duplicated multi-signal control wakes once. |
| `scheduler_wait_cancel.sv` | Kill and named disable remove multi-signal waits before publication, while wait fork observes killed and surviving children. |
| `event_waits.sv` | OR event lists wake once; event aliases and `.triggered` retain identity; `wait_order` reports success and failure. |
| `intra_assignment_delay_capture.sv` | Blocking and nonblocking intra-assignment delays capture the RHS and apply selected LHS evaluation at their specified stages. |
| `nested_control_flow.sv` | Resume labels preserve nested branch and loop state across `for`, `while`, `repeat`, `forever`, `break`, and `continue`. |
| `nested_timing_calls.sv` | Both timed-call shapes suspend twice per level at depths one through eight, preserve locals and outputs, reuse the leaf shallow and deep, and retain a plain non-suspending call. |
| `cancellation_points.sv` | Disabling an outer block while a nested block waits skips the loop, inner and outer tails; a `disable` in one branch leaves at once; a timing-free task that disables its caller's block skips its copy-out and the rest of the block. |
| `copyback_once_cancel.sv` | Timed output/inout copy-back occurs once on return and not at all after cancellation. |
| `deinlined_tasks.sv` | Event formals name the caller's event object (bound at the call), a four-level event-task chain called from three sites keeps locals and outputs, `disable` of a suspended task skips its copy-out while its callers continue, and a timed task can disable its caller's block. |
| `ref_event_tasks.sv` | A `ref` formal read by an event control keeps the actual's dependencies through a three-level forwarding chain for two module signals, a caller local never wakes it, and a disabled task never observes a later edge. |
| `stackless_join_none_real_block.sv` | A completed timing call and an already-satisfied wait are not scheduling boundaries; a detached child starts at the next real block. |
| `deep_cancellation.sv` | Kill below three timed calls and named-block disable abandon the victim while independent work continues. |
| `self_suspend.sv` | Self-suspend preserves the continuation until another process resumes it; self-resume while running is a no-op. |
| `termination_depth.sv` | Default `$stop` resumes at task depth two; `$finish` from a plain function inside a timing task abandons callers and runs finals once. |
| `function_step_budget.sv` | A non-yielding loop in a plain function terminates with the exact process-budget diagnostic instead of hanging. |

The scheduler wake-order fixture deliberately pins llg's deterministic ordering
of otherwise unordered Active processes; it is a runtime regression oracle, not
a claim that IEEE specifies this order. Other fixtures use specified outcomes;
tests that characterize legal race sets assert membership in the documented set.
