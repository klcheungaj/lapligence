# Source layout and ownership

Use [Coding Practices](coding_practices.md) when choosing a module boundary.
A facade owns shared state and presents the existing API; responsibility-named
children implement cohesive parts of that contract. File length is a navigation
signal, not a reason to cut an algorithm or introduce a generic helper module.

## Rust ownership boundaries

| Facade | Implementation domains | Ownership and contract |
| --- | --- | --- |
| [`core/db/database.rs`](../src/core/db/database.rs) | [`database/`](../src/core/db/database/): types, nodes, references, values, assertions, clocking, connections, statement/expression records and importers, capture | `Db` owns the semantic arena. `Db::from_slang` remains the single owned-snapshot import entry point. |
| [`ffi/slang.rs`](../src/ffi/slang.rs) | [`slang/`](../src/ffi/slang/): snapshot, semantics, tokens, diagnostics, values | The facade retains the raw ABI declarations, native static-link attributes, native ownership/cleanup and safe compile entry points. Native access stays inside `src/ffi/`. |
| [`sim/codegen/lowering.rs`](../src/sim/codegen/lowering.rs) | [`lowering/`](../src/sim/codegen/lowering/): assertion/clocking contexts, references, initialization, delays, selections and the domain facades below | `Codegen` owns lowering state and coordinates owned database facts into typed IR; no C source emission or frontend traversal. |
| [`lowering/collection.rs`](../src/sim/codegen/lowering/collection.rs) | [`collection/`](../src/sim/codegen/lowering/collection/): design, locals, classes, aggregates, initialization, nets, ports, gates, signatures, call contracts, arguments, calls, function bodies, processes, captures, dependencies, events, lvalues, names, constants, virtual interfaces | Collection and wiring share the existing `Codegen` owner; domain methods do not introduce new state owners. |
| [`lowering/statements.rs`](../src/sim/codegen/lowering/statements.rs) | [`statements/`](../src/sim/codegen/lowering/statements/): context, dispatch, declarations, assignments, control flow, events, forks, drivers, assertions, clocking, formatting, system tasks, calls | `EmitCtx` coordinates procedural lowering and its activation context. |
| [`lowering/expressions.rs`](../src/sim/codegen/lowering/expressions.rs) | [`expressions/`](../src/sim/codegen/lowering/expressions/): dispatch, operations, casts, aggregates, streaming, membership, array queries, system functions, external input | Expression metadata and typed conversions are decided during lowering. |
| [`lowering/containers.rs`](../src/sim/codegen/lowering/containers.rs) | [`containers/`](../src/sim/codegen/lowering/containers/): initialization, patterns, indexing, queries, fixed arrays, streaming, assignments, methods, callbacks | Container operations preserve typed storage and existing alias/callback contracts. |
| [`lowering/objects.rs`](../src/sim/codegen/lowering/objects.rs) | [`objects/`](../src/sim/codegen/lowering/objects/): classification, initialization, classes, virtual interfaces, mailboxes, processes, enumerations, handles, strings, queries, assignments, methods | Non-integral values remain distinct from packed-bit storage. |
| [`sim/ir.rs`](../src/sim/ir.rs) | [`ir/`](../src/sim/ir/): constants, expressions, lvalues, calls, statements, events, assertions, processes, functions, initialization, storage, VPI, containers and objects | The facade retains `IrModel` and re-exports the existing typed operation API. |
| [`ir/validate.rs`](../src/sim/ir/validate.rs) | [`validate/`](../src/sim/ir/validate/): tables, dependencies, values, expressions, calls, lvalues, events, statements, initialization, file input | Validation shares its original model/context owner and runs at the same pipeline boundaries. |
| [`sim/opt/passes.rs`](../src/sim/opt/passes.rs) | [`passes/`](../src/sim/opt/passes/): constants, traversal, folding, identities, control flow, storage | Passes transform typed IR without recomputing wake behavior or changing scheduling ownership. |
| [`emit_c/model.rs`](../src/sim/emit_c/model.rs) | [`model/`](../src/sim/emit_c/model/): interfaces, classes, assertions, storage, VPI, functions, DPI, processes, initialization | Render model declarations and process scaffolding from checked execution data. |
| [`emit_c/statements.rs`](../src/sim/emit_c/statements.rs) | [`statements/`](../src/sim/emit_c/statements/): assertions, system tasks, events, formatting, callbacks, force | Render statement operations without database access or new semantic decisions. |
| [`emit_c/expressions.rs`](../src/sim/emit_c/expressions.rs) | [`expressions/`](../src/sim/emit_c/expressions/): queries, system functions, input, lvalues, casts, assignments, calls | Render typed expressions and their recorded representation. |
| [`llg_ls/module_explorer.rs`](../src/bin/llg_ls/module_explorer.rs) | [`module_explorer/`](../src/bin/llg_ls/module_explorer/): catalog, budget, content, hierarchy, compatibility, presentation, types | The facade coordinates owned feature data and keeps protocol entry points stable. |

Children use the facade's shared types and narrowly scoped implementation
methods. An item moved below its former module may need `pub(super)` or
`pub(in super::super)` to retain access within that original owner; that is not
a reason to make it public outside the owner. Preserve public import paths with
facade re-exports, and keep data layouts, attributes and ownership invariants
with their types. Use ordinary module declarations in binary targets, not
`#[path]` shortcuts.

## C runtime source organization

The value, scheduler and container implementations use **private source fragments
of one translation unit each**, not independently linked modules:

- [`llg_value.c`](../src/sim/rt/llg_value.c) includes
  [`llg_value_prelude.c`](../src/sim/rt/llg_value_prelude.c) and
  [`value/`](../src/sim/rt/value/) fragments for exact-width storage ownership
  and the existing four-state operations.
- [`llg_rt.c`](../src/sim/rt/llg_rt.c) includes
  [`llg_rt_prelude.c`](../src/sim/rt/llg_rt_prelude.c) and
  [`scheduler/`](../src/sim/rt/scheduler/) domains for state, process lifecycle,
  waits, dependencies, net updates, time, formatting, file/memory input,
  assertions and scheduling.
- [`llg_container.c`](../src/sim/rt/llg_container.c) includes
  [`llg_container_prelude.c`](../src/sim/rt/llg_container_prelude.c) and
  [`container/`](../src/sim/rt/container/) domains for value descriptors,
  dynamic arrays, queues, pinned queue references and associative arrays.

The facades preserve declaration order and private `static` state. The prelude
files stay beside the public headers so direct source-tree inclusion retains
header-relative lookup. Do not compile a fragment separately, add fragments to
CMake source lists, or export scheduler globals to connect them.

[`rt/mod.rs`](../src/sim/rt/mod.rs) embeds each ordered list with
`concat!(include_str!(...), ...)`. `value_sources()`, `runtime_sources()` and
`container_sources()` still return a header and one flat implementation string.
The generated build directory therefore needs only the established flat C
filenames, not a copy of the fragment directories. Keep the facade include
order and embedded list synchronized; [`rt/tests.rs`](../src/sim/rt/tests.rs)
checks that relationship without compiling C.

The value, string, random, VPI and waveform sources retain their separate
translation-unit contracts. Vendored frontend, coroutine and GTKWave sources
are not reorganized by this layout.

## Test organization

- [`tests/lsp_stdio.rs`](../tests/lsp_stdio.rs) owns the framed stdio client,
  process setup and common assertions. [`tests/lsp_stdio/`](../tests/lsp_stdio/)
  groups wire-level regressions by diagnostics, workspaces, configuration,
  dependencies, lifecycle, navigation, tokens, rename, explorer and recovery.
  Its explicit `#[path = "lsp_stdio/..."]` declarations are integration-test
  crate paths, not binary-module shortcuts.
- [`features/tests.rs`](../src/bin/llg_ls/features/tests.rs) keeps common feature
  fixtures; [`features/tests/`](../src/bin/llg_ls/features/tests/) groups analysis,
  bindings, requests, configuration, fallback and source-coordinate regressions.
- Larger inline unit-test modules live in a sibling `tests.rs` (or
  `cast_tests.rs`) under their owning facade. Keep their `#[cfg(test)]` gate,
  module identity, fixture bytes and source-relative include paths intact.

The split stdio and feature suites add a domain component to full test names;
exact-name filters must include it. Integration-test binary names stay the same.

See [testing methodology](../tests/readme.md) for execution commands and
platform limitations. A source move must not change independent HDL oracles,
error expectations, optimizer-mode coverage, or runtime sanitizer settings.
