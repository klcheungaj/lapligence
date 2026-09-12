# Concurrent assertion fixtures

These fixtures exercise the H20 bounded concurrent-assertion subset in both
optimizer modes: one explicit packed signal clock, simple `|->`/`|=>`
implications, Preponed sampling across NBA updates, asynchronous single-signal
`disable iff`, overlapping attempts, Reactive actions, vacuity accounting, and
pending-attempt disposal at end of simulation.

General sequence/property expansion, repetition, assertion-control tasks,
sampled-value system functions, and named property instances with formal
bindings are intentionally rejected with source-bearing diagnostics until a
later phase owns those semantics.
