# Verilog/SystemVerilog simulator

The simulator lowers owned frontend data into a validated execution model, emits
C11 and builds it with CMake. Generated models link their own stackless runtime;
it is not a Rust runtime dependency.

| Component | Responsibility |
| --- | --- |
| `legacy_unsupported.rs` | Stable rejections for legacy constructs unsupported by design (MOS, trireg, charge directives, `$dumpports*`, PLA, inspection tasks, PLI 1.0 TF/ACC, rejected `assign`/`$q_*` forms). |
| `semantic/` | Frontend-neutral model, source origins, synthesis classification and executable-node coverage. |
| `codegen/` | Typed lowering of declarations, values, processes, links and scheduling. |
| `ir/` | Shared typed storage/value/statement tables and validation. |
| `execution/` | Executable blocks, effects, regions and suspend/resume plans. |
| `opt/` | Shape- and four-state-aware execution-model optimizations. |
| `emit_c/` | Ordered C setup/evaluation/cleanup and model lifecycle. |
| `rt/` | Exact-width values, scheduler, native services and optional waveform output. |

`generate_from_db_with_opts` reuses one owned DB across optimization variants.
The driver uses `generate_from_owned_db_with_codegen_options` to retain all
generation tunables while releasing the DB before execution lowering/rendering.
`GeneratedModel` carries the design name, generated C and warnings. Unsupported
executable forms fail with source-linked errors before C compilation.

Model and runtime builds share `CmakeBuildOpts.model_opt_level` (default O3).
The driver exposes it as `--model-opt-level <O0|O1|O2|O3|Os>`, including in
source-only output. Release adds only NDEBUG; explicit `--cflags` (else
`LLG_CFLAGS`) follow the selected level and can override it. Runtime archive
cache entries include the level and user flags. MSVC maps O0 to `/Od`, O1/Os
to `/O1`, and O2/O3 to `/O2`. Every llg configure passes
`-DCMAKE_BUILD_TYPE=Release` (MSVC single-config generators would otherwise
default to Debug: `/Od`, `/debug` link); self-contained `--gen-only` projects
need the same argument under MSVC with Ninja.

See [feature status](../../docs/sim_features.md),
[build and CLI usage](../../readme.md), [tests](../../tests/readme.md) and
[source layout](../../docs/source_layout.md).

Packed values use the compact descriptor with GMP kernels. The bundled
`vendor/gmp` subset ([`rt/gmp.rs`](rt/gmp.rs)) is compiled into the runtime
unless `GMP_ROOT` names a compatible installation. The legacy descriptor and
portable kernels remain as development references, selected only by
`LLG_DEV_VALUE_BACKEND`/`LLG_DEV_COMPACT_KERNELS` or by library callers. Library callers pass the same `ValueConfig`
in `CodegenOptions` and `CmakeBuildOpts`. Source export retains the selection,
ABI guards and dependency fingerprint. Compact selected models include S1–S9
and native consumer primitives.
