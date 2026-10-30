# End-of-run report fixtures

`sim_end_of_run` checks which stderr line the runtime prints when no future
event remains. Stdout, final blocks and the exit status are the same in every
case; only the diagnostic differs.

- `*_end.sv`: only permanent waiters remain (continuous assignments, gates and
  switches, `always`, `always_comb`, `always_ff`, a fork started by an `always`).
  The run ends normally.
- `*_deadlock.sv`: an `initial` procedure (also of a program block), a fork
  child, a `join_none` child or a task call it made can never finish. The run
  reports a deadlock.

Clause text the expected outputs rely on (`SystemVerilog-1800-2009.txt`):

- 9.2 L11064-11066: "The initial procedure shall execute only once, and its
  activity shall cease when the statement has finished. In contrast, an always
  procedure shall execute repeatedly, and its activity shall cease only when
  the simulation is terminated."
- 9.6.1 L12344: "Simulation automatically terminates when there is no further
  activity of any kind."
