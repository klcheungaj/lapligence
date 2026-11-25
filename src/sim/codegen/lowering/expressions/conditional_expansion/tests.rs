//! A conditional assigned by statements is lowered in proportion to its
//! source, however deep its operators nest.
use super::{CONDITIONAL_ARGUMENT_LEVELS, CONDITIONAL_EXPANSION_LEVELS};
use crate::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use crate::core::db::Db;
use crate::sim::codegen::generate;

/// The stack of a test thread is far below the one of a run, and lowering
/// recurses per level.
const DEEP_STACK_BYTES: usize = 256 << 20;

/// The C of a task that assigns `expression` to `target`, over the
/// alternatives `d0`..`d{count - 1}` of type `declare` and predicates `s`.
fn assignment(declare: &str, count: usize, expression: &str) -> String {
    let declarations: String = (0..count)
        .map(|index| format!("  {};\n", declare.replace('@', &format!("d{index}"))))
        .collect();
    let source = format!(
        "module tb;\n  typedef struct {{ int a; string t; }} rec_t;\n  logic [63:0] s;\n\
         {declarations}  {};\n\
         function automatic rec_t f(input rec_t v); return v; endfunction\n\
         function automatic logic p(input rec_t v); return v.a > 0; endfunction\n\
         task automatic go(); y = {expression}; endtask\n\
         initial go();\nendmodule\n",
        declare.replace('@', "y")
    );
    std::thread::Builder::new()
        .stack_size(DEEP_STACK_BYTES)
        .spawn(move || generated(&source))
        .expect("test thread")
        .join()
        .expect("the design generates")
}

/// The C of the task of `source`.
fn generated(source: &str) -> String {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit("tb.sv", source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("the design compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    let model = generate(&database).expect("the design generates");
    let start = model
        .model_c
        .find("static void fn_tb_go(int depth) {")
        .expect("the task");
    model.model_c[start..]
        .split("\n}\n")
        .next()
        .expect("the body")
        .to_owned()
}

/// `s[0] ? d0 : s[1] ? d1 : ... : d{levels}`.
fn chain(levels: usize) -> String {
    (0..levels)
        .map(|level| format!("s[{level}] ? d{level} : "))
        .chain([format!("d{levels}")])
        .collect()
}

/// `s[0] ? (s[1] ? (...) : d1) : d0`.
fn first_chain(levels: usize) -> String {
    (0..levels)
        .rev()
        .fold(format!("d{levels}"), |inner, level| {
            format!("s[{level}] ? ({inner}) : d{level}")
        })
}

const QUEUE: &str = "int @ [$]";
const DYNAMIC: &str = "int @ []";
const ASSOCIATIVE: &str = "int @ [string]";
const RECORD: &str = "rec_t @";

/// How often the C names signal or container `name`.
fn uses(code: &str, name: &str) -> usize {
    code.match_indices(name)
        .filter(|(at, _)| !code[at + name.len()..].starts_with(|next: char| next.is_ascii_digit()))
        .count()
}

#[test]
fn twice_the_levels_are_at_most_twice_the_code() {
    for declare in [QUEUE, DYNAMIC, ASSOCIATIVE, RECORD] {
        for build in [chain as fn(usize) -> String, first_chain] {
            let size = |levels: usize| assignment(declare, levels + 1, &build(levels)).len();
            let (eight, sixteen, thirty_two) = (size(8), size(16), size(32));
            assert!(
                sixteen * 10 <= eight * 22 && thirty_two * 10 <= sixteen * 22,
                "{declare}: {eight}, {sixteen}, {thirty_two} bytes"
            );
        }
    }
}

#[test]
fn every_alternative_is_lowered_a_fixed_number_of_times() {
    for declare in [QUEUE, DYNAMIC, ASSOCIATIVE] {
        for build in [chain as fn(usize) -> String, first_chain] {
            for levels in [CONDITIONAL_EXPANSION_LEVELS as usize + 1, 16, 32] {
                let code = assignment(declare, levels + 1, &build(levels));
                for level in 0..=levels {
                    // For known predicates, as the first and as a further one
                    // of ambiguous ones, and where its conditional holds no
                    // other, for that alone.
                    let uses = uses(&code, &format!("&G_tb_d{level}"));
                    assert!(
                        (2..=4).contains(&uses),
                        "{declare}, {levels} levels, alternative {level}: {uses}"
                    );
                }
            }
        }
    }
}

/// Short conditionals repeat their alternatives; longer ones have their
/// temporaries after every known predicate.
#[test]
fn a_known_predicate_creates_no_temporary() {
    for levels in 1..=CONDITIONAL_EXPANSION_LEVELS as usize {
        let code = assignment(QUEUE, levels + 1, &chain(levels));
        assert!(!code.contains("_llg_cond_cold_"), "{levels} levels");
        assert_eq!(uses(&code, "&G_tb_d0"), 2);
        assert_eq!(uses(&code, &format!("&G_tb_d{levels}")), 1 << levels);
    }
    let code = assignment(QUEUE, 17, &chain(16));
    let (known, cold) = code
        .split_once("_llg_cond_cold_")
        .map(|_| code.rsplit_once("_llg_cond_cold_").expect("the label"))
        .expect("the cold path");
    // The last conditional merges its two alternatives by itself.
    assert_eq!(uses(known, "llg_value_scope_begin_object"), 2);
    assert_eq!(uses(known, "llg_value_scope_begin(1)"), 0);
    assert!(uses(cold, "llg_value_scope_begin_object") > 16);
}

/// A sequence nests no block per level, so the block depth a C compiler
/// accepts does not bound it.
#[test]
fn a_chain_of_any_length_nests_the_same_blocks() {
    let depth = |levels: usize| {
        let code = assignment(QUEUE, levels + 1, &chain(levels));
        let mut depth = 0i32;
        code.chars()
            .map(|next| {
                depth += i32::from(next == '{') - i32::from(next == '}');
                depth
            })
            .max()
    };
    assert_eq!(depth(8), depth(16));
    assert_eq!(depth(8), depth(32));
}

/// `s[0] ? d0 : f(s[1] ? d1 : f(...))`.
fn arguments(levels: usize, at: usize) -> String {
    if at == levels {
        return format!("d{at}");
    }
    format!("s[{at}] ? d{at} : f({})", arguments(levels, at + 1))
}

/// An alternative that holds conditionals of its own is lowered once, so
/// nesting through call arguments does not double either.
#[test]
fn conditionals_in_call_arguments_do_not_double() {
    let size = |levels: usize| assignment(RECORD, levels + 1, &arguments(levels, 0)).len();
    let first = CONDITIONAL_EXPANSION_LEVELS as usize + 2;
    let (five, eight, sixteen) = (size(first), size(8), size(16));
    assert!(sixteen * 10 <= eight * 22, "{eight}, {sixteen} bytes");
    assert!(eight * 10 <= five * 13, "{five}, {eight} bytes");
}

/// An alternative lowered once beside a variable needs no `kept`: the target
/// of an argument receives it, and a design variable takes it from the
/// temporary that a merge reads.
#[test]
fn an_alternative_lowered_once_beside_a_variable_is_not_kept() {
    for levels in [CONDITIONAL_EXPANSION_LEVELS as usize + 2, 8, 12] {
        let code = assignment(RECORD, levels + 1, &arguments(levels, 0));
        assert!(!code.contains("_llg_cond_cold_"), "{levels} levels");
        let once = levels - CONDITIONAL_EXPANSION_LEVELS as usize - 1;
        let labels = |name: &str| {
            let mut found: Vec<_> = code
                .match_indices(name)
                .map(|(at, _)| code[at..].split([':', ';', ' ']).next())
                .collect();
            found.sort();
            found.dedup();
            found.len()
        };
        assert_eq!(labels("_llg_cond_single_"), once, "{levels} levels");
    }
}

/// Other alternatives that hold many conditionals are repeated up to
/// [`CONDITIONAL_ARGUMENT_LEVELS`], where lowering them once would cost a known
/// predicate `kept`.
#[test]
fn an_alternative_beside_a_call_is_repeated_up_to_the_argument_limit() {
    let expression = |levels: usize| format!("s[0] ? f(d0) : f({})", arguments(levels, 1));
    let limit = CONDITIONAL_ARGUMENT_LEVELS as usize;
    let code = assignment(RECORD, limit + 2, &expression(limit));
    assert!(!code.contains("_llg_cond_cold_"));
    let code = assignment(RECORD, limit + 2, &expression(limit + 1));
    assert!(code.contains("_llg_cond_cold_"));
}

/// Nor do conditionals in predicates.
#[test]
fn conditionals_in_predicates_do_not_double() {
    let levels = 16;
    let expression: String = (0..levels)
        .map(|at| format!("p(s[{at}] ? d{at} : d{}) ? d{at} : ", at + 1))
        .chain([format!("d{levels}")])
        .collect();
    let code = assignment(RECORD, levels + 1, &expression);
    let deep = expression.replace("p(s[0] ? d0 : d1)", &format!("p({})", chain(8)));
    assert!(uses(&code, "&G_tb_s") <= 2 * levels);
    let code = assignment(RECORD, levels + 1, &deep);
    assert!(uses(&code, "&G_tb_s") <= 2 * (levels + 8));
}
