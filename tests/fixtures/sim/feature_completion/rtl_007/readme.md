# RTL-007 fixed zero-time calls and evaluator environments

IEEE 1364-2001 §§10.2-10.3 and 12.4 and IEEE 1800-2009 §§6.5, 6.21-6.22,
9.4.2, 10.3, 10.6.2 and 13.3-13.5 supply the oracles. Every positive source
runs through the public CLI in both optimizer modes and on the legacy and
compact value backends; representative sources also run after snapshot/Db
destruction at native O0/O3.

- `formal_directions` passes fixed arrays and records through input, output,
  inout, ref and const-ref formals, returns and locals of functions and tasks.
  Static subroutines keep result, local and output storage per declaration
  (an omitted static output copies out its retained value); automatic outputs
  start from their typed default.
- `nested_forwarding` forwards ref, const-ref and output formals through nested
  calls and binds selected actuals: array rows, record members, and members of
  unpacked-record array elements with runtime indices (bound once, at the
  call, even when the callee changes the index variable).
- `named_defaults` mixes named, positional, omitted and reordered aggregate
  arguments; a default with an effect runs only when its actual is omitted.
- `early_exits` covers return from nested loops and named-block disable with
  aggregate locals, results and task outputs.
- `ownership` calls package, interface (two instances) and parameterized module
  subroutines; static state belongs to the declaring instance or the package.
- `descriptor_calls` repeats nested ref forwarding, package const-ref,
  hierarchical calls, named/default outputs and early returns with 65,537-cell
  arrays beyond packed capacity (descriptor transport).
- `evaluator_helpers` uses read-only helpers with private loops, locals,
  break/continue, case, private record/concatenation stores, named/default and
  const-ref arguments in continuous assignments, evaluated events and forces,
  including array and record force sources.
- `stateful_helpers` and `stateful_cancel` cover legal helpers that are not
  read-only: visible writes and persistent static state in evaluated events
  (tasks, `always`, `iff`, vector LSB edges) and continuous assignments. The
  waiting process evaluates such an event expression once when the control is
  reached and once after every dependency change (owner policy; the language
  leaves the evaluation count open, so counters are checked as lower bounds).
  Writes from a function called by a continuous assignment are procedural
  (SV 6.5). Disabling a waiting process detaches its wait.
- `descriptor_evaluators` sends a 65,537-cell array to continuous and event
  helpers and to a force source.
- `unused_results` discards results of effectful calls (`void'`, statement
  calls, unused assignments); only short-circuited operands are skipped.
- `stateful_event_witness` and `real_readonly_witness` adopt FND-002's
  L-F09-06-01 witnesses `stateful_event_helper` and `real_readonly_helper`.
- Negatives (language): actual shape/type mismatches, non-assignable output
  actuals, function/conditional temporaries for ref and const ref, delays and
  task calls in functions, output/ref formals in continuous and event
  contexts, and a variable with both a continuous driver and a procedural
  write made by a continuously called function.
- Force sources, monitor/strobe arguments, intra-assignment and nonblocking
  event controls and named-event or real lists with such helpers are covered
  by RTL-007b (`../rtl_007b/`).

All `.out` files are hand-derived from the clauses above.
