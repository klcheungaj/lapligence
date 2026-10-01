# Frame-resident value cells

`sim_frame_cells` runs these tracked inputs through the CLI in both optimizer
modes. `reuse.sv` checks packed, real and string locals across suspension, early
return, named-block disable and repeated calls to one frame. `finish.sv` checks
zero-resume packed/real cells retained until runtime unwind after `$finish`.
`ref_wait_fork.sv`
checks a local passed by ref to a timing task, joined captures and a detached
capture after task return. `local_event.sv` checks `@(local)` descriptor identity
and cancellation in an initial process; local event controls inside a task are
not currently admitted by lowering. Generated-source
assertions verify intrusive registration and the conservative heap fallback.

IEEE 1800-2009 forbids NBA writes and monitor/strobe deferred tracing of
automatic variables. `nba_return_rejected.sv` and `deferred_rejected.sv` preserve
those frontend rejections. Native `frame_cells_probe.c` independently exercises
a queued NBA whose heap descriptor survives lexical exit alongside intrusive
cells. Clocking automatic sources are also outside the admitted lowering
contract. Typed eligibility tests cover those rejected or native-only retention
edges without claiming unsupported HDL acceptance. Existing DPI/VPI, mailbox,
clocking, assertion, kill and unwind suites remain regression coverage.
