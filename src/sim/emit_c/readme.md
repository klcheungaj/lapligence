# C emitter

`emit_c.rs` is the public facade. The modules in this directory render
validated IR expressions, statements, functions, and whole models; `names.rs`
owns C identifier spelling and `error.rs` owns the typed public failure.

The emitter depends only on `sim::ir`. Public detached-node render entry points
validate their input against the supplied model before table indexing.
