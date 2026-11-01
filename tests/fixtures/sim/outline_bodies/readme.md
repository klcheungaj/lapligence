# Outlined statement runs

`tests/sim_outline_bodies.rs` runs `long_bodies.sv` through the public CLI in
both optimizer modes and checks that every long body in it is split by the C
emitter (`src/sim/emit_c/owned/outline.rs`). Each body holds runs of 160
32-bit assignments, longer than one outlined helper, so the generated C calls
helper functions from the `initial` and `final` procedures, the function, the
task and both fork branches.

`long_bodies.out` and the `$info` line were computed by an independent Python
model of the assignments (modulo 2^32), not by a simulator; changing a
statement means recomputing the output from that model.

| Construct | What the output shows |
| --- | --- |
| `#1` between two runs | the second run starts after the delay (`at 1`) |
| `disable blk1` between two runs | the second run of `blk1` does not execute; `blk2` runs through |
| `repeat` and `for` around long bodies | each iteration runs the whole body once; the automatic loop variable `j` is read after the run |
| `fork`/`join` with long branches | each branch's own variable, at its own time |
| `fold`: automatic `y` and an early `return` between runs | the second run is skipped when `x[0]` is set |
| `churn`: automatic `k` between runs and an early task `return` | `k` keeps its value across the run; `churn(3)` prints once |
| calls of `bump` inside runs of `initial` and `churn` | the call result, with the caller's recursion depth passed on |
| `$info` inside a run | the reported scope, line and column of the call |
| `final` with a long body | runs after `$finish` |

Clauses the output relies on (IEEE 1800-2009, `docs/specification/SystemVerilog-1800-2009.txt`):

- 9.6.2, line 12387: "The disable statement shall terminate the activity of a
  task or a named block. Execution shall resume at the statement following the
  block or following the task-enabling statement."
- 13.4.1, line 18809: "The return statement shall override any value assigned
  to the function name."
- 13.3, line 18504: "A task exits when the endtask is reached. The return
  statement can be used to exit the task before the endtask keyword."
- 9.3.2, line 11371: "Statements shall execute concurrently." and line 11426:
  "join The parent process blocks until all the processes spawned by this fork
  complete." The branches write separate variables, so their order does not
  matter.
- 9.2.3, line 11261: "A final procedure executes when simulation ends due to an
  explicit or implicit call to $finish."
- 21.2.1.6, line 36379: "it causes the display task to print the hierarchical
  name of the design element, subroutine, named block, or labeled statement
  that invokes the system task containing the format specifier."
- 20.9, line 34923: "The file name and line number of the severity system task
  call." The message format itself (`llg: severity info: <scope>:<line>:<column>:`)
  is llg's, as in `sim_partial_features/severity.rs`.
- 20.3.1, line 34168: "The $time system function returns an integer that is a
  64-bit time, scaled to the time unit of the module that invoked it."; Table
  20-3, line 34373: the default `%t` units are "The smallest time precision
  argument of all the `timescale compiler directives", here 1 ns.
- 11.6.1, line 15993: "The number of bits of an expression (known as the size of
  the expression) shall be determined by the operands involved in the
  expression and the context in which the expression is given." Every
  assignment is 32 bits wide, so each result is kept modulo 2^32.
