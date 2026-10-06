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
- `true_net_alias.sv` checks that both names of a true net alias receive the
  resolved driver value in the VCD catalog.
- `wide.sv` dumps 4095-, 4096-, 4097- and 65536-bit vectors (zeros, ones, and
  `z`/`x` end bits) and checks every value-change record at full width.
- `wide_limit.sv` is built with `--define LIMIT=<bytes>` and checks that a
  `$dumplimit` boundary keeps or rejects a 65536-bit record as a whole.
- `fst.sv` is the FST counterpart for the generated-model reader probe.
- `cli_wave.sv` has no waveform tasks; `llg --wave` dumps it as VCD or FST,
  with and without `--wave-depth`, and the same option overrides `depth.sv`'s
  own `$dumpfile`/`$dumpvars`.
- `output_redirect.sv` writes a waveform, a `$fopen` file and `$writememh`
  output and reads `$readmemh` input, so one built model can be rerun with
  `LLG_SIM_OUT_DIR`, `LLG_SIM_WAVE_FILE` and `LLG_SIM_LOG_FILE`.

The owning Rust suite runs every fixture in optimized and `--no-opt` modes and
uses independent VCD/FST metadata and value oracles.
