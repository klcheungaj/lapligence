# Safe native boundary

Only `src/ffi/` may contain Rust `unsafe`. Keep platform/native primitives here,
expose stable safe ownership APIs, and explain every unsafe operation's soundness.
Each module denies `clippy::undocumented_unsafe_blocks` and
`unsafe_op_in_unsafe_fn`; explain validity, lifetime, alignment, initialization
and ownership at every unsafe operation. No panic may cross C. Native owners
remain on the calling thread; do not implement `Send` or `Sync`.

## Slang capture-stream ABI

- Mirror `slang_c_api.h` layouts with `#[repr(C)]`. Validate ABI version, tags,
  known flags, reserved fields, pointer/length pairs, IDs, ranges and table windows.
  Use module error types, preserving native status/message; malformed output is
  `InvalidNativeData`, distinct from valid unsupported HDL.
- ABI v15 `CompileRequest` borrows sources/options until blocking
  `llg_slang_compile` returns, distinguishing units from include-only buffers.
  Library-unit recovery uses the same buffers/limits; reject unknown request flags.
  Cache keys are lexically normalized; include directories are lookup prefixes,
  not permission to read the filesystem.
- The source library-map flag requires a compilation-unit flag and parses the
  original admitted buffer with Slang's map preprocessor and grammar.
- Library include directories are ordered name/path pairs in the request;
  `SourceLibrary` lookup uses them only for that library's admitted buffers.
- Snapshot `source_libraries` records (v9) name one semantic node each, are
  bounded by the semantic-node limit and charged to output bytes; reject unknown
  or repeated nodes and empty library names.
- Snapshot `line_directives` records (v10) give a file id, the byte offset of
  the first line a `` `line`` directive maps, and the frontend's own logical
  line and file there. Reject unknown files and offsets outside the file;
  sort by file and offset and collapse agreeing duplicates (one include read
  as several buffers), rejecting disagreeing ones.
- Snapshot `edition_findings` records (v14) give a known-file source range and
  a nonempty label, in capture order; at most `MAX_EDITION_FINDINGS`. Only a
  Verilog-2001 compile produces them.
- Bound defines, tops, includes, parameter overrides, source bytes, diagnostics,
  value bits, output bytes, semantic records/edges and tokens on both ABI sides
  before/during allocation. The capture arrives through the `LlgSlangSink`
  stream (`slang/stream.rs` documents catalogue, order, ownership and error
  flow): header counts are limit- and budget-checked before reservation,
  strings are charged on receipt, tables must arrive in header order with
  exactly the announced counts, and type/instance/node IDs must equal their
  dense indices. Records are borrowed only for the callback; copy what is kept.
  Trampolines catch panics, record only the first error and return
  `LLG_SLANG_SINK_ABORT`; the bridge then returns status 5 and the receiver's
  error wins. Non-OK returns an error owner (destructor accepts null). HDL
  errors set the header's `has_errors`; argument/resource/setup/exception/
  bridge failures return `SlangError`. Expose no native pointer or
  native-storage lifetime.
- `SemanticNode` is size-bounded (`semantic_node_stays_compact`). Add a boolean
  as a `SemanticFlags` bit with a named accessor in `slang/node_flags.rs` (bits
  0..=31 mirror the raw ABI flag word except the definition-kind bits 13..=15,
  which the node keeps as an enum), not as a `bool` field. Node, type, constant
  and edge-window references are stored as `u32` (`CompactId`, `u32::MAX` means
  absent); decoding rejects an ID that does not fit, and readers use the
  `u64`/`Option` accessors. Raise the size bound only with a measured reason.
- Preserve paired value/unknown limbs, distinct real/shortreal widths and
  arbitrary-byte SystemVerilog strings; every other ABI string is UTF-8.
  Source ranges use admitted file IDs and zero-based half-open byte offsets;
  absent IDs use the invalid-ID sentinel. Kinds/operations/subkinds/flags/roles/
  lexical categories are stable repository codes, never Slang enums or pointers.
- Check sequence repetition/ranges and `SequenceConcat` cycle ranges; convert
  unbounded `UINT32_MAX` maxima to `Option<u32>`, rejecting invalid ranges/flags/
  kinds. Validate optional member-default constant IDs; fixed-array defaults
  cross as exact flattened bitstreams. Retain declaration-order violations in
  compilation-unit lexical uses, including macro expansions.
- Keep static `#[link]` metadata for wrapper, Slang and fmt. The library and every
  final musl target must retain the required native archives/wrapping.
- Bound UDP semantic nodes/edges; reject an edge budget above the native ceiling
  of 256 million. `Limits::default()` retains the library/interactive budgets
  (256 MiB export, 4M semantic nodes, 16M edges, 1M constants).
  `Limits::simulator(bytes)` uses the native record ceilings (64M nodes, 256M
  edges, 16M constants), retaining all other default limits. Keep these ceilings
  and the 16 GiB native export ceiling aligned with the wrapper; the simulator
  driver chooses a separate 4 GiB export budget. Each
  row owns exactly one valid window. Wildcard indexing has snapshot-wide ceilings
  of 65,536 assignments and 8 MiB key bytes; check expanded bucket/key costs before
  allocation.
- Lexical flag bit 3 (`LLG_SLANG_LEXICAL_DIRECTIVE`, owned `is_directive`)
  identifies directive text, including unexpanded macro bodies, independently of
  expanded/skipped state. Disabled conditional branch tokens also carry the skipped
  flag, even when they occur inside a directive. Edition checks must not treat
  inactive branches as source. Accept only the four defined bits and reject
  reserved bits; update both ABI sides together.
- `ConditionPattern` (`LLG_SLANG_EDGE_CONDITION_PATTERN`, role 38) pairs by index
  with `Condition`; `THEN`/`ELSE` are separate branch roles at index zero. Extending the
  semantic tag set does not change pointer/layout contracts. DB import validates
  these relationships; capture must not turn a pattern match into a Boolean test.

## Platform and filesystem calls

`process_memory.rs` reads Linux resident pages from `/proc/self/statm`, macOS
`task_info` and Windows `GetProcessMemoryInfo`. Validate handles/structures;
footprint multiplication saturates at `u64::MAX`. Preserve platform error sources.
Unsupported platforms return a typed error, never a misleading zero footprint.
Use checked/saturating conversions; shared policy remains in `memory_limit.rs`.

`secure_fs` admits paths through owned directory handles and identity checks.
Preserve capability-relative containment and race/symlink/reparse/hardlink
protections rather than reverting to path-check-then-open operations.
See [patch preparation](../../patches/README.md) for authenticated native inputs
and staging guarantees.

## Tests

Exercise layout/tag validation, ownership on every error path, budget boundaries,
exact values and copy-before-release. Prefer Rust-side FFI tests. Coordinate
capture changes with [wrapper](../wrapper/AGENTS.md) and
[owned DB](../core/AGENTS.md); consumers must not add independent native traversals.
