# class-defects fixtures

Run by [`tests/sim_feature_completion/class_defects.rs`](../../../../sim_feature_completion/class_defects.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends; negatives are rejected in both optimizer modes.
Clause line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV), which is not part of the repository.
Decisions CD-D1 and CD-D2 in [the register](../../../../../docs/lrm_decisions.md)
have portable cases for the rules used here.

Clauses used by both items:

- SV 8.6 L9766: "The lifetime of methods declared as part of a class type shall
  be automatic."
- SV 6.21 L6901-6906: "Variables declared in an automatic task, function, or
  block are local in scope, default to the lifetime of the call or block and are
  initialized on each entry to the call or block ... Specific variables within an
  automatic task, function, or block can be explicitly declared as static. Such
  variables have a static lifetime."
- SV 20.4.2 Table 20-3 L34373-34374: the default `$timeformat` units are "The
  smallest time precision argument of all the `timescale compiler directives";
  with `` `timescale 1ns/1ps `` a time of 1 ns prints as `1000` with `%0t`.

## Named blocks (`named_blocks.sv`, item 1)

Clauses used:

- SV 9.3.4 L11571-11572: "Both sequential and parallel blocks can be named by
  adding : name_of_block after the keywords begin or fork. A named block creates
  a new hierarchy scope." L11574-11575: "It allows local variables, parameters,
  and named events to be referenced hierarchically, using the block name."
- SV 9.6.2 L12387-12388: "The disable statement shall terminate the activity of a
  task or a named block. Execution shall resume at the statement following the
  block or following the task-enabling statement." L12440: "If the block is a
  loop body, it acts like a continue (see 12.8)."
- SV 21.2.1.6 L36379-36381: "The %m format specifier ... causes the display task
  to print the hierarchical name of the design element, subroutine, named block,
  or labeled statement that invokes the system task containing the format
  specifier." llg names class code below its class (S99-D10) and adds the named
  blocks, `fork`-`join` blocks included (CD-D2).
- SV 8.24 L10756-10757: "Each specialization of a class has a separate set of
  static member variables".

Derivation (times in ps):

| Line | Why |
| --- | --- |
| `C.f.fb x=3` | `x` is a block local of `f`, assigned 3; `%m` is block `fb` of `C.f`. |
| `C.g.outer.inner` | Nested named blocks. |
| `s=3` | `acc` adds 0, 1, 2; `disable sb` at `i == 3` leaves the block, then `return acc`. |
| `counted=2` | `cnt` is a static block local of `count`, entered twice; `peek` reads it as `count.cb.cnt`. |
| `1000 C.t.tf a`, `2000 C.t.tf.tb2 b` | The branches of fork `tf` wait 1 and 2; the second is in block `tb2`. |
| `base=29 hits=4` | `scan(10)`: `sq` is 0 (added), 1 (`disable step` continues), 4, 9, 16 (added), 25 (`disable outer` leaves the loop): 0+4+9+16 = 29 in four additions. |
| `Derived.scan.again extra=104` | `super.scan(3)` adds 0 and 4; plus 100. |
| `derived=104` | `b` holds the `Derived` object; virtual dispatch. |
| `P.fill.fb calls=1 v=f`, `calls=2 v=f` | `P#(4)`: `t = '1` is 4 bits; `calls` is static in `fb`. |
| `P.fill.fb calls=1 v=fff` | `P#(12)` has its own static `calls` (8.24) and a 12-bit `t`. |
| `3000 P.tick.tk c=1`, `4000 P.tick.tk c=1` | `c` is automatic, so it is 0 on each entry to `tk` (6.21); the loop starts at 2 ns. |
| `9000 tb.Wt.watch.watchers.w1 k=10` | `x.watch()` starts at 4 ns; `k` is initialized on entry to `w1` (10 for id 1); `Wt` is declared in `tb`. |
| `10000 tb.Wt.watch.watchers.w1 k=20` | `y.watch()` starts at 5 ns. |
| `12000 watched 1 1` | `y.kill()` at 12 ns disables `watchers` in both objects (CD-D1) before the second branches end (13 and 14 ns); each object counted its `w1` branch. |

## Disable from another process (`named_block_disable.sv`, item 1)

Clauses used:

- SV 9.6.2 L12387-12391: "The disable statement shall terminate the activity of a
  task or a named block. Execution shall resume at the statement following the
  block or following the task-enabling statement. ... If a task is enabled more
  than once, then disabling such a task shall disable all activations of the
  task." L12402-12403: "Disabling an automatic task or a block inside an
  automatic task proceeds as for regular tasks for all concurrent executions of
  the task." L12441: "If the block is not currently executing, the disable has
  no effect."

| Line | Why |
| --- | --- |
| `5000 entered=2 finished=0 resumed=2` | `a.run(10)` and `b.run(20)` enter `wait_blk`; `stop_idle` at 3 ns disables `idle.blk`, which is not executing; `c.stop()` at 5 ns disables `run.wait_blk` in both objects, so both resume after the block. |
| `9000 entered=3 finished=1 resumed=3` | `c.run(4)` completes its block. |
| `16000 laps=3` | `spin` of `Gen#(2)` counts laps at 11, 13 and 15 ns; `halt` disables the task at 16 ns and the `fork` joins. |
| `21000 n=20` | `t.spin()` dispatches to `FastTicker::spin`, adding 10 at 18 and 20 ns; `f.halt()` disables that task at 21 ns. |

## Negatives

- `neg_disable_function.sv`: SV 9.6.2 L12399-12400: "The disable statement can be
  used to disable named blocks within a function, but cannot be used to disable
  functions." Rejected at 11:13.
- `neg_block_automatic_reference.sv`: SV 13.3.1 L18624: "Automatic task items
  cannot be accessed by hierarchical references." The block local `x` of the
  method `t` is automatic (8.6, 6.21). Rejected at 11:17.
