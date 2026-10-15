# SIM-044 fixtures: covergroup declarations, instances and sampling

Run by [`tests/sim_feature_completion/sim_044.rs`](../../../../sim_feature_completion/sim_044.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends; negative cases run in both optimizer modes. Line
numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV). The interpretations that needed a choice
are the register decisions
[S44-D1 to S44-D9](../../../../../docs/lrm_decisions.md#covergroups-instances-and-sampling-sim-044).

## Clauses relied on

- 19.3, L32081-32085: "When the covergroup specifies a list of formal
  arguments, its instances shall provide to the new operator all the actual
  arguments that are not defaulted. Actual arguments are evaluated when the new
  operator is executed. A ref argument allows a different variable to be
  sampled by each instance of a covergroup. Input arguments will not track the
  value of their arguments; they will use the value passed to the new
  operator."
- 19.3, L32092-32093: "If a clocking event is specified, it defines the
  event at which coverage points are sampled. Because it is in the scope of
  the covergroup, the clocking event can be based on ref arguments of the
  covergroup."
- 19.3, L32105-32109: "The strobe option (see 19.7.1) can be used to specify
  that coverage points are sampled in the Postponed region, thereby filtering
  multiple clocking events so that only one sample per time slot is taken. The
  strobe option only applies to the scheduling of samples triggered by a
  clocking event. It shall have no effect on procedural calls to the built-in
  sample() method."
- 4.4.2.9, L3212-3214: "No new value changes are allowed to happen in the
  current time slot once the Postponed region is reached."
- 19.7.1, L33423-33424 (Table 19-3): strobe "When true, all samples happen at
  the end of the time slot, like the $strobe system task."; L33534 (Table
  19-4): `strobe` can be set for a covergroup ("Yes"), not for a coverpoint or
  cross ("No").
- 19.8, Table 19-5, L33562-33566: start() "Starts collecting coverage
  information"; stop() "Stops collecting coverage information".
- 19.3, L32102-32104: "When the strobe option is not set (the default), a
  coverage point is sampled the instant the clocking event takes place, as if
  the process triggering the event were to call the built-in sample() method."
- 19.3, L32113-32119: "Block event expressions that specify the begin keyword
  followed by a hierarchical identifier denoting a named block, task, function,
  or class method shall be triggered immediately before the corresponding
  block, task, function, or method begins executing its first statement. Block
  event expressions that specify the end keyword followed by a hierarchical
  identifier denoting a named block, task, function, or class method shall be
  triggered immediately after the corresponding block, task, function, or
  method executes its last statement. Block event expressions that specify the
  end of execution shall not be triggered if the block, task, function, or
  method is disabled."
- 19.4, L32233-32235: "An embedded covergroup variable may only be assigned in
  the new method. An embedded coverage group can be explicitly instantiated in
  the new method. If it is not, then the coverage group is not created and no
  data will be sampled."
- 19.5.2, L32746-32749: "The value N is determined as follows: — For an enum
  coverage point, N is the cardinality of the enumeration. — For any other
  integral coverage point, N is the minimum of 2M and the value of the
  auto_bin_max option, where M is the number of bits needed to represent the
  coverage point."
- 19.5.2, L32751-32756: "If the number of automatic bins is smaller than the
  number of possible values (N < 2M ), then the 2M values are uniformly
  distributed in the N bins. If the number of values, 2M, is not divisible by
  N, then the last bin will include the additional remaining items. ...
  Automatically created bins only consider 2-state values; sampled values
  containing X or Z are excluded."
- 19.5.6, L32865-32884: b "shall be evaluated as though it were the right-hand
  side of an assignment to a variable whose type is type(e)", a warning is
  issued "If e is unsigned and b is signed with a negative value" or "If
  assigning b to a variable of type type(e) would yield a value that is not
  equal to b", a warned singleton "shall not participate in the bins values",
  and a warned range is "treated as containing the intersection of the values
  in the range and the values expressible by type(e)". `bins.sv` reuses the
  clause's own examples (L32888-32908).
- 19.7, L33340-33341: "Option assignment statements in the covergroup
  definition are evaluated at the time that the covergroup is instantiated."
- 19.8, L33569-33572: "The get_coverage() method returns the cumulative (or
  type) coverage, which considers the contribution of all instances of a
  particular coverage item; and it is a static method that is available on both
  types (via the :: operator) and instances (using the . operator). In
  contrast, the get_inst_coverage() method returns the coverage of the
  specific instance on which it is invoked".
- 19.11, L33808-33818: a zero denominator means "The covergroup does not
  contribute to the overall coverage score" and "If the covergroup's weight is
  non-zero, a value of 0.0 is returned by get_coverage and
  get_inst_coverage"; "$get_coverage shall return a value of 100.0 if called on
  a design that has no covergroup instances".
- 19.11.1, L33858-33860: "If there is no value or transition associated with a
  bin, the bin is ignored and shall not contribute to the coverage
  computation."
- 19.11.3, L33937-33938, L33953-33954: "When type_option.merge_instances is
  false, type coverage is computed as the weighted average of all instances."
  "Likewise, the type coverage of a coverpoint or cross is computed from the
  coverage of that coverpoint or cross in each instance, weighted by
  option.weight in the coverpoint or cross scope for each instance."

## Expected values

`instances.sv` (A01): `v` is 3 bits (8 values).

- Before any instance the type has no instance: 0.00 (weight 1), and
  `$get_coverage` is 100.00.
- `a` (auto_bin_max 8, weight 1), `b` (2 bins `<0:3>`, `<4:7>`, weight 3) and
  `c` (8 bins, weight 0) are unsampled: every coverpoint has bins, so each
  instance is 0.00 and the type is 0.00.
- `a` samples 1: 1/8 = 12.50; `b` is unchanged at 0.00. `b` samples 5 (bin
  `<4:7>`): 1/2 = 50.00; `a` is unchanged.
- Type: (1 x 12.5 + 3 x 50 + 0 x 0) / (1 + 3 + 0) = 40.625, the same through
  an instance. Coverpoint `cp` type: the coverpoint weights are all 1, so
  (12.5 + 50 + 0) / 3 = 20.833.
- `b.stop()` drops the sample of 0; `a.cp.stop()` drops the sample of 2; `a`
  then samples 3: `a` covers {1, 3} = 25.00, `b` stays 50.00, `a.cp` 25.00.
- After `a = null` the instance still counts: type (25 + 150) / 4 = 43.75, and
  `$get_coverage` is the only type's 43.75.

`sampling.sv` (A02):

- `ev` samples `d` at each `posedge clk`, before the slot's nonblocking update
  `d <= d + 1`: values 0 and 1 of 4, 50.00; `d` is then 2.
- `ws.sample(d)` with `d = 1` samples 1 (25.00) although `d` becomes 3
  afterwards; `ws.sample(2'd2)` adds 2: 50.00.
- `step(1)`: `at_begin` samples `calls` = 0 before the first statement (bins
  zero/one: 50.00); `at_end` samples 1 after the last statement (1 of 5 bins:
  20.00).
- `halt(9)` is disabled after `calls = 9`: no end sample, 20.00 with
  `calls` = 9. `halt(4)` ends with `calls` = 5: bin `five`, 40.00.

`embedded.sv` (A01, A02):

- `a1` (level 3) reports 1 and 2: `ck` 2/4 = 50, `cl` low 1/2 = 50: 50.00.
  `a2` (level 12) reports 1: `ck` 25, `cl` high 50: 37.50.
- Clock edge 1 samples level 3 (`a1`) and 12 (`a2`). `a1` then gets level 4
  and its last handle is dropped; 50000 allocations under `LLG_GC_STRESS`
  collect garbage, and edge 2 still samples `a1` (levels {3, 4}: 12.50) and
  `a2` ({12}: 1/16 = 6.25).
- `$get_coverage`: `on_clock` type (12.5 + 6.25) / 2 = 9.375, `on_call` type
  (50 + 37.5) / 2 = 43.75, `held` has no instance yet: (9.375 + 43.75) / 2 =
  26.5625.
- `held` keeps the `Item` handle argument alive after `it = null`: `item.p`
  = 2 of 4 values, 25.00.

`bins.sv`:

- 19.5.6 examples: `b1` = {1, [2:5], [6:7]}, `b2` = {[1:7]}, `b3` = {1,
  [2:3]}, `b4` = {-1, [1:3]}; the nine warnings are the clause's (b1: [6:10];
  b2: -1, [1:10], 15; b3: [2:5], [6:10]; b4: [1:10], 15) plus the 4-bit-wide
  value 15 of bins `never` on 3-bit `p1`.
- `p1` = 5, `p2` = 0: `c1` both bins (100.00); `c2` neither (0.00). `p1` = 0,
  `p2` = -1: `c1` 100.00; `c2` adds `b4`: 50.00.
- `ce` (enum, 3 members) samples BLUE twice: 33.33. `cs` (signed, 3 bins)
  samples -4 and -1: bins `<-4:-3>` and `<-2:-1>`, 66.67 (decision S44-D2).
  `ci` samples only when `en` is 1 (value 1): 1/8 = 12.50. `cu`: `never` has
  no value and is excluded, `only_seven` is hit by 7: 100.00.

`ref_formals.sv`: `gx` and `gx2` read `x` (values 1 and 3: 25.00), `gy` reads
`y` (2: 12.50); the input argument `i` keeps its value at `new` (6 for `gx`
and `gy`, 1 for `gx2`): 12.50 each. Type: ((25 + 12.5) / 2 + 12.5 + (25 +
12.5) / 2) / 3 = 16.67.

`storage.sv`: `only_point` and `only_guard` are read only by the coverpoint
and its guard: the first edge samples 2 (guard 1), the second edge is guarded
off: 1/4 = 25.00. `only_sample` (9) reaches the group as a sample argument:
1/16 = 6.25.

`instance_events.sv` (A02): each instance waits on its own event; `wait
fork` in the constructing process returns at once (S44-D5). Three rising
edges at times 1, 3 and 5 see `b0.data`, `va` = 0, 1, 2 and `src.val` = 0, 2,
0 (each is updated after the edge's negative half).

- `m0` (owner property `vif` = `b0`): {0, 1, 2} of 4 = 75.00; `m1` (`b1`) sees
  one edge with `b1.data` = 0: 25.00.
- `ra` (`ref` clock `ca`, value `va`): 75.00; `rb` (`ref` `cb`, `vb`, the
  second binding variant) one edge with 0: 25.00.
- `h` (input handle `src`): {0, 2} = 50.00.
- `m0.cg.stop()`: the edge with data 3 counts nothing (75.00); after
  `start()` the next edge samples 3: 100.00. Type: (100 + 25) / 2 = 62.50.

`dropped_waiters.sv` (A02, under `LLG_GC_STRESS=1`): `keep` sees the four
edges with values 0-3: 100.00. Iteration k constructs a monitor and drops its
only handle before edge k; it still samples edges k..3: 100, 75, 50, 25. Type:
(100 + 100 + 75 + 50 + 25) / 5 = 70.00.

`strobe.sv` (A02): in slot 1, `v` = 1 and two rising edges of `clk` happen
before the nonblocking update `v <= v + 4` sets 5 (each edge schedules it
from `v` = 1).

- `called` (strobe, no event): `p.sample()` counts `v` = 1 at once, 100.00
  before the slot ends (strobe does not delay `sample()`).
- `at_event` (`at_least` 2): two immediate samples of 1: `lo` 100.00, `hi`
  0.00.
- `strobed`: one Postponed sample of 5: `lo` 0.00, `hi` 100.00;
  `strobed_twice` (`at_least` 2) has one sample only: 0.00.
- `Box` (per-instance strobe): the edge of `c` comes with `x` = 4, then `x`
  becomes 6 in the same slot; the Postponed sample sees 6: `four` 0.00, `six`
  100.00.

`block_returns.sv` (A02):

- `f(2)` returns early with `n` = 2 and result 20: the end sample counts `two`
  (1 of 3, 33.33, S44-D7). `f(0)` runs to the end with `n` = 100: 66.67.
- `t(5)` returns early with `n` = 5 (50.00); `t(6)` ends with 7: 100.00.
- `pause(3)` waits 1 time unit, then returns early with `n` = 3: 50.00 at time
  1 (a task that waits).
- `blk` runs three times with `n` = 0, 1, 2 at entry: the begin event covers
  all three bins (100.00). Only the first run reaches its end (`n` = 30); the
  second is disabled and the third leaves by `break` (S44-D8): 1 of 3, 33.33.

`null_sample.sv`: `c` was never constructed; `c.sample()` is a null handle
access and stops the run after `before`.

## Negative cases

| Fixture | Rejected construct | Owner |
| --- | --- | --- |
| `neg_strobe` | `type_option.strobe` in a coverpoint (Table 19-4) | frontend |
| `neg_merge_instances` | `type_option.merge_instances = 1` | SIM-045 |
| `neg_option_access` | `c.option.weight = 2` after instantiation | SIM-046 |
| `neg_two_arg_query` | `get_inst_coverage(covered, total)` | SIM-046 |
| `neg_wide_point` | an 80-bit coverpoint | llg limit (64 bits) |
| `neg_import_event` | a block event on an imported function, which has no body | llg limit |
| `neg_embedded_outside_new` | embedded `new` outside the class `new` (19.4) | frontend |
| `neg_sample_arguments` | `sample()` without its declared argument (19.8.1) | frontend |
