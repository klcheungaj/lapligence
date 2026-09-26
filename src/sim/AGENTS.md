# Simulator pipeline

`llg` runs compile → owned DB → semantic/execution IR → optimization → C11 →
CMake build → execution. [Lowering](codegen/AGENTS.md),
[emission](emit_c/AGENTS.md) and [runtime](rt/AGENTS.md) own domain contracts;
[source layout](../../docs/source_layout.md) locates implementation modules.

## Ownership and validation

- `codegen::{generate, generate_with_opts}` accepts owned DB data.
  `GeneratedModel` retains public `design_name: String`, `model_c` and warnings.
  `SemanticModel` owns synthesis classification and returns a checked
  `SynthDesignView` or origin-linked reasons. Never equate simulation admission
  with the conservative synthesis profile.
- Typed IR stages signals, arrays, nets, functions, processes and initialization.
  `ExecutionModel` owns executable blocks/operations, effects, suspend/resume
  plans and regions. Optimization and whole-model emission consume only it.
  Emit distinct resume-block terminators; body-controlled suspension requires a
  validated waiting operation.
- Treat `IrModelParts` as untrusted until `IrModel::from_parts` validates IDs,
  shapes, registrations and nested nodes. Keep model/detached-node validation
  before optimizer/emitter indexing; semantic and executable layers stay owned
  and independently testable.
- Simulator Rust has no unsafe/native/FFI calls. The emitter has no DB/FFI/VPI
  dependencies; preserve `G_`/`p_`/`D_` names and the first `model.c` header line.
  `tests/emit_decoupling.rs` enforces this boundary.
- Whole models use `emit_c/owned/` with registered values/scopes and explicit
  startup/teardown. Unrepresented storage/captures/callbacks produce specific
  errors, never legacy-fragment fallbacks. Detached string-only APIs cannot
  represent setup/cleanup and remain fail-closed.

## IR and optimization

Keep `OptConfig`'s `fold_constants`, `identities`, `prune_branches` and
`unused_storage`, default/none modes and per-pass bisection. Fold with exact
`core::elab::Value` X/Z semantics, f32 shortreal rounding and actual-width
arithmetic. Shape-guard identities. Prune only proven branches/cases: never pass
an unknown item or select default until all items are proven unmatched. Omit
unused storage without renumbering indices.

Read collection includes processes, functions, initialization, spawns, monitors,
links, force targets, display, waits, triggers and task temporaries. `Predicate`
retains ordered true-only evaluation, not logical-AND identities; drop suffixes
only when unreachable without discarding reached effects. Singleton concat still
forces unsigned/self-determined semantics. Array/structure conditionals retain
immediate boundaries and default-uninitialized values, not member initializers;
equal flattened width does not imply equal shape.

`PackedChain` steps are relative to the preceding selected value. Validate
nonempty/nonzero packed plans and unsigned read shape; preserve negative/X/Z
indices, partial clipping and missing-bit X/no-write behavior. Never sum offsets
and erase intermediate bounds. Count index temporaries in capacity estimates.
Fixed-array folds operate on immediate elements, use the first mapped value as
seed, preserve Z, and restore nested iterator bindings. Ordered predicates and
case patterns capture selectors once; tagged pattern discriminants and payloads
use their enclosing case mode. Ordinary checked-member guards remain exact.

## Capacity and runtime packaging

Validate each packed value against exclusive `1 << 20`, not a model maximum or
64/1024-bit IR cap. `sv4_t` widths are `uint32_t`; div/mod/pow scratch follows
operand width. Preserve typed index trees, elaborated indexed-part extents,
wide intermediate indices and member-specific two-state conversion. Emit static
extents, not the width expression's integer storage size.

Derive `LLG_MODEL_STACK_VALUES` with checked arithmetic:
`(max_function_frame * 256 + max_process_frame) * 8`, retaining the historical
minimum. Count typed expression storage across sequential statements and lexical
arms: compilers/sanitizers may retain return-by-value temporaries for the entire
C activation. Fail emission on overflow. Pass headroom at startup through
`llg_rt_init_with_args_precision_and_stack`, not the compiled runtime ABI.
Values allocate by their own widths; never restore model-maximum arrays.
`LLG_MODEL_VALUE_ABI` must match `LLG_VALUE_ABI_VERSION`.

`write_sim_sources` embeds flat value, random, scheduler, container, waveform and
libaco sources plus self-tests into `target/sim/<design>/`. Private fragments
assemble in facade order. Runtime/libaco archives belong only to generated C,
never Rust binaries; source-only output remains self-contained. Keep original
runtime/waveform ownership self-tests active. Property vectors mirror
`core::elab::Value`; regenerate with:

```sh
cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
```

## Build contract

CMake is the only model builder: C11, Release by default, executable under
`<build>/bin/`, and `m` linkage. The configure command retains:

```sh
<cmake> -S <out_dir> -B <out_dir>/build [-G <generator>] [-DCMAKE_C_COMPILER_LAUNCHER=<launcher>] -DCMAKE_C_COMPILER=<LLG_CC|$CC|cc> -DCMAKE_C_FLAGS:STRING="-O2 -Wall -Wno-unused-function [$LLG_CFLAGS]" -DLLG_RUNTIME_LIBRARY=<cache>
cmake --build --config Release
```

`CmakeBuildOpts.generator`/`--generator` overrides `CMAKE_GENERATOR`, then host
default. `launcher`/`--launcher` forwards `CMAKE_C_COMPILER_LAUNCHER` without inventing
a default. Repeatable `dpi_libraries`/`--dpi-lib` accepts validated explicit link
files; include `svdpi.h` in generated output. `generate_model_sources`/`--gen-only`
writes sources/CMake without building. `LLG_CMAKE` selects CMake, `LLG_CC`/`CC`
the compiler, and `LLG_CFLAGS` appends flags. Reject double quotes in flags;
missing-CMake errors include installation guidance. Probe availability once.

Cache by ownership ABI, runtime content, toolchain, flags, generator, launcher,
platform and waveform support. Default: `<workspace>/target/llg-runtime-cache`;
resolve relative `LLG_RUNTIME_CACHE_DIR` from the workspace root. Prune stale
sources/incompatible partial builds and retry failed configuration once cleanly.
Root portable patch preparation accepts clean/fully-applied vendors and rejects
partial/mismatched edits; retain upstream-base gitlinks.

Library optimizer comparisons reuse one DB through `generate_from_db_with_opts`;
CLI comparisons independently run checked-in HDL in both modes. Follow
[tests](../../tests/AGENTS.md) and [data semantics](../../docs/sim_data_semantics.md).
