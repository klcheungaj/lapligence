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
