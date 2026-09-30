# Module-procedural fixed-array expressions (R04)

`tests/sim_array_conditional_assignments.rs` runs each source through the public
CLI in SystemVerilog-2009 mode, both optimized and unoptimized. Expected output
is maintained in the Rust suite. Testbench delays, diagnostics and initialization
are observation scaffolding, not claims that each complete module is synthesizable.

| Fixture | Contract |
| --- | --- |
| `comb.sv` | Same-typedef direct `always_comb` RHS; known/X/Z selectors, element defaults and both-arm sensitivity. |
| `effects.sv` | One selector evaluation; one selected arm or both ambiguous arms; nested short circuiting. |
| `overlap.sv` | Snapshot before overlapping slice and whole-array pattern writes. |
| `nba.sv` | Deferred publication, issue-time values/indices, source-ordered cell updates and X merging. |
| `clocked.sv` | Direct `always_ff` array conditional with known and unknown selectors. |
| `views.sv` | Opposite/negative declaration bounds, selected-row indices evaluated once, unselected and invalid row reads. |
| `shapes.sv` | Immediate multidimensional rows, two-state defaults, packed records and 129-bit elements. |
| `expressions.sv` | Cast/pattern alternatives, conditional concatenation operands and existing per-cell widening. |
| `nested_defaults.sv` | Shared synthesized row operands, explicit outer keys and untyped one-bit defaults, with ascending/descending bounds. |
| `typed_defaults.sv` | Explicitly typed row defaults preserve runtime `8'hdd`/`8'hee` byte values rather than applying the nested pattern to packed bits; direct packed patterns also check element sizing and signed results. |
| `deep_defaults.sv` | Repeated expression identities in three unpacked dimensions with 65-bit all-one, X and Z values. |
| `repeated_values.sv` | Existing per-operand increment order, wide conditional defaults and repeated array conditionals after coroutine resumes. |
| `wrong_shape.sv` | Different unpacked ranks remain incompatible despite equal flattened bit counts. |
| `wrong_elements.sv` | Equal array extents do not permit incompatible element types. |

See [test methodology](../../../readme.md) for commands and validation layers.
Array merges use the R01 IR/emitter/runtime path.
