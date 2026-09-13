# Concurrent assertion fixtures

These fixtures exercise the H20 concurrent-assertion and H21 sampled-value
domains in both optimizer modes: one explicit packed signal clock, sampled
`|->`/`|=>` implications, Preponed sampling across NBA updates, asynchronous
single-signal `disable iff`, overlapping attempts, Reactive actions, vacuity
accounting, pending-attempt disposal at end of simulation, `##` concatenation,
all sequence repetition kinds including unbounded ranges, Boolean sequence
composition, and `first_match` endpoint selection. They also cover
explicit/default sampled clocks, initial/gated history, global-clock
status/history, and LSB/X/Z edge rules.

Match-item side effects, named property instances with formal bindings,
unsupported temporal property operators, assertion-control tasks, future
global sampled-value functions, complex sampled clock events, and sequence
`.matched` status remain intentionally rejected with source-bearing
diagnostics.
