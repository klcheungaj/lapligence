# SIM-043 fixtures: VPI callbacks, control and evaluator integration

Each `<name>.sv` has a C application `<name>.c` (a `vlog_startup_routines`
table). `tests/sim_feature_completion/sim_043.rs` loads it as a run-time
plugin (`LLG_VPI_PLUGIN`) or links it as a static archive (`--dpi-lib`), and
runs both optimizer modes on the legacy and compact value backends. Every
line of the `.out` files is derived below from the clauses quoted. Line
numbers refer to `docs/specification/SystemVerilog-1800-2009.txt` (SV) and
`Verilog-1364-2001.txt` (V); the decisions that the text leaves open are the
portable cases `tests/fixtures/sim/lrm_decisions/S43-D*`.

## Clauses

- SV 4.10 Table 4-1 (L3606-3622): `cbAfterDelay` Pre-Active,
  `cbNextSimTime` Pre-Active, `cbReadWriteSynch` "Pre-NBA or Post-NBA",
  `cbAtStartOfSimTime` Pre-Active, `cbNBASynch` Pre-NBA, `cbAtEndOfSimTime`
  Pre-postponed, `cbReadOnlySynch` Postponed.
- SV 4.4.3.10 (L3285-3287): "The Postponed region provides a PLI callback
  control point that allows PLI application routines to create read-only
  events after processing all other regions. PLI cbReadOnlySynch and other
  similar events are scheduled in the Postponed region."
- SV 4.4.2.9 (L3212-3214): "No new value changes are allowed to happen in the
  current time slot once the Postponed region is reached. Within this region,
  it is illegal to write values to any net or variable or to schedule an
  event in any previous region within the current time slot."
- SV 38.36.2 (L64939-64962): "cbAtStartOfSimTime Callback shall occur before
  execution of events in a specified time queue." / "cbNBASynch Callback
  shall occur immediately before the nonblocking assignment events are
  processed." / "cbReadWriteSynch Callback shall occur after execution of
  events for a specified time. This time may be before or after nonblocking
  assignment events have been processed." / "cbAtEndOfSimTime Callback shall
  occur after execution of nonblocking events, but before entering the
  read-only phase of the time slice." / "cbReadOnlySynch Callback shall occur
  the same as for cbReadWriteSynch, except that writing values or scheduling
  events before the next scheduled event is not allowed." / "cbNextSimTime
  Callback shall occur before execution of events in the next event queue."
  / "cbAfterDelay Callback shall occur after a specified amount of time,
  before execution of events in a specified time queue. A callback can be set
  for any time, even if no event is present."
- SV 38.36.2 (L64969-64973): "vpiSuppressTime (or NULL for the
  cb_data_p->time field) will result in an error." / "These fields shall
  contain the requested time of the callback or the delay before the
  callback."
- SV 38.36.2 (L64976-64979): "Attempting to place a cbAtStartOfSimTime
  callback with a delay of zero when simulation has progressed into a time
  slice and the application is not currently within a cbAtStartOfSimTime
  callback." / "Attempting to place a cbReadWriteSynch callback with a delay
  of zero at read-only synch time."
- SV 38.36.1 (L64642-64644): "cbValueChange After value change on some
  variables, any expression, or terminal or after execution of an event
  statement. Specifically excluded are class objects, dynamic arrays,
  strings, queues, and associative arrays."
- SV 38.36.1 (L64740-64743): "When a simulation event callback occurs, the
  application shall be passed a single argument, which is a pointer to an
  s_cb_data structure (this is not a pointer to the same structure that was
  passed to vpi_register_cb()). The time and value information shall be set
  as directed by the time type and value format fields in the call to
  vpi_register_cb()."
- SV 38.36.1 (L64761-64763): "For a cbValueChange callback, if the obj has
  the vpiArrayMenber property set to TRUE, the value in the s_cb_data
  structure shall be the value of the array member that changed value. The
  index field shall contain the index of the rightmost range of the array
  declaration."
- SV 38.36.3 (L65007-65010): "cbEndOfCompile End of simulation data structure
  compilation or build" / "cbStartOfSimulation Start of simulation (beginning
  of time 0 simulation cycle)" / "cbEndOfSimulation End of simulation
  (simulation ended because no more events remain in the event queue or a
  $finish system task executed)".
- SV 38.39 (L65372-65374): "The routine shall return a 1 (true) if successful
  and a 0 (false) on a failure. After vpi_remove_cb() is called with a handle
  to the callback, the handle is no longer valid."
- SV 38.4 (L61983-61991): "vpiStop Causes the $stop built-in SystemVerilog
  system task to be executed upon return of the application routine. This
  operation shall be passed one additional integer argument, which is the
  same as the diagnostic message level argument passed to $stop (see 20.2)."
  / "vpiFinish Causes the $finish built-in SystemVerilog system task to be
  executed upon return of the application routine." (L61992-61995: vpiReset
  runs `$reset`, Annex D, which llg does not provide.)
- SV 24.3 (L43228-43232): "If there is at least one initial procedure within
  at least one program block, the entire simulation shall terminate by means
  of an implicit call to the $finish system task immediately after all the
  threads and all their descendent threads originating from all initial
  procedures within all programs have ended."
- V 27.33.2 (L45613-45621, L45639) states the same time reasons without
  `cbNBASynch`/`cbAtEndOfSimTime` (added in 1364-2005); V 27.33.1 (L45360):
  "cbValueChange After value change on an expression or terminal, or
  execution of an event statement".

## regions (A01)

All time callbacks are registered from `cbStartOfSimulation` at time 0 with a
delay (S43-D2) of 5, `cbAfterDelay` with 10 and `cbNextSimTime`.

- `[0] cbEndOfCompile`, then a second `cbEndOfCompile` registered inside it is
  accepted but never runs (`handle none`: the action already occurred, and
  the handle stays valid until it is removed, S43-D4), then
  `[0] cbStartOfSimulation` (38.36.3).
- Time 5, Pre-Active: `cbAtStartOfSimTime`, then `cbNextSimTime #1`: the slot
  after its registration time (S43-D3); both Pre-Active, in queue order.
- Active: `initial` prints, writes `a=1`; the value-change callback runs at
  the write (S43-D6) and prints `00000001` (vpiBinStrVal of 8 bits), registers
  a second watch on `a` and removes itself (`remove first watch: 1`).
- Pre-NBA: `cbReadWriteSynch` (S43-D1) runs first and puts `c=7`: the select
  watch on `c[0]` prints `1 index=0` (bit 0 changed, S43-D7) inside the put,
  then the callback prints `put c=7 (none)`; the write wakes `always @(c)`,
  which runs before the slot goes on: `active: c=7`. Then `cbNBASynch`, last
  before NBA; NBA: `b<=2` and `clk<=1` commit; `value: b=2` at the commit; the
  woken `always @(b)` prints `active: b=2`.
- Observed: the clocking event of `cb` (posedge `clk`) wakes the module
  process, which prints before the Reactive set (SIM-034 S34-D4).
- Reactive: `program` prints (its thread stays alive, so 24.3 does not end
  the run).
- Pre-Postponed: `cbAtEndOfSimTime` reads `a=1 b=2 c=7`; Postponed:
  `$strobe`; Postponed PLI: `cbReadOnlySynch` tries `c=9`, gets
  `LLG_VPI_READ_ONLY` and reads `c=7` (S43-D5).
- Time 10: `cbAfterDelay` (registered first) then `cbNextSimTime #2`
  (re-registered at 5). `a=3` and `a=4` each call the second watch (no
  coalescing); the removed first watch is silent. `mem[2]=8'h5a` calls the
  element watch with `index=2`, the rightmost index; `mem[1]` does not.
- Time 20: the display, `$finish`, then `[20] cbEndOfSimulation a=4`.

## reentry (A01, A03)

- `vpi_get_cb_info` returns the registration (reason 1, user data, value
  format 6 = vpiIntVal). Releasing a callback handle does not remove the
  callback (`release kept handle -> 1`, and `kept` runs at 2).
- `cbStmt` and `cbForce` are not provided, a parameter is not watchable, a
  NULL object is an invalid handle and a `vpiSuppressTime` time callback is
  an error (L64969-64970); each returns NULL. A value-change callback on the
  whole unpacked array `m` is accepted (S43-D7); a second one is removed at
  once (`1`), which drops its watch on every element.
- At 1 and 2: `second` then `first`, the newest registration first (S43-D6).
  At v=2, `second` has already run when `first` removes it; removing it again
  fails (`0 LLG_VPI_HANDLE`, 38.39). `first` registers `third`, which starts
  with the next change.
- At 3: `canceller` removes the pending `doomed` (never prints). The handle of
  `kept-1`, a one-shot that ran at 1, is still valid (S43-D4): its info gives
  reason 9 (`cbAfterDelay`) and user data `kept-1`; `vpi_remove_cb` then frees
  it (`1`) and the handle is stale (`LLG_VPI_HANDLE`). 65544 late
  `cbEndOfCompile` registrations are accepted; the runtime keeps at most 65536
  fired callback handles (`LLG_VPI_MAX_FIRED_CALLBACKS`), so the oldest is
  freed and stale while the newest still reads reason 10. Then `third v=3`,
  `first v=3`, and the write `m[0] = 0` runs the whole-array callback with
  the element's index: `[3] m[0] v=0`.
- End: `first=3 second=2 third=1`.

## readonly (A02)

`timescale 1ns/100ps`; `$peek(v)` returns `v+100` and tries to write `tb.x`.

- Time 0: the continuous assignment `e = d + 1` changes the net `e` from z to
  1 after `always @(e)` is waiting (decision T0-D2), so it prints `e=1` and
  issues `q<=2`, which wakes `always @(q)`.
- `$monitor` at time 0 calls `$peek` in its Postponed evaluator: the write is
  `LLG_VPI_READ_ONLY` (4.4.2.9), the monitor prints `peek=101 x=1`.
- 2.5: the real watch prints `r=1.50` with a scaled time `2.5` (the time type
  of the registration, L64740-64743).
- Pre-NBA: the read-write callback (S43-D1) puts `d=5` with no error; the
  continuous assignment updates `e=6`, `always @(e)` prints and issues
  `q<=12`, which commits in NBA and wakes `always @(q)`: ordinary downstream
  events.
- Postponed: `$strobe` and `$monitor` evaluate `$peek(2)=102`; both writes
  are rejected.
- Postponed PLI: `cbReadOnlySynch` (L64947-64948): a no-delay put, a delayed
  put and a force are all rejected; zero-delay `cbReadWriteSynch` (L64979)
  and `cbReadOnlySynch` callbacks are refused; a read-write callback 1.0 later
  is accepted and runs at 3.5. `d` is still 5.
- 12.5: the final display; `x` stayed 2 (the rejected writes changed nothing).

## control (A03)

- In `cbStartOfSimulation`: `vpiReset` is not provided (`0
  LLG_VPI_UNSUPPORTED`, S43-D8).
- 3: `vpiStop` from a Pre-Active callback returns 1; stderr reports `$stop at
  time 3` and the default `resume` policy continues the same slot, so the HDL
  prints `after the stop`.
- 7: the HDL prints, then the read-write callback's `vpiFinish` returns 1 and
  a second request also returns 1 without a second report. The run ends after
  the callback returns: the `cbAfterDelay` at 9 and the HDL display at 27 never
  run. The final procedure prints `steps=2`, then `cbEndOfSimulation` runs once
  at 7; inside it control requests return 1 and change nothing, and a new
  `cbEndOfSimulation` registration is accepted but never runs (`handle none`,
  S43-D4).
- With `--stop-policy exit` (test only) the stop at 3 suspends the run; closing
  the model runs `cbEndOfSimulation` once at 3 and no final procedure.

## start_control (A03)

`vpi_control` from `cbStartOfSimulation` is honoured when the callback returns
(S43-D10); `SIM043_START_CONTROL` selects the request (the tests set it).

- `stop`: `vpiStop` returns 1 and stderr reports `$stop at time 0`. Under the
  default `resume` policy the run continues: `[0] hdl: initial`, `[2] hdl:
  later`, the final procedure at 2 and `cbEndOfSimulation` at 2.
- `stop` with `--stop-policy exit`: the run is suspended before any process
  runs; closing the model runs `cbEndOfSimulation` once at 0 and no final
  procedure.
- `finish`: `vpiFinish` with level 7, outside the `$finish` levels 0-2, is
  accepted (S43-D8) and returns 1; stderr reports `$finish at time 0` (level
  1, the default), no process runs, the final procedure prints `[0] final` and
  `cbEndOfSimulation` runs at 0.

## catalog

Without `--vpi`, `--dpi-lib` or a user system task call site the model has no
VPI object catalog: the run-time application gets callbacks (`after delay` at
2) but `tb.v` is `missing`, and stderr names `--vpi`. With `--vpi` it is
`found`.
