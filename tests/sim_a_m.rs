//! Grouped integration tests: Simulator suites `tests/sim_[a-m]*.rs` (except standalone ones).
//!
//! Each `mod` below is one `tests/<name>.rs` suite, compiled into this
//! single test binary so the library and Slang are linked once per group
//! instead of once per file. Register a new suite here; the
//! `test_layout` suite rejects unregistered files. See `tests/readme.md`.

#[path = "support/c_compiler.rs"]
mod c_compiler;
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

mod sim_arithmetic;
mod sim_array_conditional_assignments;
mod sim_array_sensitivity;
mod sim_audit_a1_packed_constant_patterns;
mod sim_bit_queries;
mod sim_capacity;
mod sim_casez;
mod sim_classes;
mod sim_compact_names;
mod sim_concurrent_assertions;
mod sim_conditional_policy;
mod sim_constant_eval;
mod sim_container_selects;
mod sim_container_sort;
mod sim_coroutine_semantics;
mod sim_counter;
mod sim_data_type_edges;
mod sim_data_types;
mod sim_data_types_completion;
mod sim_data_types_extended;
mod sim_data_types_next;
mod sim_delay;
mod sim_directive_effects;
mod sim_disable;
mod sim_dpi;
mod sim_dynamic_ownership;
mod sim_edition;
mod sim_emit_value_traffic;
mod sim_events;
mod sim_expression_mutations;
mod sim_feature_completion_g1;
mod sim_file_io;
mod sim_fill_literals;
mod sim_final;
mod sim_fixed_array_reductions;
mod sim_fixed_ordering_review;
mod sim_force;
mod sim_fork;
mod sim_fork_lifecycle;
mod sim_frame_cells;
mod sim_frame_hoisting;
mod sim_function;
mod sim_g1_closure;
mod sim_gates;
mod sim_gates_p40;
mod sim_geninit;
mod sim_group1_formal_repairs;
mod sim_group1_repairs;
mod sim_h04_string_format;
mod sim_hier;
mod sim_imported_probes;
mod sim_include_search;
mod sim_inout;
mod sim_instance_sharing;
mod sim_interface;
mod sim_interface_body;
mod sim_logical_ops;
mod sim_loops;
mod sim_mailboxes;
mod sim_memory;
mod sim_memory_editions;
mod sim_memory_guard;
mod sim_memory_views;
mod sim_monitor;
