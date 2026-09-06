# Next-phase datatype fixtures

- Purpose: bounded, non-exhaustive next-phase datatype inventory.
- Coverage: unions/structs, streaming and `inside`, static subprogram storage,
  containers, strings, and chandles.
- Execution: each self-checking fixture runs in optimized and unoptimized models.
- Result: exact fixture `PASS` markers are required, except explicit rejection
  cases, which must produce their expected diagnostics.
- Limits: this inventory does not imply exhaustive SystemVerilog compatibility.

Run serially:

```sh
cargo test --test sim_data_types_next -- --test-threads=1
```
