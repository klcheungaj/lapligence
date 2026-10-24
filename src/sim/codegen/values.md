# Lowering: values and selections

- Allocate exact-width values; `LLG_MAX_WIDTH` is the exclusive backend limit
  minus one, not model allocation capacity. Check constants, parameters, signals,
  concatenations, replications and intermediates; never truncate silently.
  Do not add detached C/`Verbatim` workarounds: the owned emitter rejects legacy
  fragments whose setup/cleanup cannot be represented.
- PLA system-task names have no selected legacy target. Diagnose them before
  the generic VPI task path; admitting their Verilog-2001 memory personality
  operand in the frontend does not select PLA execution.
- A built-in without a lowering stays on the VPI path only when an application
  could provide it (SV 36.3.2); names that cannot be (clock resolution) are in
  `clock_resolution_reference`, lowered as clocking events and `disable iff`
  conditions and rejected at the call anywhere else. A system function in statement position
  (void cast) is a function call, never a VPI task. Keep
  `tests/fixtures/sim/feature_completion/sim_099/builtin_inventory.tsv` in step.
- VPI call actuals go through `lower_vpi_argument`: a `string`-formal string
  literal passes its packed image (a `vpiConstant`), a scope operand of
  `$list` (admitted as metadata by `semantic.rs`, like `$sdf_annotate`'s,
  which lowering reads itself) passes its
  catalog object with a placeholder value, and string variables are located
  errors. Every `$` built-in lowered natively is listed for the startup
  override check (`populate_vpi_builtin_calls`, decision S99-D6).
- Preserve imported fill operations. Unbased fills expand in context-determined
  arithmetic, comparisons, conditionals, assignments and arguments; self-determined
  concat/replication positions stay one bit. Ordinary case uses common width/sign.
  Preserve X/Z for copies, display and case/literal equality; Z otherwise behaves
  as X under the operation's rules. Wide div/mod/pow must survive context widening.
- `==?`/`!=?` wildcards apply only to converted RHS X/Z. Known mismatch beats
  unknown bits; otherwise unmasked LHS X/Z yields X. Bit queries evaluate once:
  countones ignores X/Z, isunknown detects both; counts are signed 32-bit,
  predicates unsigned one-bit. Real operands remain invalid here.
- `randcase` (SV 18.16) lowers to a `DeclLocal` holding `IrSysFunc::Randcase`
  (weights lowered self-determined, in source order, their exact-sum width
  `widest + ceil(log2(items))` bounded by the packed-value limit with a
  source-addressed error) followed by an ordinary exact `Case` over the signed
  index (-1 selects nothing). Bodies take the common statement path, so
  suspension, `return`, `disable`, `break` and `continue` need no randcase
  code; sets `IrRandomUse::threads`. Return-flow analysis treats it as able to
  fall through.
- `randsequence` (SV 18.17, `statements/randsequence.rs`) expands small
  productions in place as structured statements. Recursive productions, those
  reaching them, and shared ones above `RANDSEQUENCE_INLINE_ELEMENT_LIMIT`
  elements (unless a `rand join` interleaves them) become states of one
  dispatch loop (`Forever` over a `Case` on a state local) with a return state
  per multi-site production. Recursive generations push their formals,
  implicit variables, return state and repeat counters onto activation queues
  (packed, real through its bits, and strings in a string queue) and check
  `RANDSEQUENCE_DEPTH_LIMIT`. Two or more rules select with
  `IrSysFunc::Randcase { strict: true }`; one rule evaluates no weight.
  `rand join` binds every item (each names a production of one rule without
  if/case/repeat items, S16-D10), then steps with `IrSysFunc::RandJoinPick`;
  a rule's weight code block is a step of its own. `break` exits through `CtrlScope::RandSequence`;
  `return` jumps to the current production's exit. Unsupported forms (non-input
  formals, recursive or overlapping joins, unsaveable recursive values) are
  located errors. Decisions S16-D1 to S16-D10.
- Get selectors, for-loop relationships, extents and delays from owned typed
  fields, not source names or frontend numeric codes. Widen signed/X/Z-aware
  coordinate arithmetic before subtraction/multiplication; unsigned high bits
  must not wrap into valid indices. Indexed widths/part bounds are static;
  bit/indexed bases may be runtime integral values. Missing metadata rejects.
- A range, indexed or element select addresses whole elements of the outermost
  packed dimension visible at its base; element width is the remaining
  dimensions times the element type (so records count whole). Map labels to bits
  only through `lowering/packed_geometry.rs`, never with per-site stride math.
  A member path whose first reference is an element select (`ps[i].hi`) names
  that select's root only as a label: resolve it through the select chain,
  never by the root's name, which would drop the select. Its selectors are
  reads of a writer (sensitivity and continuous re-evaluation).
- `PackedChain` preserves each selection relative to its preceding result,
  including remaining packed-element stride, bounds, direction and right bound.
  Partial reads keep valid bits and fill missing bits with X; invalid writes do
  nothing. Never flatten a chain to one unchecked root offset. Primitive-terminal
  projections recurse through the same typed steps, preserving per-step flags.
- Fixed layouts retain recursive shape, member identity, state domains, defaults
  and declaration order. Left dimensions vary slowest. Unpacked unions use their
  maximum storage width separately from streamed leaf order; packed members are
  one vector owner. Do not infer nominal identity from spelling or flattened size.
- Replicated patterns expand every dimension/position, even shared expression IDs.
  Check positive count and extent before allocation; do not expand twice.
  Resolve keys as expressions and preserve type/default precedence without
  assuming side-effect order. Type/default/replication evaluation multiplicity is
  undefined by SV 10.9.1; a single captured default before fan-out is one allowed
  choice. Descriptor patterns use lexical array snapshots rather than packed
  concatenations, retain uniform sparse defaults and loop over nonuniform repeats.
  Match packed keys against the owned immediate element type, preserving nominal
  enum identity and non-nominal integral width/range/sign/state equivalence.
- Lower array and direct unpacked-structure conditionals before flattening loses
  leaf boundaries: ambiguity merges leaf by leaf (R05-D1), with `ArrayMux` when
  all leaves share a width and default and `StructMux` with one period's leaf
  starts otherwise. Use default-uninitialized values, not declaration member
  initializers. Capture selector once; known truth evaluates one arm, ambiguity
  evaluates both with type-specific merging. Array assignment snapshots all RHS
  cells before writes and retains explicit conversions/casts on fallback paths.
- A whole dense packed view of at least `LLG_DENSE_CELL_RUN_MIN` cells is
  captured as one image (the concatenation of its element reads) and scattered
  by slices, so the emitter reads and writes it as one cell run
  ([emitter](../emit_c/evaluation.md)). Constant pattern leaves and constant
  copy-out selectors need no capture. Leaf-wise record equality ANDs its
  leaves as a balanced tree, so IR depth stays logarithmic in the leaf count.
- Positional-pattern assignment snapshots the RHS, freezes all destination
  selectors before any write, then scatters. Fixed bit-stream casts convert the
  complete RHS, including nested state-domain conversion, before scattering;
  reference/inout identity is a separate contract. Continuous pattern leaves
  keep constant selects (runtime leaf selects reject) and each leaf is its own
  contribution: alias views, net-array cells and their selected bits publish
  through canonical alias bindings with a per-leaf terminal. Oversized sources,
  including typed patterns, scatter descriptor rows for every assignment kind.
