//! End-to-end packed Verilog string constant simulation tests.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile;
use llg::core::db::Db;
use llg::sim;
use llg::sim::opt::OptConfig;

fn run_both(sv: &str, tag: &str) -> Result<(String, String), String> {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let source = dir.join("tb.v");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| format!("db: {error}"))?;

        let variants = [
            ("opt_on", OptConfig::default()),
            ("opt_off", OptConfig::none()),
        ];
        let mut outputs = Vec::with_capacity(variants.len());
        for (name, opts) in variants {
            let generated = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| format!("codegen({name}): {error}"))?;
            let build_dir = dir.join(name);
            let executable =
                sim::build::build_model_cmake(&build_dir, &[("model.c", &generated.model_c)])
                    .map_err(|error| format!("cmake({name}): {error}"))?;
            outputs.push(
                sim_harness::run_executable(&executable)
                    .map_err(|error| format!("run({name}): {error}"))?,
            );
        }
        Ok((outputs.remove(0), outputs.remove(0)))
    })
}

#[test]
fn packed_strings_are_integral_expression_operands() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"// llg-test-fixture: tests/sim_packed_strings.rs/operands.v
module tb;
    localparam [15:0] PARAM_EXACT = "AB";
    reg [15:0] initialized_from_param = PARAM_EXACT;
    reg [31:0] exact;
    reg [23:0] padded;
    reg [7:0] truncated;
    reg [39:0] escaped;
    reg [23:0] concatenated;
    reg [7:0] octal_escape;
    reg [71:0] multi_limb;
    reg [23:0] embedded_nul;
    reg [7:0] empty;
    reg [31:0] sv_escapes;

    initial begin
        exact = "ABCD";
        padded = "A";
        truncated = "AB";
        escaped = "A\n\t\"\\";
        concatenated = {"AB", 8'h43};
        octal_escape = "\101";
        multi_limb = "ABCDEFGHI";
        embedded_nul = "A\000B";
        empty = "";
        sv_escapes = "\v\f\a\x41";

        $display("exact=%h padded=%h truncated=%h", exact, padded, truncated);
        $display("escaped=%h concat=%h octal=%h", escaped, concatenated, octal_escape);
        $display("multi=%h", multi_limb);
        $display("nul=%h empty=%h sv_esc=%h", embedded_nul, empty, sv_escapes);
        $display("param_init=%h", initialized_from_param);
        $display("eq=%0d ne=%0d lt=%0d oct_eq=%0d", "AB" == 16'h4142,
                 "AB" != 16'h4142, "AB" < "AC", "\101" == 8'h41);
        $finish;
    end
endmodule
"#;

    let (on, off) = run_both(sv, "packed_strings").expect("both variants should run");
    let expected = concat!(
        "exact=41424344 padded=000041 truncated=42\n",
        "escaped=410a09225c concat=414243 octal=41\n",
        "multi=414243444546474849\n",
        "nul=410042 empty=00 sv_esc=0b0c0741\n",
        "param_init=4142\n",
        "eq=1 ne=0 lt=1 oct_eq=1\n",
    );
    assert_eq!(on, expected);
    assert_eq!(off, expected);
}

#[test]
fn packed_string_1032_bits_is_supported() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_type_edges/wide_string_1032.v");
    let source = std::fs::read_to_string(&fixture).expect("read wide-string fixture");
    let (on, off) =
        run_both(&source, "packed_string_1032").expect("both wide-string variants should run");
    assert_eq!(on, "PASS wide_string_1032\n");
    assert_eq!(off, "PASS wide_string_1032\n");
}
