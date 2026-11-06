use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/rtl_101";

#[test]
fn record_member_arrays_beyond_the_dense_threshold_are_columns() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/member_columns.out");
    sim_cli::run_case(SUITE, "member_columns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_columns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "member_columns", expected);
}

#[test]
fn records_beyond_the_packed_limit_move_as_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/oversized_values.out");
    sim_cli::run_case(SUITE, "oversized_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "oversized_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "oversized_values", expected);
}

#[test]
fn tagged_unions_beyond_the_packed_limit_move_as_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/tagged_columns.out");
    sim_cli::run_case(SUITE, "tagged_columns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "tagged_columns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "tagged_columns", expected);
}

#[test]
fn column_records_match_structure_patterns() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/record_patterns.out");
    sim_cli::run_case(SUITE, "record_patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "record_patterns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_patterns", expected);
}

#[test]
fn inactive_column_union_members_report_runtime_errors() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/rtl_101/tagged_inactive.sv");
    let location = sim_harness::source_display(&source);
    let expected_stderr = format!(
        "llg: runtime error: access to inactive tagged-union member w at {location}:10:23\n\
         llg: runtime error: access to inactive tagged-union member w at {location}:11:5\n"
    );
    sim_cli::run_case_checked_matrix(SUITE, "tagged_inactive", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        // The guarded read yields X and the guarded write leaves the active
        // member unchanged.
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "A x\nB 4\n",
            "{label}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    });
}

#[test]
fn generated_model_scales_with_the_declaration() {
    let expected = "5a 3 x\n";
    sim_cli::run_case(SUITE, "scale_65537", expected, "", &[]);
    sim_cli::run_case(SUITE, "scale_1048576", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "scale_1048576", expected, &[], &[]);
    for optimized in [false, true] {
        let mut sizes = Vec::new();
        for fixture in ["scale_65537", "scale_1048576"] {
            let directory = sim_harness::TempDir::new("rtl101-scale").expect("scratch");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "tests/fixtures/sim/feature_completion/rtl_101/{fixture}.sv"
            ));
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .arg(source)
                .args(["--top", "tb", "--gen-only", "--out-dir"]);
            command.arg(directory.path());
            if !optimized {
                command.arg("--no-opt");
            }
            let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
                .expect("generate column-record model");
            assert!(output.status.success(), "{output:?}");
            let model = std::fs::read_to_string(directory.path().join("sim/tb/model.c"))
                .expect("model source");
            sizes.push(model.len());
        }
        // A per-cell layout grows with the member extent; columns differ
        // only in the spelled bounds.
        assert!(sizes.iter().all(|size| *size < 100_000), "{sizes:?}");
        assert!(sizes[0].abs_diff(sizes[1]) < 256, "{sizes:?}");
    }
}

#[test]
fn whole_values_bind_and_record_results_compare() {
    // `x.tag` of the bound copy of `r` (tag 3).
    sim_cli::run_case(SUITE, "whole_binding", "3\n", "", &[]);
    // `t` is equal and every `w` cell is X, so equality is unknown.
    sim_cli::run_case(SUITE, "record_result_operand", "x\n", "", &[]);
}

#[test]
fn neg_column_record_single_writer() {
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_member_write",
        "variable storage `tb.q.a` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_output_port_member_write",
        "variable storage `tb.q.w` has both a continuous assignment",
    );
}

/// `model.c` of `source`, generated with `arguments`.
fn generated_model(source: &str, arguments: &[&str]) -> String {
    let directory = sim_harness::TempDir::new("record_choice").expect("scratch directory");
    std::fs::write(directory.path().join("t.sv"), source).expect("write source");
    let output = sim_harness::run_command(
        Command::new(env!("CARGO_BIN_EXE_llg"))
            .current_dir(directory.path())
            .args(["--gen-only", "--top", "tb", "--out-dir", "out"])
            .args(arguments)
            .arg("t.sv"),
        Duration::from_secs(60),
    )
    .expect("run llg");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(directory.path().join("out/sim/tb/model.c")).expect("read model")
}

fn selected_record(member: &str, size: usize, select: &str) -> String {
    format!(
        "module tb;
  typedef struct {{ {member} int a[{size}]; }} r_t;
  r_t r;
  int k = 3, y;
  initial begin
    r.a[{select}] = 7;
    y = r.a[{select}];
    $display(\"%0d\", y);
    $finish(0);
  end
endmodule
"
    )
}

#[test]
fn selected_member_arrays_below_the_dense_threshold_match_the_leaf_form() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/chosen_columns.out");
    sim_cli::run_case(SUITE, "chosen_columns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "chosen_columns", expected, &[], &[]);
}

#[test]
fn whole_values_of_chosen_columns_match_the_leaf_form() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_101/chosen_whole_values.out");
    sim_cli::run_case(SUITE, "chosen_whole_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "chosen_whole_values", expected, &[], &[]);
}

#[test]
fn whole_operands_of_column_records_need_no_image() {
    let model = generated_model(
        "module tb;
  typedef struct { byte t; int a[5000]; } r_t;
  r_t r, q;
  initial begin
    q = r;
    $display(\"%b\", q == r);
    $finish(0);
  end
endmodule
",
        &[],
    );
    let scratch = "llg_value_scope_begin_object(sizeof(llg_fixed_array_t)";
    assert!(!model.contains(scratch), "{model}");
}

#[test]
fn uses_columns_lack_keep_the_leaf_form() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/chosen_fallback.out");
    sim_cli::run_case(SUITE, "chosen_fallback", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "chosen_fallback", expected, &[], &[]);
}

#[test]
fn selections_of_member_arrays_do_not_grow_with_the_array() {
    let lines = |member: &str, size: usize, select: &str| {
        generated_model(&selected_record(member, size, select), &[])
            .lines()
            .count()
    };
    // A run-time index, in a record with a native member and in a fixed one.
    assert_eq!(lines("string s;", 64, "k"), lines("string s;", 4000, "k"));
    assert_eq!(lines("byte s;", 64, "k"), lines("byte s;", 4000, "k"));
    // A fixed record flattens on a constant select as well.
    assert_eq!(lines("byte s;", 64, "2"), lines("byte s;", 4000, "2"));
}

#[test]
fn leaf_storage_stays_where_columns_do_not_pay_or_observers_need_it() {
    let column = "llg_fixed_array_cell(";
    let run_time = selected_record("string s;", 64, "k");
    assert!(generated_model(&run_time, &[]).contains(column));
    // Constant selects of a native record are direct.
    let constant = selected_record("string s;", 64, "2");
    assert!(!generated_model(&constant, &[]).contains(column));
    // Below the minimum a chain of comparisons stays.
    let small = selected_record("string s;", 8, "k");
    assert!(!generated_model(&small, &[]).contains(column));
    // A catalog exposes each member leaf (`tb.r`, `tb.r.x`).
    assert!(!generated_model(&run_time, &["--vpi"]).contains(column));
}

#[test]
fn chosen_columns_dump_under_their_leaf_names() {
    let names = |size: usize| {
        let directory = sim_harness::TempDir::new("record_wave").expect("scratch directory");
        let source = format!(
            "module tb;
  typedef struct {{ logic [1:0] z; int c[2]; }} in_t;
  typedef struct {{ logic [3:0] x; int a[{size}]; in_t i; }} r_t;
  r_t r;
  int k = 1;
  initial begin
    #1 r.a[k] = 7;
    r.x = 2;
    r.i.z = 1;
    r.i.c[1] = 1;
    #1 $finish(0);
  end
endmodule
"
        );
        std::fs::write(directory.path().join("t.sv"), source).expect("write source");
        let output = sim_harness::run_command(
            Command::new(env!("CARGO_BIN_EXE_llg"))
                .current_dir(directory.path())
                .args(["--top", "tb", "--wave", "t.vcd", "--out-dir", "out", "t.sv"]),
            Duration::from_secs(120),
        )
        .expect("run llg");
        assert!(output.status.success(), "{output:?}");
        let dump = std::fs::read_to_string(directory.path().join("t.vcd")).expect("read dump");
        dump.lines()
            .filter(|line| line.starts_with("$var"))
            .filter_map(|line| line.split_whitespace().nth(4).map(str::to_owned))
            .collect::<Vec<_>>()
    };
    let names = names(16);
    assert!(names.iter().any(|name| name == "x"), "{names:?}");
    assert!(names.iter().any(|name| name == "a__i15"), "{names:?}");
    assert!(names.iter().any(|name| name == "i__z"), "{names:?}");
    assert!(names.iter().any(|name| name == "i__c__i1"), "{names:?}");
    assert!(!names.iter().any(|name| name.contains("$5B")), "{names:?}");
}

#[test]
fn a_constant_cell_of_a_column_wakes_only_on_that_cell() {
    let model = generated_model(
        "module tb;
  typedef struct { logic [7:0] t; int a[5000]; } r_t;
  r_t r;
  int y;
  always_comb y = r.a[3];
  initial begin r.a[4] = 1; $finish(0); end
endmodule
",
        &[],
    );
    let waits = model
        .lines()
        .filter(|line| line.contains("llg_arm_event_dependencies"))
        .collect::<Vec<_>>();
    assert!(!waits.is_empty(), "{model}");
    assert!(
        waits.iter().all(|line| !line.contains("_llg_contents_dep")),
        "{waits:?}"
    );
}
