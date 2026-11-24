//! FIN-002 step 3c: malformed external inputs at the tool boundary.
//!
//! Bounded corpora from the fixed seed of `support/fuzz.rs`: mutated
//! checked-in sources through the whole frontend (preprocessor, parser,
//! elaboration, owned database, lint, lowering and C generation), mutated
//! `llg.toml` and SDF files through their parsers, random command lines
//! through `llg` and malformed JSON-RPC traffic through `llg_ls`. Every
//! input must end in a defined result: a value or an error from a library
//! entry, an exit status of 0, 1 or 2 from a tool; never a panic, a signal
//! or a hang. These are component tests: no model is built or run here
//! (the run-time readers are in `sim_feature_completion::fin_002b`).
use crate::fuzz::{self, Fuzz};
use crate::sim_harness;
use llg::core::compile::{self, CompileOpts, OwnedSource};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::Command;
use std::time::Duration;

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!("fixtures/", $path))
    };
}

/// Checked-in designs the source corpus mutates: classes with mailboxes and
/// randomization, a gate-level netlist with specify blocks and UDPs, an
/// interface design with DPI imports, and the run-time reader fixture.
const SOURCES: &[&str] = &[
    fixture!("sim/feature_completion/fin_002/tb_rand_mailbox_kill.sv"),
    fixture!("sim/feature_completion/fin_002/gl_timing_checks.sv"),
    fixture!("sim/feature_completion/fin_002/tb_vif_swap.sv"),
    fixture!("sim/feature_completion/fin_002b/fuzz_inputs.sv"),
];

/// Spellings that open, close or redirect a construct of the language.
const SOURCE_TOKENS: &[&[u8]] = &[
    b"begin ",
    b" end ",
    b"(",
    b")",
    b"[",
    b"]",
    b"{",
    b"}",
    b"\"",
    b"/*",
    b"*/",
    b"//",
    b"`define X ",
    b"`ifdef X\n",
    b"`endif\n",
    b"`include \"x.svh\"\n",
    b"`X ",
    b"``",
    b"`\"",
    b"module ",
    b"endmodule ",
    b"class ",
    b"endclass ",
    b"function ",
    b"endfunction ",
    b"task ",
    b"endtask ",
    b"generate ",
    b"endgenerate ",
    b"fork ",
    b"join_none ",
    b"interface ",
    b"virtual ",
    b"typedef ",
    b"struct packed {",
    b"'{",
    b"::",
    b"#(",
    b"@(",
    b"posedge ",
    b";",
    b",",
    b".",
    b"=",
    b"<=",
    b"'x",
    b"1'bz",
    b"64'hffff_ffff_ffff_ffff",
    b"0",
    b"-1",
    b"$",
    b"$bits(",
    b"$unit::",
    b"\\escaped ",
    b"\0",
    b"\xff\xfe",
    b"\n",
    b"assert property (",
    b"covergroup ",
    b"constraint ",
    b"randomize() with {",
    b"specify ",
    b"primitive ",
    b"table ",
    b"defparam ",
    b"bind ",
    b"import \"DPI-C\" ",
    b"export ",
    b"real ",
    b"string ",
    b"chandle ",
    b"event ",
    b"[$]",
    b"[*]",
    b"[0:-1]",
    b"signed ",
];

const CONFIGURATIONS: &[&str] = &[
    fixture!("lsp/project-wide/llg.toml"),
    fixture!("lsp/lint-rules/llg.toml"),
    fixture!("dump/proj/llg.toml"),
    fixture!("sim/lint/allow_feedback_and_mixed.toml"),
];

const CONFIGURATION_TOKENS: &[&[u8]] = &[
    b"[",
    b"]",
    b"[[",
    b"]]",
    b"=",
    b"\"",
    b"'",
    b"\"\"\"",
    b"\n",
    b"#",
    b".",
    b",",
    b"{",
    b"}",
    b"true",
    b"false",
    b"0",
    b"-1",
    b"1e999",
    b"nan",
    b"inf",
    b"18446744073709551616",
    b"schema_version = 1\n",
    b"schema_version = 99\n",
    b"[sources]\n",
    b"[lint]\n",
    b"[simulator]\n",
    b"[waveform]\n",
    b"[build]\n",
    b"[output]\n",
    b"directories = [",
    b"include = [",
    b"file = \"x.vcd\"\n",
    b"depth = ",
    b"jobs = ",
    b"top = ",
    b"\\u0000",
    b"\0",
    b"\xff",
    b"0x",
    b"1979-05-27T07:32:00Z",
    b"a.b.c = 1\n",
    b"\"\" = 1\n",
];

const TIMING_FILES: &[&str] = &[
    fixture!("sim/feature_completion/fin_002/gl_cells_sdf.sdf"),
    fixture!("sim/feature_completion/fin_002/gl_scan_chain.sdf"),
];

const TIMING_TOKENS: &[&[u8]] = &[
    b"(",
    b")",
    b"(DELAYFILE",
    b"(SDFVERSION \"3.0\")",
    b"(SDFVERSION \"2.1\")",
    b"(CELL",
    b"(CELLTYPE \"",
    b"(INSTANCE ",
    b"(DELAY",
    b"(ABSOLUTE",
    b"(INCREMENT",
    b"(IOPATH ",
    b"(COND ",
    b"(TIMINGCHECK",
    b"(SETUP ",
    b"(HOLD ",
    b"(WIDTH ",
    b"(posedge ",
    b"(negedge ",
    b"(TIMESCALE 1ns)",
    b"(TIMESCALE 0ns)",
    b"(DIVIDER /)",
    b"(DIVIDER .)",
    b"::",
    b":",
    b"1.0:2.0:3.0",
    b"-1",
    b"1e999",
    b"1e-999",
    b".",
    b"\"",
    b"//",
    b"/*",
    b"*/",
    b"\\",
    b"[",
    b"]",
    b"[3:0]",
    b"*",
    b"==",
    b"&&",
    b"!",
    b"~",
    b"1'b0",
    b"\0",
    b"\xff",
    b"\n",
];

/// Options and values a command line is drawn from; the forms that need a
/// value appear with and without one.
const ARGUMENTS: &[&str] = &[
    "--top",
    "tb",
    "--edition",
    "v2001",
    "sv2009",
    "sv2030",
    "--config",
    "missing.toml",
    "--lint-only",
    "--lint-json",
    "--gen-only",
    "--no-opt",
    "--opt",
    "--wave",
    "x.vcd",
    "x.txt",
    "--wave-depth",
    "-1",
    "0",
    "4294967296",
    "--timescale",
    "1ns/1ps",
    "1ps/1ns",
    "7ns/1ps",
    "--delay-select",
    "typ",
    "--define",
    "A=1",
    "=",
    "--include-dir",
    ".",
    "--param-override",
    "W=8",
    "W",
    "--sdf",
    "min:top=x.sdf",
    "::",
    "--pulse-reject",
    "101",
    "--model-opt-level",
    "O9",
    "--build-jobs",
    "--out-dir",
    "",
    "--append-define",
    "--append-nothing",
    "-Werror",
    "-W",
    "-",
    "--",
    "+plus",
    "---",
    "--stop-policy",
    "never",
    "--code-coverage=nothing",
    "--libmap",
    "--libfile",
    "=x",
    "--library-order",
    ",",
    "--define-system-task",
    "task $t(",
    "--dpi-lib",
    "missing.so",
    "--help",
    "\u{feff}",
    "missing.sv",
    "empty.sv",
    "tb.sv",
];

/// Apply between one and three mutations to one of `seeds`.
fn mutated(fuzz: &mut Fuzz, seeds: &[&str], tokens: &[&[u8]]) -> Vec<u8> {
    let mut bytes = fuzz.pick(seeds).as_bytes().to_vec();
    for _ in 0..fuzz.range(1, 3) {
        bytes = fuzz.mutate(&bytes, tokens);
    }
    bytes
}

/// Run `stage` and turn a panic into the failure of `label`, with the input
/// that caused it. A native crash cannot be caught: the case number printed
/// before each case names it in the test log.
fn defined<T>(label: &str, input: &[u8], stage: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(stage)).unwrap_or_else(|_| {
        panic!(
            "{label}: panic on this input:\n{}",
            String::from_utf8_lossy(input)
        )
    })
}

/// A mutated design goes as far through the pipeline as its errors allow:
/// every stage returns a result.
#[test]
fn component_mutated_sources_end_in_diagnostics_or_generated_models() {
    let mut generated = 0;
    let cases = fuzz::cases(160);
    for case in cases.clone() {
        eprintln!("sources case {case}");
        let label = format!("sources case {case}");
        let mut fuzz = Fuzz::new("inputs/sources", case);
        let bytes = mutated(&mut fuzz, SOURCES, SOURCE_TOKENS);
        // Sources are text to the library: a byte sequence that is not
        // UTF-8 is covered through the command line below.
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let options = CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        };
        let sources = [OwnedSource::compilation_unit("tb.sv", text)];
        let Ok(compiled) = defined(&label, &bytes, || {
            compile::compile_sources(&sources, &options)
        }) else {
            continue;
        };
        let failed = compiled.snapshot.has_errors();
        let Ok(database) = defined(&label, &bytes, || {
            llg::core::db::Db::from_slang(&compiled.snapshot)
        }) else {
            continue;
        };
        drop(compiled);
        defined(&label, &bytes, || {
            let model = llg::core::model::DesignModel::from_db(&database);
            llg::core::lint::lint(&database, &model)
        });
        if failed {
            continue;
        }
        for optimization in [
            llg::sim::opt::OptConfig::default(),
            llg::sim::opt::OptConfig::none(),
        ] {
            let model = defined(&label, &bytes, || {
                llg::sim::codegen::generate_from_db_with_opts(&database, &optimization)
            });
            generated += usize::from(model.is_ok());
        }
    }
    // The corpus is not all noise: some mutations leave a legal design.
    assert!(
        generated > 0 || cases.end - cases.start < 160,
        "no mutated source reached C generation"
    );
}

#[test]
fn component_mutated_configurations_are_values_or_errors() {
    for case in fuzz::cases(600) {
        let mut fuzz = Fuzz::new("inputs/configurations", case);
        let bytes = mutated(&mut fuzz, CONFIGURATIONS, CONFIGURATION_TOKENS);
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let label = format!("configurations case {case}");
        let parsed = defined(&label, &bytes, || {
            llg::config::parse_config_detailed(std::path::Path::new("."), &text)
        });
        if let Err(error) = parsed {
            assert!(!error.to_string().is_empty(), "{label}: empty error");
        }
    }
}

#[test]
fn component_mutated_timing_files_are_values_or_located_errors() {
    for case in fuzz::cases(600) {
        let mut fuzz = Fuzz::new("inputs/sdf", case);
        let bytes = mutated(&mut fuzz, TIMING_FILES, TIMING_TOKENS);
        let label = format!("sdf case {case}");
        let parsed = defined(&label, &bytes, || {
            llg::sim::sdf::parse(bytes.as_slice(), llg::sim::sdf::SdfLimits::default())
        });
        if let Err(error) = parsed {
            assert!(!error.to_string().is_empty(), "{label}: empty error");
        }
    }
}

/// The exit status of a tool run that ended by itself within `timeout`.
fn exit_status(label: &str, command: &mut Command, timeout: Duration) -> (i32, String) {
    let output = sim_harness::run_command(command, timeout)
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output
        .status
        .code()
        .unwrap_or_else(|| panic!("{label}: ended by a signal: {:?}\n{stderr}", output.status));
    assert!(
        !stderr.contains("panicked at") && !stderr.contains("overflowed its stack"),
        "{label}: {stderr}"
    );
    (status, stderr)
}

/// Random command lines: 0 after a report-only run, 1 after source or lint
/// errors, 2 after a usage or configuration error. Nothing is built: the
/// only complete invocations name `--lint-only` or `--gen-only` sources.
#[test]
fn component_random_command_lines_exit_with_a_documented_status() {
    let directory = sim_harness::TempDir::new("fin-002-command-lines").expect("test directory");
    std::fs::write(directory.path().join("empty.sv"), "").expect("write source");
    std::fs::write(
        directory.path().join("tb.sv"),
        "module tb;\n  initial $finish;\nendmodule\n",
    )
    .expect("write source");
    for case in fuzz::cases(120) {
        let mut fuzz = Fuzz::new("inputs/command-lines", case);
        let mut arguments: Vec<String> = (0..fuzz.range(1, 7))
            .map(|_| (*fuzz.pick(ARGUMENTS)).to_owned())
            .collect();
        // A run that would build and execute a model belongs to the
        // simulation suites; keep these runs to the driver itself.
        if !arguments
            .iter()
            .any(|argument| argument == "--lint-only" || argument == "--gen-only")
        {
            arguments.insert(0, "--lint-only".to_owned());
        }
        let label = format!("command lines case {case}: {arguments:?}");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(&arguments);
        let (status, stderr) = exit_status(&label, &mut command, Duration::from_secs(120));
        assert!(
            matches!(status, 0..=2),
            "{label}: status {status}\n{stderr}"
        );
        if status == 2 {
            assert!(!stderr.trim().is_empty(), "{label}: silent usage error");
        }
    }
}

/// One JSON-RPC frame with a correct header.
fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// Traffic the language server must survive: each entry is the whole input
/// of one server process, which then sees end of input.
fn sessions() -> Vec<(&'static str, Vec<u8>)> {
    let initialize =
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tb.sv","languageId":"systemverilog","version":1,"text":"module tb; logic a; endmodule\n"}}}"#;
    let started = || {
        let mut bytes = frame(initialize);
        bytes.extend(frame(initialized));
        bytes.extend(frame(open));
        bytes
    };
    let after_start = |bodies: &[&str]| {
        let mut bytes = started();
        for body in bodies {
            bytes.extend(frame(body));
        }
        bytes
    };
    vec![
        ("empty input", Vec::new()),
        ("no header", b"{\"jsonrpc\":\"2.0\"}".to_vec()),
        (
            "header without a length",
            b"Content-Type: x\r\n\r\n{}".to_vec(),
        ),
        ("negative length", b"Content-Length: -5\r\n\r\n{}".to_vec()),
        (
            "length that is not a number",
            b"Content-Length: abc\r\n\r\n{}".to_vec(),
        ),
        (
            "length beyond the input",
            b"Content-Length: 4000000000\r\n\r\n{}".to_vec(),
        ),
        (
            "length beyond 64 bits",
            b"Content-Length: 99999999999999999999999999\r\n\r\n{}".to_vec(),
        ),
        (
            "truncated body",
            b"Content-Length: 50\r\n\r\n{\"jsonrpc\"".to_vec(),
        ),
        ("body that is not JSON", frame("not json at all")),
        ("body that is not UTF-8", {
            let mut bytes = b"Content-Length: 4\r\n\r\n".to_vec();
            bytes.extend([0xff, 0xfe, 0x00, 0x80]);
            bytes
        }),
        ("JSON that is not a message", frame("[1, 2, 3]")),
        ("null message", frame("null")),
        (
            "deeply nested JSON",
            frame(&format!("{}{}", "[".repeat(100_000), "]".repeat(100_000))),
        ),
        ("endless header line", vec![b'A'; 4 << 20]),
        (
            "request before initialize",
            frame(
                r#"{"jsonrpc":"2.0","id":7,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":0,"character":0}}}"#,
            ),
        ),
        (
            "initialize with wrong types",
            frame(r#"{"jsonrpc":"2.0","id":"x","method":"initialize","params":[1,2]}"#),
        ),
        ("initialize twice", {
            let mut bytes = frame(initialize);
            bytes.extend(frame(initialize));
            bytes
        }),
        (
            "positions and versions out of range",
            after_start(&[
                r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":4294967295,"character":4294967295}}}"#,
                r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":-1,"character":-1}}}"#,
                r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":1e400,"character":0.5}}}"#,
                r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///tb.sv","version":-9223372036854775808},"contentChanges":[{"range":{"start":{"line":9,"character":9},"end":{"line":0,"character":0}},"text":"x"}]}}"#,
                r#"{"jsonrpc":"2.0","id":5,"method":"textDocument/semanticTokens/range","params":{"textDocument":{"uri":"file:///tb.sv"},"range":{"start":{"line":5,"character":0},"end":{"line":1,"character":0}}}}"#,
                r#"{"jsonrpc":"2.0","id":6,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":0,"character":999999},"newName":""}}"#,
            ]),
        ),
        (
            "documents that were never opened",
            after_start(&[
                r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///ghost.sv","version":2},"contentChanges":[{"text":"module"}]}}"#,
                r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///ghost.sv"}}}"#,
                r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///tb.sv"}}}"#,
                r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///tb.sv"}}}"#,
                r#"{"jsonrpc":"2.0","id":8,"method":"textDocument/hover","params":{"textDocument":{"uri":"not a uri"},"position":{"line":0,"character":0}}}"#,
                r#"{"jsonrpc":"2.0","id":9,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///%ff%fe.sv"}}}"#,
            ]),
        ),
        (
            "unknown methods, responses without requests, cancellations",
            after_start(&[
                r#"{"jsonrpc":"2.0","id":10,"method":"no/such/method","params":null}"#,
                r#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":424242}}"#,
                r#"{"jsonrpc":"2.0","id":11,"result":{"unexpected":true}}"#,
                r#"{"jsonrpc":"2.0","id":11,"error":{"code":-1,"message":"x"}}"#,
                r#"{"jsonrpc":"1.0","id":12,"method":"shutdown"}"#,
                r#"{"id":13}"#,
                r#"{"jsonrpc":"2.0","method":17}"#,
            ]),
        ),
        (
            "requests after shutdown",
            after_start(&[
                r#"{"jsonrpc":"2.0","id":20,"method":"shutdown"}"#,
                r#"{"jsonrpc":"2.0","id":21,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tb.sv"},"position":{"line":0,"character":8}}}"#,
                r#"{"jsonrpc":"2.0","id":22,"method":"shutdown"}"#,
                r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///late.sv","languageId":"systemverilog","version":1,"text":"module"}}}"#,
            ]),
        ),
        ("a document rebuilt while requests are queued", {
            let mut bodies = Vec::new();
            for version in 2..40 {
                bodies.push(format!(
                        r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"file:///tb.sv","version":{version}}},"contentChanges":[{{"text":"module tb; logic a{version}; {} endmodule\n"}}]}}}}"#,
                        if version % 3 == 0 { "initial begin" } else { "" }
                    ));
                bodies.push(format!(
                        r#"{{"jsonrpc":"2.0","id":{},"method":"textDocument/hover","params":{{"textDocument":{{"uri":"file:///tb.sv"}},"position":{{"line":0,"character":18}}}}}}"#,
                        100 + version
                    ));
                bodies.push(format!(
                        r#"{{"jsonrpc":"2.0","id":{},"method":"textDocument/semanticTokens/full","params":{{"textDocument":{{"uri":"file:///tb.sv"}}}}}}"#,
                        200 + version
                    ));
            }
            let bodies: Vec<&str> = bodies.iter().map(String::as_str).collect();
            after_start(&bodies)
        }),
    ]
}

/// Malformed frames, wrong types, positions out of range, requests out of
/// order and a document replaced under queued requests: the server neither
/// panics nor hangs, and ends once its input ends.
#[cfg(feature = "lsp")]
#[test]
fn component_language_server_survives_malformed_traffic() {
    let directory = sim_harness::TempDir::new("fin-002-json-rpc").expect("test directory");
    for (name, input) in sessions() {
        let path = directory.path().join("input.bin");
        std::fs::write(&path, &input).expect("write the session input");
        let label = format!("language server, {name}");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg_ls"));
        command
            .current_dir(directory.path())
            .stdin(std::fs::File::open(&path).expect("open the session input"));
        let (status, stderr) = exit_status(&label, &mut command, Duration::from_secs(120));
        // Input ended without `exit`: the server reports that with 1.
        assert!(
            matches!(status, 0 | 1),
            "{label}: status {status}\n{stderr}"
        );
    }
}
