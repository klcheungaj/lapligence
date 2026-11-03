# RTL-006 wide arithmetic and single-evaluation mutations

IEEE 1364-2001 §§4.1.5–4.1.6, 4.1.12, 4.4–4.5 and IEEE 1800-2009 §§11.4.1–11.4.3,
11.4.10, 11.6–11.8 (Table 11-4), 10.3, 13.4, 23.3 and 6.5 supply the oracles.
Every positive source runs through the public CLI in both optimizer modes and
on the legacy, compact/portable and compact/GMP value backends.

Arithmetic expectations are computed in the test by an independent four-state
limb oracle (`tests/sim_feature_completion/rtl_006/oracle.rs`): Knuth division,
truncated schoolbook products, Table 11-4 powers and LRM `%h` digit rules. It
shares no code with the simulator. Thirteen operand formulas are rebuilt at
each width: 0, 1, 2, 3, all ones, the signed minimum, the signed maximum,
minimum + 1, a fixed 256-bit pattern (replicated past 256 bits), its
complement, -3, all X, and the pattern with Z in bit 0.

- `arith_matrix.sv` runs every operand pair at widths 1, 31, 32, 63, 64, 65,
  127, 128 and 129. It covers signed and unsigned `+ - * / % **`, unary minus,
  mixed-sign division, both exponent signs, and `<< >> <<< >>>` with oversized,
  signed and X/Z counts. Shifting a partly unknown operand prints in binary.
- `arith_matrix_2001.v` repeats the matrix in Verilog-2001 syntax under
  `--edition v2001`.
- `arith_wide.sv` uses 8,128 and 8,129 bits (127 versus 128 limbs). That is
  where the compact backend's GMP multiply switches to `mpn_mul_n`. Each result
  prints its low and high 64 bits and its count of one bits. Odd bases other
  than 1 and -1 take small or negative exponents, plus one full-width
  `3 ** (2**(W-1) - 1)`.
- `arith_mixed.sv` assigns mixed-width, mixed-sign operations into narrower and
  wider targets. It covers context extension that is signed only when both
  operands are signed, base-only sizing for `**` and shifts, and
  self-determined exponents and counts.
- `arith_constants_w{1,32,64,65}.sv` use literal operands, one width per
  file. The default optimizer folds what it can, while `--no-opt` evaluates at
  run time; both must match the oracle. One file held all four widths until its
  single 33,000-line C process function made GCC `-O3` take 29 s and CI time
  out; split files keep each generated function small.
- `mutation_contexts.sv` checks one evaluation of call-valued indices,
  receivers (unpacked record elements, interface and hierarchical arrays) and
  right-hand sides. It also checks prefix/postfix and assignment-expression
  results, signed and mixed-width compound operators, X and overflow, and
  mutations inside automatic and static functions. Continuous-assignment and
  port expressions call functions that mutate locals and module counters;
  each operand change evaluates them once. Its `.out` file is hand-derived.
- `computed_handle_properties.sv` (FIN-001 G3) reads, writes and mutates
  properties of a computed class handle: a function result (`get().v++`,
  `get().v += 5`, `++get().v`, `get().v--`, `get().r += 1.5`, a derived
  class's own and inherited properties through `getd()`), its string and
  handle properties (`get().s`, `get().nxt.v`), a packed select read
  (`get().b[3:0]` of 8'h5a is 10) and `pop_front()`/`pop_back()` results
  (`q.pop_front().v`, SV 7.10.2). Every operation calls the function once,
  so the counter `n` advances by one per property operation: 7 after the
  seven mutations, 9 after `x = get().v` and `y = get().v++` (`y` is the old
  value 6), 14 after the five calls of line `c`. The pops return `k` (40),
  then `h` (`"hi!"`), leaving one element; `q[1].v *= 2` doubles `h.v` (42)
  to 84. SV 11.4.1 L14478-14480: "An assignment operator is semantically
  equivalent to a blocking assignment, with the exception that any
  left-hand index expression is only evaluated once."
- Negatives: `++` in a continuous assignment, an operator assignment as a port
  expression, incrementing a call result, and `+=` under the 2001 edition.
