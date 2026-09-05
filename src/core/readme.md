# core

Shared, safe processing over Surelog/UHDM data. `compile` owns frontend
orchestration, `db` performs the canonical VPI capture, and model, elaboration,
tokens, macros, and lint consume or produce owned data.

Only frontend capture code may depend on `ffi`; downstream consumers should
prefer `Db` domain types. Keep this directory free of `unsafe` and return
recoverable errors from library paths.
