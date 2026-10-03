# Simulator pipeline

`llg` runs compile → owned DB → semantic/execution IR → optimization → C11 →
CMake build → execution. [Lowering](codegen/AGENTS.md),
[emission](emit_c/AGENTS.md) and [runtime](rt/AGENTS.md) own domain contracts;
[source layout](../../docs/source_layout.md) locates implementation modules.

## Ownership and validation

- The CLI uses the simulator frontend export policy documented by
  [FFI](../ffi/AGENTS.md) and [driver](../bin/AGENTS.md); library generation
  consumes its caller's owned DB without changing capture limits.

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

Coroutine storage is explicit POD frame data. Count typed expression storage and
embedded callee slots in `FrameLayout`; sibling blocks overlay and recursive or
oversized callees use the chain arena. There is no generated coroutine-stack
estimate or `LLG_MODEL_STACK_VALUES`. Values allocate by their own widths; never
restore model-maximum arrays. `LLG_MODEL_VALUE_ABI` must match
`LLG_VALUE_ABI_VERSION`, and generated models declare process ABI 3.

`write_sim_sources` embeds flat value, random, coroutine, scheduler, container and
waveform sources plus self-tests into `<out-dir>/sim/<design>/` (driver default
`build`). Private fragments assemble in facade order; selected value headers retain their
`value/` or `value_gmp/` relative paths. Compact backend units compile separately. Runtime archives belong only
to generated C, never Rust binaries; source-only output remains self-contained.
Keep original runtime/waveform ownership self-tests active. Property vectors mirror
`core::elab::Value`; regenerate with:

```sh
cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
```

## Build contract

CMake is the only model builder: C11, Release by default, executable under
`<build>/bin/`, and `m` linkage. The configure command retains:

```sh
<cmake> -S <out_dir> -B <out_dir>/build [-G <generator>] [-DCMAKE_C_COMPILER_LAUNCHER=<launcher>] -DCMAKE_C_COMPILER=<cc|LLG_CC|$CC|cc> -DCMAKE_C_FLAGS:STRING="[cflags|$LLG_CFLAGS]" -DLLG_RUNTIME_LIBRARY=<cache>
cmake --build <dir> --config Release --parallel <jobs> [--target llg_runtime]
```

`CmakeBuildOpts.model_opt_level`/`--model-opt-level <O0|O1|O2|O3|Os>` selects
optimization for model and runtime sources (default `DEFAULT_MODEL_OPT_LEVEL`,
O3). Both generated projects prefix that level and warning flags to
`CMAKE_C_FLAGS`; Release contributes only `-DNDEBUG` (`/DNDEBUG` on MSVC),
removing CMake's implicit optimization. MSVC maps O0 to `/Od`, O1/Os to `/O1`,
and O2/O3 to `/O2`, with `/W3` warnings. CMake detects the compiler family.
User `--cflags` replaces `LLG_CFLAGS`; those flags follow the selected level,
so a user optimization flag wins. Source-only projects retain the selected
level; manual `-DCMAKE_C_FLAGS` flags have the same precedence. Reconfiguration
shadows the cache value without accumulating default flags. Runtime cache keys
include the level, flags and shared CMake optimization setup. Linux corpus
measurements select the default; MSVC mapping is reasoned, not executed here.

Both `--build` invocations (model and runtime archive) pass `--parallel`, a
generic option since CMake 3.12 (generated projects require 3.16), so Makefile,
Ninja and MSBuild generators build translation units concurrently. `<jobs>` is
`CmakeBuildOpts.build_jobs`/`--build-jobs`, else a positive integer
`$CMAKE_BUILD_PARALLEL_LEVEL`, else `available_parallelism()` (fallback 1); see
`resolve_build_jobs`. It is not part of the runtime cache key.

With GCC or Clang, generated model sources also get
`-Wno-misleading-indentation` as a per-source CMake option; the warning's cost
is quadratic in file size. Runtime sources keep it.

On POSIX, runtime startup warns when `RLIMIT_STACK` is below the named 8 MiB
host-stack estimate for scheduler entry, one polled segment and the 256-call
recursion guard, less a 64 KiB guard allowance so default stacks reported net of
a guard page (macOS: 8176 KiB) stay quiet. Generated MSVC projects reserve the
same default with `/STACK`.

`CmakeBuildOpts.generator`/`--generator` overrides `CMAKE_GENERATOR`, then host
default. `launcher`/`--launcher` forwards `CMAKE_C_COMPILER_LAUNCHER` without inventing
a default. Repeatable `dpi_libraries`/`--dpi-lib` accepts validated explicit link
files; include `svdpi.h` in generated output. `generate_model_sources`/`--gen-only`
writes sources/CMake without building. `CmakeBuildOpts` `cmake`/`cc`/`cflags`
(`--cmake`/`--cc`/`--cflags`) win over `LLG_CMAKE`, `LLG_CC`/`CC` and
`LLG_CFLAGS`; explicit flags replace, not append to, `LLG_CFLAGS`. Reject double quotes in flags;
missing-CMake errors include installation guidance. Probe availability once.

Cache by ownership ABI, runtime content, compiler-reported target, toolchain,
flags, generator, launcher, platform and waveform support. Root:
`runtime_cache_dir` (`--runtime-cache`) >
`LLG_RUNTIME_CACHE_DIR` > library default `<cwd>/build/llg-runtime-cache` (the
driver passes `<out-dir>/llg-runtime-cache`); relative paths resolve from the CWD.
Never bake build-machine paths (`CARGO_MANIFEST_DIR`) into runtime defaults;
`.cargo/config.toml` `[env]` points Cargo-launched runs at the repo cache. Prune stale
sources/incompatible partial builds and retry failed configuration once cleanly.
Root portable patch preparation accepts clean/fully-applied vendors and rejects
partial/mismatched edits; retain upstream-base gitlinks.

Library optimizer comparisons reuse one DB through `generate_from_db_with_opts`;
CLI comparisons independently run checked-in HDL in both modes. Follow
[tests](../../tests/AGENTS.md) and [data semantics](../../docs/sim_data_semantics.md).

## Experimental value selection

The driver reads `LLG_VALUE_BACKEND=legacy|compact` and
`LLG_COMPACT_KERNELS=portable|gmp`, default legacy/portable. `CodegenOptions` and
`CmakeBuildOpts` share `value_config`; mismatching generated guards fail before
export. `gmp_root` overrides `GMP_ROOT`, required only for compact GMP kernels.
An explicit root is authoritative and has no system fallback. Hash its header and
library contents; CMake verifies version agreement, required mpn APIs and 64-bit
nail-free limbs compatible with uint64_t. Legacy never discovers/links GMP.
Source-only projects compile their selected runtime as a static archive. Apply
waveform definitions to that archive as well as the model. Every translation unit
gets both literal selector definitions. Preserve ABI 4 for legacy, 5 for compact,
and process ABI 3. Generated startup calls the selected build-identity link fence;
foreign value clients must do likewise before exchanging descriptors. Cache ready
markers contain the exact key; plain old ready markers cannot admit stale archives.
Compact selected builds embed all currently emitted value operations, including
S4/S5 and V06 consumer primitives; unavailable additions must never fall back.
