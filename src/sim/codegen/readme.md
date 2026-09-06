# Simulator lowering

`codegen.rs` is the public facade. `lowering.rs` coordinates the pipeline and
shared lowering state; its `collection`, `statements`, and `expressions` child
modules own database collection/wiring and domain-specific IR construction.
`timescale.rs` owns parsing and representation of Verilog time-unit directives,
bounded constant delays and local-precision rounding of time literals;
`error.rs` defines the typed public failure. Time-value lowering uses exact
literal spans captured in the owned database, not Surelog's transformed integer
payload. See [AGENTS.md](AGENTS.md) for supported contexts and rejection limits.
Collection also assigns independent resolved-net contribution slots to
standalone wire/tri and wired-net continuous-driver sites. The emitter rebuilds
selected contributions from Z before publishing them, while initialization
marks delayed whole-net contributions X until their first scheduled update.

This layer reads only the owned `core::db` model after its initial build. It
must validate the completed IR before and after optimization and must preserve
unknown DB domain-enum values in diagnostics rather than guessing a meaning.

Packed widths are carried through the IR without a fixed 1024-bit/64-bit
semantic ceiling. The C backend emits a model-sized `LLG_MODEL_MAX_WIDTH`
(strictly below `1 << 20`) and performs the final capacity validation; runtime
constructors retain defensive checks. Wide division/modulo/power and
packed/real conversions therefore follow the generated model capacity. See
[`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for the
standard data, sizing, signedness, and X/Z reference.
