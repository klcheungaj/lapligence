# Typed timed-call gaps (SIM-009)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `local_events` | §§6.17, 6.21, 15.5. Each `initial` block's `x` is its own event: the first wakes at 1, the second at 2. `t(3)` starts at 5 and its branch triggers its own `done` at 8. `t(4)` and `t(5)` run concurrently from 8 with separate events: 12 and 13. `count(2)` starts at 13; its branch reads `n` only in `#n`, increments `hits` at 15 and the task prints at 16. |
| `event_formals` | §§13.5, 15.5. `pick` assigns `e1` to its output after `#1` and prints the record member `q`; `h` names `e1` from time 1, so the trigger at 3 wakes `@h`. `alias_wait` rebinds `h` through its `ref` at 4 while suspended; the trigger of `e2` at 6 wakes `@h`. `swap` starts from `h` (`e2`), wakes on the trigger at 7 and its `e1` is assigned back at return; the trigger of `e1` at 9 wakes `@h`. |
| `event_local_native` | §§9.4.2, 13.5. `t` waits for `posedge l` on its own local, which the detached branch sets at 3: `z 3` (the record formal's string). `down(2)` starts at 3; each activation's branch sets its own `l` 1 later, and the activation then recurses: `down 2 at 4`, `down 1 at 5`, `down 0 at 6`. |
| `recursive_activations` | §§13.3.1, 13.5.2, 15.5. `rec(2)` builds tag `L2`, waits for the trigger at 1, adds 2 to `total` and calls `rec(1)` with its own record (tag `L21`), which waits for 2 and calls `rec(0)` (tag `L210`), which waits for 3 and adds 0. Returns print innermost first, each with its own record and queue: `L210 0 0 3`, `L21 1 10 3`, `L2 2 20 3`; `total` = 2 + 1 + 0 = 3. |
| `static_and_automatic_tasks` | §§6.21, 13.3.1. `t(1)` and `t(2)` share the static `keep`; the second write (2) is what both print after `#2`. `a(3)` and `a(4)` each print their own value. |
| `formal_event_expressions` | §§9.4.2, 13.3.1. Rising `clk` edges are at 1, 3, 5, ...; `addr` becomes 2 at 4 and 3 at 10. `chain(2)` waits for an edge with `addr >= 2` (5), then recurses: `chain(1)` at 7, `chain(0)` at 9. `wait_addr(3, p)` waits for an edge with `addr == 3`: 11, and prints its record member `dev`. |
