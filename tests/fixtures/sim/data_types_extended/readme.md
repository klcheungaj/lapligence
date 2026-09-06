# Extended datatype conformance fixtures

`tests/sim_data_types_extended.rs` compiles each checked-in fixture once, then
lowers, builds, and executes it with optimization disabled and enabled. The
fixtures are self-checking and use case inequality (`!==`) so unexpected X/Z
values cannot hide failures. Rust accepts only the exact fixture-specific PASS
line.

The cases are intentionally orthogonal:

- `wide_signed_div_mod.sv` covers positional wide division, every operand-sign
  combination, minimum-negative overflow, remainder sign, and division by zero.
- `wide_power.sv` covers positional wide results, odd/even negative bases, zero
  exponent, result truncation, and an unknown exponent.
- `equality_known_mismatch.sv` covers a known mismatch combined with X/Z above,
  below, and at the mismatch's surrounding limbs.
- `two_state_conversion.sv` isolates vector/scalar assignment and static-cast
  conversion from four-state to two-state values.
- `mixed_width_signed.sv` covers extension, mixed and all-signed arithmetic,
  part-select signedness, and mixed-signedness comparisons.
- `packed_aggregates.sv` covers packed structure and multidimensional packed
  array layout, member updates, and signed packed-structure extension.
- `shifts_concat_conditional.sv` covers boundary and high-bit shift counts,
  arithmetic fill, concatenation layout, X-controlled merging, and reductions.
- `resolved_nets.v` covers `wand`, `wor`, `tri`, `tri0`, and `tri1` resolution.
- `scalable_arithmetic.sv` provides bounded-output positional add/subtract/
  multiply checks at 2,048 and 65,536 bits.
- `max_width_probe.sv` admits the largest legal width (1,048,575 bits), while
  that fixture at 1,048,576 bits and `max_width_intermediate.sv`'s exactly
  1,048,576-bit concatenation specify the exclusive rejection boundary.

The portable functional fixtures were independently run with Icarus Verilog
12.0 before llg validation. Icarus is a reference check, not a test dependency;
the normative anchors are IEEE 1364-2001 sections 3.7, 4.1, and 7.13 and IEEE
1800-2009 sections 6.11, 6.24.1, 7.2.1, 7.4.1, and 11.4-11.8.

Run the suite serially because Surelog uses process-global state:

```sh
cargo test --test sim_data_types_extended -- --test-threads=1
```
