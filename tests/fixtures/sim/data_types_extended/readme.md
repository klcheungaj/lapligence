# Extended datatype conformance fixtures

Black-box extended datatype fixtures.

- Coverage: wide arithmetic, signed operations, packed layouts, net resolution
  and the supported-width boundary.
- Runs: the harness compiles each fixture once, then lowers, builds and runs
  optimized and unoptimized models.
- Oracle: fixtures self-check with `!==`; Rust accepts only the exact
  fixture-specific `PASS` line.
- Icarus is an optional reference check, not a dependency. Anchors: IEEE
  1364-2001 §§3.7, 4.1, 7.13; IEEE 1800-2009 §§6.11, 6.24.1, 7.2.1, 7.4.1,
  11.4–11.8.

| Fixture | Checks |
| --- | --- |
| `wide_signed_div_mod.sv` | Positional wide division, sign combinations, minimum-negative overflow, remainder sign, division by zero |
| `wide_power.sv` | Positional wide results, odd/even negative bases, zero exponent, truncation, unknown exponent |
| `equality_known_mismatch.sv` | Known mismatch with X/Z around limb boundaries |
| `two_state_conversion.sv` | Vector/scalar assignment and four-state-to-two-state static casts |
| `mixed_width_signed.sv` | Extension, signed arithmetic, part-select signedness, mixed-signedness comparisons |
| `packed_aggregates.sv` | Packed-struct and multidimensional packed-array layout, member updates, signed extension |
| `shifts_concat_conditional.sv` | Shift boundaries, arithmetic fill, concatenation, X-controlled merging, reductions |
| `resolved_nets.v` | `wand`, `wor`, `tri`, `tri0`, `tri1` resolution |
| `scalable_arithmetic.sv` | Bounded positional arithmetic at 2,048 and 65,536 bits |
| `max_width_probe.sv` | Acceptance at 1,048,575 bits; 1,048,576-bit variants define the exclusive rejection boundary |

Run serially:

```sh
cargo nextest run -E 'test(/^sim_data_types_extended::/)' --locked
```
