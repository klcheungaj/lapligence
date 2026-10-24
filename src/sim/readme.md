# Verilog/SystemVerilog simulator

The simulator lowers owned frontend data into a validated execution model, emits
C11 and builds it with CMake. Generated models link their own stackless runtime;
it is not a Rust runtime dependency.

| Component | Responsibility |
| --- | --- |
| `legacy_unsupported.rs` | Stable rejections for legacy constructs unsupported by design (MOS, trireg, charge directives, `$dumpports*`, PLA, inspection tasks, PLI 1.0 TF/ACC, every procedural `assign`/`deassign`, rejected `$q_*` forms). |
| `semantic/` | Frontend-neutral model, source origins, synthesis classification and executable-node coverage. |
| `codegen/` | Typed lowering of declarations, values, processes, links and scheduling. |
| `ir/` | Shared typed storage/value/statement tables and validation. |
| `execution/` | Executable blocks, effects, regions and suspend/resume plans. |
| `opt/` | Shape- and four-state-aware execution-model optimizations. |
| `emit_c/` | Ordered C setup/evaluation/cleanup and model lifecycle. |
| `build.rs`, `build/` | CMake model and runtime-archive builds, caches, seeds and templates ([readme](build/readme.md)). |
| `rt/` | Exact-width values, scheduler, native services and optional waveform output. |
| `sdf/` | Streaming OVI SDF 2.1/3.0 reader and resolution of a file against `codegen::sdf_targets` into an owned annotation plan (ADV-011; applying it is ADV-012). |

`generate_from_db_with_opts` reuses one owned DB across optimization variants.
The driver uses `generate_from_owned_db_with_codegen_options` to retain all
generation tunables while releasing the DB before execution lowering/rendering.
`GeneratedModel` carries the design name, generated C and warnings. Unsupported
executable forms fail with source-linked errors before C compilation.

## Pipeline contracts

- The CLI uses the simulator frontend export policy owned by the FFI and driver
  guides; library generation consumes its caller's owned DB without changing
  capture limits.
- `SemanticModel` owns synthesis classification and returns a checked
  `SynthDesignView` or origin-linked reasons. Never equate simulation admission
  with the conservative synthesis profile.
- Typed IR stages signals, arrays, nets, functions, processes and
  initialization; `ExecutionModel` ([readme](execution/readme.md)) owns
  executable blocks, effects, suspend/resume plans and regions. Emit distinct
  resume-block terminators; body-controlled suspension requires a validated
  waiting operation.
- `IrModelParts` stays untrusted until `IrModel::from_parts` (`ir.rs` docs).
  Keep model and detached-node validation before optimizer/emitter indexing;
  semantic and executable layers stay owned and independently testable.
- Legacy constructs unsupported by design (`legacy_unsupported.rs`, the 🚫 rows
  of [feature status](../../docs/sim_features.md)) stop the run before lowering
  or at their target classification. Reuse its `diagnostic` shape for a new
  by-design rejection; never leave one to a generic lowering failure, a VPI
  fallback or a no-op.
- `sdf/` (module docs in `sdf/mod.rs`, `apply.rs`, `targets.rs`): keep the
  parser streaming, with memory bounded by the longest token and `SdfLimits`,
  errors positioned and deterministic, and rule violations that keep one meaning
  as warnings (fixture readme `adv_011`; applying: `adv_012`).

## Capacity and frames

- Packed widths: `sv4_t` widths are `uint32_t`; div/mod/pow scratch follows
  operand width. Preserve typed index trees, elaborated indexed-part extents,
  wide intermediate indices and member-specific two-state conversion. Emit
  static extents, not the width expression's integer storage size.
- Coroutine storage is explicit POD frame data: count typed expression storage
  and embedded callee slots in `FrameLayout`. Recursive synchronous
  subprograms run as coroutines on the chain arena (`execution/recursion.rs`).
- There is no generated coroutine-stack estimate or `LLG_MODEL_STACK_VALUES`.
  Values allocate by their own widths; never restore model-maximum arrays.
- `LLG_MODEL_VALUE_ABI` must match `LLG_VALUE_ABI_VERSION`.

## Runtime packaging

`write_sim_sources` writes the flat value, random, coroutine, scheduler,
container and waveform sources plus self-tests into `<out-dir>/sim/<design>/`
(driver default `build`). Private fragments assemble in facade order; selected
value headers keep their `value/` or `value_gmp/` relative paths; compact backend
units compile separately. Keep the original runtime/waveform ownership
self-tests active.

Library optimizer comparisons reuse one DB; CLI comparisons independently run
checked-in HDL in both optimizer modes.

## Value selection

Compact values with GMP kernels (`ValueConfig::default()`) are the only user
configuration (`value_backend.rs` docs). Keep the development references and
their `LLG_DEV_*` selectors out of user documentation.

- `CodegenOptions` and `CmakeBuildOpts` share `value_config`; mismatching
  generated guards fail before export. Source export retains the selection,
  ABI guards and dependency fingerprint.
- The bundled `vendor/gmp` subset ([`rt/gmp.rs`](rt/gmp.rs)) compiles into the
  runtime archive unless `gmp_root`/`GMP_ROOT` names an installation; an
  explicit root is authoritative, with no system fallback. Hash its header and
  library contents; CMake verifies version agreement, required mpn APIs and
  64-bit nail-free limbs ([compact backend](rt/value_gmp/readme.md)).
- Source-only projects compile their selected runtime as a static archive.
  Apply waveform definitions to that archive as well as the model; every
  translation unit gets both literal selector definitions.
- Generated startup calls the selected build-identity link fence; foreign value
  clients must do likewise before exchanging descriptors.
- Cache ready markers contain the exact key; plain old ready markers cannot
  admit stale archives.
- Compact builds embed every emitted value operation, including S1–S9 and V06
  consumer primitives; an unavailable addition must never fall back.

See [feature status](../../docs/sim_features.md), [build and CLI usage](../../readme.md),
[model build](build/readme.md), [tests](../../tests/readme.md) and
[source layout](../../docs/source_layout.md).
