//! ADV-019 families 1 to 5. The C walkers print the typespec tree of each
//! declaration (37.23); read, find, watch and write structure and union
//! members (37.17, 37.24); and walk the structure objects of a design:
//! ports, interface ports, processes, continuous assignments, tasks and
//! functions, primitives, instance arrays and named events (37.5-37.15,
//! 37.25, 37.31, 37.37, 37.43, 37.58); and walk the expression and
//! statement trees under them (37.54-37.74); and walk the dynamic objects:
//! class objects, definitions and typespecs, strings, chandles and dynamic,
//! queue and associative arrays, with creation and reclamation callbacks
//! (37.3.7, 37.17, 37.27-37.29, 38.36.1). All link as static archives with
//! `--dpi-lib`, so they run on every host. Expected outputs are derived by
//! hand in the fixture readme. The catalog walk (A03) checks every answer of
//! every object it reaches against `src/sim/rt/vpi_catalog.txt`.
use std::collections::{BTreeMap, BTreeSet};

use super::sim_cli;
use super::sim_harness;
use super::vpi_catalog;

const SUITE: &str = "feature_completion/adv_019";

#[test]
fn typespec_trees_name_typedefs_ranges_members_and_enum_constants() {
    let directory = sim_harness::TempDir::new("adv019-types").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "typespecs", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "typespecs",
        include_str!("../fixtures/sim/feature_completion/adv_019/typespecs.out"),
        &[
            "--define-system-task",
            "task $typespecs()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn structure_and_union_members_read_watch_and_write_their_fields() {
    let directory = sim_harness::TempDir::new("adv019-members").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "members", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "members",
        include_str!("../fixtures/sim/feature_completion/adv_019/members.out"),
        &[
            "--define-system-task",
            "task $members()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn structure_walker_reaches_ports_processes_assignments_primitives_and_arrays() {
    let directory = sim_harness::TempDir::new("adv019-structure").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "structure", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "structure",
        include_str!("../fixtures/sim/feature_completion/adv_019/structure.out"),
        &[
            "--define-system-task",
            "task $structure()",
            "--define-system-task",
            "task $fire()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn module_paths_and_timing_checks_walk_their_terminals_and_conditions() {
    let directory = sim_harness::TempDir::new("adv019-timing").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "timing", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "timing",
        include_str!("../fixtures/sim/feature_completion/adv_019/timing.out"),
        &[
            "--define-system-task",
            "task $timing()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn expression_walker_reads_operations_constants_selects_and_calls() {
    let directory = sim_harness::TempDir::new("adv019-expressions").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "expressions", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "expressions",
        include_str!("../fixtures/sim/feature_completion/adv_019/expressions.out"),
        &[
            "--define-system-task",
            "task $exprs(input logic [7:0] x, input real y, input string z)",
            "--define-system-task",
            "task $again()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn statement_walker_reaches_every_statement_of_processes_tasks_and_functions() {
    let directory = sim_harness::TempDir::new("adv019-statements").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "statements", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "statements",
        include_str!("../fixtures/sim/feature_completion/adv_019/statements.out"),
        &[
            "--define-system-task",
            "task $statements()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn dynamic_objects_walk_class_objects_definitions_and_container_elements() {
    let directory = sim_harness::TempDir::new("adv019-dynamic").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "dynamic", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "dynamic",
        include_str!("../fixtures/sim/feature_completion/adv_019/dynamic.out"),
        &[
            "--define-system-task",
            "task $walk()",
            "--define-system-task",
            "task $stale()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn class_definitions_list_methods_parameters_and_property_qualifiers() {
    let directory = sim_harness::TempDir::new("adv019-classes").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "classes", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "classes",
        include_str!("../fixtures/sim/feature_completion/adv_019/classes.out"),
        &[
            "--define-system-task",
            "task $classes()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn constraints_list_their_items_qualifiers_and_object_state() {
    let directory = sim_harness::TempDir::new("adv019-constraints").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "constraints", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "constraints",
        include_str!("../fixtures/sim/feature_completion/adv_019/constraints.out"),
        &[
            "--define-system-task",
            "task $constraints()",
            "--define-system-task",
            "task $modes()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn dynamic_objects_of_packages_blocks_and_tasks_are_found_by_name_and_scope() {
    let directory = sim_harness::TempDir::new("adv019-scopes").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "scopes", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes",
        include_str!("../fixtures/sim/feature_completion/adv_019/scopes.out"),
        &[
            "--define-system-task",
            "task $scopes()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn class_object_callbacks_follow_creation_and_reclamation_under_collection_stress() {
    let directory = sim_harness::TempDir::new("adv019-gc").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "gc", &[]);
    // A collection at every safe point after an allocation reclaims an
    // object as soon as no variable refers to it; verify mode turns any
    // access to a reclaimed object into a fatal error.
    sim_cli::run_case_backend_parity(
        SUITE,
        "gc",
        include_str!("../fixtures/sim/feature_completion/adv_019/gc.out"),
        &[
            "--define-system-task",
            "task $setup()",
            "--define-system-task",
            "task $grab()",
            "--define-system-task",
            "task $check(int)",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[("LLG_GC_STRESS", "1"), ("LLG_GC_VERIFY", "1")],
    );
}

#[test]
fn instance_properties_follow_the_definition_and_its_configuration() {
    let directory = sim_harness::TempDir::new("adv019-declarations").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "declarations", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "declarations",
        include_str!("../fixtures/sim/feature_completion/adv_019/declarations.out"),
        &[
            "--top",
            "cfg",
            "--define-system-task",
            "task $declarations()",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

#[test]
fn runtime_structures_list_systfs_callbacks_time_queues_uses_and_udps() {
    let directory = sim_harness::TempDir::new("adv019-runtime").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "runtime", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "runtime",
        include_str!("../fixtures/sim/feature_completion/adv_019/runtime.out"),
        &[
            "--define-system-task",
            "task $runtime(input int)",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

/// Threads follow the initial and always procedures and fork branches, and
/// frames the task and function calls of each; frame and thread callbacks
/// run, and handles to ended frames and threads fail (ADV-019-A02).
#[test]
fn threads_and_frames_follow_processes_and_subprogram_calls() {
    let directory = sim_harness::TempDir::new("adv019-threads").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "threads", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "threads",
        include_str!("../fixtures/sim/feature_completion/adv_019/threads.out"),
        &[
            "--define-system-task",
            "task $threads(input int)",
            "--dpi-lib",
            archive.as_str(),
        ],
        &[],
    );
}

/// The answers of one catalog walk: the object types it reached and, per
/// type, the (kind, name) queries answered without an error.
struct Walk {
    types: BTreeSet<i64>,
    answers: BTreeMap<i64, BTreeSet<(String, String)>>,
}

fn parse_walk(stdout: &str, label: &str) -> Walk {
    let mut walk = Walk {
        types: BTreeSet::new(),
        answers: BTreeMap::new(),
    };
    let mut done = false;
    for line in stdout.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |i: usize| -> i64 {
            words
                .get(i)
                .and_then(|w| w.parse().ok())
                .unwrap_or_else(|| panic!("{label}: malformed line `{line}`"))
        };
        match words.first().copied() {
            Some("obj") => {
                walk.types.insert(number(1));
            }
            Some("+") => {
                let kind = if words[1] == "value" {
                    "func"
                } else {
                    words[1]
                };
                walk.answers
                    .entry(number(2))
                    .or_default()
                    .insert((kind.to_owned(), words[3].to_owned()));
            }
            // 38.6: a routine that raised an error returned a value anyway.
            Some("!") => panic!("{label}: error with a value: `{line}`"),
            Some("done") => done = true,
            _ => {}
        }
    }
    assert!(done, "{label}: the walk did not finish");
    walk
}

/// ADV-019 A03 and ADV-020 A01: `catalog.c` walks every object it reaches
/// from the top of `catalog.sv` and queries each one with every property
/// and relation the catalog or the runtime dispatch names. Each answer must
/// be a row the catalog marks implemented or extension for that object's
/// type; each such row the catalog gives to this walk must be answered;
/// every other query is a wrong-object query and must fail with an error
/// (vpiUndefined or NULL, 38.6; decision A19-D26).
#[test]
fn catalog_walk_matches_the_vpi_catalog() {
    let catalog = vpi_catalog::Catalog::load();
    let values = vpi_catalog::header_values();
    let mut by_type = BTreeMap::new();
    for node in &catalog.nodes {
        if let Some(name) = &node.type_name {
            let value = if name == "NULL" { 0 } else { values[name] };
            by_type.insert(value, node.id.clone());
        }
    }
    let walk_alias = catalog
        .tests
        .iter()
        .find(|(_, test)| *test == "adv_019::catalog_walk_matches_the_vpi_catalog")
        .map(|(alias, _)| alias.clone())
        .expect("the catalog names this test");
    let directory = sim_harness::TempDir::new("adv019-catalog").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "catalog", &[]);
    let args = [
        "--define-system-task",
        "task $catalog()",
        "--define-system-task",
        "function int $catalog_value()",
        "--dpi-lib",
        archive.as_str(),
    ];
    for optimized in [true, false] {
        let label = format!("catalog walk, optimized={optimized}");
        let (_run, output) = sim_cli::run_case_keeping_dir(SUITE, "catalog", optimized, &args, &[]);
        assert!(
            output.status.success(),
            "{label}: {}",
            sim_cli::observed_output(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("hi 1 1 2 1 4 7\n"),
            "{label}: the design did not finish: {}",
            sim_cli::observed_output(&output)
        );
        let walk = parse_walk(&stdout, &label);
        assert!(
            walk.types.len() > 100,
            "{label}: reached only {:?}",
            walk.types
        );
        let mut errors = Vec::new();
        let empty = BTreeSet::new();
        for t in &walk.types {
            let Some(id) = by_type.get(t) else {
                errors.push(format!("object type {t} is not in the catalog"));
                continue;
            };
            let rows = catalog
                .effective_rows(id)
                .unwrap_or_else(|error| panic!("{error}"));
            let answers = walk.answers.get(t).unwrap_or(&empty);
            for (kind, name) in answers {
                if vpi_catalog::IMPLICIT.contains(&(kind.as_str(), name.as_str())) {
                    continue;
                }
                match rows.get(&(kind.clone(), name.clone())) {
                    Some(status) if status.answers() => {}
                    Some(status) => errors.push(format!(
                        "{id}: {kind} {name} answered, but the catalog says {}",
                        status.word
                    )),
                    None => errors.push(format!(
                        "{id}: {kind} {name} answered, but the catalog does not list it \
                         (a wrong-object query must fail)"
                    )),
                }
            }
            for ((kind, name), status) in &rows {
                if status.answers()
                    && status.test() == Some(walk_alias.as_str())
                    && !answers.contains(&(kind.clone(), name.clone()))
                {
                    errors.push(format!("{id}: {kind} {name} is {} but failed", status.word));
                }
            }
        }
        assert!(errors.is_empty(), "{label}:\n{}", errors.join("\n"));
    }
}
