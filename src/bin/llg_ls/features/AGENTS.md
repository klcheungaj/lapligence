# LSP analysis and feature projections

Applies to `features.rs` and its children: `analysis.rs` (blocking pipeline),
`source_graph.rs` (owned source module graph), `fallback.rs`, `symbol_index.rs`,
`requests.rs` (pure request projections), and `tests.rs` (feature regressions).
Read [../AGENTS.md](../AGENTS.md) for protocol/cache/rename contracts and
[../lsp/AGENTS.md](../lsp/AGENTS.md) for snapshot serving and staging.

## Analysis and index

`Analysis` owns diagnostics, outcome, model, tokens, index, ref_bindings and
lint. `analyze()` runs Surelog + DB + tokens + lint in one blocking call;
`analyze_with_config(opts, &LintConfig)` supplies lint policy. Syntax errors
skip compile/UHDM: `parse_tree_feature_parts` returns surviving parse tokens
and a modules-only model. Recorded declaration positions feed
`SymbolIndex::from_parts`: ports, nets/regs and parameters become DECLs;
previously declared same-file uses become REFs, excluding assignment LHS via
`ancestor_is_assignment_lvalue`. This supplies non-Fatal feature data until
full compilation succeeds.

Under Cargo feature `slang`, the backend can attach diagnostics projected from
Slang's owned snapshot. This is migration-only observation data: it keeps its
Slang provider and namespaced code and does not affect `AnalysisOutcome`,
`has_feature_data`, or the retained Surelog semantic/navigation snapshot.

`SymbolIndex` provides cross-file declarations/references and merges per-root
indexes for workspace symbols. Model objects provide declarations and
`vpiRefObj` tokens provide reference sites. Refine module/instance positions
to identifier tokens (Surelog points at `module` or the instance's type name).
Module-type references must remain distinct from same-named instance names.

## Named connections and precise bindings

- Port label `.clk` resolves to the child module's port; parameter label `.W`
  in `child #(.W(4)) u0 (...)` resolves to its parameter. Actual `wa` in
  `.clk(wa)` and RHS in `.W(rhs)` resolve in the parent: enclosing-module-span
  containment of the instantiation line, nearest-above tie-break/global
  fallback, and no binding when no parent candidate exists.
- `scan_named_port_connections` pairs positions AND identifier text for both
  port and parameter connections. Port pairing uses `PORT_LABEL_MAX_SPAN`;
  overrides need exact parse-tree pairing because they precede instance names.
  Both cover multiline forms. UHDM mode folds `via_label` and `via_connection`
  into `ref_bindings`; visiting each instance's `vpiParamAssign` RHS also
  supplies elaboration targets. Dumps label these `via=label` and
  `via=connection` (parent actual/RHS).
- Parse fallback matches labels exactly against recorded child port/parameter
  declarations, resolves actuals/RHS against parent declarations, and retypes
  labels as plain reference tokens for indexing/dumps. Unresolvable connections
  are silently skipped; an unresolved override label must yield no definition,
  never a same-named parent localparam. Unnamed/`.*` positional pairing remains
  out of scope. Parent actuals stay reachable through hover/references.
- `core::tokens::RefBindings`/`DeclTarget` map 0-based reference positions to
  `vpiActual` declarations plus connection folds. UHDM wins collisions; actual
  folds fill only unbound positions. `definition_at` serves exact bound targets
  before index/name fallback.

## Genvar source bindings

Owned parse-tree facts supplement UHDM with explicit (including multi-name)
and inline loop genvar declarations, keyword spans, and lexical reference
bindings. Preserve declarations in pruned loops and syntax-error fallback;
header assignment/increment sites are references too. Resolve against the
nearest enclosing source scope, not an elaborated iteration's localparam.
Index each lexical declaration once as a variable with `genvar <name>` hover,
without an iteration-dependent parameter value.

Declaration self-bindings identify genvar reference sets. Definition,
references, and rename use exact bindings, including an empty reference set
for an unused declaration; do not fall back to same-name guesses. Respect
ordinary local declarations and existing UHDM bindings. Member selectors,
named connection labels, and block labels are not bare genvar uses. Generated
instance port labels still bind to the child's port while actuals bind in the
parent's lexical scope. Both committed and isolated semantic-token paths use
variable tokens for names and keyword tokens at the actual `genvar` span;
the isolated path normalizes these parse positions using the admitted buffer.

Wire regressions live in `tests/lsp_stdio/genvar.rs`, with checked-in Verilog
and SystemVerilog fixtures covering shadowing, pruned/nested loops, fallback,
and UTF-16 positions. These facts introduce no additional VPI traversal.

## Shadowing, details and references

The token walk must reach every inner scope: function/task bodies through
their own `vpiStmt` (not process iteration); generate interiors only through
elaborated `uhdmtopModules` → `vpiGenScopeArray` → `vpiGenScope`
(`uhdmallModules` definitions expose neither); named begin/fork via
`vpiNamedBegin`/`vpiNamedFork`/`vpiBegin`/`vpiFork` during statement descent
(VPI-mapped types, not UHDM `named_begin` discriminants). Deduplicate scopes
by `(vpiType, vpiFullName, line, col)`; recycled handle addresses are not
identities. Block/genblk/function locals gain both UHDM and parse views,
become DECL under the multi-view heuristic, and navigate to themselves.

`decl_details` (`core::tokens::DeclDetails`) maps `(file, line1, col1)` to
typespec-derived snippets (`logic [3:0] val`, `input logic [1:0] sel`), captured
in the same walk using `core::model::TypeInfo::render()` semantics. First
writer wins, so a port beats its backing variable. `Analysis::with_decl_details`
replaces name-based Port/Net/Var details and refreshes lookup maps; parameters
keep value-rich model details. `hover_at` checks bindings first (exact key,
then cursor-normalized start column, as definition does) and uses the target's
indexed declaration or position-accurate snippet instead of an outer namesake.

`references_at_with_options` anchors the set to the query binding (`heads`),
removes name-pool occurrences bound elsewhere, and adds precisely bound uses
outside that pool. `decl_details` locations count as declarations even when
classified REF-shaped. Rename uses this identical occurrence machinery.

## Parameter and macro hover

Parameter/localparam hovers at declarations and precise uses (including
override labels) append `value = 32'sd8` from the committed model only. The
target's enclosing module span selects the instance family; all same-named
direct/generate-scope parameters across clones must agree on one constant.
Thus committed `-P` overrides appear, while divergent/unresolved values omit
the line. Positions outside modules fall back to same-file packages; omit
rather than guess. Normalize an identical inline `parameter W: int = …` tail
into the value line to render once. Never parse/elaborate inside hover.

`core::macros::MacroTable` is attached once per commit and consulted first in
`hover_at`: Surelog removes macro definitions from all FFI-visible UHDM/parse
FileContents (the preprocessor parse is internal), so macro positions lack
indexed symbols. See [../../../core/AGENTS.md](../../../core/AGENTS.md) for
scan semantics, config seeding and approximations. Read exact compiled sources
once per commit (shadow paths carry open text); requests only look up positions,
allowing warm request-cache hits.

Render `macro WIDTH = 8`, `macro MAX(a, b) = …`, or body-less `macro ENABLE`
in a SystemVerilog fence, plus `defined at <file>:<line>` for source origins
mapped to real paths. Undefined usage explicitly says "`X` is not defined
under the current configuration" and names the effective config file via
commit-time `attach_macro_config_note`; never substitute an invented value.

## Enums and remaining limitations

- Parse-backed facts bind package/class-qualified enum members, imported bare
  members and bare members with one visible declaration at exact member
  positions in full and syntax-fallback analyses. Synthetic tokens index them.
  Ambiguous bare/qualified uses stay explicitly unresolved so definition,
  references and rename cannot fall through to same-name guesses.
- Away from captured bindings, navigation is name-based and scope-aware and
  can conflate shadowed declarations. Source-position bindings cannot select
  differing overrides among owning-module clones. Generate-pruned declarations
  reached only by name fallback can inherit a surviving branch's same-named
  constant.
- Classes expose definitions, methods/fields in hover and document symbols,
  and `Class::` completion; `obj.count` member-expression resolution is out of
  scope and simulator class instances are unsupported. Functions/tasks expose
  signature hover, symbols and completion. Package parameters/enum constants
  support declaration navigation and `pkg::` completion. Other package use-site
  refs folded to constants by Surelog still lack goto-definition.
