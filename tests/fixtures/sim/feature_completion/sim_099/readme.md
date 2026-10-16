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

## Rejections

| Fixture | Diagnostic | Why |
| --- | --- | --- |
| `neg_global_clock.sv` | `:7:22: built-in system subroutine `$global_clock` (IEEE 1800-2009 14.14) is not implemented by llg` | SV 14.14 L19973-19976: `$global_clock` "may be invoked anywhere that a clocking event may be specified"; llg has no lowering for it as an event expression (gate failure, see the SIM-099 report). |
| `neg_inferred_clock.sv` | `:6:7: ... `$inferred_clock` (IEEE 1800-2009 16.15.7) ...` | `$inferred_clock` as a property formal default is not lowered. |
| `neg_void_string.sv` | `:5:11: the discarded string result of `$sformatf` in statement position is not supported` | A string-valued built-in in a void cast is a limit, not a VPI task. |
| `neg_view_input_select.sv` | `:12:15 cannot assign to input port 'data'` | SV 25.5 L43860: modports declare directions; a select of an input member through the view is still a write to it (frontend diagnostic). |
| `neg_unregistered_annex_task.sv` | `llg: VPI error: no VPI registration for `$sreadmemh` (called at <path>:7)` | No lowering and no application: the run stops at startup naming the call, never a no-op. |
| `neg_unregistered_annex_function.sv` | `... `$reset_count` (called at <path>:7)` | Same for a function. |

## Built-in inventory (`builtin_inventory.tsv`)

Every Slang built-in name (`vendor/slang/scripts/systemnames.txt`) must be called
by a positive simulator fixture (any `tests/fixtures/sim` source not named
`neg_*`/`bad_*`, comments ignored) or be classified in this table, never both;
its `not-implemented` rows must equal the lowering's compile-time rejection
table. The check reads files only (about 0.2 s).
