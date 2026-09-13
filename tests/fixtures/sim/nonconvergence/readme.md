# Simulator nonconvergence regressions

These checked-in designs cover ordinary wait-free `always` repetition,
finite zero-time computation, and the distinct time-zero behavior of
`always_comb` and `always_latch` (IEEE 1364-2001 §9.9.2 and IEEE 1800-2009
§9.2.2.2–§9.2.2.3). The Rust suite invokes the public `llg` executable in both
optimized and `--no-opt` modes.

`LLG_ZERO_LOOP_LIMIT` bounds scheduler passes. `LLG_PROCESS_STEP_LIMIT` bounds
generated loop back-edges inside a coroutine (`LLG_NONCONVERGENCE_LIMIT` is an
alias); both require positive decimal `uint64_t` values. Setting only the
scheduler variable applies that value to the process budget as well.
