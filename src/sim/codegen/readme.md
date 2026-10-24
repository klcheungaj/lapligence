# Simulator lowering

`codegen.rs` lowers a validated `SemanticModel` into typed operations and an
`ExecutionModel`, returning `GeneratedModel` or a typed `CodegenError`.
It performs no native frontend calls, unsafe operations or C emission.

| Component | Responsibility |
| --- | --- |
| `lowering.rs`, `lowering/` | Shared lowering state and collection, statement, expression, container and object domains. |
| `timescale.rs` | Slang-resolved units/precision and checked femtosecond ticks across 1fs–100s. |
| `lowering/collection/fixed_*` | Checked layouts, defaults, patterns, casts and activation/member projections. |
| `lowering/expressions/conditionals.rs` | Array/structure merge plans before flattening loses immediate boundaries. |
| `lowering/packed_geometry.rs` | The one mapping from packed element labels of the outermost visible dimension to LSB-relative bits. |
| `lowering/collection/packed_elements.rs` | Dimension-relative packed selection chains over fixed-array elements and whole packed values. |
| `net_collapse.rs`, `port_net_types.rs`, `nets.rs`, `net_arrays.rs`, `net_cells.rs` | Directional net-type selection, undriven net-array cell runs, canonical electrical identities, drivers and publication. |

Oversized whole integral array copies and supported streams lower to descriptor
operations instead of coordinate expansion. `collection/fixed_calls.rs` creates
lexical array activations for automatic nonrecursive oversized array-returning
functions; packed signatures keep their existing transport.

Collection consumes the semantic coverage ledger so reachable unknown executable
nodes reject with source locations. Initialization keeps declaration identity,
lifetime, origin and edition-specific phase. Process, assertion, clocking, event
and reference metadata remain typed through emission.

`GeneratedModel::sources()` supplies the C model and its deterministic debugging
symbol map to the builder or source export. The
[emitter](../emit_c/readme.md) owns their naming and
sidecar format.

## Names, labels and scale

- Keep raw source hierarchy components in `Codegen::c_paths`, independent of
  scope lookup keys and display paths. Compose C names only through
  `collection/names.rs`; encoded C fragments are never source path components.
- Storage and runtime diagnostic labels come from HDL metadata or owned source
  components (aggregate member paths, declared array bounds); never decode C
  symbols. Internal dependency sort keys and function diagnostic labels stay
  separate from display labels and C symbols. Escaping, collision and length
  rules belong to the [emitter](../emit_c/model/readme.md).
- Generation stays linear in the instance count (`tests/sim_scale.rs`).
  Per-net, per-signal and per-name lookups use the one-pass indexes
  `nets/writer_index.rs`, `nets/global_index.rs` and
  `collection/signal_names.rs` (equivalence with the scan: their module docs).
  A step that renames a signal's `c_name` or adds `net_alias` bindings in place
  calls `invalidate_signal_names`.

## Contracts

| Topic | Document |
| --- | --- |
| Widths, fills, built-ins, VPI actuals, randcase/randsequence, selections, layouts, patterns, conditionals | [values.md](values.md) |
| Continuous/comb processes, startup order, programs, ref formals, initialization schedule, links, writer analysis | [processes.md](processes.md) |
| Inout collapse, contribution slots, strengths, UDPs, switches, net partitions, delays, module paths, timing checks, SDF, force/release | [nets.md](nets.md) |
| Activations, copy-in/out, DPI, forks, loops, callbacks, fixed reductions, container methods | [calls.md](calls.md) |
| Event waits, sensitivity, predicates, tagged unions, fixed values, record columns, native records, reals, clocking, containers, streams, memory files | [events.md](events.md) |
| Randomization, functional coverage, time, display, monitors, waveforms, assertions, mailboxes | [services.md](services.md) |
| Lowering domains | [lowering/readme.md](lowering/readme.md) |

See [feature status](../../../docs/sim_features.md) and
[source layout](../../../docs/source_layout.md).
