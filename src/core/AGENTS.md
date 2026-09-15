# core — shared semantic layer

## Purpose

`core` owns the frontend-neutral data used by the simulator, linter, and language server. Slang
is the only frontend. `compile` admits in-memory source buffers and returns owned diagnostics
and a `ffi::slang::Snapshot`; consumers must not retain native pointers or read source text back
from disk.

- `db/` validates and projects the flat Slang semantic graph into the owned `Db` arena.
  `Db::from_slang` is the only frontend import. Downstream tests can construct the same layer
  with the validated test builder.
- `value.rs` defines exact frontend-neutral values. Four-state vectors use
  little-significance-first words and `(unknown, value)` planes with `X=(1,0)` and `Z=(1,1)`.
  SystemVerilog strings can contain arbitrary bytes.
- `elab.rs` provides pure four-state value operations and decoding used by semantic lowering and
  simulation.
- `model.rs` projects `Db` into editor and hierarchy models.
- `tokens.rs` projects Slang lexical tokens and exact declaration/reference bindings.
  `macros.rs` handles source-level macro information.
- `lint/` runs shared rules over owned `Db` and `DesignModel` data.

## Database contract

Semantic node IDs are stable arena indices. Parent and structural child links are acyclic;
resolved references remain typed fields or edges and may point across the hierarchy. Slang's
implicit `InstanceBody` scope is normalized so a module instance directly owns its declarations,
processes, subroutines, and child instances. Instance-array containers retain their concrete
entries in the arena; those entries are also normalized into the enclosing instance's child list
used by downstream hierarchy consumers. Explicit statement blocks remain nodes.

Elaborated packed-range entries are keyed by declaration `NodeId`, not display paths: sibling
unnamed blocks can contain same-named objects with different ranges. Consumers must match
declaration identity; names remain display data.

Types, constants, packed dimensions, aggregate members, array categories, port bindings,
definition kinds, source ranges, and time scales come from typed ABI fields. Unknown legal
constructs remain explicit `Unsupported` values. Never infer semantic categories from display
text or raw Slang enum numbers. Preserve implicit conversions because lint rules need to
distinguish source-determined and context-determined widths.

Continuous-assignment and primitive delays retain their complete ordered expression list in
`DriverDelay`. Validate every referenced expression; never collapse rise/fall/turn-off lists to
the first expression for a consumer.

`Db` owns every admitted source buffer and exposes it through `source_text`. Source-dependent
lowering may use that bounded text; it must not open a path reported by the frontend. Invalid
ranges, IDs, windows, cycles, and table references fail construction.

## Requirements

- No `unsafe` in core. The safe ownership boundary is `ffi::slang`.
- No LSP-only dependencies in core. The library must build without `lsp`.
- Extend the typed snapshot and `Db` for shared semantic facts instead of adding frontend calls
  in a consumer.
- Keep semantic IR and executable IR separately testable. Tests of downstream layers should use
  the direct validated database builder.
- Unsupported executable constructs must reach a typed rejection path; never silently lower them
  to an empty statement or a fabricated default.
- Source-byte budget rejections use `StartupErrorKind::LimitExceeded`, including Rust preflight
  and isolated parsing, so callers can report resource guidance.

## Interactions

The FFI ownership and ABI rules are in [../ffi/AGENTS.md](../ffi/AGENTS.md). Simulator lowering
rules are in [../sim/AGENTS.md](../sim/AGENTS.md), language server rules in
[../bin/llg_ls/AGENTS.md](../bin/llg_ls/AGENTS.md), and lint contracts in
[lint/AGENTS.md](lint/AGENTS.md).
