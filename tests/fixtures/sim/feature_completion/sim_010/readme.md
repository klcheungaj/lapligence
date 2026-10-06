# Fork lifetime, named disable and capture cleanup (SIM-010)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `fork_declarations` | §§6.21, 9.3.2. Each iteration's fork creates its own `k` = 10 + `i` before its branch starts. The branches start after `spawn` returns at time 0, when the shared loop variable `i` is 3 and still alive for them; each waits `13 - k`: `k 12 i 3` at 1, `k 11 i 3` at 2, `k 10 i 3` at 3. `wait fork` in the calling process waits for all three: `done 3`. |
| `shared_automatics` | §§6.21, 9.3.2. In `t`, the parent writes `x = 7` right after the fork, so the child at 2 prints 7 and writes 9, which the parent prints at 3. Each `outlive(n)` returns at once after setting `y = 10n`; its child prints `y` at +3: 40 and 60. Two branches each increment the block's `z` (at 1 and 2): 2 at 3. In `loop`, the three branches start after the loop ends and read the one `i`: 3 each time. |
| `shared_strings` | §§6.21, 9.3.2. The parent sets `s = "b"` right after the fork; the child prints `b k` at 1 and appends `!`, which the parent prints at 2 (`b!`). In `copy_only` the joined branch copies `m` into its own `mine`: `copy m`. |
| `shared_containers` | §§6.21, 9.3.2. The producer pushes 10, 20, 30 at 1, 2, 3; the consumer pops one element just after each push (`#1` then `#0`) into the task's `got`: `got 10 20 30`. `detached` pushes 1 after starting its branch and returns at 3; the branch pushes 7 at 5 into the same queue: size 2. |
| `joined_strings` | §§6.21, 9.3.2. Joined branches append `b` at 1 and `c` at 2 to the task's `s` (`"a"`), then `y` and `z` to the block's `s` (`"x"`): `task abc`, `block xyz`. |
| `fork_wakeups` | §§9.4.2, 9.4.3. `bare` (called at 0) wakes when its branch sets `l` at 2; `edge_expr` (at 2) on `posedge (level & idle)` at 4; `real_change` (at 4) when `r` becomes 2.5 at 5; `level_wait` (at 5) when `x` becomes 2 at 7. The parent sets `b` at 8 and wakes its detached branch: `branch 8`. Joined siblings: `y` becomes 2 at 10: `sibling 10`. Storing the unchanged 2 at 11 is no event; 3 at 12 wakes: `changed y=3 12`. |
