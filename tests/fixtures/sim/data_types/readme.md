# Datatype end-to-end fixtures

black-box Verilog/SystemVerilog datatype fixtures.

Coverage: wide four-state values, arithmetic, casts, equality, and two-state
  conversion.
- Runs: each case runs in both optimizer modes.
- Oracle: exact self-checking `PASS` output.
- Limits: normative datatype and width rules are summarized in
  [sim_data_semantics.md](../../../../docs/sim_data_semantics.md).

## Contracts

- `sim_data_types.rs` copies fixtures to isolated temporary directories, builds
  one owned database and runs the C11 model in both optimization modes.
- `WIDTH` selects the vector width; runtime assignments keep the checks
  independent of frontend constant folding. `.v` fixtures use Verilog-2001
  syntax, `.sv` fixtures SystemVerilog.
- Only complete exact `PASS` markers pass. Self-checks use case inequality so
  unexpected X/Z cannot become unknown `if` conditions. Keep the campaign active
  in the regular and sanitizer gates.
- The truth-table fixture prints one wide result per line, keeping output
  formatting limits separate from value semantics. Runtime vector widths are
  `uint32_t` with defensive supported-width checks.
- The C backend's supported width is strictly below `1 << 20` bits; the
  near-limit case uses 1,048,575 bits. Selected widths do not establish
  unbounded support.
- Covered at exercised widths: packed all-bit/mixed-state structs and
  multidimensional packed-bit arrays. Unclaimed: packed unions, unpacked
  aggregates and unsupported net/member contexts.
- Authors derive expected behavior from the local `docs/specification/` files,
  never from production code. Anchors: IEEE 1364-2001 §§4.1.5–4.1.14, 4.5;
  IEEE 1800-2009 §§6.11, 6.24.1, 11.4.5, 11.8, 20.5. Icarus may check portable
  fixtures but is not a dependency and does not redefine an oracle.

| Fixture | Checks |
| --- | --- |
| `four_state_truth.v` | All 16 ordered 0/1/X/Z pairs for bitwise/logical operations, case/logical equality, X/Z conditional merging, reductions, mixed-state bytes and high-bit comparisons; independent Rust tables check every printed bit |
| `wide_arithmetic.v` | Positional carry, borrow, multiply, limb-boundary shifts, signed/unsigned comparisons, high-bit reductions, concat and select |
| `casts.sv` | Signedness, extension/truncation, select signedness, predefined casts, wide packed-to-real, rounded real-to-packed and bitcasts |
| `two_state*.sv` | X/Z-to-zero conversion keeping known bits across scalar, vector, atom assignments and isolated cast paths |
| `wide_division.v`, `wide_modulo.v`, `wide_power.v` | Positional oracles over wide exact-width operands without a 64-bit limit |
| `casts_conformance.sv` | Expected size and parameterized typedef casts |
| `equality_unknown.v` | A known mismatch decides logical equality despite an extra X/Z bit, including across the first machine-word boundary |

Run serially:

```sh
cargo nextest run -E 'test(/^sim_data_types::/)' --locked
```
