# Concurrent assertion fixtures

These fixtures exercise the H20 concurrent-assertion, H21 sampled-value, H22
sequence, and H23 property-composition domains in both optimizer modes: one
explicit packed signal clock, sampled
`|->`/`|=>` implications, Preponed sampling across NBA updates, asynchronous
single-signal `disable iff`, overlapping attempts, Reactive actions, vacuity
accounting, pending-attempt disposal at end of simulation, `##` concatenation,
all sequence repetition kinds including unbounded ranges, Boolean sequence
composition, and `first_match` endpoint selection. They also cover
explicit/default sampled clocks, initial/gated history, global-clock
status/history, and LSB/X/Z edge rules.

Match-item side effects, unsupported temporal property operators, assertion-
control tasks, future global sampled-value functions, complex sampled clock
events, and sequence `.matched` status remain intentionally rejected with
source-bearing diagnostics. Named sequence/property instances, declaration
defaults/named arguments, one-cycle property `not`/`and`/`or`/`iff`/`implies`
composition, and inherited `disable iff` metadata are covered by
`property_instances.sv`.
`unsupported_instance.sv` and `unsupported_clock_instance.sv` retain
source-bearing temporal and conflicting-clock rejection probes.
