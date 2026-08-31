//! End-to-end simulator tests for $display/$write/$monitor/$strobe,
//! hierarchical reads, signed-%d formatting, and bare event controls (Surelog
//! compile → codegen → CMake build → run, like tests/sim_counter.rs).
//!
//! Surelog writes `slpp_all/` into the process working directory, so each test
//! runs with the CWD pointed at a fresh temp dir (serialized through a mutex).

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Run one design end-to-end and return its stdout.  The caller must hold
/// `SURELOG_LOCK` and have set the CWD to a fresh temp dir.
fn run_design(dir: &std::path::Path, name: &str, sv: &str) -> Result<String, String> {
    let src = dir.join(name);
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;
    let out = compile::compile(&compile::CompileOpts {
        files: vec![src.to_string_lossy().into_owned()],
        top: Some("tb".to_string()),
        ..Default::default()
    })
    .map_err(|e| format!("compile: {e}"))?;
    if !out.ok() {
        return Err(format!("compile diagnostics: {:?}", out.diagnostics));
    }
    let design = out.uhdm_design().ok_or("no UHDM design")?;
    let gen = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;
    let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
        .map_err(|e| format!("cmake: {e}"))?;
    let output = Command::new(&exe)
        .output()
        .map_err(|e| format!("run: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "sim exited with {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run `sv` in a fresh temp dir (holding the Surelog mutex) and assert the
/// exact stdout.
fn assert_stdout(tag: &str, sv: &str, expected: &str) {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run_design(&dir, "tb.sv", sv);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = result.expect("simulation should run");
    assert_eq!(stdout, expected);
}

/// (a) $monitor of a counter: prints at registration (t=0) and then only when
/// the displayed value changes.
///
/// Hand-simulation:
///   t=0  clk=X count=X.  Spawn order: always@(posedge clk) registers its
///        waiter (last-seen clk=X), always#5 waits t=5, initial: clk=0,
///        count=0, $monitor prints "count=0" (registration, snapshot=0), #30.
///   t=5  always#5: clk=1 -> 0->1 POSEDGE -> counter wakes; count<=1 commits
///        in the NBA region; monitor check: 1 != 0 -> "count=1".
///   t=10 clk=0 (negedge).
///   t=15 clk=1 posedge -> count<=2 -> "count=2".
///   t=20 clk=0.
///   t=25 clk=1 posedge -> count<=3 -> "count=3".
///   t=30 initial: $finish (no print: value unchanged at this step).
///
/// Expected stdout (exactly):
///   count=0
///   count=1
///   count=2
///   count=3
#[test]
fn sim_monitor_change_detection() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg [3:0] count;
    always @(posedge clk) count <= count + 1;
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        count = 0;
        $monitor("count=%0d", count);
        #30 $finish;
    end
endmodule
"#;
    assert_stdout("mon", sv, "count=0\ncount=1\ncount=2\ncount=3\n");
}

/// (b) $monitoroff suspends printing (snapshot kept); $monitoron resumes and
/// re-prints when the value changed while suspended.
///
/// Hand-simulation:
///   t=0  registration prints "count=0"; #10.
///   t=5  posedge -> count=1 -> "count=1".
///   t=10 initial: $monitoroff; #10.
///   t=15 posedge -> count=2 (monitor off -> no print).
///   t=20 initial: $monitoron -> count=2 differs from last printed (1) ->
///        "count=2"; #20.  (The always#5 at t=20 is a negedge: no wake.)
///   t=25 posedge -> count=3 -> "count=3".
///   t=30 negedge.
///   t=35 posedge -> count=4 -> "count=4".
///   t=40 initial: $finish.
///
/// Expected stdout (exactly):
///   count=0
///   count=1
///   count=2
///   count=3
///   count=4
#[test]
fn sim_monitor_off_on() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg [3:0] count;
    always @(posedge clk) count <= count + 1;
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        count = 0;
        $monitor("count=%0d", count);
        #10 $monitoroff;
        #10 $monitoron;
        #20 $finish;
    end
endmodule
"#;
    assert_stdout(
        "monoff",
        sv,
        "count=0\ncount=1\ncount=2\ncount=3\ncount=4\n",
    );
}

/// (c) $strobe prints at the END of the time step (post-NBA) while a plain
/// $display at the same time sees the PRE-NBA value.
///
/// Hand-simulation:
///   t=0  clk=0, count=0 (blocking).  always@(posedge) registers its waiter;
///        always#5 waits t=5; initial waits #5 -> t=5.
///   t=5  wake order (both registered at t=0): always#5 first -> clk=1
///        (0->1 POSEDGE wakes the counter); then the initial: $display reads
///        count=0 (NBA not yet committed) -> "disp=0"; $strobe queued; #5.
///        Counter runs: count<=1; NBA region commits count=1; the strobe is
///        flushed with the committed value -> "stro=1".
///   t=10 always#5: clk=0 (negedge); initial: $finish.
///
/// Expected stdout (exactly):
///   disp=0
///   stro=1
#[test]
fn sim_strobe_sees_nba() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg [3:0] count;
    always @(posedge clk) count <= count + 1;
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        count = 0;
        #5 begin
            $display("disp=%0d", count);
            $strobe("stro=%0d", count);
        end
        #5 $finish;
    end
endmodule
"#;
    assert_stdout("stro", sv, "disp=0\nstro=1\n");
}

/// (d) A 3-part hierarchical READ (`u_mid.u_child.inner`) from the top; the
/// value is updated by the child on posedge and reset through the 2-part path
/// from the middle instance.
///
/// Hand-simulation:
///   t=0  mid's initial: u_child.inner = 0 (2-part write from mid's scope);
///        tb: clk=0.
///   t=5  posedge (clk 0->1) -> inner<=1 -> inner=1.
///   t=15 posedge -> inner=2.
///   t=25 posedge -> inner=3.
///   t=34 tb: $display("inner=3"); #6.
///   t=35 posedge -> inner=4 (not printed).
///   t=40 tb: $finish.
///
/// Expected stdout (exactly):
///   inner=3
#[test]
fn sim_hierarchical_read() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module child_mod(input logic clk);
    reg [3:0] inner;
    always @(posedge clk) inner <= inner + 1;
endmodule

module mid_mod(input logic clk);
    child_mod u_child(.clk(clk));
    initial u_child.inner = 4'd0;
endmodule

module tb;
    reg clk;
    mid_mod u_mid(.clk(clk));
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        #34 $display("inner=%0d", u_mid.u_child.inner);
        #6 $finish;
    end
endmodule
"#;
    assert_stdout("hier", sv, "inner=3\n");
}

/// (e) Signed %d: the runtime honours `is_signed` and prints negative values
/// for a signed variable whose sign bit is set, and for a negated unsized
/// decimal literal.
///
///   neg = 8'hff  -> 0xFF as a signed 8-bit value = -1.
///   neg = 8'h80  -> 0x80 as a signed 8-bit value = -128.
///   neg = 8'h7f  -> 127.
///   -3           -> unary minus of the (LRM-signed) unsized decimal literal.
///
/// Expected stdout (exactly):
///   neg=-1
///   min=-128
///   pos=127
///   lit=-3
#[test]
fn sim_signed_display() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg signed [7:0] neg;
    initial begin
        neg = 8'hff;
        $display("neg=%0d", neg);
        neg = 8'h80;
        $display("min=%0d", neg);
        neg = 8'h7f;
        $display("pos=%0d", neg);
        $display("lit=%0d", -3);
        $finish;
    end
endmodule
"#;
    assert_stdout("signed", sv, "neg=-1\nmin=-128\npos=127\nlit=-3\n");
}

/// `$write` uses the same formatting as `$display` but does not append a
/// newline. Consecutive writes therefore remain on one line, and a following
/// display terminates that line exactly once.
#[test]
fn sim_write_does_not_append_newline() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] value;
    initial begin
        value = 8'h2a;
        $write("value=%0d", value);
        $write(" hex=%0h", value);
        $display(" done");
        $write("tail");
        $finish;
    end
endmodule
"#;
    assert_stdout("write", sv, "value=42 hex=2a done\ntail");
}

/// (f) A bare event control (`always @(posedge clk);` — a statement with only
/// the wait) must compile and the process must wait without erroring.
///
/// Hand-simulation:
///   t=0  clk=0 (blocking); the bare always registers its posedge waiter;
///        always#5 waits t=5; initial waits #25.
///   t=25 initial: $display("done at 25"); $finish.
///
/// Expected stdout (exactly):
///   done at 25
#[test]
fn sim_bare_event_control() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    always @(posedge clk);
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        #25 $display("done at %0t", $time);
        $finish;
    end
endmodule
"#;
    assert_stdout("bare_ev", sv, "done at 25\n");
}
