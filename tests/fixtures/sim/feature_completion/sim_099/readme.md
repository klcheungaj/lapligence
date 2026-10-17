# SIM-099 common verification gate fixtures

Run by [`tests/sim_feature_completion/sim_099.rs`](../../../../sim_feature_completion/sim_099.rs).
Positive cases run in both optimizer modes; the backend-parity and checked-matrix
cases also run on the legacy, compact/portable and compact/GMP value backends.
Clause line numbers refer to `pdftotext -layout` extractions
`SystemVerilog-1800-2009.txt` (SV) and `Verilog-1364-2001.txt` (V2001), which are
not part of the repository.

## Integrated testbench (`testbench.sv`, step 1)

A registered ALU (`alu`) is driven by a class-based environment through
`virtual alu_if`: a driver, monitor and scoreboard joined by mailboxes, two
agents sharing a one-key semaphore, inline-constrained `randomize()`,
`constraint_mode(0)`, `randsequence`, two concurrent assertions in the
interface, an immediate assertion, an embedded covergroup sampled with
`with function sample`, the DPI-C model `ref_alu` (`testbench_dpi.c`) and a VPI
value-change callback on `tb.dut.result` (`testbench_vpi.c`). The suite runs it
with both companions linked as static archives (every host), with the DPI model
as a shared companion and the VPI application as an `LLG_VPI_PLUGIN` (Unix and
MSVC hosts; skipped where the harness cannot build them, like the other
`sim_040`/`sim_042` plugin tests), and under `LLG_GC_STRESS=1 LLG_GC_VERIFY=1`.

Clauses used:

- SV 15.4.3 L20521-20522: "The put() method stores a message in the mailbox in
  strict FIFO order. If the mailbox was created with a bounded queue, the process
  shall be suspended until there is enough room in the queue."
- SV 15.4.5 L20549: "If the mailbox is empty, then the current process blocks until
  a message is placed in the mailbox."
- SV 15.3.3 L20415: "number of keys is not available, the process blocks until the
  keys become available." SV 15.3.2 L20400: "When the semaphore.put() function is
  called, the specified number of keys is returned to the semaphore."
- SV 18.6.1 L30402: "The randomize() method returns 1 if it successfully sets all
  the random variables and objects to valid values; otherwise, it returns 0."
  SV 18.6.3 L30471: "If randomize() fails, the constraints are infeasible, and the
  random variables retain their previous values."
- SV 18.7 L30556: "The local:: qualifier (see 18.7.1) is used to bypass the scope of
  the (randomize() with object) class".
- SV 18.9 L30749-30750: "When a constraint is inactive, it is not considered by the
  randomize() method."
- SV 18.17 L31424-31425: "A production list contains a succession of production
  items, indicating that the items must be streamed in sequence." SV 18.17.4
  L31591: "The repeat production statement is used to iterate over a production a
  specified number of times."
- SV 16.5 L21447: "All variables in a concurrent assertion use the value sampled in
  the Preponed region of a time slot". SV 16.13.6 L24622-24624: overlapped
  implication starts the consequent at the end point of the antecedent match;
  "For nonoverlapped implication, the start point of the evaluation of the
  consequent property_expr is the clock tick after the end" point. SV 16.13
  L24453: "The expression of the disable iff is called the disable condition."
  SV 16.15.1 L26464: "The pass and fail statements of an assert statement are
  executed in the Reactive region."
- SV 19.4 L32233: "An embedded covergroup variable may only be assigned in the new
  method." SV 19.8.1 L33615: "Overriding the pre-defined sample() method with a
  triggering function that accepts arguments". SV 19.11.1 L33822-33828: coverpoint
  coverage `C_i = bins covered / bins`. SV 19.11 L33768: "The coverage of a coverage
  group, Cg , is the weighted average of the coverage of all items defined in the"
  group; Table 19-1 L33254 gives `weight` default 1.
- SV 25.9 L44930-44931: "Once a virtual interface has been initialized, all the
  components of the underlying interface instance are directly available to the
  virtual interface via the dot notation."
- SV 35.5 L55621: "The usage of imported functions is similar as for native
  SystemVerilog functions."
- SV 38.36 L64642: "cbValueChange After value change on some variables, any
  expression, or terminal".
- SV 9.6.3 L12548: "The disable fork statement terminates all active descendants
  (subprocesses) of the calling process." SV 9.3.2 L11426: "join The parent process
  blocks until all the processes spawned by this fork complete."
- SV 4.4.2.4 L3160: "The NBA (nonblocking assignment update) region holds the
  events to be evaluated after all the Inactive events are processed."

Derivation (`clk` rises at 5, 15, 25, ...; falls at 10, 20, ...):

| Line | Why |
| --- | --- |
| `vpi watch tb.dut.result` | `cbStartOfSimulation` registers the watch. |
| `[0] vpi result=0` | `reset(1)` drives `rst_n` x to 0 at time 0: a `negedge rst_n`, so `result <= 0` changes it from x. |
| `[10] reset released` | `repeat (1) @(negedge clk)` returns at 10. |
| `[10] A holds the bus` | `bus_lock` starts with one key; agent B waits `#1` first. |
| `[10] A queued op=0 a=10 b=20 ok=1` | `add`, first item of `arith`; every field is pinned by the inline constraint and is legal, so `randomize()` returns 1; the empty bounded mailbox accepts it. |
| `[10] A queued op=1 a=50 b=8 ok=1` | `sub`; the waiting driver takes the first request (it was blocked in `get`), so this `put` finds room at 10. |
| `[25] vpi result=30` | The driver drives request 1 at negedge 20; posedge 25 computes 10+20. |
| `[30] A queued op=2 a=60 b=15 ok=1` | `repeat (2) and_op`, n=0: the mailbox holds request 2 until the driver loops at negedge 30. |
| `[35] check op=0 a=10 b=20 dut=30 ref=30` | The monitor reads `out_valid`=1 (set at 25) at posedge 35 and queues 30; `ref_alu` returns 30. |
| `[45] vpi result=42` | Request 2 driven at 40: 50-8. |
| `[50] A queued op=2 a=61 b=15 ok=1` | Second `and_op` (n=1) waits until the driver takes request 3 at 50; A then returns the key. |
| `[50] B holds the bus` | B has waited for the key since time 11. |
| `[50] B reserved under legal: ok=0` | `op == 3` contradicts `legal`; randomize returns 0 (stderr warns `randomize() at env:222:12`). |
| `[50] B free: ok=1 legal=1` | Unconstrained call stays inside `legal`. |
| `[55] check op=1 a=50 b=8 dut=42 ref=42` | |
| `[65] vpi result=12` | Request 3 driven at 60: 60 & 15 = 12. |
| `[70] B queued op=1 a=5 b=9 ok=1` | Waits for room until the driver takes request 4 at 70. |
| `[75] check op=2 a=60 b=15 dut=12 ref=12` | |
| `[85] vpi result=13` | 61 & 15 = 13. |
| `[90] B queued op=3 a=85 b=170 ok=1` | `legal` is switched off, so op 3 and operands above 100 are allowed; room at 90. |
| `[95] check op=2 a=61 b=15 dut=13 ref=13` | |
| `[105] vpi result=252` | 5-9 truncated to 8 bits = 252 (`ref_alu` masks the same way). |
| `[115] check op=1 a=5 b=9 dut=252 ref=252` | |
| `[125] vpi result=255` | 85 ^ 170 = 255, written in the NBA region. |
| `[125] assertion reserved_op failed` | Sampled `valid`=1, `op`=3 at posedge 125; the action runs in the Reactive region, after the NBA callback. `handshake` holds at every cycle (`out_valid` follows each `valid`), and both are disabled while `rst_n` is 0. |
| `[135] check op=3 a=85 b=170 dut=255 ref=255` | Sixth check; the scoreboard returns a key to `finished`. |
| `[135] checked=6 mismatches=0` | `finished.get(1)` returns. |
| `coverage op=100.00 a=66.67 group=83.33` | ops 0,1,2,2,1,3 hit all four `cp_op` bins; `a` values 10,50,60,61,5,85 hit `low` and `mid` but not `high` (2/3); the group is the mean of the two weight-1 coverpoints. |
| `[135] done` | `disable fork` ends the driver, monitor and scoreboard; `$finish`. |
| `vpi end: 7 result changes` | 0, 30, 42, 12, 13, 252, 255. |

## Cross-task contracts (step 2)

### `gc_roots.sv` + `gc_roots.c`

Objects reachable only from a pending deferred-assertion report, from an
activation suspended across a DPI-C context import that allocates through an
exported function 200 times, and from activations waiting on a value the VPI
application writes. Run with and without `LLG_GC_STRESS=1 LLG_GC_VERIFY=1`; a
missed root ends the run with an access to a reclaimed object.

- SV 16.4 L21222-21224: "The subroutine shall be scheduled in the Reactive region.
  ... Actual argument expressions that are passed by value use the values of the
  underlying variables at the instant the deferred assertion expression was
  evaluated."
- SV 8.27 L11016: "The system shall automatically reclaim any object that is no
  longer being used."
- SV 38.34 (vpi_put_value) L64179: "vpiInertialDelay All scheduled events on the
  object shall be removed before this event is scheduled."

| Line | Why |
| --- | --- |
| `[0] deferred action sees 7 8` | The report captured `h` before `h = null`; the object and its peer stay alive. |
| `[2] after DPI churn 200: mine 20 21 kept 1` | `c_churn` returns the count of `sv_alloc(k) == k`; only `k == 0` is kept; `#1` after time 1. |
| `[3] after VPI answer ack=31: mine 30 30` | `req = 30` at 2; the callback writes `ack = 31` one unit later. |
| `[4] after VPI answer ack=41: mine 40 40` | Same at 3. |

### `selected_writes.sv` + `selected_writes.c`

A class writes `bus.data` through a virtual interface with whole, bit, part and
indexed part selects, blocking and nonblocking, once with an equal value.

- SV 9.4.2 L11807-11809: "An implicit event shall be detected on any change in the
  value of the expression. ... A change of value in any operand of the expression
  without a change in the result of the expression shall not be detected as an
  event."
- SV 4.7 L3465-3466: "active events can be taken off the Active or Reactive event
  region and processed in any order", so the test compares the lines of one time
  step as a set.
- SV 10.4.2 L13107: nonblocking assignments "are the last assignments executed in
  a time step".

| Time | Write | Lines |
| --- | --- | --- |
| 1 | `data = 00` (from x) | `data -> 00`, `data[3] -> 0`, `vpi tb.seen=00`, `vpi tb.low=0` |
| 2 | `data[3] = 1` | `data -> 08`, `data[3] -> 1`, `seen=08`, `low=8` |
| 3 | `data[7:4] = a` | `data -> a8`, `seen=a8` (low and bit 3 unchanged) |
| 4 | `data[3] = 1` again | nothing (no value change) |
| 5 | `data[1+:2] = 11` | `data -> ae`, `seen=ae`, `low=e` |
| 6 | `data[0] <= 1` | `data -> af`, `seen=af`, `low=f` |
| 7 | `data[7:4] <= 5` | `data -> 5f`, `seen=5f` |
| 8 | `data <= 0f` | `data -> 0f`, `seen=0f` |
| 9 | | `final data=0f low=f` |

### `source_map.sv`

Run-time reports keep their source position, also when the model is generated
from the owned database after the frontend snapshot and the database are
dropped. stdout is `randomize ok=0` (`v > 8` contradicts `v < 4`, SV 18.6.1
L30402) and `done`; stderr carries the `$error` of the package function (line
10), the `$warning` of the class method (line 23), the randomize warning
(line 40) and the interface assertion failure at posedge 5 (`flag` became 1 at
4; line 16, default action because the assertion has no else).

### `shuffle_ownership.sv`

- SV 7.12.2 L9366: "shuffle() randomizes the order of the elements in the array."
- SV 18.14 L31124: "The RNG is localized to threads and objects." SV 18.14.1
  L31152-31153: "Thread stability. Each thread has an independent RNG for all
  randomization system calls invoked from that thread."
- Decision [S28-D5](../../../../../docs/lrm_decisions.md): `shuffle` draws from the
  calling thread, also inside a class method; `post_randomize()` is such a method,
  run by `randomize()` in the calling thread (SV 18.6.2 L30434-30435).

Every line is a relation that holds for any generator, so each prints 1:
reseeding the object replays its `randomize()` whatever shuffles ran between;
reseeding the thread replays a shuffle and the next `$urandom` whatever the
object drew; a shuffle in `post_randomize()` equals a direct shuffle after the
same thread seed, and a different object seed does not change it.

### `random_replay.sv`

- SV 18.13.4 L31100: "The get_randstate() method returns a copy of the internal
  state of the RNG". SV 18.13.5 L31115: "The set_randstate() method copies the given
  state into the internal state of an object's RNG."
- SV 18.4.2 L29417-29418: randc variables "cycle through all the values in a random
  permutation of their declared range."
- SV 18.16 L31353: "Each call to randcase retrieves one random number".

Two rounds of every source are drawn, both states restored, and the rounds
redrawn: `replay equal: 1` (the restored streams make the second pass draw the
same values, in the same number). A third round continues the streams:
`continuation differs: 1`. Eight calls of a fresh randc object visit all eight
values: `randc cycle complete: 1`. The `checksum` line (both sums and the number of mixed values
of the first two rounds, 15 per round plus 2 to 4 randsequence code blocks)
depends on the generator, so the suite requires it to be identical in every optimizer mode, value backend and
collector setting instead of comparing it with a fixed value.

## Regressions and remaining built-ins

- `void_casts.sv`: SV 13.4.1 L18829-18830: "The function can be used as a statement
  and the return value discarded without a warning by casting the function call to
  the void type." Built-in functions in a void cast run as functions; `$random(seed)`
  still updates `seed`. Output `[3] after void casts x=10110001 seed changed=1`.
- `void_vpi_function.sv`: a user VPI function (`sim_042/systf.c`, `$sum` sized
  signed 12 bits, `vpiSizedSignedFunc` = 5) in a void cast runs its calltf once:
  `sum call line=7 size=12 functype=5`, then `after void sum`.
- `handle_semaphores.sv`: SV 15.3.4 L20441-20442: "If the specified number of keys
  is available, the method returns a positive integer ... If the specified number
  of keys is not available, the method returns 0." `take_two` waits until the put
  at 2 makes two keys; `take_nested` until the put at 4; at 6 the holder is empty
  (0), then has 3 keys: one (1), three of the remaining two (0), two (1).
- `vif_formal_writes.sv`: SV 25.9 L44930-44931 (above) and SV 10.4.2 L13107. At 0
  `drive(a, 5d)` blocks `flags[3:2] = 10` and schedules `data <= 5d`,
  `flags[1] <= 1`; at 1 it schedules `data[0] <= 0` and returns, so the display at
  1 (Active, before that NBA) shows `5d` and `1010`. `drive(b, 81)` at 1 gives
  `b.flags` 0001 -> 1001 -> 1011 and `b.data` 81 -> 80 at 2; at 3 `a.data` is
  `5c`.
- `builtins.sv`: SV 20.13 L35204-35205: "$onehot returns true if 1 and only 1 bit
  of expression is high." / "$onehot0 returns true if at most 1 bit of expression
  is high." SV 20.5 L34474: "$itor converts integer values to real values". SV
  21.2.1.2 L35976-35978 (V2001 L18268-18270): an argument without a format is shown
  in "binary format in $displayb and $writeb, octal format in $displayo and
  $writeo, and hexadecimal format in $displayh and $writeh." SV 21.3.2
  L36675-36677: "The $fstrobe and $fmonitor system tasks work just like their
  counterparts, $strobe and $monitor ... any number of $fmonitor tasks can be set
  up to be simultaneously active." SV 21.2.2 L36481-36483: `$strobe` displays "at
  the end of the current simulation time", so both strobes show `x` = ff. SV
  21.2.3 L36516-36518: `$monitor` displays at the end of a time step when an
  argument changed (and once when started). `x`=00010000 (8 bits: `%o` is 3
  digits, `020`; hex `10`), `y`=0000.
- `global_sampled.sv`: SV 16.9.4 L22897-22901: "$fell_gclk(v) ≡ $fell(v,
  @$global_clock)", likewise `$stable_gclk` and `$changed_gclk`; SV 16.9.3
  L22694-22697: "$fell returns true if the least significant bit of the expression
  changed to 0", "$stable returns true if the value of the expression did not
  change", "$changed returns true if the value of the expression changed". Global
  clock ticks at 1, 3, 5, 7 sample `v` (Preponed) as 0, 1, 0, 0; the first tick is
  not printed. 3: rose (fell 0, stable 0, changed 1); 5: fell (1, 0, 1); 7:
  unchanged (0, 1, 0).
- `annex_override.sv` + `annex_override.c`: SV 36.3.2 L56427-56429: "If a
  user-provided PLI application is associated with the same name as a built-in
  system task or system function (using the PLI mechanism), the user-provided C
  application shall override the built-in". `$reset_count` (D.8) and `$list` (D.6)
  have no llg lowering; the registered applications return 2 and print one line.
  Annex D is informative (D.1 L69791: "for informative purposes only and are not
  part of this standard").
- `vif_array_members.sv` (gate item G1): SV 25.9 L44930-44931 (above). At 0 the
  driver bound to `a` writes `mem[0]` = 50 at once and schedules `mem[1]` <= 60,
  so the display shows `xx` for `mem[1]` until 1; `grid[1][0]` = a0, `flags[1]`
  = c and `mem[0][3:0]` = 0. At 1 it rebinds to `b` (`mem[2]` = 52, `mem[3]` <= 62
  at 2). In the module, `mv` = `a`: `mem[3]` = 50 + 60 = b0; `mem[2]` = 0f, then
  `[7:4]` = 9 and bit 1 = 0 give 9d, whose bit 2 is 1, bit 1 is 0, `[7:4]` is 9
  and `[2 +: 3]` is 111 = 7. At 2 `mv` = `b` writes `grid[0][1]` = 33 and `nv`
  schedules its `[0 +: 2]` <= 00, so `b.grid[0][1]` is 30 at 3.
- `vpi_string_scope_args.sv` + `vpi_string_scope_args.c` (gate item G4), run with
  `--define-system-task 'task $show(string s, logic [7:0] b)'`: SV 32.9 gives
  `$sdf_annotate ( sdf_file [ , module_instance ...] )` and D.4/D.7 `$input` and
  `$log` file-name arguments. Each string literal reaches the calltf as a
  `vpiConstant` (type 7) whose `vpiStringVal` is the text, `8'h41` reads as "A",
  and `tb`/`u_sub` reach it as module handles named `tb` and `tb.u_sub`.
- `builtin_override.sv` + `builtin_override.c` (gate item G3): the application
  registers `$random`, the example of SV 36.3.2 L56427-56431 ("a user could
  write a random number generator as a PLI application and then associate the
  application with the name $random, thereby overriding the built-in $random
  function"). llg runs its built-ins natively and does not support the override
  (decision S99-D6); this design does not call `$random`, so nothing is
  overridden and the run prints `no built-in overridden`.

- `vif_sensitivity.sv` (gate-fixes-2 item 1, decision S99-D7): SV 9.2.2.2.1
  L11184-11186: "The implicit sensitivity list of an always_comb includes the
  expansions of the longest static prefix of each variable or select expression
  that is read within the block or within any function called within the
  block"; SV 9.4.2.2 L11899: "All net and variable identifiers that appear in the
  statement will be automatically added to the event expression"; SV 9.2.2.3
  L11231: "All statements in 9.2.2.2 shall apply to always_latch"; SV 25.9
  L44930-44932 (above). Each line shows the changes made one time unit
  earlier. `v` binds `x` until 5. At 1 the members of `x` change: every
  procedure follows them (`c_el` = `x.mem[2]` 12, `c_var` = `x.mem[i]` with
  `i` = 1, 11; `nib` = 05; `@*` runs once; `v.b` = 16). At 2 `x` changes again
  (37, 32; `@*` run 2), at 3 `i` = 2 selects `x.mem[2]` = 32, and at 4 the
  unbound `y` changes, which wakes nothing (runs stays 2). At 5 `v` = `y`
  re-runs every procedure on `y` (48, 42; run 3; `y.b` = 49), and the change of
  the now unbound `x` at 6 wakes nothing. At 7 the latch closes (`en` = 0) while
  `y` changes (6a, 62): the latch keeps 48, the others follow (run 4, `y.b` = 6b),
  and at 8 `i` = 1 selects `y.mem[1]` = 61.

- `null_handle_access.sv` (gate-fixes-2 item 2, decision S99-D8): SV 25.9
  L44893-44895: "A virtual interface shall be initialized before referencing a
  component of the virtual interface; it has the value null before it is
  initialized. Attempting to use a null virtual interface shall result in a
  fatal run-time error."; SV 8.4 L9653-9654: "Accessing nonstatic members (see
  8.8) or virtual methods (see 8.19) via a null object handle is illegal. The
  result of an illegal access via a null object is indeterminate, and
  implementations may issue an error." Each `+case=<n>` (0 to 8) runs one
  access through the null `v` or `c`: a member read stored to `k`, a member
  read in a display, an array element read and write, a bit-select write, a
  method call, a function that reads `v.a`, and a class property read stored
  and displayed. Every case prints `before`, reports the error at the access
  and exits with status 1; `k` keeps 5, and neither the display that made the
  access nor `after` prints.

- Implicit `$finish` ordering (gate-fixes-2 item 3, decision S99-D9): no
  fixture here and no change. SV 24.3 L43228-43232 ends the simulation
  "immediately after all the threads ... originating from all initial
  procedures within all programs have ended", and SV 4.4.2.6 L3184-3185 lets
  the Reactive region's events "be processed in any order", so llg's
  implicit `$finish` before a same-step assertion action conforms. The
  portable case `S99-D9_implicit_finish_reactive` records it.

- `class_method_scope.sv` (gate-fixes-2 item 4, decision S99-D10): SV 21.2.1.6
  L36379-36381: "The %m format specifier does not accept an argument. Instead,
  it causes the display task to print the hierarchical name of the design
  element, subroutine, named block, or labeled statement that invokes the system
  task containing the format specifier." The subroutine is the method; llg
  names it below its class and the class below the scope that declares it. At
  0: `C.f`, `C.t`, `C.s` (a static method) for the compilation-unit class `C`;
  `E.f` for the override called through a `C` handle; `C.base_only` for the
  method `E` inherits (through `$sformatf`); `C.N.n` for the class nested in
  `C`; `p.D.g` for the class in package `p`, named as the package function
  `p.pf` is; `P.h` for the specialization `P#(4)`; `tb.mf` for the module
  function. At 1 each instance of `sub` names its own class `L` (`tb.u1.L.k`,
  `tb.u2.L.k`), and at 2 each generate block its class `G` (`tb.g[0].G.t`,
  `tb.g[1].G.t`).

## Procedural concurrent assertions (`procedural_assertions.sv`, gate item G5)

Eight modules under `tb`, finishing at 46; `clk` rises at 5, 15, 25, 35 and 45
in every module. Lines of one time step are compared as a set, because 16.15.1
runs all actions of a step in the Reactive region in no stated order.
Decisions S99-D1 to S99-D5 in [the register](../../../../../docs/lrm_decisions.md)
have portable cases for the rules used here.

Clauses used:

- SV 16.15.6 L26748-26751: "a procedural concurrent assertion is not
  immediately evaluated when reached in procedural code. Instead, the assertion
  and the current values of all constant and automatic expressions appearing in
  its assertion arguments (see 16.15.6.1) are placed in a procedural assertion
  queue associated with the currently executing process."
- SV 16.15.6 L26757-26765: "In the Observed region of each simulation time step,
  each pending procedural assertion instance that is currently present in a
  procedural assertion queue shall mature ... if the current time step is one
  that corresponds to that assertion instance's leading clocking event, an
  evaluation attempt of the assertion begins immediately within the Observed
  region. If the assertion's leading clocking event has not occurred in this time
  step, the matured instance shall be placed on the matured assertion queue,
  which will cause the assertion to begin an evaluation attempt upon the next
  clocking event".
- SV 16.15.6 L26782-26798: the clock is inferred from the procedure before
  default clocking when there is no blocking timing control, exactly one event
  control and exactly one `edge_identifier expression1 [ iff expression2 ]`
  event expression, and "No term in expression1 appears anywhere else in the
  body of the procedure."
- SV 16.15.6.2 L27017-27023: a flush point is reached when "The process, having
  been suspended earlier due to reaching an event control or wait statement,
  resumes execution", on an `always_comb`/`always_latch` re-run, and when "The
  outermost scope of the process is disabled". L27061-27063 (the `b1` example):
  "an Observed region will always be reached after the queueing of a2 and before
  a flush point. Thus a2 will always mature. For a3, during time steps where
  either a or b changes after it has been queued, the assertion will always be
  flushed".
- SV 16.15.6.4 L27134-27140: "A specific procedural concurrent assertion may be
  disabled. Any pending procedural instances of that assertion are cleared from
  the queue." and the outermost-scope disable "flushed and all pending assertion
  instances on the queue are cleared."
- SV 24.3 L43229-43231: the simulation "shall terminate by means of an implicit
  call to the $finish system task" once every program's initial procedures have
  ended, so `prg` waits past the end of `tb`.

Derivation:

- `inferred`: `clk2` rises at 3, 9, 15, 21, 27, 33, 39, 45; `a` is 1 from 12
  (sampled 1 from 15). `i1` infers `posedge clk`: 5 fail, then 15, 25, 35, 45
  pass. `i2` infers `posedge clk iff en` (`en` 1 from 12 to 22): one instance,
  15 pass. `i3`'s procedure reads `clk`, so default clocking `posedge clk2`
  applies: the instance from 5 begins at 9 (fail), from 15 at 15, from 25 at 27,
  from 35 at 39, from 45 at 45 (pass).
- `queued`: `en` is 1 from 12 to 32, so `q1` (`b` = 0) fails at 15 and 25 and the
  loop queues `q2` twice there (two passes each). `q3` is queued at every rise of
  `clk` and waits for `clk2` (rises at 10, 30): one pass at 10 (from 5) and two at
  30 (from 15 and 25); those from 35 and 45 would begin at 50.
- `flushed` (the LRM `b1` example, default clocking `posedge clk`): at 12 `f2` is
  queued and matures, at 13 `f3`; both begin at 15 with `a`=1, `b`=0 (fail). At
  32/33 likewise, passing at 35. At 42 `f2` is queued (it matures at 42); at 43
  `f3` is queued, then the nonblocking `b <= 0` resumes `b1`, which flushes `f3`
  and queues `f2` again; at 44 `f3` is queued. At 45 `a` = `b` = 0: `f2` passes
  twice and `f3` once.
- `disabled` (16.15.6.4): at 12 `d1` is queued and passes at 15; at 22 it is
  queued and then cleared by `disable d1` (no line at 25). At 32 `d2` is queued
  and passes at 35; at 42 it is queued and then `disable b2` (from the
  nonblocking `clear <= 1`) clears `b2`'s queue (no line at 45).
- `engines`: one instance of each assertion is queued at 15 (`en` is 1 from 12
  to 22, `a` is 1 at 15 and 0 at 25): `e2` (`s_eventually a`) passes and `e4`
  covers at 15; `e1` (`a ##1 !a`) and `e3` (`a |=> !a`) pass at 25.
- `late`: `@(cb)` and the program's `@(posedge clk)` resume after the Observed
  region of the clock's step; their instances mature when the Observed region
  is reached again in that step and begin at that tick (S99-D4): `l1` fails at 5
  and passes at 15, 25, 35, 45; `l2` fails at 5 and passes at 15.
- `once`: the `initial` queues `o1` at 7, which begins at 15 (pass); the fork
  branches queue `o2` at 11 (pass at 15) and `o3` at 21 (fail at 25).
- `locals`: sequence and property local variables belong to the attempt and
  are not rejected as automatic variables; `d` counts the clocks, so `v1` and
  `v2` pass one clock after each attempt begins: 15, 25, 35, 45.

## Rejections

| Fixture | Diagnostic | Why |
| --- | --- | --- |
| `neg_global_clock.sv` | `:7:22: built-in system subroutine `$global_clock` (IEEE 1800-2009 14.14) is not implemented by llg` | SV 14.14 L19973-19976: `$global_clock` "may be invoked anywhere that a clocking event may be specified"; llg has no lowering for it as an event expression (gate failure, see the SIM-099 report). |
| `neg_inferred_clock.sv` | `:6:7: ... `$inferred_clock` (IEEE 1800-2009 16.15.7) ...` | `$inferred_clock` as a property formal default is not lowered. |
| `neg_void_string.sv` | `:5:11: the discarded string result of `$sformatf` in statement position is not supported` | A string-valued built-in in a void cast is a limit, not a VPI task. |
| `neg_view_input_select.sv` | `:12:15 cannot assign to input port 'data'` | SV 25.5 L43860: modports declare directions; a select of an input member through the view is still a write to it (frontend diagnostic). |
| `neg_unregistered_annex_task.sv` | `llg: VPI error: no VPI registration for `$key` (called at <path>:7)` | No lowering and no application: the run stops at startup naming the call, never a no-op. |
| `neg_unregistered_annex_function.sv` | `... `$reset_count` (called at <path>:7)` | Same for a function. |
| `neg_vif_array_whole.sv` | `:11:12: unpacked array member `mem` of virtual interface `bus_if` is supported only through element selects ...` | A whole array member has no per-instance value in llg; only elements are dispatched on the bound instance. |
| `neg_vif_array_update.sv` | `:10:5: a compound assignment to an element of an unpacked array member through a virtual interface is not supported ...` | Only `=` and `<=` dispatch on the bound instance. |
| `neg_vif_array_increment.sv` | `:10:5: this write to an element of an unpacked array member through a virtual interface is not supported ...` | The same for increments, decrements and output arguments. |
| `neg_vpi_string_variable.sv` | `:5:18: argument of VPI system task `$input`: a string variable or expression is not supported (only string literals reach VPI applications) in `tb`` | A string variable has no fixed-width argument image in llg's VPI call path. |
| `neg_builtin_override.sv` (with `builtin_override.c` linked by `--dpi-lib` or loaded by `LLG_VPI_PLUGIN`) | `llg: VPI error: a VPI application registers `$random`, which llg implements as a built-in (called at <path>:6); llg does not support overriding built-in system tasks and functions (IEEE 1800-2009 36.3.2)` | SV 36.3.2 requires the override, which llg does not support; the run stops at startup instead of silently running the built-in (S99-D6). |
| `neg_vif_sensitivity_formal.sv` | `:12:12: the implicit sensitivity of `tb` cannot follow a virtual-interface member that a called function reads through a handle other than a virtual-interface variable (IEEE 1800-2009 9.2.2.2.1, 25.9)` | SV 9.2.2.2.1 follows reads inside called functions; a handle formal has no value when the process arms its wait, so the read is rejected instead of left out (S99-D7). |
| `neg_vif_sensitivity_method.sv` | `:13:19: the implicit sensitivity of `tb` cannot follow the storage a virtual-interface method call reads ...` | always_comb would follow the method body, whose instance the handle selects at run time. |
| `neg_vif_sensitivity_handle.sv` | `:11:19: the implicit sensitivity of `tb` cannot follow a virtual-interface member read through a handle that is neither a variable nor a formal ...` | An element of a virtual-interface array is a handle expression the arming process does not evaluate again. |
| `neg_procedural_automatic.sv` | `:9:47: automatic variable `i` in a procedural concurrent assertion takes its value when the assertion is queued (IEEE 1800-2009 16.15.6.1); llg does not capture queued values` | SV 16.15.6.1 L26882-26884: a procedural concurrent assertion "shall not sample any const expression or automatic variable, but shall instead save the value of the expression or variable at the time the assertion evaluation attempt is added to the procedural assertion queue"; llg keeps no values with a queued instance. |
| `neg_procedural_const.sv` | `:9:45: a const cast in a procedural concurrent assertion takes its value when the assertion is queued ...` | The same rule for a `const'` cast. |
| `neg_procedural_edge.sv` | `:8:5: the clock inferred from the procedure (`edge` of the expression at line 6, column 17) cannot clock a concurrent assertion in llg ...` | SV 16.15.6 c) infers `edge clk`; llg assertion clocks are a posedge or a negedge. |

## Built-in inventory (`builtin_inventory.tsv`)

Every Slang built-in name (`vendor/slang/scripts/systemnames.txt`) must be called
by a positive simulator fixture (any `tests/fixtures/sim` source not named
`neg_*`/`bad_*`, comments ignored) or be classified in this table, never both;
its `not-implemented` rows must equal the lowering's compile-time rejection
table. The check reads files only (about 0.2 s).
