# FFI and platform boundary

This module contains the project's Rust unsafe/native operations and exposes safe
owned APIs to the rest of the library.

| Component | Responsibility |
| --- | --- |
| `slang.rs` and `slang/` | C ABI requests (`LLG_SLANG_ABI_VERSION`), the capture-stream receivers (`slang/stream.rs`), bounded error owners, layout/tag validation, exact value/text copies and RAII destruction. |
| `process_memory.rs` | Platform process-memory counters and native resource limits. |
| `secure_fs` | Handle-relative filesystem admission and identity/race protection. |

The wrapper streams its finished capture table by table into one
`StreamBuilder` (design in `slang/stream.rs`). Type, instance and semantic-node
IDs must be their dense table indices; references to later tables are checked
against the header's announced counts and window ownership is checked when each
table closes. Owned tables reserve the validated counts exactly, and each
native table is released once delivered.

`SemanticNode` is kept small because the staged snapshot holds one per
semantic construct: its booleans are one `SemanticFlags` word read through
named accessors (`node.is_bad()`; `slang/node_flags.rs`), node, type and
constant references are `u32` with an absent sentinel, and time scale and
assertion range metadata sit behind a box. Read those fields through the
accessors of the same name; a unit test bounds `size_of::<SemanticNode>()`.

Snapshot data includes source/lexical provenance, typed semantic edges, UDP tables,
sequence metadata, aggregate defaults and specify items (semantic kind 33:
blocks, module paths with their connection/polarity/edge fields, pulse styles
and timing checks with their events, edge masks, conditions, limits and
notifier; specparams are parameter subkinds 236/237). No native pointer or borrowed buffer
escapes the safe interface. See [wrapper](../wrapper/readme.md),
[owned database](../core/db/readme.md) and
[patch preparation](../../patches/README.md).

`Limits::default()` preserves the interactive/library capture budgets.
`Limits::simulator()` admits whole batch designs bounded only by memory: no
byte or value-bit budget, and record counts and source buffers at their
structural native ceilings. Export accounting covers captured records and
strings; the process-memory guard is the resource bound.

The source map flag asks Slang to parse an admitted original map buffer with
its preprocessor. It requires compilation-unit admission and preserves macro
invocation locations in configuration diagnostics.

## ABI notes beyond the header

- `CompileRequest` and `parse_metadata` borrow sources/options until the
  blocking call returns and distinguish units from include-only buffers.
  Library-unit recovery reuses the same buffers and limits. Cache keys are
  lexically normalized; include directories are lookup prefixes, not permission
  to read the filesystem.
- `parse_metadata` bounds sources by `max_source_bytes` and discards parse
  diagnostics.
- The library-map source flag requires the compilation-unit flag. Library
  include directories are ordered name/path pairs used only for that library's
  admitted buffers.
- Header counts are limit- and budget-checked before reservation and strings are
  charged on receipt. Trampolines record only the first error; the bridge then
  returns status 5 and the receiver's error wins. A non-OK return carries an
  error owner. HDL errors set the header's `has_errors`; argument, resource,
  setup, exception and bridge failures return `SlangError`.
- Source ranges use admitted file IDs and zero-based half-open byte offsets.
  Value/unknown limbs stay paired, real and shortreal widths stay distinct, and
  SystemVerilog string constants are arbitrary bytes; every other string is
  UTF-8.
- Sequence repetition and `SequenceConcat` cycle ranges are checked; unbounded
  `UINT32_MAX` maxima become `Option<u32>`. Optional member-default constant IDs
  are validated; fixed-array defaults cross as exact flattened bitstreams.
  Compilation-unit lexical uses keep declaration-order violations, including
  macro expansions.
- `edition_findings` records come only from a Verilog-2001 compile.
- `protected_envelopes` are sorted by file and start; the count is bounded by
  the lexical token limit, kinds and flags are closed sets, and `scope` is the
  enclosing declaration's name range or none.
- `fsm_pragmas` (ABI 61) are comments starting with the word `tool` (IEEE
  1800-2009 40.4), sorted by file, start and declarator ordinal, then the
  instance records of each declaration holding one. Their count has no limit
  of its own (one record per bound declarator) and is bounded by the record
  byte budget. The text must span exactly the comment range minus its
  delimiters; instance records must name distinct instance nodes of a
  reported scope. Parsing and binding happen in the owned DB.
- A `default disable iff` expression (`LLG_SLANG_EDGE_DEFAULT_DISABLE`) is owned
  by the first assertion statement that uses it. `restrict property` is kept as
  `ConcurrentAssertionKind::Restrict`, which the simulator does not verify.
- Lexical directive flags mark directive text, including unexpanded macro
  bodies, independently of expanded/skipped state; disabled conditional-branch
  tokens also carry the skipped flag. Edition checks never treat inactive
  branches as source.
- `ConditionPattern` (role 38) pairs by index with `Condition`; `THEN`/`ELSE`
  are separate roles at index zero. DB import validates the pairing.
