# Datatype completion fixtures

- Purpose: frozen black-box SystemVerilog completion contracts.
- Coverage: eight execution-positive cases and one explicit unsupported-boundary
  case for string conversions, aggregate patterns, and container reductions.
- Execution: positive cases run in both optimization modes.
- Result: the eight positive cases require exact `PASS`; the unsupported case
  requires its explicit diagnostic and must not execute.
- Limits: this is a bounded completion suite, not an exhaustive conformance
  claim.

Run serially:

```sh
cargo test --test sim_data_types_completion -- --test-threads=1
```
