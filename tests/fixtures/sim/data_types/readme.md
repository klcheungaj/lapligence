# Datatype end-to-end fixtures

- Purpose: black-box Verilog/SystemVerilog datatype fixtures.
- Coverage: wide four-state values, arithmetic, casts, equality, and two-state
  conversion.
- Execution: each case runs with optimization enabled and disabled.
- Result: exact self-checking `PASS` output is required.
- Limits: normative datatype and width rules are summarized in
  [sim_data_semantics.md](../../../../docs/sim_data_semantics.md).

Run serially:

```sh
cargo nextest run --locked --test sim_data_types
```
