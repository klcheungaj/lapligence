# Datatype and net conformance fixtures

These checked-in SystemVerilog designs run through `llg` with optimization on
and off. Rust retains independent arithmetic, bit-string and truth-table
oracles in [the conformance suite](../../../sim_type_conformance.rs).

- `mixed-types-*`: 23 ordered operand types and 18 operators.
- `data-net-*`: all four states across variable/net operations and casts.
- `conversions-*`: source widths, signedness and X/Z-to-zero conversion.
- `net-tables-*`: three-driver resolution, defaults and mixed-bit patterns.
- Other positive files: storage, subroutine/port boundaries, real precision,
  strings, typed nets and widths up to 1,048,575 bits.
- `invalid-net-*`, `trireg-*`, `uwire-overlapping` and `uwire-multiple`:
  intentional frontend or simulator rejection cases.

From the repository root:

```sh
cargo test --test sim_type_conformance -- --test-threads=1
cargo run --bin llg -- --top tb tests/fixtures/sim/type_conformance/uwire.sv
cargo run --bin llg -- --no-opt --top tb tests/fixtures/sim/type_conformance/uwire.sv
```

CMake and a C compiler are required. The [coverage map](../../../readme.md)
describes normative anchors and remaining limits.

## Integral matrix operand map

`mixed-types-N.sv` uses row N as the left operand against every row below.
The same ordering identifies `t_N` / `v_N` in conversion and data/net files.

| N | Declared type | Width | State domain |
|---:|---|---:|---|
| 0 | `reg` | 1 | Four-state |
| 1 | `logic` | 1 | Four-state |
| 2 | `bit` | 1 | Two-state |
| 3 | `reg signed [7:0]` | 8 | Four-state |
| 4 | `logic [15:0]` | 16 | Four-state |
| 5 | `bit signed [7:0]` | 8 | Two-state |
| 6 | `byte` | 8 | Two-state |
| 7 | `byte unsigned` | 8 | Two-state |
| 8 | `shortint` | 16 | Two-state |
| 9 | `shortint unsigned` | 16 | Two-state |
| 10 | `int` | 32 | Two-state |
| 11 | `int unsigned` | 32 | Two-state |
| 12 | `longint` | 64 | Two-state |
| 13 | `longint unsigned` | 64 | Two-state |
| 14 | `integer` | 32 | Four-state |
| 15 | `integer unsigned` | 32 | Four-state |
| 16 | `time` | 64 | Four-state |
| 17 | `time signed` | 64 | Four-state |
| 18 | `logic signed [64:0]` | 65 | Four-state |
| 19 | Enum over `logic signed [7:0]` | 8 | Four-state |
| 20 | Enum over `bit [15:0]` | 16 | Two-state |
| 21 | Signed packed struct of two `logic [3:0]` fields | 8 | Four-state |
| 22 | Packed union over `bit [15:0]` / `bit [1:0][7:0]` | 16 | Two-state |
