# Next-phase datatype fixtures

- Purpose: bounded, non-exhaustive next-phase datatype inventory.
- Coverage: unions/structs, streaming and `inside`, static subprogram storage,
  packed and descriptor-backed dynamic containers (including nested copy,
  resize, delete, and negative-size diagnostics), descriptor-backed real and
  string queues/associative arrays, recursive child-container queue and
  associative storage, executed
  `$typename`/`$bits`/array-query functions, strings, and chandles.
- `syn_015_fixed_stream_contexts.sv` covers fixed bit-stream casts and
  streaming through `const ref`/`ref` formals, nested fixed aggregates,
  selected rows, one-time selected destinations, and overlapping source and
  destination snapshots.
- Its source header follows IEEE 1800-2009 §§6.24.1, 6.24.3, 7.2, 11.4.14,
  and 13.5.2; the section index is
  [`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).
- Execution: each self-checking fixture runs in optimized and unoptimized models.
- Result: exact fixture `PASS` markers are required, except explicit rejection
  cases, which must produce their expected diagnostics.
- Limits: this inventory does not imply exhaustive SystemVerilog compatibility.

Run serially:

```sh
cargo nextest run --locked --test sim_data_types_next
```
