# Legacy random distributions

`basic.sv` exercises Verilog-2001 §17.9.1–17.9.3 `$random` and the seven
`$dist_*` functions with writable integer seeds. `tests/sim_random.rs` keeps
the independent fixed-seed oracle and runs the fixture with and without
optimization; `tests/runtime_random.rs` covers the embedded C module's
signed-range and invalid-parameter boundaries at both optimization levels.

The algorithms and seed recurrence follow the Annex N reference listing. The
SystemVerilog `$urandom` family and per-process/object random state are outside
this fixture's scope.
