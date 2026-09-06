# ffi — Rust ↔ C(++) FFI layer

## Purpose

The only module that talks to the vendored Surelog/UHDM C++ world, over a
C-ABI boundary (`src/wrapper/`):

- `surelog.rs` — Surelog compile sessions: `SurelogSession`, `SessionBuilder`
  (parse/compile/elaborate/`-elabuhdm` flags), structured `Diag`/`Severity`.
- `vpi.rs` — safe wrapper over the UHDM VPI traversal API: `iterate`/`handle`
  (with the `OwnedHandle` lifetime rule), `get`/`get_str`/`read_value`.
- `process_memory.rs` — platform-specific physical-footprint sampler
  (Linux/macOS/Windows) backing the shared `memory_limit` safeguard; the only
  place `unsafe` platform calls live.

## Requirements

- **This is the ONLY module allowed to contain `unsafe`** (extern blocks,
  `unsafe impl Send`, the single union read inside `vpi::read_value`).
  Enforced: `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be
  empty.
- Exposes **stable safe APIs** for `core`, `sim`, and the binaries: values are
  read via `vpi::read_value -> ValueData` (owned), never via the raw
  `VpiValueData` union.
- `SessionBuilder` owns its three C++ construction handles until `build()`
  explicitly transfers them to `SurelogSession`; abandoning the builder frees
  them in dependency order. The legacy `start_compiler()` API returns `None`
  instead of wrapping a null C++ compiler pointer.
- Use `#[repr(C)]` for shared structs and `extern "C"` for exported functions.
  Read [../wrapper/AGENTS.md](../wrapper/AGENTS.md) for boundary ownership.
- Preserve the static `surelog_c_wrapper` link attributes (root build rule).
- Safe APIs must not expose fabricatable pointers or outlive foreign resources.
  Validate foreign strings, unions, sizes, and handles before use.
- Each Rust FFI module denies Clippy's `undocumented_unsafe_blocks` lint and
  `unsafe_op_in_unsafe_fn`. Every unsafe operation needs a local safety
  explanation; extern declarations, functions, and trait implementations also
  carry explicit contracts at their declaration sites.
- Fallible public APIs use module-specific errors with preserved sources;
  process-memory operations return `MemoryError` rather than string errors.

## Interactions

- Below: `src/wrapper/` (C ABI) and `vendor/Surelog` (C++).
- Above: `src/core/` (compile, elab, db, model, tokens) — all UHDM access
  goes through here; `src/sim/` and the binaries consume `core`'s owned data
  and never call VPI directly.

## UHDM/VPI field notes (Surelog v1.87)

- **Handle lifetimes**: handles from `vpi::iterate` are borrowed; handles from
  `vpi::handle(...)` return `vpi::OwnedHandle` which frees the wrapper on drop
  — keep the OwnedHandle alive in a local and use `.raw()` for nested calls.
  Never store the raw pointer of a dropped OwnedHandle (use-after-free).
- **1-to-1 vs 1-to-many**: `vpi_iterate` returns null for single-object
  relationships (`vpiRhs`, `vpiLhs`, `vpiStmt` on a single stmt, …) — use
  `vpi_handle` for those. Prefer the `iter`/`child_handle`/`each_child` helper
  pattern from `src/bin/elab_check.rs`.
- **No `vpiValue` string property** in this UHDM build: read constants and
  parameter values via the safe `vpi::read_value` → `ValueData` (formats
  `vpiBinStrVal`…, `vpiIntVal`, `vpiUIntVal`, `vpiStringVal`). The raw
  `VpiValueData` union is read in exactly one place (inside `ffi/vpi.rs`).
  `vpiSize == -1` means an unsized literal (`'1`, `'x`): fill-on-resize
  semantics (see `elab::Value::fill`).
- **Operation op-types** use UHDM's numbering (`vpiAddOp`=24, `vpiSubOp`=11,
  `vpiEqOp`=14, …) — use the `vpi.rs` constants, never magic numbers.
- Top-level `module_inst` objects have **no `vpiFullName`** (use `vpiName`).
- `vpi_get(vpiType, …)` returns VPI-mapped constants (e.g. `vpiRefTypespec`),
  not the raw `uhdm*` discriminants.
- Port typespecs live under `vpiTypedef`; nets/vars/params under
  `vpiTypespec`.
- `gen_scope` is reached with `vpi_iterate(vpiGenScope, gen_scope_array)`
  (`vpi_handle` returns null there).
- `initial` processes report `vpiAlwaysType` = 1 (same as `always`) —
  distinguish by object type (`vpiInitial`).
- Concat operands may be reversed (Surelog sets `vpiReordered`); respect it.
- `indexed_part_select` uses `vpiBaseExpr`/`vpiWidthExpr`, not `vpiIndex`/`vpiSize`.
- Surelog prefixes top design-unit names with the library, e.g.
  `work@param_top` — strip that known top-level `lib@` qualifier for display
  and source matching. Do not apply this rule to arbitrary `vpiName` values:
  an escaped SystemVerilog source identifier can legally contain `@`.
- `-nowarning` **removes** warnings from the error container at add-time (not
  just at print time); `-noinfo` still leaks one `CM0023` info diagnostic.
- Driving flags via setters requires `set_write_pp_output()`; without it the
  design comes out empty.
- `vpi_iterate(vpiParamAssign, …)`/`vpiParameter` work on gen_scope objects too.
- The parse-tree C ABI (`SL_VObjectInfo` → `surelog::ParseNode`) carries
  `parent_index`, `child_index`, and `sibling_index`; zero is Surelog's
  invalid-node sentinel.  These are owned links used by the LSP's source
  graph and enum scanners.  Parse-node positions remain 1-based and may be
  zero/unknown, so convert with `saturating_sub` when forming 0-based keys.

The complete parse/compile/elaborate setter contract is in
[../core/AGENTS.md](../core/AGENTS.md). For native memory limits and their
platform-validation requirements, read [../AGENTS.md](../AGENTS.md).
