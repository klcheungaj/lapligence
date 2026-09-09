# Lint rule contracts

Read [../AGENTS.md](../AGENTS.md) for the rule API and configuration.
`mod.rs::default_rules` is the authoritative stable registry order.
Assignment and comparison width rules skip unknown operand widths.

Keep shared graph/data-flow helpers in `analysis.rs` rather than duplicating
them across rules. New rules need registry entries plus focused behavior and
configuration tests. Rules consume owned data through `LintCtx`, with no live
native traversal or I/O.

- `unused.rs` — `unused-signal`: signals that are never read or used.
- `width.rs` — `width-mismatch`: assignments and port links with different known widths.
- `latch.rs` — `incomplete-case`: exact cases without a default in combinational/latch logic.
- `combloop.rs` — `combinational-loop`: combinational feedback paths.
- `multidriver.rs` — `multi-driver`: signals driven by multiple processes.
- `casez.rs` — `casez-misuse`: overlapping wildcard case items and constant selectors.
- `if_latch.rs` — `if-latch`: incomplete combinational `if` assignments that may infer a latch.
- `style.rs` — `naming-style`: configured naming-convention violations.
- `blocking_in_ff.rs` — `blocking-in-always_ff`: blocking assignments in clocked processes.
- `nba_in_comb.rs` — `nba-in-always_comb`: nonblocking assignments in combinational processes.
- `unused_param.rs` — `unused-parameter`: parameters that are never referenced.
- `implicit_net.rs` — `implicit-net`: uses the owned Slang implicit-net
  declaration flag, independent of resolved type spelling or width.
- `case_default.rs` — `case-default-missing`: case/casex/casez without a
  default arm OUTSIDE the incomplete-case domain (which owns the exact
  case in combinational/latch processes): casex/casez anywhere, exact case
  in edge-sensitive/initial/final processes and in function/task bodies.
  Skips statements already in the incomplete-case domain so one location
  is never reported by both rules.
- `comparison_width.rs` — `comparison-width-mismatch`: comparisons (`==`,
  `!=`, `<`, `<=`, `>`, `>=`, `===`, `!==`) whose operands both have known
  self-determined widths that differ; skips when either width is unknown.
- `unconnected_port.rs` — `unconnected-port`: flags omitted, positional-gap,
  and explicitly open child ports. Resolved expressions and declaration
  defaults count as connected; top ports are external boundaries. Interface
  connections refer to actual instances, without frontend-generated copies.
- `mixed_assign.rs` — `mixed-assignments`: one Error per process whose
  statement body contains BOTH blocking (`=`) and non-blocking (`<=`)
  assignments ([`StmtKind::Assign`] only; proc-cont assign, force/release
  and declaration initializers are ignored), positioned at the process
  keyword.  Deliberate overlap with `blocking-in-always_ff` /
  `nba-in-always_comb` (kind-vs-block-type mismatches): no suppression,
  the diagnoses differ and this rule also covers block kinds the other
  two never check (plain level-sensitive always, initial/final).
- `undriven.rs` — `undriven-signal`: flags a declared signal that is read
  but has no active procedural/continuous driver, declaration initializer,
  connected output/inout flow, or primitive output terminal. Top-level
  external inputs/inouts, explicitly open child ports, implicit nets,
  and intrinsic pull/supply nets are skipped.
  Input/inout actual expressions are read through the owned port
  `high_expr`, while the historical direct `high` target remains available
  to model/codegen consumers.
- `sensitivity.rs` — `incomplete-sensitivity-list`: flags a plain explicit
  level-sensitive `always @(...)` when a directly read input is absent.
  A local written earlier on every represented path is treated as a
  temporary; uncertain control flow and partial/selected writes fail safe
  by keeping the signal as an input dependency. Identical findings from
  cloned module instances are source-deduplicated.
  Edge, named-event, mixed/complex, implicit `@*`, special `always_*`, and
  nested timing-control forms are skipped when correctness cannot be proven.
- `select_range.rs` — `out-of-range-select`: checks statically known bit,
  part, indexed-part, and unpacked-array selectors against owned elaborated
  bounds. Dynamic selectors, unresolved ranges, and ambiguous
  multidimensional packed shapes are intentionally quiet.
- `xz_comparison.rs` — `xz-logical-equality`: flags `==`/`!=` with a direct
  X/Z/? literal operand (including transparent casts); case/wildcard
  equality, parameter references, and nonliteral expressions are skipped.
- `duplicate_case.rs` — `duplicate-case-item`: flags every later exact
  captured literal repeated in one exact `case`. Wildcard cases and
  nonliteral/equivalent-but-differently-represented expressions are skipped.
- `empty_sensitivity.rs` — `empty-implicit-sensitivity`: flags a plain
  `always @*` / `always @(*)` whose body writes at least one resolved signal
  but has no resolved signal reads. `always_comb`, explicit event lists,
  calls, opaque/unresolved nodes, and nested timing controls are skipped;
  cloned elaborated instances are source-deduplicated.
- `assignment_condition.rs` — `assignment-in-condition`: flags a captured
  assignment operation consumed as an `if`, `while`, `for`, `wait`, or ternary
  truth predicate. Nested operations are traversed, but explicit equality
  and relational expressions form a boundary; standalone assignments and
  `repeat`/`case`/event expressions are outside the rule.
- `casex_statement.rs` — `casex-statement`: flags every `casex` in a process
  or function/task body. Exact `case` and `casez` remain quiet, and cloned
  elaborated instances are source-deduplicated.
- `analysis.rs` — shared owned-db helpers: read/write collection,
  expression-width computation, scope/instance iteration, port-link
  bookkeeping (deterministic, deduped, first-encounter order),
  and unconnected-port classification (`port_unconnected`).
