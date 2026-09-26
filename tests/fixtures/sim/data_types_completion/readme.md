# Datatype completion fixtures

frozen black-box SystemVerilog completion contracts.

Coverage: seventeen execution-positive cases for string conversions, aggregate
  patterns, resizable assignment patterns, container reductions, and array
  manipulation methods, plus eight explicit unsupported-pattern cases.
- Runs: positive cases run in both optimization modes.
- Oracle: exact fixture `PASS` line.
- Limits: this is a bounded completion suite, not an exhaustive conformance
  claim.
- SYN-028's `syn028_unpacked_record_maps.sv` checks signed mapped keys,
  repeated-key whole-record permutation, descending declaration bounds and a
  zero-time function call. Equal-key record order is deliberately unconstrained
  by IEEE 1800-2009 §7.12.2.
- `syn028_edition_boundary.sv` checks sort/rsort under 2009 and rejects the
  method under 2001 with an otherwise 2001-valid array declaration.

Run serially:

```sh
cargo nextest run --locked --test sim_data_types_completion
```
