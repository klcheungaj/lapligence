# Unused-storage optimizer references

`indirect_references.sv` runs through the public `llg` CLI in the default and
`--no-opt` modes (`sim_opt_storage`). Every index or clock in it is never
written and is read by one construct that names it outside a plain expression:
an event-array index of `->`, `->>`, `.triggered` and an event task argument, a
queue index of a method receiver and of a class-handle argument, and the clock
of `->> @(posedge clk)` and of a clocked nonblocking assignment. Before the
storage audit the optimizer omitted their storage and the default mode failed
to emit the model. `opt_storage_differential` also compares this and every
other fixture with the pass on and off.

Expected output (both modes):

```
ev[0].triggered after -> = 1
method call on item 7
handle argument item 7
woke at 1 2 3; late fired 0; never = 0
```

and runtime stderr `llg: $finish at time 4 at tb:64:5`.

Clause basis (`docs/specification/SystemVerilog-1800-2009.txt`):

- Every index is 0 and `clk` stays 0. 6.8, line 5307: "Table 6-7 contains the
  default values for variables if no initializer is specified." and line 5315:
  "2-state integral '0".
- `woke[0] = 1` and `woke[2] = 3`. 15.5.1, line 20703: "Named events
  triggered via the -> operator unblock all processes currently waiting on that
  event." 15.5.2, line 20719: "The @ operator blocks the calling process until
  the given event is triggered."
- `ev[0].triggered after -> = 1`. 15.5.3, lines 20735-20736: "The triggered
  event property evaluates to true if the given event has been triggered in the
  current time step and false otherwise."
- `woke[1] = 2`, and `late fired 0`. 15.5.1, lines 20708-20711: "The effect of
  the ->> operator is that the statement executes without blocking and it
  creates a nonblocking assign update event in the time in which the delay
  control expires or the event control occurs. The effect of this update event
  shall be to trigger the referenced event in the nonblocking assignment region
  of the simulation cycle." `clk` never changes, so `@(posedge clk)` never
  occurs.
- `never = 0`. 9.4.5, lines 12123-12124: "An intra-assignment delay or event
  control shall delay the assignment of the new value to the left-hand side, but
  the right-hand expression shall be evaluated before the delay, instead of
  after the delay."
- Waiters only record their wake time, and one process prints after all of
  them, so the output does not depend on the order of processes within a time
  slot (4.7, line 3466: events of the same region are "processed in any
  order").
