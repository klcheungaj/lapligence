# core

Shared, safe processing over Surelog/UHDM data. `compile` owns frontend
orchestration, `db` performs the canonical VPI capture, and model, elaboration,
tokens, macros, and lint consume or produce owned data.

`diagnostics::user_message` translates recognized Surelog syntax errors into
readable explanations shared by the LSP, simulator CLI, and elaboration checker.
It explains expected names/punctuation and suggests checking module placement
for instance-shaped errors (including standalone instantiation templates).
Advice is conditional: it uses the diagnostic text, without reading source files
or assuming that a template filename proves the cause. Original diagnostics,
severity, and positions remain intact; unknown messages pass through unchanged.

Only frontend capture code may depend on `ffi`; downstream consumers should
prefer `Db` domain types. Keep this directory free of `unsafe` and return
recoverable errors from library paths.

The `db` facade owns the validated arena and exposes read-only roots, nodes,
initializer metadata, and elaborated ranges. Integer-valued VPI properties are
converted during capture into domain enums that preserve unsupported values as
`Unknown(raw)`. The private `db/capture/` implementation is split by hierarchy,
declarations, expressions, primitives, and statements so VPI interpretation stays at the
capture boundary; `db/domain.rs` contains the owned representations and
`db/validate.rs` checks cross-node invariants before a database is returned.

Compile and parse-only setup failures return `compile::StartupError`, with a
typed kind and preserved diagnostic message. Checked compilation keeps frontend
diagnostics separate and chains startup failures through `Error::source`.
