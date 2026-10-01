# Wait storage acceptance

The public CLI runs both fixtures with the default optimizer and `--no-opt`.
`sim_wait_storage.rs` owns the independent exact transcripts.

- `rearm.sv` expects `4 4 4 4 1`: four scalar posedges, four 65-bit changes,
  four named events and four mixed-list wakes. Disabling `parked` after its first
  wake cancels the pending event arm before the first trigger.
- `four_state.sv` expects `3 3 4 4`: 0→X, Z→1 and 0→1 are posedges;
  1→Z, Z→0 and 1→0 are negedges. X→Z is neither. Four changes to the
  64-bit and 65-bit values exercise the inline boundary and heap fallback,
  including X→Z changes in the most significant limb and the LSB.

Native storage, allocation counting, level targets and cancellation/teardown
coverage live in `tests/runtime_value_storage/wait_inline_probe.c`.
