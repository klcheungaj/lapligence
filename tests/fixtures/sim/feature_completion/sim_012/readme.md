# Virtual-interface methods and dynamic receivers (SIM-012)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses (IEEE 1800-2009 §§25.5, 25.9, 13.5, 15.5), not captured from llg.
Times are printed with `%0d $time` in the default unit.

| Fixture | Clause and independent oracle |
| --- | --- |
| `timed_dispatch` | §§25.9, 9.3.2. `v.bump(2)` starts on `a` at 0 and stays there although `v` is rebound to `b` at 1: `a` = 2 at 2; `h.run(3)` bumps `b` at 3: `a=2 b=3 t=3`. `v.bump(1)` now runs on `b`: `b` = 4 at 4 (`get=4`). From 4, the associative element (`a`, +4 at 8), the queue element (`b`, +5 at 9) and the class array element (`a`, +6 at 10) run concurrently: `a=12 b=9 t=10`. `use_rec` calls through its record formal's member: `b` = 11 at 12. Writes through the class properties set `b` to 20 and `a` to 30. |
| `event_formals` | §§25.9, 13.5, 15.5. `v.fire(go)` triggers the caller's `go` at 1 through its `ref`, waking `waiter`, which returns `a.done` through its output (`fired 1`, `waiter 1`). `got` then names `a.done`: its trigger at 2 wakes `@(got)`. `v.swap(h)` on `b` returns `b.done` through the inout at 3; the trigger of `a.done` at 4 does not wake `@(h)`, that of `b.done` at 5 does. |
| `null_call` | §25.9. `slow` started on `a` writes `a.x` at 5 although `v` names `b` from 1: `a.x=1 b.x=0`. The call through the null `v` is a run-time error at `tb:20:5`; `unreached` is not printed. |
| `neg_modport_write` | §25.5. Writing the input modport member `x` through view `ro` is a frontend error. |
| `neg_specialization` | §25.9. An `ifc #(8)` instance cannot be assigned to a `virtual ifc #(4)`. |
| `null_member` | §25.9. Through `a` the write and read work: `x=3`. The write through the null `v` is a run-time error at `tb:15:5`; `unreached` is not printed. |
