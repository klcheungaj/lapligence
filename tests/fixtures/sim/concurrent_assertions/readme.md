# Concurrent assertion fixtures

These fixtures exercise the H20 concurrent-assertion, H21 sampled-value, H22
sequence, H23 property-composition, and H24 bounded local-state domains in both
optimizer modes: one
explicit packed signal clock, sampled
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
status/history, and LSB/X/Z edge rules.

Output/inout/ref formal copy-out, delayed or nested local-formal invocations,
selected-local lvalues, repeated match-item bodies, unsupported temporal
property operators, assertion-control tasks, future global sampled-value
functions, complex sampled clock events, and sequence `.matched` status remain
intentionally rejected with source-bearing diagnostics. Named sequence/property
instances, declaration defaults/named arguments, one-cycle property
`not`/`and`/`or`/`iff`/`implies` composition, and inherited `disable iff`
metadata are covered by `property_instances.sv`.
`unsupported_instance.sv` and `unsupported_clock_instance.sv` retain
source-bearing temporal and conflicting-clock rejection probes.
