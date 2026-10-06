//! Grouped integration tests: Frontend, LSP, lint, CLI, configuration and tooling suites.
//!
//! Each `mod` below is one `tests/<name>.rs` suite, compiled into this
//! single test binary so the library and Slang are linked once per group
//! instead of once per file. Register a new suite here; the
//! `test_layout` suite rejects unregistered files. See `tests/readme.md`.

#[path = "support/c_compiler.rs"]
mod c_compiler;
#[path = "support/generated_c_lint.rs"]
mod generated_c_lint;
#[allow(dead_code)]
#[path = "../build_support/host_platform.rs"]
mod host_platform;
#[path = "support/sim.rs"]
mod sim_harness;
mod support;

mod compilation_units;
mod compile_errors;
mod compiler_cache;
mod config_effect;
mod dump_tokens;
mod elab_resolve;
mod emit_decoupling;
mod generated_c_determinism;
mod generated_c_frame_lint;
mod include_dir_search;
mod lint_config_cli;
mod llg_config_cli;
mod lsp_stdio;
mod model_tests;
mod parse_only;
mod probe_bindings;
mod property_elab;
mod region_conformance;
mod shadowing;
mod slang_frontend;
mod slang_semantics;
mod slang_static_function_return;
mod support_harness;
mod syn003_pattern_lvalues_import;
mod test_layout;
mod vendor_patches;
