# IR optimization

`opt.rs` accepts only `ExecutionModel`, exposes configuration, and revalidates
scheduling summaries after transformations. `passes.rs` coordinates constant
evaluation, traversal, folding, identities, control flow, and storage; unit tests
live in `passes/tests.rs`. Callers validate the model after enabled passes.

Passes preserve table indices and lowering-time sensitivity sets; they do not
recompute wake behavior. Key folding boundaries are:

- Singleton concatenations retain unsignedness, self-determined sizing, and
  literal fill markers. Equal width alone does not make a child interchangeable.
- `ArrayMux` folds immediate elements using captured defaults; `StructMux` uses
  immediate unpacked-member boundaries. Both retain operand traversal, reads,
  and effects; only a proven selector can discard an arm.
- `Predicate` is neither logical AND nor concatenation. A constant false/X/Z
  prefix can discard an unreachable suffix, but a reached nonconstant/effectful
  clause blocks that fold even when a later clause is zero. One-clause predicates
  still normalize to one-bit truth. Walkers inspect every potential clause;
  prefix folding never weakens lowering-time wake dependencies.

See the [pipeline](../readme.md) and [source map](../../../docs/source_layout.md).
