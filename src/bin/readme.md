# bin

Executable entry points for the simulator, language server, elaboration checker,
and low-level demonstrations. Binaries should contain argument/transport wiring
and presentation only; reusable processing belongs in the library.

The simulator and elaboration checker format Surelog parser errors through
`core::diagnostics::user_message` for readable explanations and repair hints.

The `llg_ls` binary remains feature-gated because its asynchronous LSP
dependencies must not enter minimal library builds.

Each executable's `main` collects arguments or selects its transport, calls a
runner, and maps the returned status to `ExitCode`. Normal completion and error
returns therefore drop Surelog sessions and process-memory guards before exit.
The LSP's mandatory `exit` notification retains its transport-owned emergency
cleanup/termination path. `llg` parses `DriverOptions` separately from the
compile/lint/build/run orchestration.
