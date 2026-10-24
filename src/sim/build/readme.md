# Model build

`build.rs` and `build/` turn generated sources into a CMake project, build the
cached runtime archive and the model, and export source-only projects. CMake is
the only model builder. Most contracts live in the module docs; this page lists
where, plus the rules the code comments do not state.

## Where the rules live

| Topic | Source of truth |
| --- | --- |
| Build flow, stale-output pruning, clean retry, generator and launcher order, environment fallbacks, cache key, cache root and auxiliary directories | `build.rs` module docs |
| Optimization level, MSVC mapping, flag precedence, `--parallel` and job count | `CmakeBuildOpts` field docs, `ModelOptLevel`, `resolve_build_jobs` |
| `-Wno-misleading-indentation` on model sources only | comment above the per-source option in `build.rs` |
| Host-stack estimate, POSIX warning and MSVC `/STACK` reserve | `rt/llg_rt.h` stack-estimate comment and `build.rs` |
| Toolchain seeds (key, self-check, markers, `LLG_CMAKE_TOOLCHAIN_SEED=0`, `toolchain_args`) | `build/toolchain_seed.rs` module docs |
| Configured-tree templates (`LLG_CMAKE_TREE_TEMPLATES=1`, qualifying paths, spellings, rejection) | `build/tree_template.rs` module docs |
| Compiler self-report probes and their memo files | `build/compiler_probe.rs` module docs |
| `sim.def` and the import library for DLL companions and VPI plugins | `build/import_library.rs` module docs |
| `LLG_BUILD_TIMINGS`, `LLG_CMAKE_PROFILE_DIR` | `build/timings.rs` module docs |
| DPI-C `byte` as C `char` with explicit sign extension | `emit_c/model/dpi.rs` comments |
| Value backend selection and GMP | `value_backend.rs`, `build/value.rs`, [compact backend](../rt/value_gmp/readme.md) |

## Additional contracts

- Projects are C11, Release, link `m` and place the executable under
  `<build>/bin/`. The configure command is:

  ```sh
  <cmake> -S <out_dir> -B <out_dir>/build -G <generator> [-DCMAKE_C_COMPILER_LAUNCHER=<launcher>] -DCMAKE_C_COMPILER=<cc> -DCMAKE_C_FLAGS:STRING="[cflags]" -DLLG_RUNTIME_LIBRARY=<cache>
  ```

- Every llg configure passes `-DCMAKE_BUILD_TYPE=Release` (MSVC
  single-config generators would otherwise default to Debug); self-contained
  `--gen-only` projects need the same argument under MSVC with Ninja.
- MSVC builds use `/W3`; the MSVC level mapping is reasoned, not measured.
- Manual `-DCMAKE_C_FLAGS` flags on a source-only project have the same
  precedence as `--cflags`.
- The job count is not part of the runtime cache key.
- Re-measure before changing the host-stack estimate or the `/STACK` reserve.
- The `llg` driver resolves the launcher in `settings::layered` before it
  fills the option.
- Keep the `sim.def` routine lists in step with the runtime (unit-tested).
- With MSVC the runtime cache key records `cl`'s stderr banner plus
  `VCToolsVersion`, `WindowsSDKVersion` and `VSCMD_ARG_TGT_ARCH`.
- Missing-CMake errors include installation guidance.
- The driver passes `<out-dir>/llg-runtime-cache` as the cache root. Never bake
  build-machine paths (`CARGO_MANIFEST_DIR`) into runtime defaults;
  `.cargo/config.toml` `[env]` points Cargo-launched runs at the repo cache.
- A new auxiliary cache directory must be added to
  `sim::build::RUNTIME_CACHE_AUX_DIRS`.
- Seed publication runs under a non-blocking entry lock; the comparison also
  ignores tree-path spellings (plain, forward-slash, canonical and
  Ninja-escaped `$:`/`$ `/`$$`) and detection scratch such as MSVC's
  `CMakeFiles/ShowIncludes/`, and a rejection names each differing file with
  its first differing line.
- Projects keep the shared `PROJECT_PREAMBLE` before `project()`.
- Runtime-archive trees always configure; templates apply only to seeded
  fresh model trees.
- `LLG_CMAKE_PROFILE_DIR` stays out of every cache and seed key;
  `--cmake-profiles` summarizes the profiles.

## Platform notes

General platform handling: [cross-platform guide](../../../docs/cross_platform.md)
([Windows](../../../docs/cross_platform.md#windows-msvc),
[macOS](../../../docs/cross_platform.md#macos)). Models run on the host. The Windows install-prefix and MSVC try-compile (`/Zi`) workarounds are
in `toolchain_seed.rs` (`toolchain_args`).

See [tests](../../../tests/readme.md) and
[data semantics](../../../docs/sim_data_semantics.md).
