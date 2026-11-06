# SIM-025 fixtures: postponed output and monitor dependencies

Exact-output fixtures for `$strobe`, `$monitor`, `$monitoron/$monitoroff`,
`$fstrobe`, `$fmonitor` and `$fclose` (`tests/sim_feature_completion/sim_025.rs`).
Every positive fixture runs in both optimizer modes and on both packed-value
backends. Expected files are hand-derived from the clauses below. Clause
quotes are from `SystemVerilog-1800-2009.txt` (line numbers of the
`pdftotext -layout` extraction); `llg policy` marks what the text leaves open.

## A01: final settled values (`monitor_final`, `strobe_final`, `scope_time`, `static_subroutines`)
- `monitor_time_args` (T1-D7): `$time`, `$stime`, `$realtime` and expressions of them alone print but trigger nothing, also where a whole-array read marks the list changed (21.2.3 L36517-36518, quoted in the fixture). A simulator misses `2 a 5` (a slice actual of a helper) and `1 15 d` (a helper's own read); the text asks for both, so those two lines follow it.

- Postponed region, L3210-3213: "$monitor, $strobe and other similar events
  are scheduled in the Postponed region." / "No new value changes are allowed
  to happen in the current time slot once the Postponed region is reached."
- `$strobe`, L36482-36484: "end of the current simulation time, when all the
  simulation events have occurred for that simulation time, just before
  simulation time is advanced."
- `$monitor`, L36516-36519 and L36530-36531: "each time a variable or an
  expression in the argument list changes value-with the exception of the
  $time, $stime, or $realtime system functions-the entire argument list is
  displayed at the end of the time step as if reported by the $display task.
  If two or more arguments change value at the same time, only one display is
  produced that shows the new values."
- `monitor_final`: `a` changes in Active (`10`, `20`), again in the NBA
  region (`30`) and drives `b` through an `always`; `x`, `r` and `s` change
  in the same slot. One report per slot shows `a=30 b=31` and the nested
  expressions (`{s, "!"}`, `s.len()`, a function call, `a + b`) re-evaluated
  on the settled values. A change of only `x`, only `s` or only `r` reports.
- `strobe_final`: each `$strobe` prints once, after the NBA region and the
  `#0` (Inactive) update, so `a` reads 50 although the call saw 1. Three
  strobes issued by a loop in one slot all print the end-of-slot values.
  Reports of one slot appear in reverse call order, the last issued first
  (decision S25-D6, industry-standard behaviour: the text names no order).
- `strobe_container_actuals`: queue, dynamic and associative actuals of input
  formals, element records and strings, nested calls and an assignment pattern
  all read at the end of the slot (21.2.2). The two pattern lines are derived by
  hand from 21.2.2 because other simulators reject a pattern actual there.
- `strobe_built_actuals`: a conversion between container kinds and an unpacked
  concatenation are built by the evaluator from the end-of-slot values (L36482-36484,
  S25-D8), and helpers with string or container formals report the same way. Some
  simulators read the concatenation at the call (6, not 7) and reject the queue
  and string `$monitor` lines; the file is derived by hand from the clauses.
- `neg_strobe_container_conversion`: an actual with a side effect cannot be built
  in the Postponed region, so it rejects at the call and names the formal.
- `scope_time`: `$time` alone never triggers a report (L36517-36518); two
  `$fmonitor` lists registered by different instances stay active together
  (L36676-36677) and `%m` names each instance.
- `dynamic_members`: storage chosen through a class or virtual-interface
  handle has no fixed signal to watch, so a `$monitor` whose arguments call a
  method, read a handle formal, print an object with `%p` or select a
  virtual-interface member re-evaluates at every settled slot and reports when
  the value differs (a rebound handle included). A process handle prints
  `process` in the Postponed region.
- `class_property_reports` (decision S25-D9): properties through a handle, a
  chain, a handle array element, static and parameterized classes, with strings,
  reals, containers and selects. L18646-18652 bars only variables of automatic
  subroutines. L36516-36519 sets each report; the lines at 3, 4, 6, 11 and 23 show
  no report for a write of the same value, an equal value through a new object
  and a write to an object the path no longer selects. Some simulators print a
  property read through a module handle as it stood at the `$strobe` call
  (`30 fs`, `31 st`) and reject the `$monitor` lines, so the file is derived by
  hand from these clauses.
- `exact_class_triggers`: L36516-36519 ties a report to a change of the list; the
  evaluation counter in the printed list shows no evaluation in a slot where
  nothing on the path changed, which is what keeps a monitor on a path as cheap as
  one on fixed storage.
- `class_method_reports` (decision S25-D9): a report in a method, a task, a
  constructor and a fork branch of a method. The report keeps its object after
  `c = null`; a monitor keeps it until replaced. The same derivation.
- `report_receiver_gc`: L36516-36519 keeps the list active, so the object it reads
  stays reachable; collector statistics show only the replaced `e` freed.
- `static_handle_formals`: string, chandle, class and process formals and a
  string local of static subroutines (L18646-18652 bars only automatic ones) read
  at the end of the slot (L36482-36484): `s=x` before any later call, `l=L
  s=later` after the stores, `v=2`. The `$monitor` lines are derived by hand
  because some simulators reject a string in `$monitor`; the others match them.
- `static_subroutines`: SV 13.3.2 bans only variables of automatic
  subroutines (L18646-18652). Static formals and locals and module variables
  are reported at the end of the slot. Within one slot strobes print before
  monitors (llg policy).

## A02: replacement, enable and file outputs (`monitor_control`, `file_monitors`, `file_strobes`)

- L36533-36534: "Only one $monitor display list can be active at any one
  time; however, a new $monitor task with a new display list can be issued any
  number of times during simulation." `monitor_control` replaces the list and
  then writes the first list's signal: no stale report.
- L36536-36541: "The $monitoron and $monitoroff tasks control a monitor flag
  ... A call to $monitoron shall produce a display immediately after it is
  invoked, regardless of whether a value change has taken place ... By
  default, the monitor flag is turned on at the beginning of simulation."
  Two consecutive `$monitoron` calls print twice with equal values. The flag
  is separate from the display list: a `$monitor` issued while the flag is off
  stays silent until `$monitoron` (llg reading of "flag", decision S25-D1).
- L36675-36679: "Unlike $monitor, any number of $fmonitor tasks can be set up
  to be simultaneously active. However, there is no counterpart to $monitoron
  and $monitoroff tasks." `file_monitors` keeps seven lists active: two on one
  file, one on a file descriptor, one on a multichannel descriptor of two
  files, one on standard output and a `$monitor`; `$monitor` neither replaces
  nor is replaced by them, and `$monitoroff` would not affect them.
- L36635-36636: "Active $fmonitor and/or $fstrobe operations on a file
  descriptor or multichannel descriptor are implicitly cancelled by an $fclose
  operation. The $fopen function shall reuse channels that have been closed."
  `file_monitors` closes a file, reopens the freed channel in the same slot
  and checks that the new file receives only its own monitor (no stale
  registration of the old channel). `file_strobes` cancels a pending
  `$fstrobe`; a multichannel strobe keeps its remaining channels (llg choice,
  decision S25-D2).
- In one slot, monitors report first in registration order, then strobes in
  reverse call order (decision S25-D6, industry-standard behaviour).

## A03: illegal forms and safe failure (`neg_*`, `invalid_descriptors`, `postponed_readonly`)

- L18646-18652: "Because variables declared in automatic tasks are
  deallocated at the end of the task invocation, they shall not be used in
  certain constructs that might refer to them after that point: ... They shall
  not be traced with system tasks such as $monitor and $dumpvars."
  `neg_auto_task_monitor` and `neg_auto_local_strobe` (a loop variable) are
  rejected by the frontend with the variable name and location.
- L3213: "Within this region, it is illegal to write values to any net or
  variable or to schedule an event in any previous region within the current
  time slot." `neg_strobe_helper_write` calls a function that writes a module
  variable; `postponed_readonly` shows that helpers with state of their own
  are allowed and that the reports leave the design variables and the time
  unchanged.
- `invalid_descriptors`: a zero, unknown (X), unopened or closed descriptor
  registers nothing and the other monitors keep running. `$ferror` of the
  closed multichannel descriptor is 0 (`closed err=0`; decision S25-D7,
  industry-standard behaviour: L37203-37205 gives the error code of "the most
  recent file I/O operation" and does not cover a descriptor that is not an
  open file descriptor; changed: it was 1). The unopened file descriptor is
  `32'h8000_0011`: a descriptor number beyond an implementation's descriptor
  table is outside the text, and some simulators stop the run on one, so the
  fixture uses a small number that no `$fopen` here returns (it was
  `32'h8000_007f`).
- `null_report`, `null_monitor` (decision S25-D9): L9653-9654: "Accessing
  nonstatic members (see 8.8) or virtual methods (see 8.19) via a null object
  handle is illegal. The result of an illegal access via a null object is
  indeterminate, and implementations may issue an error." A report evaluates at
  the end of the slot, so only the handle it finds then matters: `null_report`
  prints the first line, whose handle was null at the call, and stops at the
  second. A failed evaluation prints nothing.
- `neg_method_formal_strobe`: a method's formals are automatic (L18646-18652);
  its properties are not.
- Limits, not LRM rules: `neg_monitor_record_property` (a member of an unpacked
  structure property publishes no change) rejects with a located diagnostic. It
  is listed in `docs/known_issues.md`.
