//! FIN-002 step 4: source that nests up to `llg::stack::MAX_NESTING_DEPTH` and
//! beyond it, long lists that nest nothing, and chains of names.
//!
//! Every construct just below the limit gets through the frontend and lint,
//! and through generation where that takes a second or so; just above it the
//! source is refused at the token that exceeds it. `llg` runs as it ships, so
//! an unoptimized run also proves that the pipeline's stack holds the limit
//! in every stage. Component tests: nothing is built or run here (a design
//! nested a few hundred deep runs in `sim_feature_completion::fin_002e`).
use crate::sim_harness;
use llg::stack::MAX_NESTING_DEPTH;
use std::process::Command;
use std::time::Duration;

/// A construct whose `levels` nest `guards` deep each.
struct Construct {
    name: &'static str,
    guards: u32,
    source: fn(usize) -> String,
}

/// Levels around the limit that a construct's fixed surroundings may take.
const MARGIN: usize = 16;

impl Construct {
    fn limit(&self) -> usize {
        (MAX_NESTING_DEPTH / self.guards) as usize
    }

    /// The deepest source all constructs of the tables accept.
    fn deep(&self) -> String {
        (self.source)(self.limit() - MARGIN)
    }

    fn too_deep(&self) -> String {
        (self.source)(self.limit() + MARGIN)
    }
}

fn module(items: &str) -> String {
    format!("`timescale 1ns/1ns\nmodule tb;\n{items}\nendmodule\n")
}

fn value(expression: &str) -> String {
    module(&format!(
        "  logic [31:0] a = 1, y;\n  initial begin\n    y = {expression};\n    $display(\"%0d\", y);\n  end"
    ))
}

fn statements(body: &str) -> String {
    module(&format!(
        "  int a = 1;\n  initial begin\n    {body}\n    $display(\"%0d\", a);\n  end"
    ))
}

fn wrap(levels: usize, open: &str, core: &str, close: &str) -> String {
    format!("{}{core}{}", open.repeat(levels), close.repeat(levels))
}

const EXPRESSIONS: &[Construct] = &[
    Construct {
        name: "parentheses",
        guards: 1,
        source: |n| value(&wrap(n, "(", "a", ")")),
    },
    Construct {
        name: "binary chain",
        guards: 1,
        source: |n| value(&format!("a{}", " + a".repeat(n))),
    },
    Construct {
        name: "right-nested binary chain",
        guards: 2,
        source: |n| value(&wrap(n, "a + (", "a", ")")),
    },
    Construct {
        name: "unary chain",
        guards: 1,
        source: |n| value(&wrap(n, "~(", "a", ")")),
    },
    Construct {
        name: "logical chain",
        guards: 1,
        source: |n| value(&format!("a{}", " && a".repeat(n))),
    },
    Construct {
        name: "conditional chain",
        guards: 1,
        source: |n| {
            let arms: String = (0..n)
                .map(|arm| format!("a == {} ? {arm} : ", arm + 2))
                .collect();
            value(&format!("{arms}7"))
        },
    },
    Construct {
        name: "concatenations",
        guards: 1,
        source: |n| value(&wrap(n, "{", "a", "}")),
    },
    Construct {
        name: "target concatenations",
        guards: 1,
        source: |n| statements(&format!("{} = 5;", wrap(n, "{", "a", "}"))),
    },
    Construct {
        name: "streaming concatenations",
        guards: 1,
        source: |n| value(&wrap(n, "{>>{", "a", "}}")),
    },
    Construct {
        name: "calls",
        guards: 4,
        source: |n| {
            module(&format!(
                "  function automatic int f(int x); return x + 1; endfunction\n  int y;\n  initial begin y = {}; $display(\"%0d\", y); end",
                wrap(n, "f(", "1", ")")
            ))
        },
    },
    Construct {
        name: "inside sets",
        guards: 2,
        source: |n| value(&wrap(n, "(a inside {", "a", "})")),
    },
    Construct {
        name: "casts",
        guards: 2,
        source: |n| value(&wrap(n, "8'(", "a", ")")),
    },
];

const STATEMENTS: &[Construct] = &[
    Construct {
        name: "begin blocks",
        guards: 1,
        source: |n| statements(&wrap(n, "begin ", "a = 2; ", "end ")),
    },
    Construct {
        name: "if statements",
        guards: 1,
        source: |n| statements(&format!("{}a = 2;", "if (a > 0) ".repeat(n))),
    },
    Construct {
        name: "else if chain",
        guards: 1,
        source: |n| {
            let arms: String = (0..n)
                .map(|arm| format!("if (a == {}) a = {arm};\n    else ", arm + 2))
                .collect();
            statements(&format!("{arms}a = 3;"))
        },
    },
    Construct {
        name: "wait statements",
        guards: 1,
        source: |n| statements(&format!("{}a = 2;", "wait (a > 0) ".repeat(n))),
    },
    Construct {
        name: "delays",
        guards: 1,
        source: |n| statements(&format!("{}a = 2;", "#1 ".repeat(n))),
    },
    Construct {
        name: "fork blocks",
        guards: 2,
        source: |n| statements(&wrap(n, "fork begin ", "a = 2; ", "end join ")),
    },
];

const DECLARATIONS: &[Construct] = &[
    Construct {
        name: "else generate chain",
        guards: 1,
        source: |n| {
            let arms: String = (0..n)
                .map(|arm| format!("  if (P == {arm}) begin : g{arm} end\n  else "))
                .collect();
            module(&format!(
                "  parameter int P = 1000000;\n{arms}initial $display(\"g\");"
            ))
        },
    },
    Construct {
        name: "nested classes",
        guards: 1,
        source: |n| {
            let open: String = (0..n).map(|level| format!("  class c{level};\n")).collect();
            module(&format!(
                "{open}  int v;\n{}  c0 h;\n  initial h = new;",
                "  endclass\n".repeat(n)
            ))
        },
    },
    Construct {
        name: "constraint implications",
        guards: 1,
        source: |n| {
            module(&format!(
                "  class c; rand int x; constraint k {{ {}x > 0; }} endclass\n  c h;\n  initial begin h = new; void'(h.randomize()); end",
                "x > 0 -> ".repeat(n)
            ))
        },
    },
    Construct {
        name: "property expressions",
        guards: 2,
        source: |n| {
            module(&format!(
                "  logic clk = 0, a = 1;\n  p: assert property (@(posedge clk) {});\n  initial #1 clk = 1;",
                wrap(n, "not (", "a", ")")
            ))
        },
    },
    Construct {
        name: "property operators",
        guards: 1,
        source: |n| {
            module(&format!(
                "  logic clk = 0, a = 1;\n  p: assert property (@(posedge clk) {}a);\n  initial #1 clk = 1;",
                "not nexttime ".repeat(n / 2)
            ))
        },
    },
    Construct {
        name: "event list",
        guards: 1,
        source: |n| {
            module(&format!(
                "  logic a = 0, y;\n  always @(a{}) y = a;\n  initial #1 a = 1;",
                " or a".repeat(n)
            ))
        },
    },
];

/// Generate blocks elaborate in time quadratic in their depth, so they nest
/// a quarter of the limit where they are accepted.
const GENERATES: &[Construct] = &[
    Construct {
        name: "if generate blocks",
        guards: 1,
        source: |n| {
            module(&wrap(
                n,
                "if (1) begin ",
                "initial $display(\"g\"); ",
                "end ",
            ))
        },
    },
    Construct {
        name: "case generate blocks",
        guards: 1,
        source: |n| {
            module(&wrap(
                n,
                "case (1) 1: begin ",
                "initial $display(\"g\"); ",
                "end endcase ",
            ))
        },
    },
    Construct {
        name: "loop generate blocks",
        guards: 1,
        source: |n| {
            let open: String = (0..n)
                .map(|level| format!("for (genvar i{level} = 0; i{level} < 1; i{level}++) begin\n"))
                .collect();
            module(&format!(
                "{open}initial $display(\"g\"); {}",
                "end ".repeat(n)
            ))
        },
    },
];

struct Outcome {
    status: i32,
    stderr: String,
}

/// Run `llg` on `source`; it must end by itself.
fn llg(label: &str, source: &str, arguments: &[&str]) -> Outcome {
    let directory = sim_harness::TempDir::new("deep-nesting").expect("test directory");
    std::fs::write(directory.path().join("t.sv"), source).expect("write the source");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .current_dir(directory.path())
        .args(arguments)
        .args(["--out-dir", "out", "t.sv"]);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(300))
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output
        .status
        .code()
        .unwrap_or_else(|| panic!("{label}: ended by a signal: {:?}\n{stderr}", output.status));
    Outcome { status, stderr }
}

fn accepted(label: &str, source: &str, arguments: &[&str]) {
    let outcome = llg(label, source, arguments);
    assert_eq!(outcome.status, 0, "{label}: {}", head(&outcome.stderr));
}

fn refused(label: &str, source: &str, message: &str) {
    let outcome = llg(label, source, &["--lint-only"]);
    assert_eq!(
        outcome.status,
        1,
        "{label} is refused: {}",
        head(&outcome.stderr)
    );
    assert!(
        outcome
            .stderr
            .lines()
            .any(|line| line.contains("t.sv:") && line.contains(message)),
        "{label}: {}",
        head(&outcome.stderr)
    );
}

fn head(text: &str) -> String {
    text.lines().take(5).collect::<Vec<_>>().join("\n")
}

const TOO_DEEP: &str = "[SLANG-ParseTreeTooDeep] language constructs are too deeply nested";

fn check(constructs: &[Construct], generated: &[&str]) {
    for construct in constructs {
        let deep = construct.deep();
        accepted(construct.name, &deep, &["--lint-only"]);
        if generated.contains(&construct.name) {
            accepted(construct.name, &deep, &["--gen-only"]);
        }
        refused(construct.name, &construct.too_deep(), TOO_DEEP);
    }
    for name in generated {
        assert!(
            constructs.iter().any(|construct| construct.name == *name),
            "{name}"
        );
    }
}

#[test]
fn component_nested_expressions_meet_the_limit() {
    check(
        EXPRESSIONS,
        &[
            "parentheses",
            "binary chain",
            "right-nested binary chain",
            "unary chain",
            "concatenations",
            "target concatenations",
            "streaming concatenations",
            "calls",
            "inside sets",
            "casts",
        ],
    );
    // Generation of a conditional chain is slower than its depth.
    for construct in EXPRESSIONS {
        if construct.name == "conditional chain" {
            let source = (construct.source)(construct.limit() / 4);
            accepted(construct.name, &source, &["--gen-only"]);
        }
    }
}

#[test]
fn component_nested_statements_meet_the_limit() {
    check(STATEMENTS, &["delays"]);
    // Generation of nested statements is slower than their depth.
    for construct in STATEMENTS {
        let levels = construct.limit() / 16;
        accepted(construct.name, &(construct.source)(levels), &["--gen-only"]);
    }
}

#[test]
fn component_nested_declarations_meet_the_limit() {
    check(
        DECLARATIONS,
        &["constraint implications", "property expressions"],
    );
}

#[test]
fn component_nested_generate_blocks_meet_the_limit() {
    for construct in GENERATES {
        let deep = (construct.source)(construct.limit() / 4);
        accepted(construct.name, &deep, &["--gen-only"]);
        refused(construct.name, &construct.too_deep(), TOO_DEEP);
    }
}

/// Lists nest nothing, however long they are.
#[test]
fn component_long_lists_are_not_deep() {
    let items = MAX_NESTING_DEPTH as usize + 1000;
    let list = |separator: &str, item: &dyn Fn(usize) -> String| {
        (0..items).map(item).collect::<Vec<_>>().join(separator)
    };
    let ports = list(", ", &|index| format!("input p::t a{index}"));
    for (name, source) in [
        (
            "ports with package types",
            format!("package p; typedef logic t; endpackage\nmodule tb({ports}); endmodule\n"),
        ),
        (
            "declarators with dimensions",
            module(&format!(
                "  logic {};",
                list(", ", &|index| format!("a{index} [2]"))
            )),
        ),
        (
            "concatenation",
            module(&format!(
                "  logic a;\n  logic [{}:0] y;\n  assign y = {{{}}};",
                items - 1,
                list(", ", &|_| "a".to_owned())
            )),
        ),
        (
            "case items",
            statements(&format!(
                "case (a) {} endcase",
                list(" ", &|index| format!("{index}: a = {index};"))
            )),
        ),
        (
            "statements",
            statements(&list(" ", &|index| format!("a = {index};"))),
        ),
        (
            "members",
            module(&format!(
                "  typedef struct packed {{ {} }} t;\n  t v;",
                list(" ", &|index| format!("logic m{index};"))
            )),
        ),
        (
            "enumeration",
            module(&format!(
                "  typedef enum {{ {} }} t;\n  t v;",
                list(", ", &|index| format!("E{index}"))
            )),
        ),
        (
            "module instances",
            format!(
                "module m; endmodule\nmodule tb;\n{}\nendmodule\n",
                list("\n", &|index| format!("m u{index}();"))
            ),
        ),
        (
            "sequence of cycle delays",
            module(&format!(
                "  logic clk, a;\n  p: assert property (@(posedge clk) {});",
                list(" ##1 ", &|_| "a".to_owned())
            )),
        ),
        (
            "elsif directives",
            format!(
                "`ifdef A0\n{}\n`endif\nmodule tb; endmodule\n",
                list("\n", &|index| format!("`elsif B{index}"))
            ),
        ),
    ] {
        let outcome = llg(name, &source, &["--lint-only"]);
        assert_eq!(outcome.status, 0, "{name}: {}", head(&outcome.stderr));
    }
}

/// A chain of calls, tasks, base classes or types is as long as the design
/// makes it; walks that follow it do not recurse per link.
#[test]
fn component_chains_of_names_generate() {
    const LINKS: usize = 300;
    let chain = |link: &dyn Fn(usize) -> String| (0..LINKS).map(link).collect::<String>();
    for (name, source) in [
        (
            "functions",
            module(&format!(
                "{}  function automatic int f{LINKS}(int x); return x; endfunction\n  int y;\n  initial y = f0(1);",
                chain(&|i| format!(
                    "  function automatic int f{i}(int x); return f{}(x) + 1; endfunction\n",
                    i + 1
                ))
            )),
        ),
        (
            "tasks",
            module(&format!(
                "{}  task automatic t{LINKS}(ref int x); #1 x = 1; endtask\n  int y;\n  initial t0(y);",
                chain(&|i| format!(
                    "  task automatic t{i}(ref int x); #1 t{}(x); endtask\n",
                    i + 1
                ))
            )),
        ),
        (
            "base classes",
            module(&format!(
                "  class c0; int v; endclass\n{}  c{LINKS} h;\n  initial h = new;",
                chain(&|i| format!("  class c{} extends c{i}; endclass\n", i + 1))
            )),
        ),
        (
            "types",
            module(&format!(
                "  typedef int t0;\n{}  t{LINKS} v = 1;\n  initial $display(\"%0d\", v);",
                chain(&|i| format!("  typedef t{i} t{};\n", i + 1))
            )),
        ),
    ] {
        accepted(name, &source, &["--gen-only"]);
    }
}

/// An expansion binds inside the one that names it, so its chain is limited
/// as a chain of instances is.
#[test]
fn component_chains_of_expansions_are_limited() {
    let lets = |links: usize| {
        let chain: String = (0..links)
            .map(|i| format!("  let l{} = l{i} + 1;\n", i + 1))
            .collect();
        module(&format!(
            "  int a;\n  let l0 = a;\n{chain}  initial $display(\"%0d\", l{links});"
        ))
    };
    let sequences = |links: usize| {
        let chain: String = (0..links)
            .map(|i| format!("  sequence s{}; s{i} ##1 a; endsequence\n", i + 1))
            .collect();
        module(&format!(
            "  logic clk = 0, a = 1;\n  sequence s0; a; endsequence\n{chain}  p: assert property (@(posedge clk) s{links});\n  initial #1 clk = 1;"
        ))
    };
    for (kind, source) in [
        ("let", &lets as &dyn Fn(usize) -> String),
        ("sequence", &sequences),
    ] {
        accepted(kind, &source(100), &["--gen-only"]);
        refused(
            kind,
            &source(200),
            &format!("{kind} instantiation exceeded maximum depth of 128"),
        );
    }
}

/// Classes, interfaces, modules and checkers that name themselves with ever new
/// parameter values: no source line nests, and nothing ends the chain but a
/// limit on it. Those with two branches per level never get wide.
#[test]
fn component_endless_specializations_are_refused() {
    const CLASS: &str =
        "[SLANG-RecursiveClassSpecialization] potentially infinitely recursive specialization of class";
    const INTERFACE: &str =
        "[SLANG-MaxInstanceDepthExceeded] interface instantiation exceeded maximum depth of 128";
    const MODULE: &str =
        "[SLANG-MaxInstanceDepthExceeded] module instantiation exceeded maximum depth of 128";
    const CHECKER: &str =
        "[SLANG-MaxInstanceDepthExceeded] checker instantiation exceeded maximum depth of 64";
    const USE: &str = "module tb; c #(1) h; endmodule";
    let used = |class: &str| format!("{class}\n{USE}\n");
    for (name, source, message) in [
        (
            "member",
            used("class c #(int N = 1); c #(N + 1) v; endclass"),
            CLASS,
        ),
        (
            "typedef",
            used("class c #(int N = 1); typedef c #(N + 1) next; endclass"),
            CLASS,
        ),
        (
            "scope of a typedef",
            used("class c #(int N = 1); typedef c #(N + 1)::t t; t v; endclass"),
            CLASS,
        ),
        (
            "base class",
            used("class c #(int N = 1) extends c #(N + 1); endclass"),
            CLASS,
        ),
        (
            "two classes",
            used(
                "typedef class d;\nclass c #(int N = 1); d #(N + 1) v; endclass\n\
                 class d #(int N = 1); c #(N + 1) v; endclass",
            ),
            CLASS,
        ),
        (
            "nested class",
            used(
                "class c #(int N = 1);\n  class d #(int M = 1); c #(N + M) v; endclass\n\
                 \x20 d #(1) v;\nendclass",
            ),
            CLASS,
        ),
        (
            "two branches",
            used("class c #(bit [511:0] N = 1); c #(2 * N) v; c #(2 * N + 1) w; endclass"),
            CLASS,
        ),
        (
            "type parameter",
            used("class c #(type T = int); c #(c #(T)) v; endclass"),
            CLASS,
        ),
        (
            "growing type parameter",
            used(
                "class p #(type A = int, type B = int); A a; B b; endclass\n\
                 class c #(type T = int); c #(p #(T, T)) v; endclass",
            )
            .replace("c #(1) h", "c #(int) h"),
            CLASS,
        ),
        (
            "parameter",
            used("class c #(int N = 1); localparam int P = c #(N + 1)::P; endclass"),
            CLASS,
        ),
        (
            "static function",
            "class c #(int N = 1);\n  static function int f(); return c #(N + 1)::f(); endfunction\n\
             endclass\nmodule tb; initial $display(c #(1)::f()); endmodule\n"
                .to_string(),
            CLASS,
        ),
        (
            "static task",
            "class c #(int N = 1);\n  static task t(); c #(N + 1)::t(); endtask\n\
             endclass\nmodule tb; initial c #(1)::t(); endmodule\n"
                .to_string(),
            CLASS,
        ),
        (
            "virtual interface",
            "interface i #(int N = 1); class k; virtual i #(N + 1) v; endclass endinterface\n\
             module tb; i #(1) u(); endmodule\n"
                .to_string(),
            INTERFACE,
        ),
        (
            "virtual interface of a class",
            "typedef class c;\ninterface i #(int N = 1, type T = c #(N + 1)); T v; endinterface\n\
             class c #(int N = 1); virtual i #(N) v; endclass\nmodule tb; c #(1) h; endmodule\n"
                .to_string(),
            INTERFACE,
        ),
        (
            "two modules",
            "module m #(bit [511:0] N = 1); m #(2 * N) u(); m #(2 * N + 1) v(); endmodule\n\
             module tb; m #(1) u(); endmodule\n"
                .to_string(),
            MODULE,
        ),
        (
            "two checkers",
            "checker k(logic clk); k u(clk); k v(clk); endchecker\n\
             module tb; logic clk; k u(clk); endmodule\n"
                .to_string(),
            CHECKER,
        ),
    ] {
        let source = format!("`timescale 1ns/1ns\n{source}");
        refused(name, &source, message);
    }
}

/// A chain that ends is as long as the nesting limit allows.
#[test]
fn component_long_chains_of_specializations_are_accepted() {
    let chain = |length: usize| {
        format!(
            "`timescale 1ns/1ns\nclass c #(int N = 1);\n  c #(N > 0 ? N - 1 : 0) v;\nendclass\n\
             module tb; c #({length}) h; endmodule\n"
        )
    };
    let limit = MAX_NESTING_DEPTH as usize;
    accepted("chain", &chain(limit), &["--lint-only"]);
    refused(
        "chain",
        &chain(limit + 1),
        "t.sv:3:5: [SLANG-RecursiveClassSpecialization]",
    );
    let nest = wrap(200, "c #(", "int", ")");
    accepted(
        "nest",
        &format!(
            "`timescale 1ns/1ns\nclass c #(type T = int); T v; endclass\n\
             module tb; {nest} h; endmodule\n"
        ),
        &["--lint-only"],
    );
}

/// The macros and files an include directive goes through.
#[test]
fn component_chains_of_includes_are_limited() {
    let macros = |links: usize| {
        let chain: String = (0..links)
            .map(|i| format!("`define M{} `M{i}\n", i + 1))
            .collect();
        format!("`define M0 \"leaf.svh\"\n{chain}module tb;\n`include `M{links}\nendmodule\n")
    };
    let run = |files: &[(String, String)]| {
        let directory = sim_harness::TempDir::new("deep-includes").expect("test directory");
        for (name, text) in files {
            std::fs::write(directory.path().join(name), text).expect("write a source");
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .current_dir(directory.path())
            .args(["--lint-only", "t.sv"]);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .expect("run the driver");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let leaf = || ("leaf.svh".to_owned(), "int leaf;\n".to_owned());
    for links in [100, MAX_NESTING_DEPTH as usize - 1] {
        let (status, stderr) = run(&[("t.sv".to_owned(), macros(links)), leaf()]);
        assert_eq!(status, Some(0), "{links} macros: {stderr}");
    }
    let (status, stderr) = run(&[
        ("t.sv".to_owned(), macros(MAX_NESTING_DEPTH as usize + 1)),
        leaf(),
    ]);
    assert_eq!(status, Some(1), "{stderr}");
    assert!(
        stderr.contains("the macros of an `include directive nest more than 4096 deep: `M4097"),
        "{stderr}"
    );

    let files = |links: usize| {
        let mut files = vec![("t.sv".to_owned(), "`include \"i0.svh\"\n".to_owned())];
        files.extend((0..links).map(|i| {
            (
                format!("i{i}.svh"),
                format!("`include \"i{}.svh\"\n", i + 1),
            )
        }));
        files.push((format!("i{links}.svh"), "module tb; endmodule\n".to_owned()));
        files
    };
    let (status, stderr) = run(&files(200));
    assert_eq!(status, Some(0), "{stderr}");
    let (status, stderr) = run(&files(300));
    assert_eq!(status, Some(1), "{stderr}");
    assert!(
        stderr.contains("i256.svh: include files nest more than 256 deep"),
        "{stderr}"
    );
}

/// A nested value on the command line has no source location.
#[test]
fn component_a_deep_parameter_value_is_refused() {
    let value = wrap(5000, "(", "1", ")");
    let outcome = llg(
        "parameter value",
        "module tb #(parameter int P = 0); endmodule\n",
        &["--lint-only", "-G", &format!("P={value}")],
    );
    assert_eq!(outcome.status, 1, "{}", head(&outcome.stderr));
    assert!(
        outcome
            .stderr
            .contains("language constructs are too deeply nested"),
        "{}",
        head(&outcome.stderr)
    );
}
