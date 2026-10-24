# Agent guide: src/sim/emit_c

Renders a validated `ExecutionModel` into one standalone C11 `model.c` (plus
`model.symbols.tsv`) with ordered setup, evaluation and cleanup, linked against
the embedded [runtime](../rt/AGENTS.md). Applies to `emit_c.rs`, `emit_c/` and
`owned/`. Module overview: [readme](readme.md).

## Core abstractions

- **`Frame`** — one procedure body being rendered; owns declarations
  (`Frame::declare`), C lines and braces (`Frame::line`), scopes and resume sites.
- **`CStack` / `CoFrame`** — plain C storage vs coroutine POD frame storage,
  decided after the whole block tree is seen.
- **`FrameLayout`** (`frame_layout.rs`) — exact block tree, flattened chains,
  sibling `uN.bK` overlays, call-site unions, LP64 upper bounds, GC maps.
- **`Value`** (`owned.rs`) — result code, shape and ownership (owner vs borrow).
- **Destinations** (`destinations.rs`) — every packed/string result goes
  through the runtime's `_to` destination forms.
- **`PackedConstants`** (`constants.rs`) — per-model deduplicated constants.
- **Sharing** (`model/sharing.rs`) — exact instance body sharing with
  per-instance records.
- **Names** (`names.rs`) — source-name escapes and the 32-character
  identifier registry.

## Directory map

- `emit_c.rs` — facade; legacy fragment APIs stay fail-closed.
- `owned.rs`, `owned/` — expressions, lvalues, calls, captures, native
  services, scopes; `owned/model/` storage, initialization and lifecycle API.
  Map: [owned/readme.md](owned/readme.md).
- `model.rs`, `model/` — whole-model assembly: storage, processes, functions,
  classes, DPI, VPI, net batches, recursion, sharing.
- `frame_layout.rs`, `destinations.rs`, `constants.rs`, `names.rs`,
  `containers.rs`, `objects.rs`, `expressions/`, `statements/` — shared helpers.

## Conventions and pitfalls

- Consume only validated `ExecutionModel`: no frontend, DB, FFI or VPI
  dependency and no reparsing of opaque C fragments
  (`tests/emit_decoupling.rs`). Keep `G_`/`p_`/`D_` names and the first
  `model.c` header line.
- Unrepresented storage, captures or callbacks fail with specific errors;
  never add a legacy-fragment or `Verbatim` fallback.
- Declare all typed procedure storage through `Frame::declare` and emit every
  body brace through `Frame::line`; never keep an overlaid field access after
  its block closes.
- Debug poisoning goes inside `#ifdef LLG_CO_DEBUG` only; release output and
  frame layouts must not change.
- Install packed/string results through `destinations`; register a new
  value-returning runtime call there with its `_to` declaration.
- Rendering stays linear in instances: build model-wide indexes once
  (`StorageNames`); no per-access scans of signals, functions or objects.
- Generated identifiers are standard C (never `$`) and at most 32 characters;
  runtime API names and foreign DPI symbols keep their exact spelling. Labels,
  VPI/waveform names and diagnostics stay source-derived.
- The owned (production) and borrowed (test) paths must produce
  byte-identical `model.c` and `model.symbols.tsv`.
- The process-global runtime runs one model at a time; keep value ABI 4/5 and
  `LLG_MODEL_PROCESS_ABI 3` aligned with the runtime.
- Component or hand-written C probes do not prove emitter behaviour; use real
  IR-to-C emission and HDL runs.

## Commands

```sh
cargo nextest run --test general emit_decoupling generated_c_frame_lint generated_c_determinism
cargo nextest run --test sim_stack_bounds --test sim_instance_sharing --test sim_compact_names
cargo nextest run --lib sim::emit_c
```

## Contracts

- [Evaluation and scopes](evaluation.md): frames, overlays, poisoning,
  narrowing, ownership, constants, UDP/path/timing/SDF tables, dense cells.
- [Storage and native boundaries](storage.md): publication, references,
  DPI/strings/handles, containers, streams, fork and event captures.
- [Model lifecycle](model/readme.md): start/advance/close, frame guards, net
  tables, names and the symbol map, validation layers, body sharing,
  generation memory.
- [Runtime ownership](../rt/value/ownership.md),
  [frame-resident cells](../rt/scheduler/readme.md#frame-resident-cells),
  [ownership validation](../../../tests/readme.md#dynamic-ownership-validation).
