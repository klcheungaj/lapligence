# Simulator type conformance fixtures

HDL designs are checked-in `.sv` files with top module `tb`, passed directly
to the `llg` executable by `tests/support/sim_cli.rs`. Do not embed or regenerate
HDL inside Rust tests. Keep Rust orchestration and independent expected output
in `tests/sim_type_conformance.rs` and its sibling directory.

Positive cases run with default optimization and `--no-opt` in separate
temporary child directories. Assert exact stdout, expected lowering warnings
and runtime stderr; frontend warnings are permitted for intentional boundary
probes. Negative cases assert exit status 1 and the specific diagnostic in both
modes. Require CMake, preserve timeouts, and inherit generated-runtime sanitizer
settings. Extend fixtures and independent oracles together.
