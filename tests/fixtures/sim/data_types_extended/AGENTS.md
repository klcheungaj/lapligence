# Extended datatype fixture contracts

The harness compiles each fixture once, lowers, builds, and executes it in
optimized and unoptimized models. Fixtures use `!==`; Rust accepts only the
exact fixture-specific `PASS` line.

Inventory:

- `wide_signed_div_mod.sv`: positional wide division, sign combinations,
  minimum-negative overflow, remainder sign, and division by zero.
- `wide_power.sv`: positional wide results, odd/even negative bases, zero
  exponent, truncation, and unknown exponent.
- `equality_known_mismatch.sv`: known mismatch with X/Z around limb boundaries.
- `two_state_conversion.sv`: vector/scalar assignment and four-state-to-two-
  state static casts.
- `mixed_width_signed.sv`: extension, signed arithmetic, part-select
  signedness, and mixed-signedness comparisons.
- `packed_aggregates.sv`: packed-structure and multidimensional packed-array
  layout, member updates, and signed extension.
- `shifts_concat_conditional.sv`: shift boundaries, arithmetic fill,
  concatenation, X-controlled merging, and reductions.
- `resolved_nets.v`: `wand`, `wor`, `tri`, `tri0`, and `tri1` resolution.
- `scalable_arithmetic.sv`: bounded positional arithmetic at 2,048 and 65,536
  bits.
- `max_width_probe.sv`: acceptance at 1,048,575 bits; 1,048,576-bit variants
  define the exclusive rejection boundary.

Portable functional fixtures were independently run with Icarus Verilog 12.0;
Icarus is a reference check, not a dependency. Normative anchors verified in
the local specification indexes are IEEE 1364-2001 §§3.7, 4.1, and 7.13 and
IEEE 1800-2009 §§6.11, 6.24.1, 7.2.1, 7.4.1, and 11.4–11.8.
