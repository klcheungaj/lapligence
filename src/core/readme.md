# Shared semantic core

The simulator, linter and language server share owned frontend-neutral data.
Slang capture enters through `compile` and `Db::from_slang`; consumers neither
retain native pointers nor reopen source paths.

| Component | Responsibility |
| --- | --- |
| `compile/` | Bounded source/include admission, library/configuration mapping, edition checks and diagnostics. |
| `db/` | Validated semantic arena, typed references, source buffers and import/test builders. |
| `value.rs`, `elab.rs` | Exact four-state values, arbitrary-byte strings and pure value operations. |
| `model.rs` | Editor and hierarchy projections. |
| `tokens.rs`, `macros.rs` | Lexical tokens, exact declaration/reference bindings and macro information. |
| `lint/` | Shared rules over the owned DB/design model. |

Library-map selection, source-preserving native map configuration parsing and strict
edition checks are shared by execution and navigation. `compile_checked` rejects
every protected (encrypted) envelope with one unsupported-by-design error
(`compile/protected.rs`); unchecked compiles keep the envelopes for the language
server's black-box handling. See the
[database](db/readme.md), [linter](lint/readme.md) and
[source map](../../docs/source_layout.md).

Per-library `-incdir` in a map adds ordered search prefixes for that library.
Paths are relative to the containing map; Rust admits reachable headers before
Slang's cache-only lookup. Local source directory, global command-line include
directories, then library directories is the selected precedence. Wildcard directory
matches are sorted; logical directories are represented by admitted buffers
under their path, so an empty logical directory cannot be selected.

Macro and conditional expansion supplies the bounded Rust map resolver with the
declarations Slang parses from each original map buffer. This keeps configuration
diagnostics at macro uses. Command-line defines seed each map; a map's local
definitions stay within that map, including when another map is named by a map
`include` declaration. Design source units have separate macro environments.

## Library maps

- Match ranks: explicit filenames, then wildcard filenames, then
  trailing-directory matches (rank rules: `compile/library_mapping.rs`).
  Same-library matches deduplicate and explicit library overrides are kept. Disk
  and logical inputs share one bounded resolver; metadata is captured once, with
  the original buffers/handles as source truth and no native I/O.
- Missing or inadmissible `-incdir` directories reject at map admission; a quoted
  `"-incdir"` stays a literal file pattern. In-memory maps select logical
  directories that hold admitted buffers or are listed in
  `logical_directories`.
- Library sources follow the compilation-unit mode: separate gives each file its
  own preprocessor; merged shares one per library, in admission order, in Rust
  include preflight and in the wrapper alike. `Db::source_library` keeps each
  instance's, package's and unit's bound library for `%l`.

## Edition checks

Edition checks use classified tokens and owned semantic edges shared by execution
and navigation (`compile/editions.rs`).

- The 2001 profile gates keyword-free later grammar: type shapes
  (queue/dynamic/associative, multiple packed ranges), subroutine formals and
  bodies, unnamed-block declarations and procedural initializers (semantic
  nodes); labels, `.name`, `edge`, casts, time literals, inline genvars,
  `[size]`, `localparam` ports and empty `()` (token sequences); generate
  constructs (`generate` regions).
- It also rejects variables driven by continuous assignments, gate outputs and
  output/inout ports, and variable input/inout ports. The 2009 profile rejects
  2012 covergroup bins forms.
- Sequence rules skip macro-expanded tokens, whose shared use-site range has no
  order; the wrapper's closed syntax profile (`edition_findings`) covers
  macro-expanded trees and is merged after these rules, dropping a finding at an
  already reported position.
- Body rules need executable capture; navigation snapshots skip them. Directive
  bodies are ignored until expanded; executable for-loop header edges differ
  from navigation-only capture.
- Memory-storage exemptions are argument-specific: `$fread` and legacy PLA
  personality argument 0, `$readmem*` argument 1, `$sreadmem*` argument 0. PLA
  syntax admission does not select a simulator PLA target.
