# Loop regression fixtures

- `tests/sim_loops.rs` runs these checked-in sources through both public CLI
  optimizer modes. Rust owns exact stdout oracles and rejection substrings.
- `foreach_extended.sv` retains the pre-existing omissions, resizable-container,
  scope and real-local coverage.
- `foreach_mixed_order.sv`: IEEE 1800-2009 12.7.3 mixed unpacked/packed
  coordinates, four-dimensional declaration-order traversal, and bit reads/writes.
- `foreach_mixed_omissions.sv`: leading, middle and trailing omitted indices,
  shorter prefixes, all-omitted and empty lists. Includes the standard's
  `bit [3:0][2:1] B [5:1][4]` shape.
- `foreach_mixed_control.sv`: source-loop break/continue, nested for loops,
  shadowed iterator names and signed 32-bit endpoints without wraparound.
- `foreach_mixed_calls.sv`: typedef shapes on function inputs, automatic local
  arrays, fixed-array returns and procedural block locals.
- `foreach_mixed_types.sv`: typedef-based packed byte arrays, implicit integer
  and packed-record vectors, normalized enum ranges, scalar versus singleton
  vector elements, and a pure-packed control.
- `foreach_mixed_ports.sv`: formal bounds differing from actual bounds and
  combinational wakeups after array-element changes.
- `foreach_mixed_too_many.sv`, `foreach_mixed_scalar_extra.sv` and
  `foreach_mixed_readonly.sv`: excess dimensions, a nonexistent scalar dimension
  and assignment to an implicit read-only iterator remain frontend errors.

Run the R02 cases on a configured host:

```sh
cargo test --locked --lib foreach_mixed
cargo test --locked --test slang_semantics foreach_mixed -- --test-threads=1
cargo test --locked --test sim_loops foreach_mixed -- --test-threads=1
```

Use the full `sim_loops` suite to check the existing resizable-container and
omission cases as well. Passing fixture-integrity checks only establishes that
inputs are present and registered; it is not language-execution validation.
