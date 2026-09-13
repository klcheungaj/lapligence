# Process semantic regressions

These file-backed probes cover the process-family contracts in IEEE
1800-2009 §§9.2.2.2–9.2.2.4 and the Verilog-2001 `@*` sensitivity rule in
§9.7.5. `always_comb` executes at time zero, follows reads in called
functions, and removes written/local storage from its implicit sensitivity;
plain `@*` retains its call-site-only behavior. The legal latch and edge/level
flop probes guard against false positives for ordinary functions and legal
event controls. Negative fixtures assert that writer, timing, and blocking
violations reject with lint disabled.

Fixed-array and resizable-container process dependencies remain covered by
the shared `sim_array_sensitivity.rs` suite.

The Rust suite invokes the public `llg` executable in optimized and
`--no-opt` modes.
