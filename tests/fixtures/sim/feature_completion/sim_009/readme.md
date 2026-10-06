# Typed timed-call gaps (SIM-009)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `local_events` | §§6.17, 6.21, 15.5. Each `initial` block's `x` is its own event: the first wakes at 1, the second at 2. `t(3)` starts at 5 and its branch triggers its own `done` at 8. `t(4)` and `t(5)` run concurrently from 8 with separate events: 12 and 13. `count(2)` starts at 13; its branch reads `n` only in `#n`, increments `hits` at 15 and the task prints at 16. |
