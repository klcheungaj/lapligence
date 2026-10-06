# Semaphore collections and shared-lifetime synchronization (SIM-016)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `shared_handles` | §§9.3.2, 15.3, 15.4. The first branch's `get` starts at 0 on the original semaphore, which never receives a key. The task rebinds `s` to `other` at 1 and puts 7 into the mailbox at 2: `box 7 2`. The third branch puts into `s` at 3, which is now `other`, so the fourth branch (waiting on `other` since 2) resumes: `other got 3`. |
