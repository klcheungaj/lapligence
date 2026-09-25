# IR optimization

- **Facade:** `opt.rs` accepts only `ExecutionModel`, exposes optimization
  configuration, and revalidates scheduling summaries after transformations.
- **Passes:** `passes.rs` walks and transforms typed IR conservatively.
- **Singleton concatenations:** identities preserve unsignedness and the
  self-determined boundary, including fill markers carried by literal nodes.
  Equal bit width alone does not justify replacing the concat with its child.
- **Invariants:** passes preserve table indices and lowering-time sensitivity
  sets; they do not recompute wake behavior.
- **Validation:** callers validate the model after enabled passes complete.
- **Array conditionals:** `ArrayMux` folds whole immediate elements with its
  captured default payload, not packed mux bits. Its operands remain visible
  to traversal/read collection; only a proven selector may discard an arm.
  `StructMux` uses the same rule at each immediate unpacked-structure member
  boundary, while preserving child traversal and effect analysis.

See [the simulator README](../readme.md) for this stage's place in the model
pipeline.

## Source organization

`passes.rs` coordinates `passes/` domains: constant evaluation, traversal,
folding, identities, control flow and storage. Unit tests live in
`passes/tests.rs`. Public optimization entry points and validation boundaries
remain in the existing facade.

See [the source map](../../../docs/source_layout.md).

`Predicate` is not a logical-AND or concatenation identity. Folding may remove
an unreachable suffix after a constant false/X/Z prefix, but a reached
nonconstant/effectful clause blocks that fold even when a later clause is zero.
A one-clause predicate still produces normalized one-bit truth, not the original
vector. Walkers retain every potential clause for reads, effects and rewriting;
existing lowering-time wake dependencies are not weakened by prefix folding.
