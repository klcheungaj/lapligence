# Partial features fixture contracts

Follow [repository test rules](../../../AGENTS.md). Keep checked-in `.sv` top-`tb`
designs, passed directly through `support/sim_cli.rs`; orchestration and independent
oracles belong to `sim_partial_features.rs` and its domain directory. Do not embed/regenerate HDL.

Run both optimizer modes in isolated directories; require CMake, timeouts and
inherited generated-runtime sanitizer settings. Positives assert exact stdout,
lowering warnings and stderr, allowing frontend warnings only for intentional
boundary probes. Negatives require exit 1 and their specific diagnostic in both
modes. Extend fixtures and independent oracles together.
