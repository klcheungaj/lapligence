# Fork lifetime, named disable and capture cleanup (SIM-010)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `fork_declarations` | §§6.21, 9.3.2. Each iteration's fork creates its own `k` = 10 + `i` before its branch starts at time 0; the branch for `i` waits `3 - i`: `k 12` at 1, `k 11` at 2, `k 10` at 3. `wait fork` in the calling process waits for all three: `done 3`. |
