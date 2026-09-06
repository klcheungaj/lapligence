# Datatype fixture contracts

`sim_data_types.rs` copies the fixtures to isolated temporary directories,
compiles/elaborates them, builds one owned database, and runs C11 models in
both optimization modes. `WIDTH` selects vector width; runtime assignments
keep these black-box checks independent of frontend constant folding. `.v`
fixtures use Verilog-2001 syntax and `.sv` fixtures use SystemVerilog.

Fixture inventory:

- `four_state_truth.v`: all 16 ordered 0/1/X/Z pairs for bitwise/logical
  operations, case/logical equality, X/Z conditional merging, reductions,
  mixed-state bytes, and high-bit comparisons; independent Rust tables check
  every printed bit across model-sized widths.
- `wide_arithmetic.v`: positional carry, borrow, multiply, limb-boundary
  shifts, signed/unsigned comparisons, high-bit reductions, concat, and select.
- `casts.sv`: signedness, extension/truncation, select signedness, predefined
  casts, wide packed-to-real and rounded real-to-packed values, and bitcasts.
- `two_state*.sv`: X/Z-to-zero conversion while preserving known bits across
  scalar, vector, atom assignments, and isolated cast paths.
- `wide_division.v`, `wide_modulo.v`, `wide_power.v`: positional arithmetic
  oracles across wide model-sized operands without a separate 64-bit limit.
- `casts_conformance.sv`: expected size and parameterized typedef casts.
- `equality_unknown.v`: known mismatch determines logical equality despite an
  additional X/Z bit, including across the first machine-word boundary.

The truth-table fixture prints one wide result per line, keeping output
formatting limits separate from value semantics. Runtime vector widths are
`uint32_t` and retain defensive model-capacity checks.

Datatype fixture authors derive expected behavior from the local
`docs/specification/` files without reading production implementation code.

The campaign accepts only complete exact `PASS` markers. Self-checking HDL
uses case inequality so unexpected X/Z values cannot become unknown `if`
conditions. The original 40-case campaign is active without ignored tests and
has regular and sanitizer coverage. Independent truth-table, positional
arithmetic, cast, and partial-limb oracles cover model-sized widths. Packed
all-bit/mixed-state structs and multidimensional packed-bit arrays are covered
at exercised widths; packed unions, unpacked aggregates, and unsupported
net/member contexts remain unclaimed.

The C backend capacity is strictly below `1 << 20` bits; the near-limit
campaign uses 1,048,575 bits. Selected widths exercise configured capacity but
do not establish unbounded support. Wide division/modulo/power, two-state X/Z
coercion, wide real conversion, and logical equality with a known mismatch are
active oracle contracts.

Normative anchors verified against `docs/specification/spec-reference-verilog.md`
and `spec-reference-sv.md`: IEEE 1364-2001 §§4.1.5–4.1.14 and 4.5; IEEE
1800-2009 §§6.11, 6.24.1, 11.4.5, 11.8, and 20.5. Icarus may independently
check portable fixtures but is not a dependency and does not redefine an HDL
oracle.
