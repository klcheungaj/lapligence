# Lint rules

Rules read owned `Db`/`DesignModel` through `LintCtx`, with no native traversal or
I/O. `mod.rs` owns registry order, `analysis.rs` shared graph/data-flow helpers,
and rule modules their policies/diagnostics; each module's docs describe its rule.
See [shared lint](../readme.md).

## Behavior beyond the module docs

- `unused-signal` (`unused.rs`): subroutine bodies count wherever the subroutine
  is declared (the body is the Db's recorded `body`; extra scope-only `Begin`
  children of a subroutine with loops or blocks are not it), and so do formal
  defaults. A concatenation, streaming or assignment-pattern write target counts
  for each member (`collect_writes_with_members`). Generate scopes (loop, if,
  case) are analysed like module scopes. Signals connected to a black box (an
  instance of an unknown module, `black_box_connection_signals`) are skipped.
  The `default disable iff` condition a checker instance takes for
  `$inferred_disable` counts as read.
- `width-mismatch` (`width.rs`): a port link compares the port or port expression
  with the connected expression (`.a(n[0])` is one bit), not the whole selected
  signal. A concatenated port (`.p({a, b})`) is judged once, by its name or
  expression, against the sum of the slices its components connect.
- `multi-driver` (`multidriver.rs`): writes resolve through unpacked element
  selects (`collect_element_writes`); a bit range inside an element still counts
  as disjoint. Each assignment of a checker procedure drives only its constant
  target bits (the 17.7.1 Single Assignment Rule keeps them disjoint), so two
  checker procedures assigning different bits are not two drivers.
- `unconnected-port` (`unconnected_port.rs`): top ports are external boundaries;
  interface connections refer to actual instances, without frontend-generated
  copies.
- `undriven-signal` (`undriven.rs`): drivers also include connected output/inout
  flow, primitive output terminals and timing-check notifiers; output, inout and
  non-`const` `ref` actuals of any call (DPI imports, tasks,
  `$sscanf`/`$fscanf`/`$fgets`/`$fread`/`$readmem*`/`$sformat` destinations,
  `std::randomize` arguments, which Slang wraps in an assignment operation) and
  the `$random` seed; assignments in expression position (`for` initializers and
  steps, compound `+=`), `++`/`--`, `foreach` and declared `for` variables,
  concatenation/streaming targets (each member), struct/union member writes (the
  owning object), `rand`/`randc` properties, built-in
  `push_back`/`push_front`/`insert`/`putc`/`itoa`-family receivers, mailbox
  `get`/`peek` messages, `obj.randomize(v, ...)` arguments and both sides of
  `ref` port connections. Constraint `foreach` loop variables are never reported.
  Skipped: top-level external inputs/inouts, explicitly open child ports,
  signals connected to a black box (an instance of an unknown module),
  implicit nets and intrinsic pull/supply nets. Input/inout actuals are read
  through the owned port `high_expr`; the direct `high` target stays for
  model/codegen consumers.
- `incomplete-sensitivity-list` (`sensitivity.rs`): partial/selected writes keep
  the signal as an input; identical findings from cloned instances are
  source-deduplicated. Edge, named-event, mixed/complex, `@*`, `always_*` and
  nested timing-control forms are skipped.
- `empty-implicit-sensitivity` (`empty_sensitivity.rs`): calls and nested timing
  controls are also skipped; cloned instances are source-deduplicated.
- `duplicate-case-item` (`duplicate_case.rs`): flags every later repeated literal
  in one exact `case`; wildcard cases are skipped.
- `xz-logical-equality` (`xz_comparison.rs`): case/wildcard equality is skipped.
- `assignment-in-condition` (`assignment_condition.rs`): nested operations are
  traversed; standalone assignments and `repeat`/`case`/event expressions are
  outside the rule.
- `casex-statement` (`casex_statement.rs`): every `casex` in a process or
  function/task body; cloned instances are source-deduplicated.
