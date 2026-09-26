# Datatype completion fixtures

frozen black-box SystemVerilog completion contracts.

Coverage: seventeen execution-positive cases for string conversions, aggregate
  patterns, resizable assignment patterns, container reductions, and array
  manipulation methods, plus eight explicit unsupported-pattern cases.
- Runs: positive cases run in both optimization modes.
- Oracle: exact fixture `PASS` line.
- Limits: this is a bounded completion suite, not an exhaustive conformance
  claim.

Run serially:

```sh
cargo nextest run --locked --test sim_data_types_completion
```
