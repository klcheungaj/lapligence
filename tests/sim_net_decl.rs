//! End-to-end simulator coverage for true-net declaration assignments and
//! codegen rejection of executable statement placeholders.

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;
#[path = "support/sim_cli.rs"]
mod sim_cli;

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
    let db =
        llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
    sim::codegen::generate(&db).map_err(|error| format!("codegen: {error}"))
}

#[test]
fn dynamic_true_net_declarations_match_explicit_assigns() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"// llg-test-fixture: tests/sim_net_decl.rs/equivalence.sv
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
"#;
    let result = sim_harness::run_sim(sv, "tb", "net_decl_equivalence");

    assert_eq!(
        result.expect("simulation should run"),
        "t0=10/10 copy=11110001/11110001\n\
         t1=05/05 gen=05,06\n\
         t2=xx/xx copy=10xz0011/10xz0011\n\
         t3=xx/xx copy=zz010011/zz010011\n"
    );
}

#[test]
fn net_declaration_tracks_unpacked_array_and_index_changes() {
    sim_cli::run_case(
        "regression_81",
        "net_decl_array_reads",
        "value=11\nvalue=33\nvalue=22\nvalue=44\n",
        "",
        &[],
    );
}

#[test]
fn net_declaration_rejects_unsupported_net_class() {
    let net_class_error = sim_harness::with_frontend_temp_cwd("net_class_reject", |dir| {
        compile_and_generate(
            dir,
            "net_class_reject.sv",
            r#"// llg-test-fixture: tests/sim_net_decl.rs/net_class_reject.sv
module tb;
    logic source;
    trireg value = source;
    initial $finish;
endmodule
"#,
        )
    })
    .err()
    .expect("unsupported trireg declaration must be rejected");
    assert!(
        net_class_error.contains("unsupported net type")
            && net_class_error.contains("outside the standalone subset"),
        "unexpected net-class rejection: {net_class_error}"
    );
}
