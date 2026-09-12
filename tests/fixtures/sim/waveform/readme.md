# Waveform fixtures

These file-backed designs cover the ordinary Verilog waveform tasks in IEEE
1364-2001 §18.1.1–18.1.6: `$dumpfile`, `$dumpvars`, `$dumpon`, `$dumpoff`,
`$dumpall`, `$dumplimit`, and `$dumpflush`.

- `controls.sv` checks dump activation, snapshots, flush, byte limits, X/Z,
  real values, and final-block writes.
- `depth.sv` checks finite hierarchy depth and descending/nonzero array bounds.
- `named.sv` checks named scalar and whole-array selection with declared elements.
- `unlimited.sv` checks unlimited hierarchy, escaped/colliding identifiers,
  array names, and packed/real storage declarations.
- `aliases.sv` checks that reference-port aliases retain every declared HDL
  identity while sharing one waveform value identifier.
- `fst.sv` is the FST counterpart for the generated-model reader probe.

The owning Rust suite runs every fixture in optimized and `--no-opt` modes and
uses independent VCD/FST metadata and value oracles.
