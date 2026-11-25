//! Generation does work in proportion to its source: each test counts one kind
//! of work at two sizes, so it does not depend on the host's speed.

use crate::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use crate::core::db::Db;
use crate::core::lint::{LintConfig, LintCtx, LintRegistry};
use crate::core::model::DesignModel;
use crate::sim::codegen::GeneratedModel;
use crate::sim::opt::OptConfig;
use crate::work::{all_counted, counted, Work};

fn database(source: &str) -> Db {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit("tb.sv", source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("the source compiles");
    Db::from_slang(&compiled.snapshot).expect("owned database")
}

fn generate(db: &Db) -> GeneratedModel {
    crate::sim::codegen::generate_from_db_with_opts(db, &OptConfig::default())
        .expect("the design generates")
}

/// The `work` of generating `source`, from its text to its C, on the stack a
/// compile runs on.
fn generation_work(work: Work, source: &str) -> usize {
    crate::stack::run_pipeline(|| {
        counted(work, || {
            generate(&database(source));
        })
    })
    .expect("a pipeline thread")
}

#[test]
fn only_bodies_with_a_counterpart_are_normalized() {
    let processes = |count: usize| -> String {
        (0..count)
            .map(|index| {
                format!(
                    "logic [7:0] v{index};\n\
                     function automatic int f{index}(int x); return x + v{index}; endfunction\n\
                     task automatic t{index}(); #1 v{index} = f{index}(2); endtask\n\
                     initial t{index}();\n"
                )
            })
            .collect()
    };
    let distinct = format!("module tb;\n{}endmodule\n", processes(64));
    assert_eq!(generation_work(Work::NormalizedBytes, &distinct), 0);
    let instances = |count: usize| {
        format!(
            "module cell_{count}; {}endmodule\nmodule tb;\n\
             for (genvar g = 0; g < {count}; g++) cell_{count} c();\nendmodule\n",
            processes(1)
        )
    };
    let normalized = [8, 16].map(|count| generation_work(Work::NormalizedBytes, &instances(count)));
    assert!(normalized[0] > 0);
    assert!(
        normalized[1] <= 2 * normalized[0] + normalized[0] / 4,
        "{normalized:?}"
    );
}

/// `count` pieces in a row, each made of its index.
fn row(count: usize, piece: impl Fn(usize) -> String) -> String {
    (0..count).map(piece).collect()
}

/// A process that runs `body` in a module with `items`.
fn bench(items: &str, body: &str) -> String {
    format!("`timescale 1ns/1ns\nmodule tb; {items} initial begin {body} end endmodule\n")
}

/// What a source of one construct nested or chained `count` deep is, by name.
fn shape(name: &str, count: usize) -> String {
    let show = "$display(\"x\");";
    let nested = |open: &str, close: &str| {
        bench(
            "int a = 1;",
            &format!("{}{show}{}", open.repeat(count), close.repeat(count)),
        )
    };
    let numbered =
        |open: &dyn Fn(usize) -> String| bench("int a[1];", &format!("{}{show}", row(count, open)));
    match name {
        "for" => numbered(&|i| format!("for (int i{i} = 0; i{i} < 1; i{i}++) ")),
        "foreach" => numbered(&|i| format!("foreach (a[i{i}]) ")),
        "fork" => nested("fork begin ", "end join "),
        "if" => nested("if (a) ", ""),
        "else if" => bench(
            "int a = 1;",
            &row(count, |i| format!("if (a == {i}) $display(\"{i}\"); else ")),
        )
        .replace("else  end", "else ; end"),
        "case" => nested("case (a) 1: ", " endcase"),
        "randcase" => nested("randcase 1: ", " endcase"),
        "repeat" => nested("repeat (1) ", ""),
        "wait" => nested("wait (a) ", ""),
        "begin" => nested("begin ", " end"),
        "statements" => nested("a = a + 1; ", ""),
        "conditional records" => bench(
            "typedef struct { int a; string s; } r; logic s; r d0, d1, y;",
            &format!("y = {}d1;", "s ? d0 : ".repeat(count)),
        ),
        "record calls" => bench(
            "typedef struct { int a; string s; } r; logic s; r d0, d1, y; \
             function automatic r f(input r v); return v; endfunction",
            &format!(
                "y = {}d1{};",
                "s ? d0 : f(".repeat(count),
                ")".repeat(count)
            ),
        ),
        "conditional" => bench(
            "int a, y;",
            &format!("y = {}7;", row(count, |i| format!("a == {i} ? {i} : "))),
        ),
        "concatenation" => bench(
            "string p, y;",
            &format!("y = {}p{};", "{".repeat(count), ", p}".repeat(count)),
        ),
        "handles" => bench(
            "class c; c n; int v; endclass c o = new;",
            &format!("$display(o{}.v);", ".n".repeat(count)),
        ),
        "tasks" => bench(
            &format!(
                "int y; {} task automatic t{count}(input int x); #1 y = x; endtask",
                row(count, |i| format!(
                    "task automatic t{i}(input int x); #1 t{}(x); endtask ",
                    i + 1
                ))
            ),
            "t0(1);",
        ),
        "references" => bench(
            &format!(
                "int y; {} task automatic t{count}(ref int x); #1 x = 1; endtask",
                row(count, |i| format!(
                    "task automatic t{i}(ref int x); #1 t{}(x); endtask ",
                    i + 1
                ))
            ),
            "t0(y);",
        ),
        "functions called twice" => bench(
            &format!(
                "int y; {} function automatic int f{count}(int x); y = x; return x; endfunction",
                row(count, |i| format!(
                    "function automatic int f{i}(int x); return f{n}(x) + f{n}(x); endfunction ",
                    n = i + 1
                ))
            ),
            "y = f0(1);",
        ),
        "unpacked dimensions" => bench(
            &format!("int v{};", "[1]".repeat(count)),
            &format!("v{0} = 3; $display(v{0});", "[0]".repeat(count)),
        ),
        "packed dimensions" => bench(
            &format!("logic {} v;", "[0:0]".repeat(count)),
            &format!("v{0} = 1; $display(v{0});", "[0]".repeat(count)),
        ),
        _ => panic!("no shape {name}"),
    }
}

fn lint(db: &Db) {
    let model = DesignModel::from_db(db);
    LintRegistry::default_rules().lint(&LintCtx::new(db, &model), &LintConfig::default());
}

/// All work of linting `source` and generating it.
fn all_work(source: &str) -> Vec<(Work, usize)> {
    crate::stack::run_pipeline(|| {
        all_counted(|| {
            let db = database(source);
            lint(&db);
            generate(&db);
        })
    })
    .expect("a pipeline thread")
}

/// Which kinds of work of a shape grow by more than `limit` times when it is
/// twice as deep. Passes over the generated text are left out: the text has an
/// indentation, and names of nested frame blocks, of the depth they are at.
fn steep(name: &str, depth: usize, limit: f64) -> Vec<String> {
    let [small, large] = [depth, 2 * depth].map(|depth| all_work(&shape(name, depth)));
    small
        .into_iter()
        .zip(large)
        .filter(|((work, _), _)| {
            !matches!(
                work,
                Work::NameScanBytes | Work::FieldLookups | Work::OutlineWeighing
            )
        })
        .filter(|((_, small), (_, large))| *large as f64 > limit * (*small as f64) + 64.0)
        .map(|((work, small), (_, large))| format!("{name}: {work:?} {small} to {large}"))
        .collect()
}

/// Twice the depth is at most this much work: it is twice as much, or with a
/// logarithm, and the paths of scopes in the VPI catalog are as long as the
/// scopes are deep.
const DEEP_GROWTH: f64 = 2.75;

#[test]
fn work_grows_with_the_depth_of_nested_source() {
    let steep = [
        "for",
        "foreach",
        "fork",
        "if",
        "else if",
        "case",
        "randcase",
        "repeat",
        "wait",
        "begin",
        "statements",
        "conditional records",
        "record calls",
        "conditional",
        "concatenation",
        "handles",
        "tasks",
        "references",
        "functions called twice",
    ]
    .into_iter()
    .flat_map(|name| steep(name, 64, DEEP_GROWTH))
    .collect::<Vec<_>>();
    assert!(steep.is_empty(), "{steep:#?}");
}

/// The database keeps the dimensions of every part of a type, and the indices
/// of every part of a select, so these grow with the square of the dimensions.
#[test]
fn work_grows_with_the_square_of_the_dimensions_at_most() {
    let steep = ["unpacked dimensions", "packed dimensions"]
        .into_iter()
        .flat_map(|name| steep(name, 32, 4.4))
        .collect::<Vec<_>>();
    assert!(steep.is_empty(), "{steep:#?}");
}

#[test]
fn outlining_weighs_a_bounded_part_below_every_statement() {
    let [small, large] = [200, 400].map(|depth| {
        let source = shape("for", depth);
        generation_work(Work::OutlineWeighing, &source)
    });
    assert!(small > 0);
    assert!(
        large as f64 <= DEEP_GROWTH * small as f64,
        "{small} to {large}"
    );
}

#[test]
fn a_valid_design_describes_no_node() {
    let source = shape("tasks", 16);
    assert_eq!(generation_work(Work::NodeDescriptions, &source), 0);
}

#[test]
fn the_lint_rules_share_their_lists_of_nodes() {
    let db = database(&format!(
        "module leaf(input logic [3:0] a, output logic [3:0] y); \
         always_comb case (a) 0: y = 1; default: y = a; endcase endmodule\n\
         module tb; {} endmodule\n",
        row(32, |i| format!(
            "logic [3:0] a{i}, y{i}; leaf u{i}(.a(a{i}), .y(y{i}));\n"
        ))
    ));
    let nodes = db.node_ids().count();
    let listed = counted(Work::ListedNodes, || lint(&db));
    assert!(
        listed <= 2 * nodes,
        "{listed} nodes listed of {nodes} in the design"
    );
}

/// A process that declares, calls and waits `count` times.
fn long_process(count: usize) -> String {
    bench(
        "int y; task automatic t(input int x); #1 y = x; endtask",
        &row(count, |i| {
            format!("begin automatic int v{i} = y; t(v{i}); #1 y = y + v{i}; end\n")
        }),
    )
}

/// The `work` of generating `db` and the length of its C.
fn work_and_text(work: Work, db: &Db) -> (usize, usize) {
    let mut text = 0;
    let amount = counted(work, || text = generate(db).model_c.len());
    (amount, text)
}

#[test]
fn a_coroutine_body_is_searched_a_fixed_number_of_times() {
    // The names of the runtime are found once, with the first model.
    generate(&database(&long_process(1)));
    for count in [20, 200] {
        let db = database(&long_process(count));
        let (searched, text) = work_and_text(Work::NameScanBytes, &db);
        assert!(
            searched <= 4 * text,
            "{searched} bytes searched of {text} for {count} calls"
        );
    }
}

#[test]
fn few_names_that_are_no_frame_field_are_looked_up_as_one() {
    for count in [20, 200] {
        let db = database(&long_process(count));
        let (lookups, text) = work_and_text(Work::FieldLookups, &db);
        assert!(
            lookups <= 100 * count,
            "{lookups} lookups in {text} bytes for {count} calls"
        );
    }
}
