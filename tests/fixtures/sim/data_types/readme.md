# Datatype end-to-end fixtures

black-box Verilog/SystemVerilog datatype fixtures.

Coverage: wide four-state values, arithmetic, casts, equality, and two-state
  conversion.
- Runs: each case runs in both optimizer modes.
- Oracle: exact self-checking `PASS` output.
- Limits: normative datatype and width rules are summarized in
  [sim_data_semantics.md](../../../../docs/sim_data_semantics.md).

Run serially:

```sh
cargo nextest run --locked --test sim_data_types
```
