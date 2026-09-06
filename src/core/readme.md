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
Constant nodes retain available exact source spans so consumers can identify
time literals whose numeric payload Surelog has already transformed. Continuous
assignments retain drive strengths, and unpacked net arrays retain their net
subtype, allowing unsupported resolution contexts to be rejected explicitly.
Packed declarations retain ordered dimensions, top-level struct/union member
offsets, and their recursively derived two-state domain. Cast nodes retain a
numeric size token and whether their type-vs-size classification had reliable
source or UHDM decompile provenance; consumers reject ambiguous source-less
integer casts rather than accepting a guessed 32-bit result.
The new constant-source capture is opt-in through `Db::build_with_source_files`;
ordinary `Db::build` adds no constant-source reads. Simulator callers use
`CompileOut::frontend_source_files` to admit physical parsed files, never logical
VPI filename remappings. This inventory does not guarantee included headers.
The cache accepts only non-symlink regular files and bounds files to 8 MiB, total retained
keys/text/line offsets/spans to 64 MiB, paths to 1024, and paths/spans to 4096
bytes. Unavailable provenance is retained so simulation can reject suspect
time-literal payloads; older delay/event source capture is unchanged.

Compile and parse-only setup failures return `compile::StartupError`, with a
typed kind and preserved diagnostic message. Checked compilation keeps frontend
diagnostics separate and chains startup failures through `Error::source`.
