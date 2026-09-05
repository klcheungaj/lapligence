//! End-to-end simulator coverage for true-net declaration assignments and
//! codegen rejection of executable statement placeholders.

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

fn compile_and_generate(
    dir: &std::path::Path,
    file: &str,
    sv: &str,
) -> Result<sim::codegen::GeneratedModel, String> {
    let src = dir.join(file);
    std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
    let out = compile::compile_checked(&compile::CompileOpts {
        files: vec![src.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    let design = out.uhdm_design().ok_or("no UHDM design")?;
    sim::codegen::generate(design).map_err(|error| format!("codegen: {error}"))
}

fn in_temp_dir<R>(tag: &str, run: impl FnOnce(&std::path::Path) -> R) -> R {
    let dir = std::env::temp_dir().join(format!("llg_sim_net_decl_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run(&dir);
    std::env::set_current_dir(original).expect("restore cwd");
    let _ = std::fs::remove_dir_all(dir);
    result
}

#[test]
fn dynamic_true_net_declarations_match_explicit_assigns() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let result = in_temp_dir("equivalence", |dir| {
        let generated = compile_and_generate(
            dir,
            "equivalence.sv",
            r#"// llg-test-fixture: tests/sim_net_decl.rs/equivalence.sv
module tb;
    logic [3:0] a = 4'hf;
    logic [3:0] b = 4'h1;
    wire [4:0] declared = a + b;
    wire [4:0] explicit;
    assign explicit = a + b;
    tri [7:0] declared_copy = {a, b};
    wire [7:0] explicit_copy;
    assign explicit_copy = {a, b};
    wire [9:0] generated_bus;

    genvar i;
    for (i = 0; i < 2; i = i + 1) begin : g
        wire [4:0] generated = a + b + i;
        assign generated_bus[i * 5 +: 5] = generated;
    end

    initial begin
        $display("t0=%h/%h copy=%b/%b", declared, explicit,
                 declared_copy, explicit_copy);
        a = 4'h2;
        b = 4'h3;
        #1 $display("t1=%h/%h gen=%h,%h", declared, explicit,
                    generated_bus[4:0], generated_bus[9:5]);
        a = 4'b10xz;
        b = 4'b0011;
        #1 $display("t2=%h/%h copy=%b/%b", declared, explicit,
                    declared_copy, explicit_copy);
        a = 4'bzz01;
        #1 $display("t3=%h/%h copy=%b/%b", declared, explicit,
                    declared_copy, explicit_copy);
        $finish;
    end
endmodule
"#,
        )?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", &generated.model_c)])
            .map_err(|error| format!("cmake: {error}"))?;
        let output = Command::new(exe)
            .output()
            .map_err(|error| format!("run: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "sim exited with {:?}, stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    });

    assert_eq!(
        result.expect("simulation should run"),
        "t0=10/10 copy=11110001/11110001\n\
         t1=05/05 gen=05,06\n\
         t2=xx/xx copy=10xz0011/10xz0011\n\
         t3=xx/xx copy=zz010011/zz010011\n"
    );
}

#[test]
fn net_declaration_rejects_unrepresentable_sensitivity_and_net_class() {
    let _guard = SURELOG_LOCK.lock().unwrap();

    let array_error = in_temp_dir("array_reject", |dir| {
        compile_and_generate(
            dir,
            "array_reject.sv",
            r#"// llg-test-fixture: tests/sim_net_decl.rs/array_reject.sv
module tb;
    logic [7:0] memory [0:1];
    logic index;
    wire [7:0] value = memory[index];
    initial $finish;
endmodule
"#,
        )
    })
    .err()
    .expect("array-sensitive net declaration must be rejected");
    assert!(
        array_error.contains("reads an unpacked array")
            && array_error.contains("sensitivity cannot be represented"),
        "unexpected array rejection: {array_error}"
    );

    let net_class_error = in_temp_dir("net_class_reject", |dir| {
        compile_and_generate(
            dir,
            "net_class_reject.sv",
            r#"// llg-test-fixture: tests/sim_net_decl.rs/net_class_reject.sv
module tb;
    logic source;
    wand value = source;
    initial $finish;
endmodule
"#,
        )
    })
    .err()
    .expect("unsupported resolved-net declaration must be rejected");
    assert!(
        net_class_error.contains("unsupported net type")
            && net_class_error.contains("only wire/tri/logic nets"),
        "unexpected net-class rejection: {net_class_error}"
    );
}
