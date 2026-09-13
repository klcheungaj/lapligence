# Concurrent assertion fixtures

These fixtures exercise the H20 concurrent-assertion, H21 sampled-value, H22
sequence, H23 property-composition, H24 bounded local-state, and H25
clock/control domains in both optimizer modes: one explicit packed signal clock,
sampled
`|->`/`|=>` implications, Preponed sampling across NBA updates, asynchronous
single-signal `disable iff`, overlapping attempts, Reactive actions, vacuity
accounting, pending-attempt disposal at end of simulation, `##` concatenation,
all sequence repetition kinds including unbounded ranges, Boolean sequence
composition, and `first_match` endpoint selection. H24 adds Slang-owned
sequence locals, per-attempt local input-formal captures, ordered local
match-item assignments and increments, and overlapping-attempt isolation.
`h24_branch_locals.sv` additionally checks that distinct `or` sequence threads
retain independent local snapshots at a common endpoint, while
`h24_formal_default.sv` covers a typed local input formal with a declaration
default. Existing fixtures also cover
explicit/default sampled clocks, initial/gated history, global-clock
status/history, and LSB/X/Z edge rules. H25 adds legal clock-flow across
`##0`/`##1` multiclock sequence segments, nearest default-clock inheritance,
conditional properties, and accept/reject controls with synchronous forms.

Output/inout/ref formal copy-out, delayed or nested local-formal invocations,
selected-local lvalues, repeated match-item bodies, unsupported temporal
property operators, pass/fail/vacuity assertion-action controls, future global
sampled-value functions, complex sampled clock events, and sequence
`.triggered` status remain intentionally rejected with source-bearing
diagnostics. H26 adds bounded blocking `expect`, `$asserton`/`$assertoff`/
`$assertkill`, the level-0 ON/OFF/KILL `$assertcontrol` forms, and sequence
`.matched` endpoint evaluation; invalid control arguments and non-hierarchical
scopes are also rejected. Named sequence/property instances, declaration
defaults/named arguments, one-cycle property
`not`/`and`/`or`/`iff`/`implies` composition, and inherited `disable iff`
metadata are covered by `property_instances.sv`.
`unsupported_instance.sv`, `unsupported_clock_instance.sv`, and
`h25_unsupported_multiclock.sv` retain source-bearing temporal, conflicting
clock, and out-of-subset cross-clock rejection probes.
