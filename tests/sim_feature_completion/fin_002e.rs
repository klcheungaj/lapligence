//! FIN-002 step 4: conditional operator chains (IEEE 1800-2009 11.4.11) and
//! deeply nested designs.
//!
//! A chain of `?:` operators used to emit C that doubled with every level.
//! The fixtures run chains 4 to 32 levels deep with known, X and Z
//! predicates at every position; `fin_002e/readme.md` states where each
//! expected transcript comes from.
//!
//! | Fixture | What it holds |
//! | --- | --- |
//! | `cond_chain_packed` | priority chains in a continuous assignment, a procedure and a function; a two-state chain |
//! | `cond_chain_effects` | which predicates and alternatives are evaluated, and in which order |
//! | `cond_chain_types` | wide, signed, mixed-width, two-state, real, string, enumeration and packed structure operands |
//! | `cond_chain_shapes` | conditionals nested in the first alternative, in both, in the predicate; mixed types along a chain |
//! | `cond_chain_clause` | unpacked arrays and structures, class handles, reals and increments under the clause text |
//! | `deep_nesting` | every construct nested tens to hundreds of levels deep; `deep_nesting` in the `general` binary goes up to the limit without building |
//! | `call_diamonds` | functions that reach the last one along 2^30 call paths, in every context that looks into bodies |
//! | `task_diamonds` | tasks that reach the last one along 2^30 call paths: the C compiler's time does not follow them |
//! | `recursive_specializations` | classes and a module that name themselves with other parameter values until the chain ends |
use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/fin_002e";

/// Upper bound on the size of one fixture's `model.c`. The largest holds
/// under half a megabyte; with code that doubles per level the 32-level
/// chains alone would need more than the address space.
const MODEL_SOURCE_LIMIT: usize = 1 << 20;

/// The fixtures end when no process remains; this is the run-time report.
const ENDED_AT_0: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 0\n";
const ENDED_AT_312: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 312\n";

macro_rules! expected {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/fin_002e/",
            $name,
            ".out"
        ))
    };
}

#[test]
fn priority_chains_select_and_merge_at_every_depth() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_packed",
        expected!("cond_chain_packed"),
        ENDED_AT_312,
        &[],
    );
}

#[test]
fn chain_alternatives_are_evaluated_once_in_source_order() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_effects",
        expected!("cond_chain_effects"),
        ENDED_AT_0,
        &[],
    );
}

#[test]
fn chains_keep_the_type_of_their_operands() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cond_chain_types",
        expected!("cond_chain_types"),
        &[],
        &[],
    );
}

#[test]
fn nested_conditionals_of_every_shape_agree() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_shapes",
        expected!("cond_chain_shapes"),
        ENDED_AT_0,
        &[],
    );
}

#[test]
fn aggregate_handle_and_real_chains_follow_the_clause() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cond_chain_clause",
        expected!("cond_chain_clause"),
        &[],
        &[],
    );
}

/// A model whose generation used to overflow the stack builds and runs. The C
/// compiler's optimizer is not under test, and takes longer than the rest.
#[test]
fn deeply_nested_constructs_build_and_run() {
    sim_cli::run_case_with_args(
        SUITE,
        "deep_nesting",
        expected!("deep_nesting"),
        "",
        &[],
        &["--model-opt-level", "O1"],
    );
}

/// The deepest nesting of structure and union definitions in C `source`,
/// counting the outermost.
fn definition_depth(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut open = Vec::new();
    let (mut depth, mut deepest) = (0usize, 0usize);
    // After `struct` or `union` and an optional tag, a brace opens a
    // definition.
    let (mut definition, mut tag) = (false, false);
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if source[at..].starts_with("/*") {
            at += source[at..].find("*/").map_or(bytes.len(), |end| end + 2);
            continue;
        }
        if source[at..].starts_with("//") {
            at += source[at..].find('\n').unwrap_or(bytes.len() - at);
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            at += 1;
            while at < bytes.len() && bytes[at] != byte {
                at += if bytes[at] == b'\\' { 2 } else { 1 };
            }
            at += 1;
            (definition, tag) = (false, false);
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let end = source[at..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map_or(bytes.len(), |end| at + end);
            let word = &source[at..end];
            (definition, tag) = if word == "struct" || word == "union" {
                (true, true)
            } else {
                (definition && tag, false)
            };
            at = end;
            continue;
        }
        match byte {
            b'{' => {
                open.push(definition);
                if definition {
                    depth += 1;
                    deepest = deepest.max(depth);
                }
            }
            b'}' => {
                if open.pop() == Some(true) {
                    depth -= 1;
                }
            }
            _ if byte.is_ascii_whitespace() => {
                at += 1;
                continue;
            }
            _ => {}
        }
        (definition, tag) = (false, false);
        at += 1;
    }
    deepest
}

/// Blocks nest deeper in `deep_nesting` than C nests definitions on every
/// compiler (MSVC refuses the 65th level, C1046), so its frame names the
/// structs that would go past the portable limit; both optimizer modes keep
/// within it.
#[test]
fn deeply_nested_blocks_keep_frames_within_the_portable_definition_depth() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("deep_nesting.sv");
    for optimized in [true, false] {
        let directory = sim_harness::TempDir::new("deep-frames").expect("scratch");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .args(["--gen-only", "--top", "tb", "--out-dir"])
            .arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(command.arg(&source), Duration::from_secs(120))
            .expect("generate the deep model");
        assert!(output.status.success(), "optimized={optimized}: {output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model.c");
        assert!(
            model.contains("typedef struct {\n    llg_co_frame_t co;\n")
                && model.contains("} llg_frame_part_0_t;\n"),
            "optimized={optimized}: no frame names a struct"
        );
        let depth = definition_depth(&model);
        assert!(
            depth <= llg::sim::emit_c::MAX_NESTED_DEFINITIONS,
            "optimized={optimized}: definitions nest {depth} deep"
        );
    }
}

#[test]
fn definition_depth_counts_struct_and_union_definitions_only() {
    let source = "typedef struct tag { union { struct { int a; } s; } u; } t;\n\
                  /* struct { { */ int f(void) { if (1) { return sizeof(struct tag); } }\n\
                  const char* text = \"struct { struct {\";\n";
    assert_eq!(definition_depth(source), 3);
}

#[test]
fn recursive_specializations_that_end_run() {
    sim_cli::run_case_with_args(
        SUITE,
        "recursive_specializations",
        expected!("recursive_specializations"),
        "",
        &[],
        &[],
    );
}

/// The lines of a transcript that starts each line with its time, grouped
/// by time and sorted within a group: processes that one change wakes run in
/// an order the text leaves open (4.7, L3465-3466).
fn time_steps(text: &str) -> Vec<(&str, Vec<&str>)> {
    let mut steps: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in text.lines() {
        let time = line.split(' ').next().unwrap_or_default();
        match steps.last_mut() {
            Some((open, lines)) if *open == time => lines.push(line),
            _ => steps.push((time, vec![line])),
        }
    }
    for (_, lines) in &mut steps {
        lines.sort_unstable();
    }
    steps
}

#[test]
fn call_graphs_with_many_paths_build_and_run() {
    let expected = expected!("call_diamonds").replace("\r\n", "\n");
    for &optimized in sim_harness::optimizer_modes() {
        let (_directory, output) =
            sim_cli::run_case_keeping_dir(SUITE, "call_diamonds", optimized, &[], &[]);
        let label = format!("call_diamonds, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: {}",
            sim_cli::observed_output(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
        assert_eq!(time_steps(&stdout), time_steps(&expected), "{label}");
        assert!(super::sim_099::llg_lines(&output).is_empty(), "{label}");
    }
}

#[test]
fn task_graphs_with_many_paths_build_and_run() {
    sim_cli::run_case_with_args(
        SUITE,
        "task_diamonds",
        expected!("task_diamonds"),
        "",
        &[],
        &[],
    );
}

/// The generated C is proportional to the source: every fixture, with its
/// 32-level chains, stays far below the bound in both optimizer modes.
#[test]
fn generated_code_is_proportional_to_the_chain() {
    for fixture in [
        "cond_chain_packed",
        "cond_chain_effects",
        "cond_chain_types",
        "cond_chain_shapes",
        "cond_chain_clause",
    ] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(format!("{fixture}.sv"));
        for optimized in [true, false] {
            let directory = sim_harness::TempDir::new("conditional-chain").expect("scratch");
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .args(["--gen-only", "--top", "tb", "--out-dir"])
                .arg(directory.path());
            if !optimized {
                command.arg("--no-opt");
            }
            let output = sim_harness::run_command(command.arg(&source), Duration::from_secs(60))
                .expect("generate the conditional chain model");
            assert!(output.status.success(), "{fixture}: {output:?}");
            let model = std::fs::read(directory.path().join("sim/tb/model.c")).expect("model.c");
            assert!(
                model.len() < MODEL_SOURCE_LIMIT,
                "{fixture}, optimized={optimized}: model.c has {} bytes",
                model.len()
            );
        }
    }
}
