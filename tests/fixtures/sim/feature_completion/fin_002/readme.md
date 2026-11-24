# FIN-002 composed cross-feature scenarios

Run by [`tests/sim_feature_completion/fin_002.rs`](../../../../sim_feature_completion/fin_002.rs).
Six class-based testbench designs (`tb_*`) and six gate-level designs
(`gl_*`) each combine six or more features so that their interactions
matter. Every scenario runs in both optimizer modes on the legacy,
compact/portable and compact/GMP value backends.

**Oracle.** The expected transcripts are industry-standard behaviour: each
`.out` is what the design prints under an independent IEEE 1800-2009
implementation. Lines for which that is not possible are derived from the
clause text and marked *clause-derived* below. Lines of one time step are
compared as a set, because 4.7, L3465-3466, leaves their order open: "One
source of nondeterminism is the fact that active events can be taken off the
Active or Reactive event region and processed in any order." Random values
differ between tools, so the randomized scenario prints invariants only.

Clause line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt`, which is not part of the repository.

**Portability switches.** Three sources keep a second form for tools that
lack a construct; the transcript is the same:

- `tb_cov_assert.sv`: `CHECKER_AS_MODULE` replaces the checker by a module
  with the same assertions (the suite runs both forms); `NO_VACUOUS_CONTROL`
  leaves out `$assertvacuousoff` for tools whose pass actions never run on a
  vacuous success.
- `gl_cells_sdf.sv`: `CLI_SDF` leaves the annotation to `--sdf` (the suite
  runs both forms).
- `gl_pad_force.sv` relies on `PATHPULSE$` limits taking effect without an
  invocation option (decision A8-D5).

## Testbench scenarios

### `tb_force_clocking.sv` (VPI application `shared_force_vpi.c`)

A generated three-stage pipeline (`always_comb`, `always_ff`, a latch on the
output) sits behind an interface with a clocking block and modports. A class
driver and monitor use it through `virtual bus_if.tb`. The interface holds a
concurrent assertion clocked by the clocking block event. The VPI
application forces and releases the interface variable the clocking block
samples (driven by the pipeline through an output port), forces and releases
a flip-flop variable inside a generate block, and deposits a third.

- 14.13, L19909-19911: "All clocking block inputs (input or inout) are
  sampled at the corresponding clocking event. If the input skew is not an
  explicit #0, then the value sampled corresponds to the signal value at the
  Postponed region of the time step skew time units prior to the clocking
  event (see Figure 14-1 in 14.4)." So `mon dout=` at an edge is the value
  before the edge.
- 14.13, L19921-19923: "Upon processing its specified clocking event, a
  clocking block shall update its sampled values before triggering the event
  associated with the clocking block name. This event shall be triggered in
  the Observed region."
- 14.16, L20064-20067: "For zero skew clocking block outputs with no cycle
  delay, synchronous drives shall schedule new values in the Re-NBA region of
  the time step corresponding to the clocking event. For clocking block
  outputs with non-zero skew, or drives with non-zero cycle delay, the
  corresponding signal shall be scheduled to change value in the Re-NBA
  region of a future time step." The drives land 2 after each edge.
- 14.16.2, L20294-20297: "It is possible to use a procedural assignment to
  assign to a signal associated with an output clockvar. When the associated
  signal is a variable, the procedural assignment assigns a new value to the
  variable, and the variable shall hold that value until another assignment
  occurs (either from a drive to a clocking block output, or another
  procedural assignment)." The test sets the reset values this way at time 0.
- 16.5, L21447: "All variables in a concurrent assertion use the value
  sampled in the Preponed region of a time slot".
- 38.34, L64186-64190: "vpiForceFlag The object shall be forced to the passed
  value with no delay (same as the SystemVerilog procedural force). Argument
  time_p shall be ignored and can be set to NULL. vpiReleaseFlag The object
  shall be released from a forced value (same as the SystemVerilog procedural
  release)."
- 10.6.2, L13390-13393: "When released, then if the variable is not driven by
  a continuous assignment and does not currently have an active assign
  procedural continuous assignment, the variable shall not immediately change
  value and shall maintain its current value until the next procedural
  assignment to the variable is executed." L13393-13395: "Releasing a
  variable that is driven by a continuous assignment or currently has an
  active assign procedural continuous assignment shall reestablish that
  assignment and schedule a reevaluation in the continuous assignment’s
  scheduling region."
- 38.36.1, L64642-64644: "cbValueChange After value change on some variables,
  any expression, or terminal or after execution of an event statement."

Clock edges are at 5, 15, 25, ...; an input accepted at edge k leaves the
pipeline (plus 1 + 2 + 3 = 6) after edge k + 2 and is sampled at edge k + 3.

| Lines | Why |
| --- | --- |
| `[58] forced bus.dout: bus=99 dut=16` | The force names the interface variable; the pipeline's own output keeps 16. |
| `[65] mon dout=99 held=16`, `[65] a_sum failed` | The clocking block and the assertion sample the forced value; the latch reads the pipeline's output. |
| `[68] released bus.dout: bus=26 dut=26` | `bus.dout` is driven through an output port, so it holds the driven value when the VPI release returns (decisions CS-D3, CS-D4). |
| `[78] forced stage 1 ...`, `[88] released stage 1: q=200` | A flip-flop variable keeps the forced value after the release until its next procedural assignment at 95. |
| `[95]`, `[105] a_sum failed`; `mon dout=203` twice | Stage 2 captured 200 + 3 at 85 and again at 95. |
| `[108] deposited stage 0: q=5 nxt1=83`, `[135] a_sum failed`, `mon dout=10` | A no-delay put is overwritten by the next clocked assignment; 5 + 2 + 3 = 10 arrives two edges later. |
| `vpi changes=15: ...` | Every change of `bus.dout`, the force (99) and the release (26) included. |

### `tb_dpi_disable.sv` (DPI-C companion `tb_dpi_disable_dpi.c`)

Four class jobs (an abstract base with a pure virtual timed `step`, a static
counter, a process handle) run in named fork branches and write through a
`ref` structure with a queue member. A context import declared in a package
calls exported functions of module `tb` (after `svSetScope`), which disable
one job by block name and kill another through its process handle while both
wait; a third job is disabled while its import task waits in an exported
task. Two recursive re-entrant timed tasks with `ref` queues run beside them;
a semaphore, an event, a mailbox and an associative array carry the evidence.

- 9.6.2, L12387-12388: "The disable statement shall terminate the activity of
  a task or a named block. Execution shall resume at the statement following
  the block or following the task-enabling statement."
- 9.7, L12619: "KILLED means the process was forcibly killed (via kill or
  disable)." L12631-12632: "The kill() function terminates the given process
  and all its subprocesses, that is, processes spawned using fork statements
  by the process being killed."
- 13.5.2, L19103-19104: "Arguments passed by reference are not copied into
  the subroutine area, rather, a reference to the original argument is passed
  to the subroutine."
- 35.5.3, L55743: "Some DPI imported subroutines require that the context of
  their call be known."
- 35.9, L56270-56271: "When an exported task returns due to a disable, it
  shall return a value of 1. Otherwise, it shall return 0." L56272-56273:
  "When an imported task returns due to a disable, it shall return a value of
  1. Otherwise, it shall return 0." L56274-56275: "Before an imported
  function returns due to a disable, it shall call the API function
  svAckDisabledState()."
- 15.3.4, L20441-20442: "If the specified number of keys is available, the
  method returns a positive integer and execution continues. If the specified
  number of keys is not available, the method returns 0."
- 15.5.1, L20703: "Named events triggered via the -> operator unblock all
  processes currently waiting on that event."

| Lines | Why |
| --- | --- |
| `[30] job3 completed`, `job3 slept=30` | Three `c_sleep(10)` steps through the exported task. |
| `[35] cancel job0 -> 1`, `job0 (0): steps=4 last=3 status=KILLED` | The export disabled block `job_0` while the virtual `step` waited in its fourth `#10`; 9.7 calls a disabled process KILLED. |
| `[45] kill job1 -> 1`, `steps=5` | Killed inside its fifth `c_sleep`. |
| `[55] job2 disabled`, `steps=6` | Disabled inside its sixth `c_sleep`. |
| `left=`, `right=` | Each activation pushes its depth, waits, recurses and pushes the negated depth. |
| `lock free=0` | Job 0 held the key when it was disabled; nothing returns it. |
| `acknowledged=2` | *Clause-derived* (35.9, decisions S41-D5 and A18-D1): the exported task returned 1 to the C code of job 1 (killed) and job 2 (disabled), which acknowledged. Some simulators never report the disable to the C side and print 0. |

### `tb_cov_assert.sv`

An arbiter state machine is checked by concurrent assertions with pass and
fail action blocks, a multiclock assertion into a slower domain, a deferred
assertion in `always_comb`, a checker instance and an immediate assertion in
a class method. The action blocks sample covergroups (bins, a cross,
`option.at_least`) and update a class tally; another covergroup samples the
state on the clock with transition bins. `$assertkill` cancels an attempt
before its action block runs and a `force` of the grant makes the checker
fail.

- 16.15.1, L26464: "The pass and fail statements of an assert statement are
  executed in the Reactive region."
- 16.14.2, L25975-25978: "|=> synchronizes between posedge clk0 and posedge
  clk1. Starting at the point at which the implication is being evaluated,
  for each match of s0 clocked by clk0, time is advanced from the end point
  of the match to the nearest strictly future occurrence of posedge clk1, and
  from that point there must exist a match of s1 clocked by clk1."
- 16.4.1, L21262-21264: "In the Observed region of each simulation time step,
  each pending assertion report that has not been flushed from its queue
  shall mature, or be confirmed for reporting. Once a report matures, it may
  no longer be flushed."
- 17.3, L28284-28286: "A checker may be instantiated wherever a concurrent
  assertion may appear (see 16.15) with the following exceptions: it shall be
  illegal to instantiate checkers in fork...join, fork...join_any, or
  fork...join_none blocks."
- 20.11, L35054-35055: "$assertkill shall abort execution of any currently
  executing specified assertions and then stop the checking of all specified
  assertions until a subsequent $asserton."
- 20.12, L35137-35138: "$assertvacuousoff shall stop execution of the pass
  action of all the specified assertions on vacuous success until a
  subsequent $assertpasson."
- 19.7, L33273-33274: "Minimum number of hits for each bin. A bin with a hit
  count that is less than number is not considered covered."
- 19.5.1, L32543-32544: "It represents the value of coverage point at two
  successive sample points, that is, value1 followed by value2 at the next
  sample point."
- 19.8, L33584-33587: "The get_coverage() and get_inst_coverage() methods
  both accept an optional set of arguments, a pair of int values passed by
  reference. When the optional arguments are specified, the get_coverage()
  and get_inst_coverage() methods assign to the first argument the number of
  covered bins and to the second argument the number of coverage bins defined
  for the given coverage item."

`clk` rises at 5, 15, ...; `sclk` at 7, 21, 35, 49, 63, 77, 91, 105, ...

| Lines | Why |
| --- | --- |
| `[35]`, `[65] a_g0 passed` | Requests sampled at 25 and 55; the grant is sampled one cycle later. |
| `[75] a_g1 failed`, `[135] a_g1 passed` | At 55 both request; grant 0 wins, so grant 1 is not there within two cycles. |
| `[63] a_ack failed`, `[91]`, `[105]`, `[161] a_ack passed` | After a rising grant sampled at 35, 65, 85 and 135, the next strictly later `sclk` tick and one more; `ack` there is the grant one `sclk` tick earlier. The force at 142 keeps the grant up at 147. |
| `[50] deferred a_req ...` | Both requests while idle, reported once when the step's value is final. |
| `[107] assertions killed` and no `a_g0` at 115 | The attempt started at 105 is aborted. |
| `[145] chk a_cause failed`, `chk a_mutex failed` | The forced grant `11` is sampled at 145. |
| `result cov=72.22 ... cross=50.00`, `who bins 2/3`, `cross bins 3/6` | Samples (1,1) twice, (0,2), (1,2): 2/2, 2/3 and 3/6 bins; (100 + 66.67 + 50) / 3. |
| `ack cov=50.00` | `ok` has 3 hits, `bad` 1 with `at_least` 2. |
| `fsm cov=83.33` | Five of six bins: no `G0 => G1` at successive samples. |

The discarded two-argument queries (`void'(...)`) were a defect this scenario
found; its focused regression is
[`sim_046/discarded_counts`](../sim_046/readme.md).

### `tb_rand_mailbox_kill.sv`

Packets (an abstract base with a static counter, a derived class and a
further derived class, virtual `describe`, `cost` and `legal`) are randomized
under `inside`, `dist`, implication, `solve before` and array size
constraints with `randc`, `pre_randomize` and `post_randomize`, and travel
through a bounded mailbox inside a parameterized channel class with static
members. `disable fork` kills the first consumer while it holds a packet and
a semaphore key; a second consumer takes over. Processes killed while blocked
in a mailbox `get` and a semaphore `get` must not take later messages or
keys.

- 9.6.3, L12548: "The disable fork statement terminates all active
  descendants (subprocesses) of the calling process."
- 15.4.5, L20548-20549: "The get() method retrieves one message from the
  mailbox, that is, removes one message from the mailbox queue. If the
  mailbox is empty, then the current process blocks until a message is placed
  in the mailbox."
- 15.4.2, L20504: "The num() method returns the number of messages currently
  in the mailbox."
- 18.4.2, L29417-29418: "Variables declared with the randc keyword are
  random-cyclic variables that cycle through all the values in a random
  permutation of their declared range."
- 18.5.4, L29677-29678: "An exception to this rule is a weight of zero, which
  is treated as a constraint."
- 18.5.5, L29750-29751: "This states that if the expression is true, then
  random numbers generated are constrained by the constraint (or constraint
  set)."
- 18.6.2, L30434-30435: "Every class contains pre_randomize() and
  post_randomize() methods, which are automatically called by randomize()
  before and after computing new random values." 18.6.3, L30473: "If
  randomize() fails, post_randomize() is not called."
- 18.8, L30648-30650: "Inactive variables are not randomized by the
  randomize() method, and their values are treated as state variables by the
  solver." 18.9, L30749-30750: "When a constraint is inactive, it is not
  considered by the randomize() method."
- 18.13.3, L31082-31083: "The srandom() method allows manually seeding the RNG
  of objects or threads. The RNG of a process can be seeded using the
  srandom() method of the process (see 9.7)."
- 8.24, L10756-10758: "Each specialization of a class has a separate set of
  static member variables (this is consistent with C++ templated classes)."

| Lines | Why |
| --- | --- |
| `[23] A killed: status=KILLED took=5`, `A took packet#0 packet#1 big#2 packet#3 packet#4` | The generator puts every 2 units into a depth-2 mailbox, consumer A takes 5 units per packet: gets at 0, 5, 10, 15 and 20. Every third packet is the big class. |
| `key free=0 queued=2` | A held the key; packets 5 and 6 wait, the generator is blocked on 7. |
| `[120] B took=19 first=big#5 last=big#23` | B starts at 25 and takes one packet per 5 units. |
| `created=24 puts=24 gets=24 illegal=0`, `kinds counted=23` | Nothing is lost; A never counted packet 4. |
| `[122] after killed waiters: queued=1 key=1 value=0` | The killed reader and the killed key waiter took nothing. |
| `channel instances: packet=1 int=2` | One static per specialization. |
| `randc permutations=1 pre=16 post=16` | Two cycles of eight values each. |
| `conflict returns 0 pre=1 post=0` | `len >= 6` with `kind == CTRL -> len <= 2`. |

### `tb_vif_swap.sv` (DPI-C `tb_vif_swap_dpi.c`, VPI `shared_force_vpi.c`)

Two generated channels on different clocks each have an interface instance
with a clocking block, a modport and a timed task, and feed an accumulator
through an interface port. A class monitor parameterized by its virtual
interface type waits on an edge, on a clocking block event and in a `wait`
through one handle; the test rebinds the handle while all three waits are
pending. The monitor folds what it sees through a pure DPI-C function and the
VPI application reads the same interface variables by name.

- 25.9, L44868: "A virtual interface is a variable that represents an
  interface instance." L44862-44863: "A virtual interface allows the same
  subprogram to operate on different portions of a design and to dynamically
  control the set of signals associated with the subprogram."
- 25.9, L44930-44931: "Once a virtual interface has been initialized, all the
  components of the underlying interface instance are directly available to
  the virtual interface via the dot notation."
- 15.5.2, L20719: "The @ operator blocks the calling process until the given
  event is triggered."

The text does not say what a pending event control on `vif.clk` does when
`vif` is rebound; the transcript is the industry-standard behaviour, which
decision S09-D6 already records for `ref` formals.

| Lines | Why |
| --- | --- |
| `[22] edge monitor: 22` | Channel 0's clock is low and channel 1's high at 22: the rebinding is a rising edge of `vif.clk`, and channel 1 is valid with 22. |
| `[25] clocking monitor: 22` | The clocking wait still ends at channel 0's edge and then reads channel 1's sample. |
| `[36] 3 beats reached` | The `wait` now reads channel 1's count. |
| `[42] monitor moved to channel 0`, no monitor line at 42 | Both clocks are low: no edge. |
| `edges=9 ticks=9 folded=349569` | ((((11 x 31 + 22) x 31 + 23) x 31) + 13. |

### `tb_fifo_env.sv` (DPI-C `tb_fifo_env_dpi.c`, VPI `tb_fifo_env_vpi.c`)

A generated FIFO (clocked slots, combinational status, a latched read port)
is driven from a program block by a class environment: a mailbox-fed driver,
a monitor with an embedded covergroup and a parameterized scoreboard whose
reference model lives in C behind a `chandle` and reports through an exported
function. The VPI application's value-change callback on the fill count
schedules a delayed write of `stall`, which the clocking block samples, and a
timed callback that clears it. An overflow assertion clocked by the clocking
block triggers an event the environment counts.

- 24.3.1, L43248-43249: "Statements and constructs within a program block
  that are sensitive to changes (e.g., update events) on design signals are
  scheduled in the Reactive region."
- 38.34, L64179-64180: "All scheduled events on the object shall be removed
  before this event is scheduled." (`vpiInertialDelay`), L64184-64185: "The
  object shall be set to the passed value with no delay. Argument time_p
  shall be ignored and can be set to NULL." (`vpiNoDelay`).
- 38.36.2, L64960-64962: "Callback shall occur after a specified amount of
  time, before execution of events in a specified time queue. A callback can
  be set for any time, even if no event is present." (`cbAfterDelay`).

| Lines | Why |
| --- | --- |
| `[65] vpi: full, stall requested`, `[90] vpi: stall cleared` | The fourth push is accepted at 65; `stall` rises at 67 and falls at 90, so the edges at 75 and 85 sample it. |
| `stalled=4`, `fills=2 clears=2` | Two fills, two stalled cycles each. |
| `pushes=11 pops=11 ... overflows=4` | The script's requests the FIFO accepts; each push while full fails the overflow assertion. |
| `checks: values=46 flags=28 mismatches=0` | The two `scoreboard #(int)` objects share one static counter; the C model agrees on every level, flag and value. |
| `note refused pop=2`, `note refused push=4` | The exported function called from C. |
| `vpi: count changes=17` | 11 pushes and 11 pops, 5 of them in the same cycle as the other. |

## Gate-level scenarios

### `gl_cells_sdf.sv`, `gl_cells_sdf.sdf`

A standard-cell netlist (inverter, NAND, a UDP multiplexer with
state-dependent paths, a UDP flip-flop with a notifier) with edge-sensitive
paths and timing checks, annotated from SDF, under a behavioural clock,
stimulus and reference model.

- 29.9, L49197-49199: "When the input changes, the edge-sensitive cases are
  processed first, followed by level-sensitive cases. Thus, when
  level-sensitive and edge-sensitive cases specify different output values,
  the result is specified by the level-sensitive case."
- 31.3.1, L50983-50985: "The $setup timing check reports a timing violation
  in the following case: (beginning of time window) < (timestamp time) <
  (end of time window)". 31.3.2, L51026-51028: "The $hold timing check
  reports a timing violation in the following case: (beginning of time
  window) <= (timecheck time) < (end of time window)". 31.4.4, L51818-51821:
  "The $width timing check reports a violation in the following case:
  threshold < (timecheck time) - (timestamp time) < limit".
- 31.6, L52040-52041: "Whenever a timing violation occurs, the timing check
  updates the value of the notifier."
- 32.5, L52971-52972: "annotation of an SDF construct can be changed by
  annotation of a subsequent construct that either modifies (INCREMENT) or
  overwrites (ABSOLUTE) it."
- 30.5.2, Table 30-3, L50112-50133: a transition to x takes the smallest
  delay of the transitions from that value, a transition from x the largest
  to that value.
- 30.7.4.2, L50483-50485: "Under normal operation, if the schedule for a
  trailing pulse edge is earlier than the schedule for a leading pulse edge,
  then the leading edge is cancelled."

| Lines | Why |
| --- | --- |
| `[15.410] q=01`, `[15.500] q=11` | Annotated rise of `f0` 0.41; `f1` 0.40 + 0.10 (INCREMENT). |
| `[25.000] f0 notifier`, `[25.410] q=1x` | `d[0]` falls at 24.700 and reaches the flip-flop 0.260 later (annotated `B` path), 0.040 before the edge: inside the annotated 0.600 setup limit. The flip-flop first captures 0 (fall due 25.460), then the notifier makes it x: 0 to x takes the 0.41 rise delay and is due earlier, so the fall is cancelled and x stays at 25.410 (decision CS-D1). |
| `[45.300] f1 notifier`, `[45.550] q=x0` | `d[1]` reaches `f1` 0.300 after the edge: outside the 0.300 specify limit, inside the annotated 0.450. |
| `[62.800]` both notifiers, `q=xx` | A reset pulse of 0.800 against width limits 1.200 (`f0`, annotated) and 1.000. |

The violation reports on stderr are asserted with their check, times and
limits. They name exactly the violations above.

### `gl_strength_bus.sv` (VPI application `gl_strength_bus_vpi.c`)

Three pad cells with tri-state drivers share a bus under a pull-up; beside it
are continuous assignments with strengths on one net, `wand`, `wor`, `tri0`
and `tri1` nets, a driver with an unknown enable over a weak pull-down, and a
supply-strength gate against a strong assignment. A sequential UDP captures
the resolved bus on a behavioural clock and the VPI application reads
strength values.

- 28.12.1, L47610-47611: "If two or more signals of unequal strength combine
  in a wired net configuration, the stronger signal shall dominate all the
  weaker drivers and determine the result."
- 28.12.2, L47654-47656: "When two signals of equal strength and opposite
  value combine, the result shall be a value x, along with the strength
  levels of both signals and all the smaller strength levels."
- 28.6, L47222-47225: "Some combinations of data input values and control
  input values can cause these gates to output either of two values, without
  a preference for either value (see 28.12.2)."
- 6.6.3, L4805-4807: "The wor and trior nets shall create wired or
  configurations so that when any of the drivers is 1, the resulting value of
  the net is 1. The wand and triand nets shall create wired and
  configurations so that if any driver is 0, the value of the net is 0."
- 6.6.5, L5054: "The tri0 and tri1 nets model nets with resistive pulldown and
  resistive pullup devices on them."
- 21.2.1.5, L36268-36270: "The strength of a scalar net is reported in a
  three-character format. The first two characters indicate the strength. The
  third character indicates the current logic value of the scalar and can be
  any one of the values given in Table 21-4."
- 38.15, L62675-62676: "If the format field in the s_vpi_value structure is
  set to vpiStrengthVal, the value.strength pointer shall point to an array
  of s_vpi_strengthval structures."

| Lines | Why |
| --- | --- |
| `step 2 bus=StX`, `q=x` | Two enabled pads drive 0 and 1 with equal strength; the UDP captures x. |
| `maybe=36X` (step 5), `maybe=630` (step 6) | An unknown enable gives H or L from strong down to high impedance over the weak pull-down. |
| `vpi maybe: logic=0 s0=8 s1=64` | A known value with a strength range: weak in `s0`, strong in `s1` (decision CS-D2). |
| `step 7 bus=651`, `vpi bus: logic=1 s0=32 s1=64` | A strong 1 with an unknown enable over the pull-up. |
| `rail=Su0`/`Su1` | The supply-strength gate beats the strong assignment. |

### `gl_switch_network.sv`

A pass-gate multiplexer of `tranif0`/`tranif1`, a chain of three `rtran`
switches against a weak driver at its far end, two tri-state drivers joined
by a `tran`, a supply reached through a switch and a switch with an unknown
control.

- 28.8, L47365: "The bidirectional pass switches shall not delay signals
  propagating through them."
- 28.13, L48297-48298: "The tran, tranif0, and tranif1 switches shall not
  affect signal strength across the bidirectional terminals, except that a
  supply strength shall be reduced to a strong strength."
- 28.14, L48303-48304: "The rnmos, rpmos, rcmos, rtran, rtranif1, and
  rtranif0 devices shall reduce the strength of signals that pass through
  them according to Table 28-8."

| Lines | Why |
| --- | --- |
| `r=St1,Pu1,We1,We0` | Strong, pull and weak along the chain; at the far end the reduced signal (medium) loses to the weak driver, whose value in turn arrives one level weaker and loses at `r2`. |
| `pwr=St1` | The supply through `tranif1` is strong and beats the pull-down. |
| `step 5 y=56X`, `tx=36X` | Unknown controls: the value may or may not pass. |

### `gl_timing_checks.sv`

A set/reset flip-flop cell with `$setuphold` (conditioned), `$recrem`,
`$recovery`, `$removal`, two `$width` checks and `$period`. Each phase
produces one violation, which toggles the notifier and makes the flip-flop x
until the next reset; the last phase shows the condition switching a check
off.

- 31.3.3, L51130-51132: "And the $setuphold timing check reports a timing
  violation in the following case: (beginning of time window) < (timestamp
  time) <= (end of time window)".
- 31.3.5, L51254-51256: "The $recovery timing check reports a timing violation
  in the following case: (beginning of time window) <= (timecheck time) <
  (end of time window)". 31.3.4, L51200-51202: "The $removal timing check
  reports a timing violation in the following case: (beginning of time
  window) < (timestamp time) < (end of time window)".
- 31.4.5, L51890-51893: "The $period timing check reports a violation in the
  following case: (timecheck time) - (timestamp time) < limit".
- 31.7, L52161-52162: "A construct called a conditioned event ties the
  occurrence of timing checks to the value of a conditioning signal."

| Phase | Violation | Notifier |
| --- | --- | --- |
| A | data 0.3 before the clock, setup limit 0.5 | 10.000 |
| B | data 0.2 after the clock, hold limit 0.3 | 20.200 |
| C | clock 0.3 after the reset release, recovery limit 0.6 | 32.300 |
| D | reset release 0.2 after the clock, removal limit 0.4 | 44.200 |
| E | clock pulse 1.0, width limit 2.0 | 55.000 |
| F | clock edges 6.0 apart, period limit 8.0 | 68.000 |
| G | reset pulse 0.6, width limit 1.0 | 80.600 |
| H | clock 0.4 after the set release, recovery limit 0.6 | 90.400 |
| I | data 0.1 before the clock while reset holds `chk_en` low | none |

`violations=8`. The suite also runs it with `--no-timing-checks` and
`--no-notifiers` (no notifier toggles, so `violations=0` and no x;
*clause-derived*) and `--no-timing-check-messages` (same transcript, no
reports).

### `gl_scan_chain.sv`, `gl_scan_chain.sdf` (VPI application `gl_scan_chain_vpi.c`)

An instance array of scan flip-flop cells (UDP multiplexer, sequential UDP
with a notifier, an edge-sensitive path, conditioned setup checks) and
generated majority cells (a combinational UDP with a multiple-source path),
annotated from SDF. A late scan input makes one flip-flop x and the x travels
down the chain and through the majority cells. The VPI application inspects
the primitives, the paths and the timing checks of three cell instances.

- 29.3.4, L48801-48802: "All combinations of input values that are not
  explicitly specified result in a default output state of x."
- 38.23, L63447-63448: "The VPI routine vpi_iterate() shall be used to
  traverse one-to-many relationships, which are indicated as double arrows in
  the data model diagrams." The order is not specified, so the primitive
  lines compare as a set.
- 38.10, L62198: "Synopsis: Retrieve the delays or pulse limits of an
  object."

| Lines | Why |
| --- | --- |
| `[60.000] notifiers xxx0` | The scan input changes 0.2 before the edge at 60: inside `ff[0]`'s 0.4 setup limit. |
| `q=110x`, `q=10x0`, `q=0x00` | The x shifts one flip-flop per edge. |
| `vote=1`, `vote=x`, `vote=0` | The majority UDP is known whenever two known inputs agree. |
| `path tb.dut.ff[2].CK$Q: 0.50 0.55`, `check limit: 0.40`, `0.70`, `0.10` | *Clause-derived*: the delays and limits after annotation (`IOPATH` 0.50/0.55 and `SETUP SI` 0.7 on `ff[2]`); some simulators report the values before annotation, or after it only with an access option. |

30.4.6, L49858-49859: "Multiple module paths may be described in a single
statement by using the symbol *> to connect a comma-separated list of sources
to a comma-separated list of destinations." So the majority cell's
`(A, B, C *> Y)` is three `vpiModPath` objects (decision A19-D9), each with the
delays of its own input: the declared 0.12 and 0.14 in `v[0].m`, the three
annotated `IOPATH` pairs in `v[1].m` (*clause-derived*, as above).

### `gl_pad_force.sv` (VPI application `shared_force_vpi.c`)

Two bidirectional pad cells (tri-state drivers with rise, fall and turn-off
delays, a receiver behind a module path with pulse limits) share a pulled-up
pad net; a capture cell (sequential UDP with a notifier and a setup check)
samples one receiver. The VPI application forces and releases the resolved
net and counts its changes. The suite also dumps this run as VCD and FST and
compares the changes of `pad`, `y` and `q` in both files with the transcript.

- 28.16, L48383: "When a value changes to the unknown (x) value, the delay is
  the smallest of the three delays."
- 30.7, L50266-50269: "Pulses greater than or equal to the error limit pass
  unfiltered. Pulses less than the error limit but greater than or equal to
  the reject limit are filtered to X. Pulses less than the reject limit are
  rejected, and no pulse emerges."
- 30.7.1, L50315: "Pulse limit values may be set from within the specify
  block with the PATHPULSE$ specparam."
- 10.6.2, L13398-13399: "When released, the net shall immediately be assigned
  the value determined by the drivers of the net."

| Lines | Why |
| --- | --- |
| `[32.600] pad=St0`, `[33.200] pad=St1`, `[33.600] y=xx`, `[34.200] y=11` | A 0.600 pulse on the pad between the 0.300 reject and 0.800 error limits shows as x on the receiver. |
| `[42.600] pad=StX` | The pads drive opposite values; to x after the smallest delay. |
| `[46.000] forced: pad=St1 vpi=1`, `[57.000] released: pad=StX` | The force overrides both drivers; the release restores the conflict at once. |
| `[80.000] notifier`, `[80.200] q=x` | The receiver changes 0.300 before the edge, setup limit 0.500. |

## Unsupported gate-level features (`neg_gl_*.sv`)

Step 2 names features llg does not simulate; a netlist that uses one stops
with its own diagnostic and no output:

| Fixture | Diagnostic |
| --- | --- |
| `neg_gl_trireg` | `ELAB-012` `` `trireg` net `store` (trireg charge storage) is not supported by llg `` |
| `neg_gl_mos` | `ELAB-011` `` `nmos` primitive (MOS and resistive switch primitives) is not supported by llg `` |
| `neg_gl_dumpports` | `ELAB-014` ``system task `$dumpports` (extended VCD port dumping) is not supported by llg`` |
| `neg_gl_reset` | `SIM-016` ``VPI error: no VPI registration for `$reset` `` when the model starts |

## Defect regression: `vpi_release_reestablishes.sv`

Scenario 1 found that a release through VPI left a continuously driven
variable at its forced value until an operand of the assignment changed
(decision CS-D4, clauses 38.34 and 10.6.2 above). The fixture releases, one
by one, a variable driven by a continuous assignment, by an output port, an
array element, a constant assignment and a packed structure, the output of
one of six instances that share a generated driver body, one variable only
a procedure writes, and one that a `release` statement and a VPI release
both name. It reads them in the releasing process before it suspends and
again when the time step ends (`$strobe`).

The `end of the time step` lines and every later line are the
industry-standard behaviour. The `at the release(s)` lines are llg's
(*clause-derived*, 10.6.2 "schedule a reevaluation"): some simulators
deliver every driver before `vpi_put_value` returns.

| Lines | Why |
| --- | --- |
| `[3] at the releases: assigned=101 ported=21 row=23,103 both=25 stored=105 tied=77 pair=14`, `s.q=111,114,66` | The operand became 20 while the forces held. The port copy, the constant and the structure copy are delivered inside the VPI release, `both` inside its `release` statement (CS-D3); the expression drivers of `assigned`, `row[1]` and `g[4].s.q` are scheduled and still show the forced value. `stored` keeps 105 and `g[1].s.q` its force. |
| `[3] end of the time step: assigned=22 ... row=23,24 ...`, `s.q=111,65,66` | Every scheduled driver ran: `g[4].s.q` is `seed[4] + 1` of its own instance. |
| `[5] at the release: ... both=107`, `[5] end of the time step: ... both=25`, `s.q=62,65,66` | A VPI release of the variable a `release` statement also names follows the VPI rule: scheduled, not in place. |
| `[7] next operand: ... stored=36` | The procedure assigns `stored` again. |

## Release rule regression: `release_in_place.sv`

Decision CS-D3 rests on the 10.6.2 sentences quoted for scenario 1 and on
4.7, L3468-3470: "At any time while evaluating a procedural statement, the
simulator may suspend execution and place the partially completed event as a
pending event in the event region." A released variable that a continuous
assignment drives holds the driven value in the statement after the release. The portable decision case
covers assignments, ports, parts, structures and small arrays (delayed
assignments are decision CS-D5); this fixture adds the remaining forms. The operand is 2 at the
releases (1 at the forces).

| Lines | Why |
| --- | --- |
| `in the function: 3`, `in the method: 4`, `in the final procedure: 7` | The release repeats the driver wherever the statement runs; the final procedure sees operand 4. |
| `one element: 6,205`, `both elements: 6,7` | *Clause-derived*: elements 4000 and 7 of a 5000-cell array, which llg keeps in a keyed container; each takes `in + 4` and `in + 5` when it is released and the other keeps its force. Some simulators give 0 here. |
| `real: 3.0` | *Clause-derived*: 10.6.2 makes no exception for a real variable. Some simulators keep 99.5 until the operand changes. |
| `released by its driver: 9`, `next operand: 10` | The assignment's function releases its own target while the assignment is evaluated: the evaluation under way delivers the value, the release does not start it again. |

## Release rule regression: `release_shared_instances.sv`

The same rule (CS-D3) where llg shares one generated driver body between
the six instances of `leaf` (parameter `K` 1 or 11), the six iterations of a
generate loop (`direct[i] = in[i] + i`) and the six port connections
(`out[i]`). The operands are `in[i] = 40 + 2i` at the releases. The expected
output is the industry-standard behaviour.

| Lines | Why |
| --- | --- |
| `g[4].u.q at its release: 59`, `g[1].u.twice at its release: 84` | `in[4] + 11` and `2 * in[1]`: the release repeats the driver of the named instance with its own operand and parameter. |
| `direct[5] at its release: 55` | `in[5] + 5`: the iteration's index is the driver's own. |
| `out[5] at its release: 205` | The port connection copies `g[5].u.q`, which is still forced. |
| `four released: q=200,201,202,203,59,205 ...` | The other instances keep their forces. |
| `at the releases: q=41,43,45,57,61 ...` | Every remaining instance, released one after the other, holds its own driven value at once. |
