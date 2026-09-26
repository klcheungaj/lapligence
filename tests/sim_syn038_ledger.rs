//! Structural and public-CLI checks for the finite SYN-038 grammar/context ledger.
//!
//! The structural checks keep the documented denominator and audited evidence
//! map reviewable. Focused public-CLI witnesses run the corrected grammar rows
//! and the high-risk context combinations through both optimizer modes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LEDGER_START: &str = "### SYN-038 selected Core grammar-by-context ledger";
const DISPOSITION_START: &str = "#### SYN-038 72-group disposition";
const LEDGER_END: &str = "## Validation scope";
const EVIDENCE_MAP_START: &str = "#### SYN-038 audited evidence map";
const EVIDENCE_MAP_END: &str = "#### SYN-038 selected-profile exclusions";

#[test]
fn selected_core_pairwise_manifest_matches_frozen_rules() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new("python3")
        .arg(root.join("scripts/check_syn038_pairwise_manifest.py"))
        .current_dir(&root)
        .output()
        .expect("run the SYN-038 pairwise manifest checker");
    assert!(
        result.status.success(),
        "SYN-038 pairwise manifest checker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn selected_core_pairwise_source_format_regressions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new("python3")
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_syn038_pairwise_source.py",
        ])
        .current_dir(&root)
        .output()
        .expect("run the compact SYN-038 source regression tests");
    assert!(
        result.status.success(),
        "SYN-038 source regressions failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn section<'a>(document: &'a str, start: &str, end: &str) -> &'a str {
    let start_at = document
        .find(start)
        .unwrap_or_else(|| panic!("missing SYN-038 section marker {start:?}"));
    let body = &document[start_at..];
    let end_at = body
        .find(end)
        .unwrap_or_else(|| panic!("missing SYN-038 section end marker {end:?}"));
    &body[..end_at]
}

fn table_cells(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}

fn code_spans(value: &str) -> impl Iterator<Item = &str> {
    value
        .split('`')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
}

fn fixture_paths(value: &str) -> Vec<PathBuf> {
    code_spans(value)
        .filter(|span| span.starts_with("tests/"))
        .map(PathBuf::from)
        .collect()
}

fn next_standalone_test_attribute(source: &str) -> Option<usize> {
    source.match_indices("#[test]").find_map(|(index, _)| {
        let line_start = source[..index].rfind('\n').map_or(0, |newline| newline + 1);
        source[line_start..index]
            .trim()
            .is_empty()
            .then_some(line_start)
    })
}

fn named_test_body<'a>(source: &'a str, test_name: &str) -> Option<&'a str> {
    let start = source.find(&format!("fn {test_name}"))?;
    let tail = &source[start..];
    let end = next_standalone_test_attribute(tail).unwrap_or(tail.len());
    Some(&tail[..end])
}

fn test_owner_invokes_fixture(source: &str, test_name: &str, fixture_stem: &str) -> bool {
    if let Some(body) = named_test_body(source, test_name) {
        return body.contains(fixture_stem);
    }

    // Macro-generated tests declare their test name and fixture together at
    // the invocation site, so check that exact invocation instead of any
    // unrelated occurrence in the same test file.
    let marker = format!("{test_name},");
    let Some(name_at) = source.find(&marker) else {
        return false;
    };
    let invocation_start = source[..name_at].rfind("!(").unwrap_or(name_at);
    let invocation_tail = &source[name_at..];
    let invocation_end = invocation_tail
        .find(");")
        .map(|end| name_at + end + 2)
        .unwrap_or(source.len());
    if source[invocation_start..invocation_end].contains(fixture_stem) {
        return true;
    }

    // Some test macros bind a fixture in the macro body and take only a
    // generated test name and width at each invocation. In that case, verify
    // the named invocation uses the macro whose definition selects the file.
    let macro_name = source[..invocation_start]
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .rfind(|part| !part.is_empty());
    let Some(macro_name) = macro_name else {
        return false;
    };
    let definition = format!("macro_rules! {macro_name}");
    let Some(definition_start) = source.find(&definition) else {
        return false;
    };
    let definition_tail = &source[definition_start..];
    let definition_end = definition_tail
        .find("macro_rules!")
        .filter(|end| *end > definition.len())
        .unwrap_or(definition_tail.len());
    definition_tail[..definition_end].contains(fixture_stem)
}

fn owner_passes_cli_arg(owner_body: &str, source: &str, argument: &str) -> bool {
    owner_body.contains(&format!("\"{argument}\""))
        || (owner_body.contains("EDITION")
            && source.lines().any(|line| {
                line.trim_start().starts_with("const EDITION:")
                    && line.contains(&format!("\"{argument}\""))
            }))
}

fn named_function_body<'a>(source: &'a str, function_name: &str) -> &'a str {
    let marker = format!("fn {function_name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing function {function_name}"));
    let tail = &source[start..];
    let end = ["\nfn ", "\npub(crate) fn ", "\n#[test]"]
        .into_iter()
        .filter_map(|next_item| tail.find(next_item))
        .min()
        .unwrap_or(tail.len());
    &tail[..end]
}

fn optimized_mode_loop_body(owner_body: &str) -> &'_ str {
    let header = "for optimized in [false, true] {";
    let start = owner_body
        .find(header)
        .expect("manual CLI owner must have the explicit two-mode loop");
    let tail = &owner_body[start + header.len()..];
    let end = tail
        .find("\n    }\n")
        .expect("manual CLI optimizer loop must close before its test function");
    &tail[..end]
}

struct ManualCliWitness {
    row_id: &'static str,
    owner: &'static str,
    suite: &'static str,
    fixture: &'static str,
    stdout_assertion: &'static str,
}

const MANUAL_CLI_WITNESSES: &[ManualCliWitness] = &[
    ManualCliWitness {
        row_id: "SYN038-W44",
        owner: "tests/sim_syn038_interface_processes.rs::interface_body_latch_and_ff_members_match_in_both_modes",
        suite: "syn038_pairwise",
        fixture: "interface_processes",
        stdout_assertion: "output.stdout.as_slice(),\n            expected_stdout.as_bytes(),\n            \"{label}\"",
    },
    ManualCliWitness {
        row_id: "SYN038-W46",
        owner: "tests/sim_syn038_union_interface.rs::interface_union_field_nba_and_child_input_actual_match_in_both_modes",
        suite: "syn038_pairwise",
        fixture: "union_interface",
        stdout_assertion: "output.stdout.as_slice(),\n            expected_stdout.as_bytes(),\n            \"{label}\"",
    },
    ManualCliWitness {
        row_id: "SYN038-W50",
        owner: "tests/sim_syn038_record_auto_ref.rs::automatic_record_array_initializer_and_selected_ref_match_oracle",
        suite: "syn038_pairwise",
        fixture: "record_auto_ref",
        stdout_assertion: "assert_eq!(output.stdout.as_slice(), expected_stdout, \"{label}\");",
    },
    ManualCliWitness {
        row_id: "SYN038-W51",
        owner: "tests/sim_syn038_interface_record_inout.rs::interface_record_field_inout_task_preserves_neighbor_field_in_both_modes",
        suite: "syn038_pairwise",
        fixture: "interface_record_inout",
        stdout_assertion: "assert_eq!(output.stdout.as_slice(), b\"22 45\\n\", \"{label}\");",
    },
    ManualCliWitness {
        row_id: "SYN038-W55",
        owner: "tests/sim_syn038_record_reduction_init.rs::fixed_record_reduction_initializes_automatic_local",
        suite: "syn038_pairwise",
        fixture: "record_reduction_init",
        stdout_assertion: "assert_eq!(output.stdout.as_slice(), expected_stdout, \"{label}\");",
    },
    ManualCliWitness {
        row_id: "SYN038-W56",
        owner: "tests/sim_syn038_return_slot_formals.rs::static_function_return_slots_bind_each_writable_formal_direction",
        suite: "syn038_pairwise",
        fixture: "return_slot_formals",
        stdout_assertion: "assert_eq!(output.stdout.as_slice(), expected_stdout, \"{label}\");",
    },
    ManualCliWitness {
        row_id: "SYN038-W106",
        owner: "tests/sim_syn038_co_same_root_assignment_rhs.rs::same_root_assignment_rhs_witnesses_match_in_both_optimizer_modes",
        suite: "syn038_pairwise",
        fixture: "co_same_root_assignment_rhs",
        stdout_assertion: "assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, \"{label}\");",
    },
];

fn manual_cli_witness(row_id: &str) -> Option<&'static ManualCliWitness> {
    MANUAL_CLI_WITNESSES.iter().find(|witness| {
        row_id == witness.row_id || row_id.starts_with(&format!("{} ", witness.row_id))
    })
}

fn assert_manual_cli_contract(
    root: &Path,
    owner_body: &str,
    owner: &str,
    witness: &ManualCliWitness,
) {
    assert_eq!(
        owner, witness.owner,
        "manual CLI witness owner must remain the reviewed test"
    );
    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "manual CLI witness must exercise both optimizer modes exactly once"
    );
    assert_eq!(
        owner_body.matches("sim_cli::invoke_with_env(").count(),
        1,
        "manual CLI witness must use one public-CLI invocation inside the mode loop"
    );
    let mode_loop = optimized_mode_loop_body(owner_body);
    let invocation = format!(
        "sim_cli::invoke_with_env(\n            \"{}\",\n            \"{}\",\n            optimized,\n            &[\"--edition\", \"2009\"],\n            &[],\n            &[],\n        );",
        witness.suite, witness.fixture
    );
    for required in [
        invocation.as_str(),
        "output.status.success(),",
        witness.stdout_assertion,
        "assert_eq!(output.stderr.as_slice(), b\"\", \"{label}\");",
    ] {
        assert!(
            mode_loop.contains(required),
            "{} manual owner no longer proves required CLI behavior: {required}",
            witness.row_id
        );
    }

    let cli_helper = fs::read_to_string(root.join("tests/support/sim_cli.rs"))
        .expect("read public-CLI invocation helper");
    let invoke_body = named_function_body(&cli_helper, "invoke_with_env");
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".join(\"tests/fixtures/sim\")",
        ".join(suite)",
        ".join(format!(\"{fixture}.sv\"))",
        "assert!(source.is_file()",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args(args)",
        "command.arg(source)",
    ] {
        assert!(
            invoke_body.contains(required),
            "{} public-CLI helper no longer guarantees the named fixture, mode, and arguments: {required}",
            witness.row_id
        );
    }
}

fn assert_static_return_continuous_cli_contract(
    root: &Path,
    test_source: &str,
    owner_body: &str,
    owner: &str,
) {
    assert_eq!(
        owner,
        "tests/sim_syn038_static_return_continuous.rs::static_function_result_accepts_hierarchical_continuous_variable_assignment",
        "W66 must retain its source-bound public-CLI owner"
    );

    let fixture_path_body = named_function_body(test_source, "fixture_path");
    for required in [
        "std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"))",
        ".join(\"tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv\")",
    ] {
        assert!(
            fixture_path_body.contains(required),
            "W66 fixture_path no longer resolves its documented fixture: {required}"
        );
    }

    let invoke_body = named_function_body(test_source, "invoke");
    for required in [
        "let source = fixture_path();",
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".args([\"--top\", \"tb\"])",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args([\"--edition\", \"2009\"])",
        "if let Some(define) = define",
        "command.args([\"--define\", define])",
        "command.arg(source)",
        "sim_harness::run_command(&mut command",
    ] {
        assert!(
            invoke_body.contains(required),
            "W66 invoke helper no longer proves its CLI path, edition, and optimizer options: {required}"
        );
    }

    let exact_cli_body = named_function_body(test_source, "assert_exact_cli");
    for required in [
        "for optimized in [false, true]",
        "let output = invoke(optimized, define)",
        "assert_eq!(output.status.code(), Some(expected_status)",
        "String::from_utf8_lossy(&output.stdout),\n            expected_stdout,",
        "String::from_utf8_lossy(&output.stderr),\n            expected_stderr,",
    ] {
        assert!(
            exact_cli_body.contains(required),
            "W66 exact CLI helper no longer checks modes and outputs: {required}"
        );
    }

    for required in [
        "let source = fixture_path();",
        "Warning: {}:10:27 non-void function 'f' does not return a value\\n",
        "source.display()",
        "assert_exact_cli(None, 0, \"result=1\\n\", &expected_stderr);",
    ] {
        assert!(
            owner_body.contains(required),
            "W66 positive owner no longer derives and asserts its documented exact oracle: {required}"
        );
    }
    assert!(
        owner_body.contains(
            "fn static_function_result_accepts_hierarchical_continuous_variable_assignment"
        ),
        "W66 owning test function is not present in its extracted body"
    );

    let fixture = root.join("tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv");
    assert!(
        fixture.is_file(),
        "W66 absolute-path oracle fixture must exist"
    );
}

fn assert_op_consumer_source_bound_cli_contract(
    root: &Path,
    test_source: &str,
    owner_body: &str,
    owner: &str,
) {
    assert_eq!(
        owner,
        "tests/sim_syn038_op_consumer_matrix.rs::expression_consumers_keep_distinct_contexts_in_both_cli_modes",
        "W69 must retain its reviewed source-bound CLI owner"
    );

    for required in [
        "const FIXTURE_SOURCE: &str = include_str!(\"fixtures/sim/syn038_pairwise/op_consumer_matrix.sv\");",
        "const EXPECTED_STDOUT: &[u8] = b\"calls=18,0,18,18 events=1,1,1 widths=5,7,2\\n\";",
        "const EXPECTED_STDERR: &[u8] = b\"\";",
    ] {
        assert!(
            test_source.contains(required),
            "W69 owner lost its checked-in fixture or exact CLI oracle: {required}"
        );
    }

    let fixture = root.join("tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv");
    let fixture_source = fs::read_to_string(&fixture).expect("read W69 fixture");
    assert!(
        fixture_source.starts_with(
            "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv\n"
        ),
        "W69 fixture source header changed"
    );

    let anchors_start = test_source
        .find("const FOCAL_SOURCE_ANCHORS: &[&str] = &[\n")
        .expect("W69 must retain its focal-source anchor inventory");
    let anchors_tail = &test_source[anchors_start..];
    let anchors_end = anchors_tail
        .find("\n];")
        .expect("W69 focal-source anchor inventory must close");
    let anchors = &anchors_tail[..anchors_end];
    let required_anchors = [
        "consume(select ? a : b)",
        "consume_bit(a == b)",
        "consume(two_byte_t'(a))",
        "consume('{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]})",
        "return x == y;",
        "always @(select ? a[0] : b[0])",
        "always @(two_byte_t'(a))",
        "always @(byte_t'{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]})",
        ".value(two_byte_t'(a))",
        "typedef logic [(CONST_SELECT ? 5 : 7)-1:0] conditional_width_t;",
        "typedef logic [((CONST_A == CONST_B) ? 5 : 7)-1:0] equality_width_t;",
        "typedef logic [(int'(CONST_A[3:0]) - 1):0] cast_width_t;",
        "byte_t conditional_decl = CONST_SELECT ? CONST_A : CONST_B;",
    ];
    assert_eq!(
        anchors
            .lines()
            .filter(|line| line.trim_start().starts_with('"'))
            .count(),
        required_anchors.len(),
        "W69 focal-source anchor inventory changed size"
    );
    for anchor in required_anchors {
        assert!(
            anchors.contains(&format!("\"{anchor}\"")),
            "W69 focal-source inventory lost anchor {anchor:?}"
        );
        assert!(
            fixture_source.contains(anchor),
            "W69 fixture no longer contains focal source {anchor:?}"
        );
    }

    let required_result_assertions = [
        "if (conditional_decl != 8'h12)",
        "if (observed != 8'h12)",
        "if (conditional_call != 18 || equality_call != 0 || cast_call != 18 || pattern_call != 18)",
        "if (equal_return(a,b) != 0)",
        "if (event_conditional < 1 || event_cast < 1 || event_pattern < 1)",
        "if ($bits(conditional_width_t) != 5 || $bits(equality_width_t) != 7 || $bits(cast_width_t) != 2)",
    ];
    for assertion in required_result_assertions {
        assert!(
            fixture_source.contains(assertion),
            "W69 fixture lost its semantic result check {assertion:?}"
        );
    }

    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W69 owner must invoke both optimizer modes exactly once"
    );
    for required in [
        "Path::new(env!(\"CARGO_MANIFEST_DIR\"))",
        ".join(\"tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv\")",
        "let fixture_path = Path::new(env!(\"CARGO_MANIFEST_DIR\"))",
    ] {
        assert!(
            owner_body.contains(required),
            "W69 owner no longer resolves the exact checked-in fixture path: {required}"
        );
    }
    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".args([\"--top\", \"tb\"])",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args([\"--edition\", \"2009\"]).arg(&fixture_path)",
        "sim_harness::run_command(&mut command, Duration::from_secs(180))",
        "assert_eq!(\n            output.status.code(),\n            Some(0),",
        "assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, \"{label}\");",
        "assert_eq!(output.stderr.as_slice(), EXPECTED_STDERR, \"{label}\");",
    ] {
        assert!(
            mode_loop.contains(required),
            "W69 owner no longer traces the exact public-CLI fixture, edition, modes, and oracle: {required}"
        );
    }
    assert!(
        owner_body.contains("fn expression_consumers_keep_distinct_contexts_in_both_cli_modes"),
        "W69 owner test function is not present in its extracted body"
    );
}

fn assert_operation_context_matrix_cli_contract(test_source: &str, owner_body: &str, owner: &str) {
    assert_eq!(
        owner,
        "tests/sim_syn038_operation_context_matrix.rs::typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes",
        "W98 must retain its reviewed source-bound CLI owner"
    );
    for required in [
        "include_str!(\"fixtures/sim/syn038_pairwise/operation_context_matrix.sv\")",
        "const EXPECTED_STDERR: &[u8] = b\"\";",
        "const CONST_REF_NEGATIVES: [(&str, &str, usize, usize, &str, &str); 4] = [",
    ] {
        assert!(
            test_source.contains(required),
            "W98 owner lost its checked-in fixture, exact stderr oracle, or four negative controls: {required}"
        );
    }
    for fixture in [
        "constref_conditional_rejected.sv",
        "constref_equality_rejected.sv",
        "constref_cast_rejected.sv",
        "constref_pattern_rejected.sv",
    ] {
        assert!(
            test_source.contains(&format!("\"{fixture}\"")),
            "W98 negative-control inventory lost {fixture}"
        );
    }
    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W98 must run both public-CLI optimizer modes exactly once"
    );
    for required in [
        "let fixture_path = Path::new(env!(\"CARGO_MANIFEST_DIR\"))",
        ".join(\"tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv\")",
    ] {
        assert!(
            owner_body.contains(required),
            "W98 owner no longer resolves the exact checked-in positive fixture: {required}"
        );
    }

    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        "command.current_dir(directory.path()).args([\"--top\", \"tb\"]);",
        "if !optimized",
        "command.arg(\"--no-opt\");",
        "command.args([\"--edition\", \"2009\"]).arg(&fixture_path);",
        "sim_harness::run_command(&mut command, Duration::from_secs(180))",
        "output.status.code(),\n            Some(0),",
        "assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, \"{label}\");",
        "assert_eq!(output.stderr.as_slice(), EXPECTED_STDERR, \"{label}\");",
        "for (filename, source, line, column, variable_actual, expression_actual) in\n            CONST_REF_NEGATIVES",
        "let negative_path = negative_root.join(filename);",
        "negative_command\n                .current_dir(negative_directory.path())\n                .args([\"--top\", \"tb\"]);",
        "negative_command.arg(\"--no-opt\");",
        "negative_command\n                .args([\"--edition\", \"2009\"])\n                .arg(&negative_path);",
        "negative_output.status.code(),\n                Some(1),",
        "assert!(negative_output.stdout.is_empty(), \"{negative_label}\");",
        "assert_eq!(\n                negative_output.stderr.as_slice(),\n                expected_negative_stderr.as_bytes(),",
    ] {
        assert!(
            mode_loop.contains(required),
            "W98 owner no longer proves its positive/negative CLI path, modes, and exact oracles: {required}"
        );
    }
    assert!(
        test_source.contains(
            "invalid expression for pass by reference; only variables, class properties, and members of unpacked structs and arrays are allowed"
        ),
        "W98 exact const-ref rejection diagnostic changed"
    );
}

fn assert_interface_runtime_initializer_cli_contract(
    root: &Path,
    test_source: &str,
    owner_body: &str,
    owner: &str,
) {
    assert_eq!(
        owner,
        "tests/sim_syn038_interface_runtime_initializer.rs::module_initializer_reads_bound_interface_storage_in_both_modes",
        "W87 must retain its reviewed source-bound CLI owner"
    );

    for required in [
        "const FIXTURE_SOURCE: &str =\n    include_str!(\"fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv\");",
        "const EXPECTED_STDOUT: &str = \"copy=00,source=5a,control=00\\n\";",
    ] {
        assert!(
            test_source.contains(required),
            "W87 owner lost its checked-in fixture or exact stdout oracle: {required}"
        );
    }

    let fixture = root.join("tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv");
    let fixture_source = fs::read_to_string(&fixture).expect("read W87 fixture");
    assert!(
        fixture_source.starts_with(
            "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv\n"
        ),
        "W87 fixture source header changed"
    );
    for required in [
        "logic [7:0] same_scope_copy = local_seed;",
        "logic [7:0] copy = bus.value;",
        "copy !== 8'h00",
        "local_seed = 8'h34;",
        "bus.value = 8'h5a;",
        "same_scope_copy !== 8'h00",
    ] {
        assert!(
            fixture_source.contains(required),
            "W87 fixture lost its source, control, or value oracle: {required}"
        );
    }

    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W87 owner must invoke both optimizer modes exactly once"
    );
    for required in [
        "Path::new(env!(\"CARGO_MANIFEST_DIR\"))",
        ".join(\"tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv\")",
        "let mut command = Command::new(env!(\"CARGO_BIN_EXE_llg\"));",
    ] {
        assert!(
            owner_body.contains(required),
            "W87 owner no longer binds the public CLI to its exact fixture: {required}"
        );
    }

    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "command.current_dir(directory.path()).args([\"--top\", \"tb\"]);",
        "if !optimized {\n            command.arg(\"--no-opt\");\n        }",
        "command.args([\"--edition\", \"2009\"]).arg(&fixture_path);",
        "sim_harness::run_command(&mut command, Duration::from_secs(180))",
        "output.status.success(),",
        "String::from_utf8_lossy(&output.stdout),\n            EXPECTED_STDOUT,",
        "String::from_utf8_lossy(&output.stderr),\n            expected_stderr,",
    ] {
        assert!(
            mode_loop.contains(required),
            "W87 owner no longer proves its isolated public-CLI invocation and exact oracle: {required}"
        );
    }

    for required in [
        "Warning: {}:{}:{} initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\\n",
        "Warning: {}:{}:{} initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\\n",
        "line.contains(\"same_scope_copy = local_seed;\")",
        "line.contains(\"logic [7:0] copy = bus.value;\")",
        ".find(\"local_seed\")",
        ".find(\"bus.value\")",
        "fixture_path.display(), control_line.0, control_column,",
        "fixture_path.display(), warning_line.0, warning_column",
    ] {
        assert!(
            owner_body.contains(required),
            "W87 owner lost its exact source-located warning oracle: {required}"
        );
    }
}

fn assert_storage_write_remainders_cli_contract(root: &Path, owner_body: &str, owner: &str) {
    assert_eq!(
        owner,
        "tests/sim_syn038_storage_write_remainders.rs::storage_write_remainder_cells_run_in_both_optimizer_modes",
        "W88 must retain its reviewed public-CLI owner"
    );

    for required in [
        "let source = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"))\n        .join(\"tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv\");",
        "let expected_stdout = b\"storage_write_remainders=passed\\n\";",
        "Warning: {}:101:33 non-void function 'hier_result' does not return a value\\n\\\n         llg: $finish at time 4000 at tb:153:9\\n",
    ] {
        assert!(
            owner_body.contains(required),
            "W88 owner lost its exact fixture path or stdout/stderr oracle: {required}"
        );
    }

    let fixture = root.join("tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv");
    let fixture_source = fs::read_to_string(&fixture).expect("read W88 fixture");
    assert!(
        fixture_source.starts_with(
            "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv\n"
        ),
        "W88 fixture source header changed"
    );
    for required in [
        "function static logic [7:0] hier_result;",
        "hier_result()",
        "$display(\"storage_write_remainders=passed\");",
    ] {
        assert!(
            fixture_source.contains(required),
            "W88 fixture lost its warning or output source: {required}"
        );
    }

    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W88 owner must invoke both optimizer modes exactly once"
    );
    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "sim_cli::invoke_with_env(\n            \"syn038_pairwise\",\n            \"storage_write_remainders\",\n            optimized,\n            &[\"--edition\", \"2009\"],\n            &[],\n            &[],\n        );",
        "output.status.code(), Some(0),",
        "output.stdout.as_slice(), expected_stdout, \"{label}\"",
        "String::from_utf8_lossy(&output.stderr),\n            expected_stderr,",
    ] {
        assert!(
            mode_loop.contains(required),
            "W88 owner no longer checks its exact public-CLI invocation and oracle: {required}"
        );
    }

    let cli_helper = fs::read_to_string(root.join("tests/support/sim_cli.rs"))
        .expect("read public-CLI invocation helper");
    let invoke_body = named_function_body(&cli_helper, "invoke_with_env");
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".join(\"tests/fixtures/sim\")",
        ".join(suite)",
        ".join(format!(\"{fixture}.sv\"))",
        "assert!(source.is_file()",
        "current_dir(directory.path())",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args(args)",
        "command.arg(source)",
    ] {
        assert!(
            invoke_body.contains(required),
            "W88 public-CLI helper no longer guarantees its fixture, modes, and arguments: {required}"
        );
    }
}

fn storage_write_remainders_warning_oracle_matches(
    row_id: &str,
    expected: &str,
    test_source: &str,
) -> bool {
    row_id == "SYN038-W88"
        && expected
            == concat!(
                "Warning: {absolute_fixture_path}:101:33 non-void function 'hier_result' does not return a value\\n",
                "llg: $finish at time 4000 at tb:153:9\\n",
            )
        && test_source.contains(
            "Warning: {}:101:33 non-void function 'hier_result' does not return a value\\n",
        )
        && test_source.contains("llg: $finish at time 4000 at tb:153:9\\n")
        && test_source.contains("source.display()")
}

fn assert_static_return_ref_actual_cli_contract(root: &Path, owner_body: &str, owner: &str) {
    assert_eq!(
        owner,
        "tests/sim_syn038_static_return_ref_actual.rs::hierarchical_static_function_return_slot_binds_to_task_ref_formal",
        "W89 must retain its reviewed public-CLI owner"
    );
    let fixture = root.join("tests/fixtures/sim/syn038_pairwise/static_return_ref_actual.sv");
    let fixture_source = fs::read_to_string(&fixture).expect("read W89 fixture");
    assert!(
        fixture_source.starts_with(
            "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_return_ref_actual.sv\n"
        ),
        "W89 fixture source header changed"
    );

    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W89 owner must invoke both optimizer modes exactly once"
    );
    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "sim_cli::invoke_with_env(\n            \"syn038_pairwise\",\n            \"static_return_ref_actual\",\n            optimized,\n            &[\"--edition\", \"2009\"],\n            &[],\n            &[],\n        );",
        "assert_eq!(output.status.code(), Some(0), \"{label}\");",
        "output.stdout.as_slice(),\n            b\"static-return-ref=passed\\n\",",
        "String::from_utf8_lossy(&output.stderr),\n            expected_stderr,",
        "let expected_stderr = \"llg: $finish at time 0 at tb:18:9\\n\";",
    ] {
        assert!(
            mode_loop.contains(required) || owner_body.contains(required),
            "W89 owner no longer checks its exact public-CLI invocation and oracle: {required}"
        );
    }

    let cli_helper = fs::read_to_string(root.join("tests/support/sim_cli.rs"))
        .expect("read public-CLI invocation helper");
    let invoke_body = named_function_body(&cli_helper, "invoke_with_env");
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".join(\"tests/fixtures/sim\")",
        ".join(suite)",
        ".join(format!(\"{fixture}.sv\"))",
        "assert!(source.is_file()",
        "current_dir(directory.path())",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args(args)",
        "command.arg(source)",
    ] {
        assert!(
            invoke_body.contains(required),
            "W89 public-CLI helper no longer guarantees its fixture, modes, and arguments: {required}"
        );
    }
}

fn assert_read_only_ref_continuous_variable_cli_contract(
    root: &Path,
    test_source: &str,
    owner_body: &str,
    owner: &str,
) {
    const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv";
    assert_eq!(
        owner,
        "tests/sim_syn038_ref_read_continuous_variable.rs::read_only_ref_of_continuously_driven_logic_runs_in_both_cli_modes",
        "W123 must retain its reviewed source-bound public-CLI owner"
    );
    assert!(test_source.contains(&format!("const FIXTURE: &str = \"{FIXTURE}\";")));
    assert!(test_source.contains(
        "include_str!(\"fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv\")"
    ));

    let fixture_path_body = named_function_body(test_source, "fixture_path");
    for required in [
        "Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(FIXTURE)",
        "fn fixture_path() -> std::path::PathBuf",
    ] {
        assert!(
            fixture_path_body.contains(required),
            "W123 fixture_path no longer resolves the checked-in fixture: {required}"
        );
    }

    let fixture = fs::read_to_string(root.join(FIXTURE)).expect("read W123 fixture");
    assert!(fixture.starts_with(&format!("// llg-test-fixture: {FIXTURE}\n")));
    for required in [
        "assign source = 8'h5a;",
        "task automatic capture_ref(ref logic [7:0] value);",
        "        observed = value;",
        "        #1;\n        capture_ref(source);",
        "$display(\"ref-read=%02h\", observed);",
    ] {
        assert!(
            fixture.contains(required),
            "W123 fixture no longer contains its reviewed source anchor: {required}"
        );
    }
    let call_line = fixture
        .lines()
        .position(|line| line.trim() == "capture_ref(source);")
        .expect("W123 source has the ref actual call")
        + 1;
    assert_eq!(
        call_line, 15,
        "W123 diagnostic line follows the source anchor"
    );
    assert_eq!(
        fixture
            .lines()
            .nth(call_line - 1)
            .and_then(|line| line.find("source")),
        Some(20),
        "W123 warning column follows the ref actual source token"
    );

    let cli_body = named_function_body(test_source, "assert_public_cli_output");
    for required in [
        "Command::new(env!(\"CARGO_BIN_EXE_llg\"))",
        ".args([\"--top\", \"tb\"])",
        "if !optimized",
        "command.arg(\"--no-opt\")",
        "command.args([\"--edition\", \"2009\"]).arg(&source)",
        "sim_harness::run_command(&mut command",
        "output.status.success(),",
        "output.stdout, EXPECTED_STDOUT.as_bytes()",
        "source_line(\"capture_ref(source);\")",
        "assert_eq!(\n        output.stderr,\n        expected_stderr.as_bytes(),",
    ] {
        assert!(
            cli_body.contains(required),
            "W123 direct CLI helper no longer proves its fixture, mode, and exact oracle: {required}"
        );
    }
    for required in [
        "const EXPECTED_STDOUT: &str = \"ref-read=5a\\n\";",
        "const WARNING: &str = \"cannot mix continuous and procedural assignments to variable 'source'\";",
        "let expected_stderr = format!(\n        \"Warning: {}:{}:21 {WARNING}\\n\",",
        "let source = fixture_path();",
        ".current_dir(working_directory.path())",
    ] {
        assert!(
            test_source.contains(required),
            "W123 owner lost its source-bound exact oracle or public CLI setup: {required}"
        );
    }

    assert_eq!(
        owner_body.matches("for optimized in [false, true]").count(),
        1,
        "W123 owner must exercise both optimizer modes exactly once"
    );
    let mode_loop = optimized_mode_loop_body(owner_body);
    for required in [
        "assert_public_cli_output(optimized);",
        "FIXTURE_SOURCE.starts_with(",
        "FIXTURE_SOURCE.contains(\"assign source = 8'h5a;\")",
        "FIXTURE_SOURCE.contains(\"observed = value;\")",
        "FIXTURE_SOURCE.contains(\"capture_ref(source);\")",
    ] {
        assert!(
            owner_body.contains(required),
            "W123 named test no longer binds its fixture and mode loop: {required}"
        );
    }
    assert!(mode_loop.contains("assert_public_cli_output(optimized);"));

    let database_owner = named_test_body(
        test_source,
        "owned_continuous_driver_and_ref_actual_share_source_identity",
    )
    .expect("W123 owned-DB identity test remains present");
    assert!(database_owner.contains("assert_owned_driver_and_read_only_call(&db);"));
    let compile_body = named_function_body(test_source, "compile_fixture");
    for required in [
        "compile::compile_checked(&CompileOpts {",
        "files: vec![fixture_path().to_string_lossy().into_owned()],",
        "top: Some(\"tb\".into()),",
        "LanguageEdition::SystemVerilog2009",
        "Db::from_slang(&compiled.snapshot)",
    ] {
        assert!(
            compile_body.contains(required),
            "W123 DB must compile the exact checked-in fixture into the owned database: {required}"
        );
    }
    let db_contract = named_function_body(test_source, "assert_owned_driver_and_read_only_call");
    for required in [
        "db.source_identity(writer_target),\n        db.source_identity(actual_target),",
        "continuous driver LHS and ref actual share one owned source identity",
        "assert_task_body_only_reads_formal(db, *body, formals[0], observed);",
    ] {
        assert!(
            db_contract.contains(required),
            "W123 owned-DB contract no longer proves source identity and read-only ref use: {required}"
        );
    }
    let read_only_body = named_function_body(test_source, "assert_task_body_only_reads_formal");
    for required in [
        "let formal_identity = db.source_identity(formal);",
        "assert_eq!(db.source_identity(lhs), db.source_identity(result));",
        "assert_eq!(db.source_identity(rhs), formal_identity);",
        "db.source_identity(target) == formal_identity",
    ] {
        assert!(
            read_only_body.contains(required),
            "W123 owned task-body proof no longer establishes a read-only formal: {required}"
        );
    }
}

fn read_only_ref_continuous_warning_oracle_matches(
    row_id: &str,
    expected: &str,
    test_source: &str,
) -> bool {
    row_id == "SYN038-W123"
        && expected
            == "Warning: {absolute fixture path}:15:21 cannot mix continuous and procedural assignments to variable 'source'\\n"
        && test_source.contains("source_line(\"capture_ref(source);\")")
        && test_source.contains("Warning: {}:{}:21 {WARNING}\\n")
        && test_source.contains("assert_eq!(\n        output.stderr,\n        expected_stderr.as_bytes(),")
}

fn static_return_ref_actual_finish_oracle_matches(
    row_id: &str,
    expected: &str,
    test_source: &str,
) -> bool {
    row_id == "SYN038-W89"
        && expected == "llg: $finish at time 0 at tb:18:9\\n"
        && test_source.contains("let expected_stderr = \"llg: $finish at time 0 at tb:18:9\\n\";")
}

fn interface_runtime_initializer_warning_oracle_matches(
    row_id: &str,
    expected: &str,
    test_source: &str,
) -> bool {
    row_id == "SYN038-W87"
        && expected
            == concat!(
                "Warning: {absolute_fixture_path}:9:35 initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\\n",
                "Warning: {absolute_fixture_path}:10:24 initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\\n",
            )
        && test_source.contains(
            "Warning: {}:{}:{} initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\\n",
        )
        && test_source.contains(
            "Warning: {}:{}:{} initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\\n",
        )
        && test_source.contains("fixture_path.display(), control_line.0, control_column,")
        && test_source.contains("fixture_path.display(), warning_line.0, warning_column")
}

fn static_return_continuous_path_oracle_matches(row_id: &str, expected: &str) -> bool {
    row_id == "SYN038-W66"
        && expected
            == "Warning: {absolute_fixture_path}:10:27 non-void function 'f' does not return a value\\n"
}

fn computed_context_oracle_matches(row_id: &str, expected: &str, test_source: &str) -> bool {
    match row_id {
        "SYN038-W22" => {
            if ![
                "let tag = 0x2au16 + 1;",
                "let payload = 0xbeefu32 + 0x0101;",
                "rt={tag:02x} rp={payload:04x}",
                "of={tag:02x}{payload:04x}",
                "sim_cli::run_case(SUITE, \"interface_modport_struct\", &expected, \"\", &[]);",
            ]
            .into_iter()
            .all(|source| test_source.contains(source))
            {
                return false;
            }

            let tag = 0x2au16 + 1;
            let payload = 0xbeefu32 + 0x0101;
            let computed_lines = [
                format!("t2 wv=0 rv=1 rt={tag:02x} rp={payload:04x} ov=0 of=000000\\n"),
                format!(
                    "t3 wv=0 rv=0 rt={tag:02x} rp={payload:04x} ov=1 of={tag:02x}{payload:04x}\\n"
                ),
            ];
            computed_lines.iter().any(|line| line == expected)
        }
        "SYN038-W23" => {
            if ![
                "let src = [0x10u8, 0x20, 0x30, 0x40];",
                "let mut part = [0u8; 4];",
                "part[1] = src[0];",
                "part[2] = src[1];",
                "part[3] = src[0];",
                "let sum: u16 = src.iter().map(|value| u16::from(*value)).sum();",
                "part={p0:02x}{p1:02x}{p2:02x}{p3:02x}",
                "sum={sum}\\n",
                "sim_cli::run_case(SUITE, \"aggregate_functions\", &expected, \"\", &[]);",
            ]
            .into_iter()
            .all(|source| test_source.contains(source))
            {
                return false;
            }

            let src = [0x10u8, 0x20, 0x30, 0x40];
            let mut part = [0u8; 4];
            part[1] = src[0];
            part[2] = src[1];
            part[3] = src[0];
            let sum: u16 = src.iter().map(|value| u16::from(*value)).sum();
            let computed_lines = [
                format!(
                    "arr src=10203040 dst=10203040 part={:02x}{:02x}{:02x}{:02x}\\n",
                    part[0], part[1], part[2], part[3]
                ),
                format!("sum={sum}\\n"),
            ];
            computed_lines.iter().any(|line| line == expected)
        }
        _ => false,
    }
}

fn assert_sim_cli_oracle_contract(root: &Path) {
    let helper = fs::read_to_string(root.join("tests/support/sim_cli.rs"))
        .expect("read public-CLI case helper");
    assert!(
        helper.contains("for optimized in [false, true]"),
        "public-CLI case helper must run optimized and --no-opt modes"
    );
    assert!(
        helper.contains("if actual != expected")
            && helper.contains("assert_eq!(runtime_stderr, expected_stderr"),
        "public-CLI case helper must compare exact stdout and stderr"
    );
    assert!(
        helper.contains("assert_eq!(warnings, expected_warnings"),
        "public-CLI case helper must compare the exact lowering-warning set"
    );
    let rejection_helper = named_function_body(&helper, "reject_case_with_args");
    for required in [
        "for optimized in [false, true]",
        "assert_eq!(output.status.code(), Some(1)",
        "assert!(output.stdout.is_empty()",
        "assert!(stderr.contains(diagnostic)",
    ] {
        assert!(
            rejection_helper.contains(required),
            "public-CLI rejection helper lacks its expected failure contract: {required}"
        );
    }
}

fn assert_fixture_exists(root: &Path, row_id: &str, value: &str) {
    let paths = fixture_paths(value);
    assert!(
        !paths.is_empty(),
        "{row_id} must name a checked-in fixture path in backticks"
    );
    for path in paths {
        assert!(
            root.join(&path).is_file(),
            "{row_id} fixture does not exist: {}",
            path.display()
        );
    }
}

#[test]
fn selected_rows_are_traceable_and_unique() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("tests/syn038_coverage_ledger.md"))
        .expect("read the SYN-038 coverage ledger");
    let ledger = section(&document, LEDGER_START, EVIDENCE_MAP_START);
    let selected = ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-CORE-"))
        .collect::<Vec<_>>();
    assert_eq!(
        selected.len(),
        78,
        "SYN-038 selected Core grammar denominator changed"
    );

    let mut ids = HashSet::new();
    for line in selected {
        let cells = table_cells(line);
        assert_eq!(cells.len(), 7, "malformed selected SYN-038 row: {line}");
        let id = cells[0];
        assert!(ids.insert(id), "duplicate selected SYN-038 ID: {id}");
        assert!(
            matches!(cells[1], "V2001" | "SV2009" | "V2001/SV2009"),
            "{id} has no exact edition gate: {}",
            cells[1]
        );
        assert!(
            cells[2].contains("B.") || cells[2].contains("Annex A") || cells[2].contains("SYN-"),
            "{id} has no Annex A/B production or named extension: {}",
            cells[2]
        );
        assert!(
            matches!(cells[5], "PASS" | "REJECT"),
            "{id} has no explicit expected outcome: {}",
            cells[5]
        );
        let owners = code_spans(cells[6]).collect::<Vec<_>>();
        assert!(!owners.is_empty(), "{id} has no behavioral test owner");
        let owner_sources = owners
            .iter()
            .map(|owner| {
                let (test_file, test_name) = owner
                    .split_once("::")
                    .unwrap_or_else(|| panic!("{id} owner must be file::test: {owner}"));
                assert!(
                    test_file.starts_with("tests/") && test_file.ends_with(".rs"),
                    "{id} owner is not a repository test path: {owner}"
                );
                let source = fs::read_to_string(root.join(test_file))
                    .unwrap_or_else(|error| panic!("{id} owner cannot be read: {error}"));
                assert!(
                    source.contains(test_name),
                    "{id} owner name is missing from {test_file}: {test_name}"
                );
                (test_name, source)
            })
            .collect::<Vec<_>>();
        let fixtures = fixture_paths(cells[4]);
        for fixture in &fixtures {
            let stem = fixture
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("fixture path has a UTF-8 stem");
            let owner_matches = owner_sources
                .iter()
                .any(|(test_name, source)| test_owner_invokes_fixture(source, test_name, stem));
            assert!(
                owner_matches,
                "{id} fixture {} has no matching behavioral test owner",
                fixture.display()
            );
        }
        assert!(
            !cells[2].contains("TBD") && !cells[2].contains("unassigned"),
            "{id} leaves its selected production unresolved"
        );
        assert_fixture_exists(&root, id, cells[4]);
    }
}

#[test]
fn audited_evidence_map_names_real_fixtures_and_test_invocations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("tests/syn038_coverage_ledger.md"))
        .expect("read the SYN-038 coverage ledger");
    let evidence_map = section(&document, EVIDENCE_MAP_START, EVIDENCE_MAP_END);
    let rows = evidence_map
        .lines()
        .filter(|line| {
            line.starts_with("| SYN038-CORE-")
                || line.starts_with("| SYN038-PAIR-")
                || line.starts_with("| SYN038-3WAY-")
                || line.starts_with("| SYN038-W")
        })
        .collect::<Vec<_>>();
    assert!(
        rows.len() >= 17,
        "audited context map lost its named witnesses"
    );

    assert_sim_cli_oracle_contract(&root);
    let mut witnesses = HashSet::new();
    for line in rows {
        let cells = table_cells(line);
        assert_eq!(cells.len(), 6, "malformed audited evidence row: {line}");
        let fixture = fixture_paths(cells[2]);
        assert_eq!(fixture.len(), 1, "{} must name one exact fixture", cells[0]);
        assert!(
            root.join(&fixture[0]).is_file(),
            "{} fixture does not exist: {}",
            cells[0],
            fixture[0].display()
        );

        let owner_spans = code_spans(cells[3]).collect::<Vec<_>>();
        let owner = *owner_spans
            .first()
            .unwrap_or_else(|| panic!("{} has no test owner", cells[0]));
        let (test_file, test_name) = owner
            .split_once("::")
            .unwrap_or_else(|| panic!("{} owner must be file::test: {owner}", cells[0]));
        assert!(
            test_file.starts_with("tests/") && test_file.ends_with(".rs"),
            "{} owner is not a repository test path: {owner}",
            cells[0]
        );
        let test_source = fs::read_to_string(root.join(test_file))
            .unwrap_or_else(|error| panic!("{} owner cannot be read: {error}", cells[0]));
        let mut additional_owner_bodies = Vec::<&str>::new();
        if cells[0] == "SYN038-W60" {
            assert_eq!(
                owner_spans.len(),
                2,
                "W60 names positive and negative owners"
            );
            let (negative_file, negative_name) = owner_spans[1]
                .split_once("::")
                .expect("W60 negative owner must be file::test");
            assert_eq!(negative_file, test_file, "W60 controls share one test file");
            let negative_body = named_test_body(&test_source, negative_name)
                .unwrap_or_else(|| panic!("W60 negative test is missing: {negative_name}"));
            let stem = fixture[0]
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("W60 fixture has a UTF-8 stem");
            assert!(
                test_owner_invokes_fixture(&test_source, negative_name, stem),
                "W60 negative owner does not invoke its documented fixture"
            );
            for required in [
                "sim_cli::reject_case_with_args(",
                "\"syn038_pairwise\"",
                "\"static_local_continuous\"",
                "\"semantic error: multiple continuous assignments to variable storage\"",
                "&[\"--edition\", \"2009\", \"--define\", \"SYN038_DUPLICATE_DRIVER\"],",
            ] {
                assert!(
                    negative_body.contains(required),
                    "W60 negative owner lacks its exact rejection invocation: {required}"
                );
            }
            additional_owner_bodies.push(negative_body);
        } else if cells[0].starts_with("SYN038-W106 ") {
            assert_eq!(
                owner_spans.len(),
                2,
                "W106 names separate CLI and owned-DB behavioral witnesses"
            );
            let database_name = owner_spans[1];
            let database_body = named_test_body(&test_source, database_name)
                .unwrap_or_else(|| panic!("W106 DB binding test is missing: {database_name}"));
            for required in [
                "db.source_identity(*target) == db.source_identity(declaration)",
                "assert_same_root_assignment(&db, 22, true, \"whole\"",
                "assert_same_root_assignment(&db, 26, true, \"record\"",
                "assert_same_root_assignment(&db, 30, true, \"concat_value\"",
                "assert_same_root_assignment(\n        &db,\n        34,",
                "assert_same_root_assignment(&db, 38, false, \"nba_value\"",
            ] {
                assert!(
                    test_source.contains(required),
                    "W106 owned-DB evidence lost its source identity or one of five same-root bindings: {required}"
                );
            }
            assert!(
                database_body.contains(
                    "fn slang_binds_each_assignment_rhs_to_its_own_destination_declaration"
                ),
                "W106 second owner must assert source and target DB identities"
            );
            additional_owner_bodies.push(database_body);
        } else {
            assert_eq!(owner_spans.len(), 1, "{} must have one CLI owner", cells[0]);
        }
        let generated_conversion_case = cells[0] == "SYN038-W13";
        let default_edition_case = matches!(cells[0], "SYN038-W21" | "SYN038-W22" | "SYN038-W23");
        let source_bound_static_return_case = cells[0] == "SYN038-W66";
        let source_bound_op_consumer_case = cells[0] == "SYN038-W69";
        let source_bound_interface_runtime_initializer_case = cells[0] == "SYN038-W87";
        let source_bound_storage_write_case = cells[0] == "SYN038-W88";
        let source_bound_static_return_ref_case = cells[0] == "SYN038-W89";
        let source_bound_operation_context_case = cells[0] == "SYN038-W98";
        let source_bound_same_root_case = cells[0].starts_with("SYN038-W106 ");
        let source_bound_read_only_ref_case = cells[0] == "SYN038-W123";
        let source_bound_cli_case = source_bound_static_return_case
            || source_bound_op_consumer_case
            || source_bound_interface_runtime_initializer_case
            || source_bound_storage_write_case
            || source_bound_static_return_ref_case
            || source_bound_operation_context_case
            || source_bound_same_root_case
            || source_bound_read_only_ref_case;
        let manual_witness = manual_cli_witness(cells[0]);
        let (owner_body, runner_source) = if generated_conversion_case {
            assert_eq!(
                test_name, "conversion_cases!::conversions_129",
                "W13 must identify its generated test case precisely"
            );
            assert!(test_source.contains("macro_rules! conversion_cases"));
            assert!(test_source.contains("conversions_129: 129"));
            assert!(test_source.contains("conversion_matrix($width)"));
            assert!(test_source.contains("run_case(&format!(\"conversions-{width}\"), &expected);"));
            let runner = fs::read_to_string(root.join("tests/sim_type_conformance.rs"))
                .expect("read W13 public-CLI runner");
            assert!(runner.contains(
                "sim_cli::run_case(\"type_conformance\", fixture, expected, \"\", warnings);"
            ));
            (test_source.as_str(), Some(runner))
        } else {
            let owner_start = test_source
                .find(&format!("fn {test_name}"))
                .unwrap_or_else(|| panic!("{} test function is missing: {owner}", cells[0]));
            let owner_tail = &test_source[owner_start..];
            let owner_body = next_standalone_test_attribute(owner_tail)
                .map(|end| &owner_tail[..end])
                .unwrap_or(owner_tail);
            (owner_body, None)
        };
        let owner_bodies = std::iter::once(owner_body)
            .chain(additional_owner_bodies.iter().copied())
            .collect::<Vec<_>>();
        assert!(
            witnesses.insert((cells[0], fixture[0].clone(), owner)),
            "duplicate audited evidence witness: {} / {} / {owner}",
            cells[0],
            fixture[0].display()
        );
        let invocation = code_spans(cells[4])
            .next()
            .unwrap_or_else(|| panic!("{} has no CLI fixture invocation", cells[0]));
        let fixture_text = fixture[0].to_string_lossy();
        let fixture_invocation = fixture_text
            .strip_prefix("tests/fixtures/sim/")
            .and_then(|path| path.strip_suffix(".sv"))
            .unwrap_or_else(|| panic!("{} fixture must be beneath tests/fixtures/sim", cells[0]))
            .replace('\\', "/");
        assert_eq!(
            invocation, fixture_invocation,
            "{} CLI invocation does not name its documented fixture",
            cells[0]
        );
        let (suite, case) = fixture_invocation.rsplit_once('/').unwrap_or_else(|| {
            panic!(
                "{} fixture invocation must include suite and case: {invocation}",
                cells[0]
            )
        });
        if generated_conversion_case {
            assert_eq!(suite, "type_conformance");
            assert_eq!(case, "conversions-129");
            assert!(cells[4].contains("default edition SystemVerilog-2009"));
            assert!(runner_source
                .as_deref()
                .is_some_and(|source| source.contains("sim_cli::run_case(\"type_conformance\"")));
        } else if source_bound_interface_runtime_initializer_case {
            assert_interface_runtime_initializer_cli_contract(
                &root,
                &test_source,
                owner_body,
                owner,
            );
            assert!(
                cells[4].contains("direct public CLI")
                    && cells[4].contains("--edition 2009")
                    && cells[4].contains("both optimizer modes"),
                "W87 invocation must document its source-bound CLI, explicit edition, and both modes"
            );
        } else if source_bound_storage_write_case {
            assert_storage_write_remainders_cli_contract(&root, owner_body, owner);
            assert!(
                cells[4].contains("syn038_pairwise/storage_write_remainders")
                    && cells[4].contains("--edition 2009")
                    && cells[4].contains("optimized and `--no-opt`"),
                "W88 invocation must document its fixture, explicit edition, and both modes"
            );
        } else if source_bound_static_return_ref_case {
            assert_static_return_ref_actual_cli_contract(&root, owner_body, owner);
            assert!(
                cells[4].contains("syn038_pairwise/static_return_ref_actual")
                    && cells[4].contains("--edition 2009")
                    && cells[4].contains("optimized and `--no-opt`"),
                "W89 invocation must document its fixture, explicit edition, and both modes"
            );
        } else if source_bound_operation_context_case {
            assert_operation_context_matrix_cli_contract(&test_source, owner_body, owner);
            assert!(
                cells[4].contains("syn038_pairwise/operation_context_matrix")
                    && cells[4].contains("--top tb --edition 2009")
                    && cells[4].contains("optimized and `--no-opt`"),
                "W98 invocation must document its fixture, top, explicit edition, and both modes"
            );
        } else if source_bound_same_root_case {
            assert_manual_cli_contract(
                &root,
                owner_body,
                owner,
                manual_witness.expect("W106 retains the exact-output manual CLI contract"),
            );
            assert!(
                cells[4].contains("syn038_pairwise/co_same_root_assignment_rhs")
                    && cells[4].contains("--top tb --edition 2009")
                    && cells[4].contains("optimized and `--no-opt`")
                    && cells[4].contains("source-identity equality"),
                "W106 invocation must document its fixture, top, edition, modes, and DB binding proof"
            );
        } else if source_bound_read_only_ref_case {
            assert_read_only_ref_continuous_variable_cli_contract(
                &root,
                &test_source,
                owner_body,
                owner,
            );
            assert!(
                cells[4].contains("syn038_pairwise/ref_read_continuous_variable")
                    && cells[4].contains("--edition 2009")
                    && cells[4].contains("optimized and `--no-opt`")
                    && cells[4].contains("#1` precedes the call"),
                "W123 invocation must document its source-bound fixture, settle, edition, and both modes"
            );
        } else if source_bound_static_return_case {
            assert_static_return_continuous_cli_contract(&root, &test_source, owner_body, owner);
            assert!(
                cells[4].contains("--edition 2009")
                    && cells[4]
                        .contains("helper chain `assert_exact_cli` → `invoke` → `fixture_path`"),
                "W66 invocation must document its explicit edition and exact helper chain"
            );
        } else if source_bound_op_consumer_case {
            assert_op_consumer_source_bound_cli_contract(&root, &test_source, owner_body, owner);
            assert!(
                cells[4].contains("--edition 2009")
                    && cells[4].contains("explicit public-CLI runs with and without optimization"),
                "W69 invocation must document the explicit edition and both CLI modes"
            );
        } else {
            let suite_is_local_constant =
                test_source.contains(&format!("const SUITE: &str = \"{suite}\";"));
            assert!(
                (owner_body.contains(&format!("\"{suite}\"")) || suite_is_local_constant)
                    && owner_body.contains(&format!("\"{case}\"")),
                "{} fixture invocation is missing from {owner}: {invocation}",
                cells[0]
            );
            if let Some(witness) = manual_witness {
                assert_manual_cli_contract(&root, owner_body, owner, witness);
            } else {
                assert!(
                    owner_body.contains("sim_cli::run_case"),
                    "{} owner must use the exact-output public-CLI helper: {owner}",
                    cells[0]
                );
            }
            if default_edition_case {
                assert!(
                    cells[4].contains(
                        "default edition SystemVerilog-2009, with no `--edition` argument"
                    ),
                    "{} must document the CLI default edition precisely",
                    cells[0]
                );
                assert!(
                    !owner_body.contains("--edition"),
                    "{} owner must leave the CLI edition at its documented default: {owner}",
                    cells[0]
                );
            } else {
                assert!(
                    owner_bodies.iter().any(|body| owner_passes_cli_arg(
                        body,
                        &test_source,
                        "--edition"
                    )),
                    "{} owner must pass an explicit edition to the public CLI: {owner}",
                    cells[0]
                );
            }
        }
        let edition_spans = code_spans(cells[4])
            .filter(|span| span.starts_with("--edition "))
            .collect::<Vec<_>>();
        assert!(
            generated_conversion_case
                || default_edition_case
                || source_bound_cli_case
                || !edition_spans.is_empty(),
            "{} invocation must name explicit edition argument(s): {}",
            cells[0],
            cells[4]
        );
        for span in edition_spans {
            let (_, edition) = span
                .split_once(' ')
                .expect("edition span has flag and value");
            assert!(
                source_bound_cli_case
                    || owner_bodies.iter().any(|body| owner_passes_cli_arg(
                        body,
                        &test_source,
                        edition
                    )),
                "{} edition {edition} is not passed by {owner}",
                cells[0]
            );
        }
        for span in code_spans(cells[4])
            .filter(|span| span.starts_with("--") && span.contains(' '))
            .filter(|_| !source_bound_cli_case)
        {
            let (flag, argument) = span
                .split_once(' ')
                .unwrap_or_else(|| panic!("{} malformed CLI argument span {span:?}", cells[0]));
            assert!(
                owner_bodies.iter().any(|body| {
                    owner_passes_cli_arg(body, &test_source, flag)
                        && owner_passes_cli_arg(body, &test_source, argument)
                }),
                "{} CLI arguments {flag} {argument} are not passed by {owner}",
                cells[0]
            );
        }
        assert!(
            cells[5].contains("stdout") && cells[5].contains("stderr"),
            "{} oracle must state exact stdout and stderr expectations",
            cells[0]
        );
        for expected in code_spans(cells[5]) {
            let generated_finish_diagnostic = matches!(cells[0], "SYN038-W12" | "SYN038-W14")
                && expected == "llg: $finish at time 0 at tb:12:5\\n"
                && test_source.contains("at tb:{finish_line}:5\\n")
                && test_source.contains(case)
                && test_source.contains("12,");
            let generated_context_oracle =
                computed_context_oracle_matches(cells[0], expected, &test_source);
            let source_bound_read_only_ref_oracle =
                read_only_ref_continuous_warning_oracle_matches(cells[0], expected, &test_source);
            let source_bound_initializer_oracle =
                interface_runtime_initializer_warning_oracle_matches(
                    cells[0],
                    expected,
                    &test_source,
                );
            let source_bound_storage_warning_oracle =
                storage_write_remainders_warning_oracle_matches(cells[0], expected, &test_source);
            let source_bound_return_ref_oracle =
                static_return_ref_actual_finish_oracle_matches(cells[0], expected, &test_source);
            let generated_path_oracle =
                static_return_continuous_path_oracle_matches(cells[0], expected);
            let diagnostic_classification = cells[0] == "SYN038-W66"
                && expected == "NoReturnStatement"
                && cells[5].contains("Slang `NoReturnStatement`");
            assert!(
                test_source.contains(expected)
                    || generated_finish_diagnostic
                    || generated_context_oracle
                    || source_bound_read_only_ref_oracle
                    || source_bound_initializer_oracle
                    || source_bound_storage_warning_oracle
                    || source_bound_return_ref_oracle
                    || generated_path_oracle
                    || diagnostic_classification,
                "{} expected oracle {expected:?} is not present or derived by the owning test in {test_file}",
                cells[0]
            );
        }
    }
}

#[test]
fn escaped_identifier_grammar_fixture_executes_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "review_bundle",
            "r12_escaped_identifier",
            "dot=1 under=0 dash=1\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn constant_time_literal_parameter_fixture_executes() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r12_time_literal_parameter",
        "param=2\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn attribute_and_pragma_fixture_executes() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r12_attribute_pragma",
        "attribute_pragma=1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn record_conditional_two_state_nba_context_executes() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r12_record_conditional_2state_nba",
        "record_nba=xx,0\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_record_grid_executes_across_three_unpack_dimensions() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r12_record_grid_3d",
        "record_grid3d=7,56 flags=0,1\n",
        "llg: $finish at time 0 at tb:21:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_record_array_member_and_return_context_executes() {
    sim_cli::run_case_with_args(
        "rtl_completion",
        "fixed_struct_calls",
        "source=2,9,2,3\nresult=f,1,5,3\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn hierarchical_task_and_function_calls_execute_per_instance() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r12_hierarchical_subroutines",
        "hier=6,2,12,6,25\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn owned_record_array_database_lowers_after_snapshot_drop() {
    sim_harness::with_frontend_temp_cwd("syn038-record-grid-snapshot-drop", |dir| {
        let compiled = llg::core::compile::compile_sources_checked(
            &[llg::core::compile::OwnedSource::compilation_unit(
                "r12_record_grid_3d.sv",
                include_str!("fixtures/sim/review_bundle/r12_record_grid_3d.sv"),
            )],
            &llg::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                edition: llg::core::compile::LanguageEdition::SystemVerilog2009,
                ..Default::default()
            },
        )
        .map_err(|error| format!("compile: {error}"))?;
        let database = llg::core::db::Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("owned record database: {error}"))?;
        drop(compiled);
        database
            .validate()
            .map_err(|error| format!("database after snapshot drop: {error}"))?;
        for (variant, options) in [
            ("unoptimized", llg::sim::opt::OptConfig::none()),
            ("optimized", llg::sim::opt::OptConfig::default()),
        ] {
            let model = llg::sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = llg::sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} generated-model build: {error}"))?;
            let output = sim_harness::run_executable_output(&executable)
                .map_err(|error| format!("{variant} execution: {error}"))?;
            if output.stdout != b"record_grid3d=7,56 flags=0,1\n" {
                return Err(format!("{variant} stdout: {:?}", output.stdout));
            }
            if output.stderr != b"llg: $finish at time 0 at tb:21:5\n" {
                return Err(format!("{variant} stderr: {:?}", output.stderr));
            }
        }
        Ok(())
    })
    .expect("owned record array executes after native snapshot drop");
}

#[test]
fn edition_gates_match_sv_only_boundaries_and_witnesses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("tests/syn038_coverage_ledger.md"))
        .expect("read the SYN-038 coverage ledger");
    let ledger = section(&document, LEDGER_START, LEDGER_END);

    // These rows either name an SV-only production or use a witness whose
    // syntax is SV-only. Keep their edition gate strict so a shared V/SV label
    // cannot accidentally make the PASS evidence look legal in V2001.
    let sv_only_rows = [
        "SYN038-CORE-LX-02",
        "SYN038-CORE-LX-03",
        "SYN038-CORE-LX-05",
        "SYN038-CORE-LX-06",
        "SYN038-CORE-TY-01",
        "SYN038-CORE-TY-02",
        "SYN038-CORE-TY-03",
        "SYN038-CORE-TY-04",
        "SYN038-CORE-TY-09",
        "SYN038-CORE-EX-01",
        "SYN038-CORE-EX-05",
        "SYN038-CORE-EX-06",
        "SYN038-CORE-EX-07",
        "SYN038-CORE-EX-08",
        "SYN038-CORE-AS-01",
        "SYN038-CORE-AS-02",
        "SYN038-CORE-AS-03",
        "SYN038-CORE-AS-09",
        "SYN038-CORE-PR-02",
        "SYN038-CORE-PR-07",
        "SYN038-CORE-PR-08",
        "SYN038-CORE-SB-01",
        "SYN038-CORE-SB-02",
        "SYN038-CORE-SB-04",
        "SYN038-CORE-SB-07",
        "SYN038-CORE-SB-08",
        "SYN038-CORE-HY-01",
        "SYN038-CORE-HY-02",
        "SYN038-CORE-HY-03",
        "SYN038-CORE-HY-04",
        "SYN038-CORE-HY-05",
        "SYN038-CORE-HY-06",
        "SYN038-CORE-HY-10",
        "SYN038-CORE-ED-05",
    ];
    for id in sv_only_rows {
        let line = ledger
            .lines()
            .find(|line| line.starts_with(&format!("| {id} |")))
            .unwrap_or_else(|| panic!("missing edition-audit row: {id}"));
        let cells = table_cells(line);
        assert_eq!(cells.len(), 7, "malformed edition-audit row: {line}");
        assert_eq!(cells[1], "SV2009", "{id} overclaims V2001 legality");
    }

    // A V2001/SV2009 PASS row may use a .sv witness only when that witness
    // has been manually audited as dual-edition syntax. New mixed-edition
    // rows must use a .v witness until they receive the same review.
    let dual_edition_sv_witnesses = [
        "SYN038-CORE-LX-01",
        "SYN038-CORE-LX-04",
        "SYN038-CORE-EX-02",
        "SYN038-CORE-PR-01",
        "SYN038-CORE-PR-04",
        "SYN038-CORE-PR-09",
        "SYN038-CORE-PR-10",
        "SYN038-CORE-SB-06",
        "SYN038-CORE-PI-01",
        "SYN038-CORE-PI-03",
        "SYN038-CORE-PI-04",
    ];
    for line in ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-CORE-") && line.contains("| V2001/SV2009 |"))
    {
        let cells = table_cells(line);
        let id = cells[0];
        for path in fixture_paths(cells[4]) {
            if path.extension().and_then(|extension| extension.to_str()) == Some("sv") {
                assert!(
                    dual_edition_sv_witnesses.contains(&id),
                    "{id} uses an unaudited .sv witness for a V2001/SV2009 row"
                );
            }
        }
    }
}

#[test]
fn exclusions_and_context_axes_are_explicit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("tests/syn038_coverage_ledger.md"))
        .expect("read the SYN-038 coverage ledger");
    let ledger = section(&document, LEDGER_START, LEDGER_END);

    let manifest_bytes = fs::read(root.join("tests/syn038_pairwise.json"))
        .expect("read the compact SYN-038 pairwise source");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).expect("parse the compact SYN-038 pairwise source");
    let factor_ids = manifest["factors"]
        .as_object()
        .unwrap_or_else(|| panic!("SYN-038 manifest must define factor IDs"))
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    assert!(
        !factor_ids.is_empty(),
        "SYN-038 factor denominator is empty"
    );
    let factor_table = section(
        ledger,
        "| Factor | Semantic distinction | Finite levels in the v4 checker |",
        "Edition, compilation-unit policy, optimizer mode",
    );
    let documented_factor_rows = factor_table
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells = table_cells(line);
            assert_eq!(cells.len(), 3, "malformed SYN-038 factor row: {line}");
            cells[0].trim_matches('`').to_owned()
        })
        .collect::<Vec<_>>();
    let documented_factor_ids = documented_factor_rows
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(
        documented_factor_ids.len(),
        documented_factor_rows.len(),
        "SYN-038 factor table contains duplicate IDs"
    );
    assert_eq!(
        documented_factor_ids, factor_ids,
        "SYN-038 factor table must match the compact source IDs"
    );
    let normalized_ledger = ledger.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized_ledger.contains("Edition, compilation-unit policy, optimizer mode")
            && normalized_ledger.contains("native snapshot drop probe"),
        "SYN-038 validation qualifiers and snapshot-drop scope must remain explicit"
    );

    let exclusions = ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-EX-"))
        .collect::<Vec<_>>();
    assert!(
        exclusions.len() >= 12,
        "selected profile exclusions must remain explicit"
    );
    let mut exclusion_ids = HashSet::new();
    for line in exclusions {
        let cells = table_cells(line);
        assert_eq!(cells.len(), 5, "malformed SYN-038 exclusion row: {line}");
        assert!(
            exclusion_ids.insert(cells[0]),
            "duplicate exclusion ID: {}",
            cells[0]
        );
        assert!(
            matches!(cells[1], "V2001" | "SV2009" | "V2001/SV2009"),
            "{} has no exact exclusion edition: {}",
            cells[0],
            cells[1]
        );
        assert!(
            !cells[2].is_empty(),
            "{} has no excluded production",
            cells[0]
        );
        assert!(!cells[4].is_empty(), "{} has no exclusion reason", cells[0]);
        let paths = fixture_paths(cells[3]);
        for path in paths {
            assert!(
                root.join(&path).is_file(),
                "{} exclusion fixture does not exist: {}",
                cells[0],
                path.display()
            );
        }
    }
}

#[test]
fn all_historical_groups_have_one_disposition() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("tests/syn038_coverage_ledger.md"))
        .expect("read the SYN-038 coverage ledger");
    let ledger = section(&document, LEDGER_START, LEDGER_END);
    let disposition = ledger
        .split_once(DISPOSITION_START)
        .map(|(_, rest)| rest)
        .expect("SYN-038 disposition marker");

    let mut ids = Vec::new();
    let allowed = ["CORE", "RETAIN", "EXT", "POLICY", "CAPACITY", "OUTSIDE"];
    for line in disposition.lines().filter(|line| line.starts_with('|')) {
        let cells = table_cells(line);
        if cells.len() != 4 || cells[0] == "Old ID" || cells[0].starts_with("---") {
            continue;
        }
        let id = cells[0]
            .parse::<u8>()
            .unwrap_or_else(|_| panic!("invalid old group ID in row: {line}"));
        assert!((1..=72).contains(&id), "old group ID outside 1..=72: {id}");
        assert!(
            allowed.iter().any(|code| cells[2].contains(code)),
            "old group {id} has no plan disposition: {}",
            cells[2]
        );
        assert!(
            !cells[3].is_empty(),
            "old group {id} has no evidence boundary"
        );
        ids.push(id);
    }
    ids.sort_unstable();
    assert_eq!(ids.len(), 72, "SYN-038 must classify all 72 old groups");
    assert_eq!(ids, (1..=72).collect::<Vec<_>>());
}
