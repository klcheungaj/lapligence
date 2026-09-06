# Datatype end-to-end fixtures

`tests/sim_data_types.rs` copies these files to isolated temporary directories,
compiles/elaborates them with Surelog, builds one owned database, and runs C11
models with optimization disabled and enabled. A top-level `WIDTH` override
selects the vector width. Operands are assigned at runtime before delayed checks;
the tests do not merely validate frontend constant folding.

| Fixture | Oracle and coverage |
| --- | --- |
| `four_state_truth.v` | All 16 ordered 0/1/X/Z pairs for bitwise/logical operations, case/logical equality, and X/Z-controlled conditional merging; reductions, mixed-state bytes, high-bit comparisons; independent Rust tables check every printed bit across model-sized widths |
| `wide_arithmetic.v` | Independent positional expected values for carry, borrow, multiply, limb-boundary shifts, signed/unsigned comparisons, high-bit reductions, concat/select across model-sized widths |
| `casts.sv` | Signedness, extension/truncation, select signedness, supported predefined casts, wide packed-to-real values, rounded real-to-packed values and IEEE bitcasts across model-sized widths |
| `two_state*.sv` | X/Z-to-zero conversion while preserving known low/high bits; scalar/vector/atom assignments and separately isolated cast paths |
| `wide_division.v`, `wide_modulo.v`, `wide_power.v` | Independent positional arithmetic oracles across wide model-sized operands; no separate 64-bit restriction in the expected behavior |
| `casts_conformance.sv` | Expected-correct size and parameterized typedef casts |
| `equality_unknown.v` | A known differing bit determines logical equality despite another X/Z bit, with unknowns on both sides of the first machine-word boundary |

The `.v` fixtures use Verilog-2001 syntax; `.sv` fixtures use SystemVerilog.
Assertions in self-checking fixtures use case inequality so an unexpected X/Z
cannot turn a failure into an unknown `if` condition. Only a complete exact
`PASS` marker is accepted. The truth-table fixture prints one wide result per
line, keeping output formatting limits separate from value semantics.

The original 40-case datatype campaign is active without ignored tests and has
complete regular and sanitizer coverage. The independent edge campaign remains
tracked separately; tested all-bit/mixed-state packed structs and
multidimensional packed-bit arrays are covered at the exercised widths, while
packed unions, unpacked aggregates, and unsupported net/member contexts remain
explicit and unclaimed.

Run the full campaign:

```sh
cargo test --test sim_data_types -- --test-threads=1
```

The C backend capacity is model-sized and strictly below `1 << 20` bits
(the near-limit campaign uses 1,048,575 bits); runtime vector widths are
`uint32_t` and retain defensive model-capacity checks. Wide division/modulo/
power, two-state X/Z-to-zero coercion, wide real conversions, and logical
equality known-mismatch behavior are part of the active oracles. The selected
widths exercise the configured capacity but do not prove unbounded support.

Specification anchors: IEEE 1364-2001 §§4.1.5–4.1.14 and §4.5;
IEEE 1800-2009 §§6.11, 6.24.1, 11.4.5, 11.8, and 20.5. Local reference
indexes are under `docs/specification/`. Icarus Verilog can independently check
the portable fixtures, but is not a dependency of this suite; reference-tool
limitations must not redefine the HDL oracle.
