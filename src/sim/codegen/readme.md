# Simulator lowering

`codegen.rs` is the public facade. `lowering.rs` coordinates the pipeline and
shared lowering state; its `collection`, `statements`, and `expressions` child
modules own database collection/wiring and domain-specific IR construction.
`timescale.rs` owns parsing and representation of Verilog time-unit directives,
and `error.rs` defines the typed public failure.

This layer reads only the owned `core::db` model after its initial build. It
must validate the completed IR before and after optimization and must preserve
unknown DB domain-enum values in diagnostics rather than guessing a meaning.
