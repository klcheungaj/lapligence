# class-defects fixtures

Run by [`tests/sim_feature_completion/ki_class_defects.rs`](../../../../sim_feature_completion/ki_class_defects.rs).
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

## Class parameters (`class_parameters.sv`, item 2)

Clauses used:

- SV 8.24 L10756-10757: "The combination of a generic class and the actual
  parameter values is called a specialization. Each specialization of a class
  has a separate set of static member variables". 8.24 L10823-10824: "The default
  specialization of a parameterized class is the specialization of the
  parameterized class with an empty parameter override list."
- SV 8.22 L10516-10519: "the :: operator also allows access to static members
  (class properties and methods), class parameters, and class local parameters
  from outside the class ... A class parameter or local parameter is a public
  element of a class."
- SV 8.24.1 L10842-10844: "Use of the class resolution operator with a prefix that
  is the unadorned name of a parameterized class (see 8.24) shall be restricted to
  use within the scope of the named parameterized class and within its
  out-of-block declarations (see 8.23)."
- SV 8.22 L10576-10578: "Nested classes have lexically-scoped, unqualified access
  to the static properties and methods, parameters, and local parameters of the
  containing class."
- SV 18.6.1 L30402: "The randomize() method returns 1 if it successfully sets all
  the random variables and objects to valid values".

| Line | Why |
| --- | --- |
| `W=4 L=8 bits=4 tmp=15` | `P#(4)`: `L = W*2`; `T` defaults to `logic [3:0]`, so `'1` is 15. |
| `W=16 L=32 bits=8 tmp=255` | `P#(16, logic [7:0])`. |
| `W=8 L=16 bits=4 tmp=15` | `P` is the default specialization. |
| `5 17 12` | `sw` returns `W+1` for `W` 4 and 16; the out-of-block `ex` of `P#(4)` returns `L+W` = 12. |
| `48 24` | `ex` for `P#(16, ...)` (32+16) and the default (16+8). |
| `12 40` | The out-of-block static `sx` returns `2*W + $bits(T)`: 8+4 and 32+8. |
| `50 6 11 5 five` | `Outer#(5, "five")`: `init_w = W*10`, static `sinit = D = 6`, the nested class reads `W+D` = 11, the default argument `a = W`, the string parameter. |
| `30 4 7 3 o` | The default specialization: `W` 3, `D` 4. |
| `10 15 9` | `loop_sum` adds 0..4; `arr` of `Outer#(5,...)` returns `$size(a)` 5 plus `a[4]` = 4+6; for `W` 3, 3 plus `a[2]` = 2+4. |
| `9 13 5` | `Kid#(3)` extends `Outer#(6)`: `W+K` = 9, `W+D` = 6+7; `Outer#(5, "five")::W` is 5. |
| `randomized 1 1 module W=99` | `x < W` is satisfiable for `W` 3 and 7; the module parameter `W` is a different parameter. |
| `tb.R.show W=3 below=1`, `tb.R.show W=7 below=1` | Each specialization reads its own `W` in the task and in its constraint. |

## Negatives

- `neg_disable_function.sv`: SV 9.6.2 L12399-12400: "The disable statement can be
  used to disable named blocks within a function, but cannot be used to disable
  functions." Rejected at 11:13.
- `neg_block_automatic_reference.sv`: SV 13.3.1 L18624: "Automatic task items
  cannot be accessed by hierarchical references." The block local `x` of the
  method `t` is automatic (8.6, 6.21). Rejected at 11:17.
- `neg_type_parameter_value.sv`: SV 6.20.3 L6733-6734: "A parameter constant can
  also specify a data type"; a type parameter is not a value. Rejected at 4:12.
