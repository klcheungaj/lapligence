# IR optimization

`opt.rs` accepts only `ExecutionModel`, exposes configuration, and revalidates
scheduling summaries after transformations. `passes.rs` coordinates constant
evaluation, traversal, folding, identities, control flow, and storage; unit tests
live in `passes/tests.rs`. Callers validate the model after enabled passes.

## Where the rules live

| Rule | Source of truth |
| --- | --- |
| Pass list, default/none modes, X/Z-exact folding, never recomputing wake behavior, no index remapping | `passes.rs` module docs and `OptConfig` |
| Shortreal rounding, sequential-predicate prefix folding | `passes/folding.rs` comments |
| Identity shape guard, unsigned self-determined singleton concatenation | `passes/identities.rs` comments |
| Default-arm pruning only when every item is provably unmatched | `passes/control_flow.rs` (`pick_case_branch`) |
| Exhaustive expression walker | `passes/traversal.rs` module docs |
| Unused-storage walk and the non-read references it must see | `passes/storage.rs` module docs |
| Fixed-array reduction seeding | `ir/fixed_array_reductions.rs` docs |
| `ArrayMux`/`StructMux` leaf merges, `PackedChain` steps | [IR contracts](../ir/readme.md) |

## Additional contracts

- Keep per-pass bisection.
- Fold with actual-width arithmetic.
- `Predicate` keeps ordered true-only evaluation; drop a suffix only when
  unreachable, never discarding reached effects. A reached nonconstant or
  effectful clause blocks the fold even when a later clause is zero.
  One-clause predicates still normalize to one-bit truth. Walkers inspect
  every potential clause; prefix folding never weakens lowering-time wake
  dependencies.
- `ArrayMux` folds uniform leaves using captured defaults; `StructMux` uses
  the leaf starts of each repeated period. Both retain operand traversal,
  reads and effects.
- Array/structure conditional folds keep leaf boundaries (R05-D1) and the
  default-uninitialized value, never member initializers; equal flattened
  width does not imply equal shape. Only a proven selector discards an arm.
- `PackedChain`: never sum offsets and erase intermediate bounds; preserve
  negative/X/Z indices, partial clipping and missing-bit X/no-write behavior;
  count index temporaries in capacity estimates.
- Fixed-array folds restore nested iterator bindings.
- Ordered predicates and case patterns capture selectors once; tagged pattern
  discriminants and payloads use their enclosing case mode; ordinary
  checked-member guards stay exact.
- A new IR kind must be classified in the exhaustive walkers (and in the
  IR-owned `expressions()`/`chandles()` visitors of chandle, container and
  randomize nodes) before it compiles. `opt_storage_differential` compares
  every fixture with `unused_storage` on and off.

See the [pipeline](../readme.md) and [source map](../../../docs/source_layout.md).
