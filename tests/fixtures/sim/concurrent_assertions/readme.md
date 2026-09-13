# Concurrent assertion fixtures

These fixtures exercise the H20 bounded concurrent-assertion subset and H21
sampled-value domains in both
optimizer modes: one explicit packed signal clock, simple `|->`/`|=>`
implications, Preponed sampling across NBA updates, asynchronous single-signal
`disable iff`, overlapping attempts, Reactive actions, vacuity accounting, and
pending-attempt disposal at end of simulation, plus explicit/default sampled
clocks, initial/gated history, global-clock status/history, and LSB/X/Z edge
rules.

General sequence/property expansion, repetition, assertion-control tasks,
future global sampled-value functions, complex sampled clock events, sequence
`.matched` status, and named property instances with formal bindings are
intentionally rejected with source-bearing diagnostics until a later phase
owns those semantics.
