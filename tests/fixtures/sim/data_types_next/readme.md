# Next-phase datatype fixtures

- Purpose: bounded, non-exhaustive next-phase datatype inventory.
- Coverage: unions/structs, streaming and `inside`, static subprogram storage,
  packed and descriptor-backed dynamic containers (including nested copy,
  resize, delete, and negative-size diagnostics), descriptor-backed real and
  string queues/associative arrays, recursive child-container queue and
  associative storage, executed
  `$typename`/`$bits`/array-query functions, strings, and chandles.
- Execution: each self-checking fixture runs in optimized and unoptimized models.
- Result: exact fixture `PASS` markers are required, except explicit rejection
  cases, which must produce their expected diagnostics.
- Limits: this inventory does not imply exhaustive SystemVerilog compatibility.

Run serially:

```sh
cargo nextest run --locked --test sim_data_types_next
```
