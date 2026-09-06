# Datatype completion fixtures

This directory contains independently authored black-box SystemVerilog
fixtures. Their behavior was derived from the local IEEE 1800-2009 LRM and
frozen before `llg` execution, without consulting simulator implementation
source. The suite runs each execution-positive case with optimization disabled
and enabled.

Run the suite serially because Surelog uses process-global state:

```sh
cargo test --test sim_data_types_completion -- --test-threads=1
```

## Inventory

| Fixture | Contract | Local LRM basis |
| --- | --- | --- |
| `string_real_conversion.sv` | Positive: numeric-prefix, whitespace/sign/exponent, invalid-string `atoreal`, and nonempty exact-value `realtoa` round trips | §6.16.10, §6.16.15 |
| `string_wide_real_contexts.sv` | Positive: 128-/512-bit real/integral assignment and argument conversions above 64 bits | §6.12.2, §6.16.10, §6.16.15 |
| `packed_struct_assignment_patterns.sv` | Positive: 128-/512-bit positional, named, default, simple-type, and named-type declaration patterns, including mixed two-/four-state members and generate scope | §7.2.1, §10.9.2 |
| `packed_union_assignment_patterns.sv` | Positive: single-member packed-union declaration patterns and shared 128-/512-bit representation | §7.3.1, §10.9 |
| `unpacked_aggregate_assignment_patterns.sv` | Positive: unpacked struct and union declaration patterns over fixed packed integral members | §7.2, §7.3, §10.9.2 |
| `dynamic_array_reductions.sv` | Positive: element-typed 128-bit reductions, modular arithmetic, X/Z propagation, and empty identities | §7.12.3 |
| `queue_reductions.sv` | Positive: signed 512-bit reductions and empty identities | §7.12.3 |
| `associative_array_reductions.sv` | Positive: order-independent 128-bit reductions retaining high bits and empty identities | §7.12.3 |
| `reduction_with_unsupported.sv` | Explicit unsupported boundary: a legal width-changing reduction `with` clause must fail codegen with a clause-specific diagnostic, never execute after silently dropping the clause | §7.12.3 |

The inventory therefore contains eight positive execution contracts and one
explicit unsupported-boundary contract. Tagged unions, classes, and virtual
interfaces are intentionally absent.

## Validation status

Root-run normal and ASan/UBSan/leak validation reports all nine cases passing
in both optimization modes. This is a bounded completion suite, not an
exhaustive conformance claim; nominal type keys, nested recursive defaults,
nested unpacked/object members, and aggregate ports/nets/subprogram storage
remain outside its support contract.
