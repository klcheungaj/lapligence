# Process semantic regressions

These file-backed probes cover the process-family contracts in IEEE
1800-2009 §§9.2.2.2–9.2.2.4 and the Verilog-2001 `@*` sensitivity rule in
§9.7.5. `always_comb` executes at time zero, follows reads in called
functions, and removes written/local storage from its implicit sensitivity;
plain `@*` retains its call-site-only behavior. The legal latch and edge/level
flop probes guard against false positives for ordinary functions and legal
event controls. Negative fixtures assert that writer, timing, and blocking
violations reject with lint disabled.

`callee_locals` and `callee_locals_shared` call the same subroutines from two
`always_comb` procedures (and continuous assignments, sub-instances, package,
interface and class methods): automatic locals of every storage kind, `ref`,
output and value formals, results, recursion and automatic locals of a static
function. Expected values are computed by hand from 6.21 (an automatic variable
is a new one in each activation); they are the industry-standard output too.
Static locals: decisions SD10-D1 and SD10-D2.

Fixed-array and resizable-container process dependencies remain covered by
the shared `sim_array_sensitivity.rs` suite.

The Rust suite invokes the public `llg` executable in optimized and
`--no-opt` modes.
