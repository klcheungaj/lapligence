//! FIN-002 step 3a: mutated IR models against the validator.
//!
//! Models lowered from small designs are damaged in one or two tables each
//! (removed or repeated entries, so that references dangle or name the
//! wrong kind of entry; zero, one-bit and oversized widths; exchanged
//! types; empty bodies; aliases that form a cycle) from the fixed corpus
//! seed. The contract of `IrModelParts` in the simulator guide: validation
//! rejects a damaged model with an error and never panics, and a model it
//! accepts goes through execution lowering, every optimizer pass and
//! emission without a panic and to C the host compiler accepts (checked on
//! a sample of the accepted models, to bound the time).
use super::*;
use crate::fuzz::{self, Fuzz};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Designs that fill different tables: variables and nets with drivers,
/// arrays, functions and tasks with events, classes with containers.
const DESIGNS: &[&str] = &[
    "module tb;\n  logic [7:0] a, b;\n  wire [7:0] w = a & b;\n  logic [3:0] m [0:3];\n  \
     always @(a or b) m[a[1:0]] = b[3:0];\n  initial begin\n    a = 1; b = 2;\n    \
     #1 $display(\"%h %h\", w, m[1]);\n    $finish;\n  end\nendmodule\n",
    "module tb;\n  event e;\n  integer n;\n  function automatic integer twice(input integer v);\n    \
     return v * 2;\n  endfunction\n  task automatic wait_e(output integer r);\n    @e;\n    \
     r = twice(n);\n  endtask\n  initial begin\n    n = 3;\n    fork\n      begin integer r; wait_e(r); \
     $display(\"%0d\", r); end\n      #1 -> e;\n    join\n    $finish;\n  end\nendmodule\n",
    "module tb;\n  class item;\n    rand bit [3:0] v;\n    int q [$];\n    function void push(int x);\n      \
     q.push_back(x);\n    endfunction\n  endclass\n  item it;\n  string s;\n  int map [string];\n  \
     initial begin\n    it = new;\n    it.push(4);\n    s = \"k\";\n    map[s] = it.q.size();\n    \
     $display(\"%0d %0d\", map[s], it.q[0]);\n    $finish;\n  end\nendmodule\n",
];

/// Prefix of an emitter error for a model the validator accepted.
const REFUSED: &str = "emission of an accepted model";

/// Invariants only the emitter checks (docs/known_issues.md, "IR
/// validation leaves one container invariant to the emitter"): the emitter
/// refuses such a model with an error. Any other refusal fails the test.
const KNOWN_REFUSALS: &[&str] = &["class container property used outside its class methods"];

/// Every n-th case compiles the C of its accepted model.
const COMPILED_SAMPLE: u64 = 24;

/// The host compiler accepts `source` (syntax and declarations only).
fn compile(source: &str, label: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| crate::sim::build::DEFAULT_C_COMPILER.to_owned());
    // The frame lint of the integration tests owns the portable flag set;
    // this sample needs only a compiler that takes C on standard input.
    if cfg!(windows) && !compiler.contains("gcc") && !compiler.contains("clang") {
        return;
    }
    let config = crate::sim::value_backend::ValueConfig::default();
    let mut child = Command::new(&compiler)
        .args(["-x", "c", "-std=c11", "-fsyntax-only", "-"])
        .arg(format!("-DLLG_SV4_USE_GMP={}", config.backend.selector()))
        .arg(format!(
            "-DLLG_SV4_GMP_KERNELS={}",
            config.kernel.selector()
        ))
        .arg(format!("-I{}/src/sim/rt", env!("CARGO_MANIFEST_DIR")))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{label}: start {compiler}: {error}"));
    child
        .stdin
        .take()
        .expect("compiler input")
        .write_all(source.as_bytes())
        .expect("write the model");
    let output = child.wait_with_output().expect("compiler result");
    assert!(
        output.status.success(),
        "{label}: the emitted C does not compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn lowered(source: &str) -> IrModel {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "tb.sv", source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("the design compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    lower_model(&database, &Default::default())
        .expect("the design lowers")
        .model
}

/// Remove, repeat or exchange entries of `table`; false when it is empty.
fn damage<T: Clone>(table: &mut Vec<T>, fuzz: &mut Fuzz) -> bool {
    if table.is_empty() {
        return false;
    }
    let at = fuzz.index(table.len());
    match fuzz.below(5) {
        0 => {
            table.pop();
        }
        1 => {
            table.remove(at);
        }
        2 => table.clear(),
        3 => {
            let entry = table[at].clone();
            table.push(entry);
        }
        _ => {
            let other = fuzz.index(table.len());
            table.swap(at, other);
        }
    }
    true
}

/// One mutation of `model`; the text names it in a failure.
fn mutate(model: &mut IrModel, fuzz: &mut Fuzz) -> String {
    for _ in 0..64 {
        let (name, done) = match fuzz.below(18) {
            0 => ("signals", damage(&mut model.signals, fuzz)),
            1 => ("arrays", damage(&mut model.arrays, fuzz)),
            2 => ("events", damage(&mut model.events, fuzz)),
            3 => ("functions", damage(&mut model.funcs, fuzz)),
            4 => ("processes", damage(&mut model.processes, fuzz)),
            5 => ("net groups", damage(&mut model.net_groups, fuzz)),
            6 => ("containers", damage(&mut model.containers, fuzz)),
            7 => ("classes", damage(&mut model.classes, fuzz)),
            8 => ("objects", damage(&mut model.objects, fuzz)),
            9 => ("spawns", damage(&mut model.spawns, fuzz)),
            10 => ("initialization steps", damage(&mut model.init_steps, fuzz)),
            11 | 12 if !model.signals.is_empty() => {
                let at = fuzz.index(model.signals.len());
                let width =
                    *fuzz.pick(&[0, 1, 63, 64, 65, LLG_MAX_WIDTH, LLG_MAX_WIDTH + 1, u32::MAX]);
                model.signals[at].ty = if fuzz.chance(1, 6) {
                    IrType::Real { shortreal: false }
                } else {
                    IrType::Packed {
                        width,
                        signed: fuzz.chance(1, 2),
                        two_state: fuzz.chance(1, 2),
                    }
                };
                ("signal type", true)
            }
            13 if !model.signals.is_empty() => {
                // An alias of itself, of another alias or of nothing.
                let at = fuzz.index(model.signals.len());
                let target = fuzz.index(model.signals.len() + 2);
                model.signals[at].alias = Some(target);
                ("signal alias", true)
            }
            14 if !model.processes.is_empty() => {
                let at = fuzz.index(model.processes.len());
                model.processes[at].body.clear();
                ("empty process body", true)
            }
            15 if model.processes.len() > 1 => {
                let at = fuzz.index(model.processes.len());
                let other = fuzz.index(model.processes.len());
                let body = model.processes[other].body.clone();
                model.processes[at].body = body;
                ("process body of another process", true)
            }
            16 if !model.funcs.is_empty() => {
                let at = fuzz.index(model.funcs.len());
                let signals = model.signals.len();
                model.funcs[at].return_signal = Some(fuzz.index(signals + 3));
                ("function return signal", true)
            }
            17 if !model.spawns.is_empty() => {
                let at = fuzz.index(model.spawns.len());
                model.spawns[at] = "llg_no_such_process".to_owned();
                ("spawn of an unknown process", true)
            }
            _ => ("", false),
        };
        if done {
            return name.to_owned();
        }
    }
    "nothing".to_owned()
}

#[test]
fn component_mutated_models_are_rejected_or_emitted() {
    let models: Vec<IrModel> = DESIGNS.iter().map(|source| lowered(source)).collect();
    for model in &models {
        model.validate().expect("the lowered model is valid");
    }
    let (mut rejected, mut emitted) = (0u32, 0u32);
    for case in fuzz::cases(900) {
        let mut fuzz = Fuzz::new("ir/tables", case);
        let design = fuzz.index(models.len());
        let mut model = models[design].clone();
        let mut applied = Vec::new();
        for _ in 0..fuzz.range(1, 2) {
            applied.push(mutate(&mut model, &mut fuzz));
        }
        let label = format!("ir/tables case {case}: design {design}, {applied:?}");
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            model.validate().map_err(|error| error.to_string())?;
            for optimization in [
                crate::sim::opt::OptConfig::default(),
                crate::sim::opt::OptConfig::none(),
            ] {
                let mut execution = crate::sim::execution::ExecutionModel::lower(model.clone())
                    .map_err(|error| error.to_string())?;
                crate::sim::opt::run(&mut execution, &optimization)
                    .map_err(|error| error.to_string())?;
                execution.validate().map_err(|error| error.to_string())?;
                let source = crate::sim::emit_c::render(&execution)
                    .map_err(|error| format!("{REFUSED}: {error}"))?;
                if case % COMPILED_SAMPLE == 0 {
                    compile(&source, &label);
                }
            }
            Ok(())
        }));
        match outcome {
            Ok(Ok(())) => emitted += 1,
            Ok(Err(error)) => {
                assert!(
                    !error.starts_with(REFUSED)
                        || KNOWN_REFUSALS.iter().any(|known| error.ends_with(known)),
                    "{label}: {error}"
                );
                rejected += 1;
            }
            Err(_) => panic!("{label}: panic"),
        }
    }
    // Both outcomes occur: the corpus damages references and also leaves
    // models that are still consistent.
    if fuzz::cases(900) == (0..900) {
        assert!(
            rejected > 100 && emitted > 10,
            "{rejected} rejected, {emitted} emitted"
        );
    }
}
