//! Whole unpacked-array values over runs of consecutive cells, in dense and
//! descriptor storage: hand-computed values in both optimizer modes, and
//! generated C that does not grow with the element count.
use crate::sim_cli;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

const SUITE: &str = "dense_cell_runs";
/// Generated C budget for the large fixtures, about twice the 4096-element
/// one. Per-element emission produced 42 MB for that fixture (12 MB for one
/// whole read of a 4096-record array) and the formal's default value as a
/// limb table another 60 KB; with range calls and a periodic default the
/// model is about 31 KB. `descriptor_runs` (5000 elements) is about 41 KB;
/// per element it was 17.3 MB.
const MODEL_C_CAP_BYTES: usize = 64 * 1024;

#[test]
fn whole_values_keep_element_semantics() {
    sim_cli::run_case(
        SUITE,
        "whole_values",
        "A 0 15 27\n\
         shift 0 0 21 27\n\
         cond 5 0 4 40\n\
         pick 1 60\n\
         rb 0 0 x01z 30 7 70\n\
         eq x 1 x 0\n\
         ne 0 0\n\
         out -1 10 70\n\
         m2 10 14 24\n\
         m0 30 34 10\n\
         overlap '{1, 1, 2, 3, 4, 5, 6, 8}\n\
         cont 1 1 8\n\
         inout '{2, 2, 3, 4, 5, 6, 7, 9}\n\
         func '{9, 7, 6, 5, 4, 3, 2, 2}\n\
         fill 5a 5a\n\
         default '{51, 51, 68, 51, 51, 51, 51, 51}\n\
         records c 7 c 7\n\
         bits '{100, 101, 102, 103, 104, 105, 106, 107} 1\n\
         short '{1, 2, 3, 4, 5, 6, 7} '{8, 9, 10, 11, 12, 13, 14, 15}\n\
         nba-before 0\n\
         nba-after 60 x01z\n",
        "",
        &[],
    );
}

/// Generate `fixture` in both optimizer modes (no C compiler) and check the
/// `model.c` budget.
fn assert_model_c_within_budget(fixture: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("fixture compiles");
    let db = Db::from_slang(&compiled.snapshot).expect("owned database");
    for (variant, opts) in [("opt", OptConfig::default()), ("no-opt", OptConfig::none())] {
        let model = sim::codegen::generate_from_db_with_opts(&db, &opts).expect("codegen");
        assert!(
            model.model_c.len() <= MODEL_C_CAP_BYTES,
            "{fixture} {variant}: model.c has {} bytes",
            model.model_c.len()
        );
    }
}

#[test]
fn whole_read_of_4096_elements_is_bounded_and_exact() {
    assert_model_c_within_budget("whole_read_4096");
    sim_cli::run_case(
        SUITE,
        "whole_read_4096",
        "4095 17 1 255 1 1 18721\n",
        "",
        &[],
    );
}

#[test]
fn descriptor_array_runs_are_bounded_and_exact() {
    assert_model_c_within_budget("descriptor_runs");
    sim_cli::run_case(
        SUITE,
        "descriptor_runs",
        "pass 1 10 x 7 z -2 x 0 1\n\
         bump 2 6 xx 15001 x, x, 6}\n",
        "",
        &[],
    );
}

#[test]
fn whole_copy_of_different_sizes_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "size_mismatch",
        "value of type 'int$[24:1]' cannot be assigned to type 'int$[10:1]'",
    );
}
