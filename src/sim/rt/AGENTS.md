# Agent guide: src/sim/rt

The embedded C11 runtime that generated models compile and link: exact-width
four-state values, the IEEE region scheduler, stackless coroutines (`llg_co`),
strings, containers, constraints, coverage, DPI/VPI and optional waveforms.
It ships only as sources written beside a generated model and built by CMake;
it is never linked into Rust binaries. Module overview and component map:
[readme](readme.md).

## Core abstractions

- **`sv4_t`** — owned exact-width packed value behind the value facade
  (compact ABI 5 with GMP kernels; legacy ABI 4 is a development reference).
- **`llg_co_frame_t` / `llg_co_desc_t`** — POD coroutine frame and immutable
  descriptor; process ABI 3 ([process_abi.md](process_abi.md)).
- **`llg_proc_t`** — scheduled process; one per initial/always/link/continuous
  process or fork branch.
- **Regions** — typed queues for every IEEE region and PLI control point.
- **Scope nodes and retainers** — caller-owned frame cells vs heap owners.
- **`llg_net_t`** — resolved net with driver slots, strengths and delays.
- **Collected objects** — class objects under `gc/`, rooted by model tables.

## Directory map

- `llg_value.*`, `value/` — legacy facade and shared value ops; `value_gmp/`
  — compact backend ([readme](value_gmp/readme.md)).
- `llg_rt.*`, `llg_rt_prelude.c`, `scheduler/` — scheduler, nets, events,
  assertions, I/O.
- `llg_co.*` — self-contained coroutine library.
- `llg_string.*`, `llg_container.*`, `container/` — native owners.
- `llg_constraint.*`, `constraints/` — randomization solver
  ([readme](constraints/readme.md)).
- `coverage/`, `gc/` — covergroups/coverage database and the collector.
- `llg_vpi.*`, `llg_dpi.*`, `svdpi.h`, `vpi_user.h`, `sv_vpi_user.h` —
  foreign interfaces; `llg_wave.*` — VCD/FST writer.
- `llg_compiler.h`, `llg_platform*.h` — the only place for platform
  conditionals.
- `mod.rs`, `gmp.rs`, `tests.rs` — Rust source embedding and checks.

## Conventions and pitfalls

- Generated code and other runtime parts use only the public value facade
  ([facade contract](value/facade.md), [ownership](value/ownership.md)).
- Compiler/OS conditionals go only in the platform layer (rules in the
  `llg_platform.h` header comment); add a service there first.
- Keep `llg_co` free of globals and TLS.
- `llg_arm_*` copy contract: `llg_rt.h`; an arm that must retain a pointer
  says so there, and the emitter keeps it in a frame field or registered cell.
- Registries and queues grow with checked allocation; no fixed ceilings.
- Keep the original runtime and waveform self-tests active; component tests do
  not replace emitted-model, HDL, parity, sanitizer or native-platform gates.
- Keep facade-only translation units and the flat embedding order in step.
- Write C that also passes MSVC `/W4 /WX`
  ([platform notes](readme.md#platform-notes)).
- Feature/status inventories live in [sim_features](../../../docs/sim_features.md).

## Commands

```sh
cargo nextest run --test runtime --test runtime_value_storage --test runtime_strict_sources
cargo nextest run --test runtime_coroutine_library --test runtime_value_facade
cargo test --test general property_elab::gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
```

The last command regenerates the C property vectors (`tests/property_elab.rs`).
Native probes: [runtime tests](../../../tests/runtime_value_storage/readme.md).

## Contracts

- [Values and native owners](values.md); [compact backend](value_gmp/readme.md)
- [Scheduler](scheduler/readme.md): frame-resident cells, coroutines, regions,
  processes, nets, switches, paths, timing checks, SDF, waits, limits
- [Stop, synchronization and assertions](scheduler/control.md)
- [Collected objects](gc/readme.md); [functional coverage](coverage/readme.md)
- [I/O, VPI and waveform](io.md)
- [Source assembly, validation and platform notes](readme.md)
