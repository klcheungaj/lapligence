//! Gate-path differential: randomized gate and UDP netlists run on the
//! default runtime and on the earlier gate path kept behind the
//! development-only C define `LLG_DEV_LEGACY_GATE_PATH` (per-wake dependency
//! registration, full driver-index updates and indexed temporaries). Both
//! builds of one generated model must print byte-identical traces.
//!
//! The netlists mix every built-in gate kind, combinational and sequential
//! UDPs, one/two/three-value delays, strengths, multiply driven and wired
//! nets, gate arrays on vector nets and X/Z stimulus with same-slot glitches
//! (back-to-back writes and `#0`-separated writes). An `always @*` observer
//! prints on every wake, so wake order and glitches are compared, not only
//! settled values. The generator is a fixed-seed xorshift, so every run checks
//! the same netlists.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use llg::core::compile;
use llg::sim;

use crate::sim_harness;

const SEEDS: [u64; 3] = [
    0x9e37_79b9_7f4a_7c15,
    0x2545_f491_4f6c_dd1d,
    0x1405_7b7e_f767_814f,
];
const GATES: usize = 48;
const STEPS: usize = 60;
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// A random delay; only enable gates take a third (turn-off) value.
fn delay(rng: &mut Rng, turn_off: bool) -> String {
    match rng.below(if turn_off { 5 } else { 4 }) {
        0 | 1 => String::new(),
        2 => format!(" #{}", rng.below(4)),
        3 => format!(" #({},{})", rng.below(4), rng.below(4)),
        _ => format!(" #({},{},{})", rng.below(4), rng.below(4), rng.below(4)),
    }
}

fn strength(rng: &mut Rng) -> &'static str {
    match rng.below(6) {
        0 => " (weak0, strong1)",
        1 => " (pull0, weak1)",
        _ => "",
    }
}

/// One netlist: inputs `in`, scalar nodes `w*` driven in topological order,
/// wired and multiply driven nets, a gate array on a vector net and UDPs.
fn netlist(seed: u64) -> String {
    let mut rng = Rng(seed);
    let mut out = String::new();
    out.push_str(
        "primitive llg_dff (q, c, d);\n  output q; reg q;\n  input c, d;\n  initial q = 0;\n  table\n    r 0 : ? : 0 ;\n    r 1 : ? : 1 ;\n    f ? : ? : - ;\n    ? * : ? : - ;\n  endtable\nendprimitive\n",
    );
    out.push_str(
        "primitive llg_mux (y, s, a, b);\n  output y;\n  input s, a, b;\n  table\n    0 0 ? : 0 ;\n    0 1 ? : 1 ;\n    1 ? 0 : 0 ;\n    1 ? 1 : 1 ;\n    ? 0 0 : 0 ;\n    ? 1 1 : 1 ;\n  endtable\nendprimitive\n",
    );
    out.push_str("module tb;\n  reg [7:0] in;\n  reg clk;\n");
    let mut sources: Vec<String> = (0..8).map(|bit| format!("in[{bit}]")).collect();
    let mut observed = Vec::new();
    let mut body = String::new();
    for gate in 0..GATES {
        let name = format!("w{gate}");
        let pick = |rng: &mut Rng, sources: &[String]| sources[rng.below(sources.len())].clone();
        let kind = rng.below(12);
        let line = match kind {
            0..=5 => {
                let op = ["and", "or", "nand", "nor", "xor", "xnor"][kind];
                let inputs = (0..2 + rng.below(2))
                    .map(|_| pick(&mut rng, &sources))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{op}{}{} g{gate} ({name}, {inputs});",
                    strength(&mut rng),
                    delay(&mut rng, false)
                )
            }
            6 | 7 => {
                let op = ["buf", "not"][kind - 6];
                format!(
                    "{op}{} g{gate} ({name}, {});",
                    delay(&mut rng, false),
                    pick(&mut rng, &sources)
                )
            }
            8 | 9 => {
                let op = ["bufif0", "bufif1", "notif0", "notif1"][rng.below(4)];
                format!(
                    "{op}{} g{gate} ({name}, {}, {});",
                    delay(&mut rng, true),
                    pick(&mut rng, &sources),
                    pick(&mut rng, &sources)
                )
            }
            10 => format!(
                "llg_mux{} g{gate} ({name}, {}, {}, {});",
                delay(&mut rng, false),
                pick(&mut rng, &sources),
                pick(&mut rng, &sources),
                pick(&mut rng, &sources)
            ),
            _ => format!(
                "llg_dff{} g{gate} ({name}, clk, {});",
                delay(&mut rng, false),
                pick(&mut rng, &sources)
            ),
        };
        let _ = writeln!(out, "  wire {name};");
        let _ = writeln!(body, "  {line}");
        sources.push(name.clone());
        observed.push(name);
    }
    // Two drivers on one wire, on wired-AND/OR nets and on pulled nets.
    for (index, net) in ["wire", "wand", "wor", "tri0", "tri1"].iter().enumerate() {
        let name = format!("m{index}");
        let _ = writeln!(out, "  {net} {name};");
        for driver in 0..2 {
            let op = ["bufif1", "and", "bufif0", "or"][rng.below(4)];
            let a = sources[rng.below(sources.len())].clone();
            let b = sources[rng.below(sources.len())].clone();
            let _ = writeln!(
                body,
                "  {op}{}{} md{index}_{driver} ({name}, {a}, {b});",
                if op.starts_with("buf") {
                    ""
                } else {
                    strength(&mut rng)
                },
                delay(&mut rng, op.starts_with("buf"))
            );
        }
        observed.push(name);
    }
    // Gate arrays on a vector net, one undelayed and one delayed.
    out.push_str("  wire [3:0] va, vb;\n");
    let _ = writeln!(body, "  xor ga[3:0] (va, in[3:0], in[7:4]);");
    let _ = writeln!(
        body,
        "  nand{} gb[3:0] (vb, va, in[5:2]);",
        delay(&mut rng, false)
    );
    let _ = writeln!(body, "  llg_dff ua[3:0] (vb, clk, in[3:0]);");
    observed.push("va".to_owned());
    observed.push("vb".to_owned());
    out.push_str(&body);
    let all = observed.join(", ");
    let _ = writeln!(
        out,
        "  always @({}) $display(\"%0t %b\", $time, {{{all}}});",
        observed.join(" or ")
    );
    // A dependency waiter killed while waiting (disable fork) releases its
    // retained wait; its count shows when it stopped.
    out.push_str(
        "  integer seen = 0;\n  initial begin\n    fork\n      forever @* seen = seen + (in[0] ^ in[1]);\n    join_none\n    #50 disable fork;\n  end\n",
    );
    out.push_str("  initial begin\n    clk = 0;\n    in = 8'b0;\n");
    let states = ['0', '1', 'x', 'z'];
    for _ in 0..STEPS {
        let value: String = (0..8)
            .map(|_| {
                states[if rng.below(4) == 0 {
                    2 + rng.below(2)
                } else {
                    rng.below(2)
                }]
            })
            .collect();
        let _ = writeln!(out, "    #{} in = 8'b{value};", 1 + rng.below(3));
        match rng.below(5) {
            0 => {
                let _ = writeln!(out, "    in[{}] = ~in[{}];", rng.below(8), rng.below(8));
            }
            1 => {
                let _ = writeln!(
                    out,
                    "    #0 in[{}] = 1'b{};",
                    rng.below(8),
                    states[rng.below(4)]
                );
            }
            2 => out.push_str("    clk = ~clk;\n"),
            3 => out.push_str("    #0 clk = ~clk;\n"),
            _ => {}
        }
    }
    let _ = writeln!(
        out,
        "    #8 $display(\"%v %v %v %v %v %0d\", m0, m1, m2, m3, m4, seen);\n    $finish(0);\n  end\nendmodule"
    );
    out
}

fn build_and_run(dir: &Path, model_c: &str, cflags: Option<String>) -> String {
    std::fs::create_dir_all(dir).expect("create build directory");
    let executable = sim::build::build_model_cmake_with_opts(
        dir,
        &[("model.c", model_c)],
        &sim::build::CmakeBuildOpts {
            value_config: sim::value_backend::ValueConfig::from_env().expect("value config"),
            cflags,
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("cmake: {error}"));
    let output = sim_harness::run_command(&mut Command::new(&executable), RUN_TIMEOUT)
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(
        output.status.success(),
        "simulation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn check_seed(seed: u64) {
    let source = netlist(seed);
    let tag = format!("gate-path-diff-{seed:x}");
    sim_harness::with_temp_cwd(&tag, |dir| {
        let path = dir.join("tb.v");
        std::fs::write(&path, &source).expect("write netlist");
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .unwrap_or_else(|error| panic!("compile: {error}\n{source}"));
        let database = llg::core::db::Db::from_slang(&compiled.snapshot).expect("database");
        let generated = sim::codegen::generate_from_db_with_codegen_options(
            &database,
            &sim::codegen::CodegenOptions {
                value_config: sim::value_backend::ValueConfig::from_env().expect("value config"),
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("codegen: {error}\n{source}"));
        let fast = build_and_run(&dir.join("fast"), &generated.model_c, None);
        // Keep any sanitizer or debug flags of the surrounding run.
        let legacy_flags = match std::env::var("LLG_CFLAGS") {
            Ok(flags) if !flags.trim().is_empty() => format!("{flags} -DLLG_DEV_LEGACY_GATE_PATH"),
            _ => "-DLLG_DEV_LEGACY_GATE_PATH".to_owned(),
        };
        let legacy = build_and_run(&dir.join("legacy"), &generated.model_c, Some(legacy_flags));
        assert!(fast.lines().count() > STEPS, "trace too short:\n{fast}");
        assert_eq!(
            fast, legacy,
            "gate paths differ for seed {seed:#x}\n{source}"
        );
        Ok(())
    })
    .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn gate_paths_agree_seed_0() {
    check_seed(SEEDS[0]);
}

#[test]
fn gate_paths_agree_seed_1() {
    check_seed(SEEDS[1]);
}

#[test]
fn gate_paths_agree_seed_2() {
    check_seed(SEEDS[2]);
}
