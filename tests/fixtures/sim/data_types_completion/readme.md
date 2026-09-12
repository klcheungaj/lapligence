# Datatype completion fixtures

- Purpose: frozen black-box SystemVerilog completion contracts.
- Coverage: thirteen execution-positive cases for string conversions, aggregate
  patterns, container reductions, and array manipulation methods.
- Execution: positive cases run in both optimization modes.
- Result: every fixture requires its exact `PASS` line in both optimizer modes.
- Limits: this is a bounded completion suite, not an exhaustive conformance
  claim.

Run serially:

```sh
cargo test --test sim_data_types_completion -- --test-threads=1
```
