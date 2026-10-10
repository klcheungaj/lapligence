# SIM-025 fixtures: postponed output and monitor dependencies

Exact-output fixtures for `$strobe`, `$monitor`, `$monitoron/$monitoroff`,
`$fstrobe`, `$fmonitor` and `$fclose` (`tests/sim_feature_completion/sim_025.rs`).
Every positive fixture runs in both optimizer modes and on both packed-value
backends. Expected files are hand-derived from the clauses below. Clause
quotes are from `SystemVerilog-1800-2009.txt` (line numbers of the
`pdftotext -layout` extraction); `llg policy` marks what the text leaves open.

## A01: final settled values (`monitor_final`, `strobe_final`, `scope_time`, `static_subroutines`)

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
  Reports of one slot appear in call order (llg policy: the text names no
  order).
- `scope_time`: `$time` alone never triggers a report (L36517-36518); two
  `$fmonitor` lists registered by different instances stay active together
  (L36676-36677) and `%m` names each instance.
- `dynamic_members`: storage chosen through a class or virtual-interface
  handle has no fixed signal to watch, so a `$monitor` whose arguments call a
  method, read a handle formal, print an object with `%p` or select a
  virtual-interface member re-evaluates at every settled slot and reports when
  the value differs (a rebound handle included). A process handle prints
  `process` in the Postponed region.
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
- Reports of one slot appear in registration order (llg policy).

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
  registers nothing and sets the I/O error status (L37201-37204,
  "$ferror ... error encountered by the most recent file I/O operation"); the
  other monitors keep running.
- Limits, not LRM rules: `neg_static_string_formal` (a static task keeps a
  string formal in the call activation) and `neg_class_method_strobe` (a
  deferred report in a class method) reject with a located diagnostic. They
  are listed in `docs/known_issues.md`.
