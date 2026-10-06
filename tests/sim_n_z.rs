//! Grouped integration tests: Simulator suites `tests/sim_[n-z]*.rs` (except `sim_syn*`).
//!
//! Each `mod` below is one `tests/<name>.rs` suite, compiled into this
//! single test binary so the library and Slang are linked once per group
//! instead of once per file. Register a new suite here; the
//! `test_layout` suite rejects unregistered files. See `tests/readme.md`.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;
mod support;

mod sim_net_decl;
mod sim_net_defaults;
mod sim_net_interval;
mod sim_net_partition;
mod sim_net_resolution;
mod sim_nonconvergence;
mod sim_operator_semantics;
mod sim_opt_differential;
mod sim_p30_fixed_arrays;
mod sim_packed_strings;
mod sim_param_override;
mod sim_partial_features;
mod sim_physical_time;
mod sim_plusargs;
mod sim_port_net_types;
mod sim_procedural_assign;
mod sim_procedural_control;
mod sim_process_control;
mod sim_process_semantics;
mod sim_program;
mod sim_random;
mod sim_random_streams;
mod sim_real;
mod sim_real_conversions;
mod sim_reference_args;
mod sim_resume_locations;
mod sim_review_batch2;
mod sim_review_batch3;
mod sim_review_batch4;
mod sim_review_bundle;
mod sim_review_bundle_composition;
mod sim_review_bundle_patterns;
mod sim_review_next4;
mod sim_review_tasks08_11;
mod sim_review_tasks12_15;
mod sim_review_tasks16_19;
mod sim_review_tasks20_23;
mod sim_review_tasks24_27;
mod sim_rtl_completion;
mod sim_rtl_composition;
mod sim_sampled_values;
mod sim_semantic;
mod sim_semaphore;
mod sim_sequential_predicates;
mod sim_stack_bounds;
mod sim_static_review;
mod sim_stochastic;
mod sim_stress;
mod sim_tagged_union_access;
mod sim_task_lowering;
mod sim_time_literals;
mod sim_time_values;
mod sim_timescale;
mod sim_type_conformance;
mod sim_u01_coverage;
mod sim_udp;
mod sim_undefined_behavior;
mod sim_unique_priority;
mod sim_unknown_digits;
mod sim_value_backends;
mod sim_variable_lifetime;
mod sim_varinit;
mod sim_virtual_interfaces;
mod sim_vpi;
mod sim_wait;
mod sim_wait_storage;
mod sim_waveform;
mod sim_wildcard_eq;
