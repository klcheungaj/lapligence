//! Generation does work in proportion to its source: each test counts one kind
//! of work at two sizes, so it does not depend on the host's speed.

use crate::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use crate::core::db::Db;
use crate::sim::codegen::GeneratedModel;
use crate::sim::opt::OptConfig;
use crate::work::{counted, Work};

fn database(source: &str) -> Db {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit("tb.sv", source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("the source compiles");
    Db::from_slang(&compiled.snapshot).expect("owned database")
}

fn generate(db: &Db) -> GeneratedModel {
    crate::sim::codegen::generate_from_db_with_opts(db, &OptConfig::default())
        .expect("the design generates")
}

/// The `work` of generating `source`, from its text to its C, on the stack a
/// compile runs on.
fn generation_work(work: Work, source: &str) -> usize {
    crate::stack::run_pipeline(|| {
        counted(work, || {
            generate(&database(source));
        })
    })
    .expect("a pipeline thread")
}

#[test]
fn only_bodies_with_a_counterpart_are_normalized() {
    let processes = |count: usize| -> String {
        (0..count)
            .map(|index| {
                format!(
                    "logic [7:0] v{index};\n\
                     function automatic int f{index}(int x); return x + v{index}; endfunction\n\
                     task automatic t{index}(); #1 v{index} = f{index}(2); endtask\n\
                     initial t{index}();\n"
                )
            })
            .collect()
    };
    let distinct = format!("module tb;\n{}endmodule\n", processes(64));
    assert_eq!(generation_work(Work::NormalizedBytes, &distinct), 0);
    let instances = |count: usize| {
        format!(
            "module cell_{count}; {}endmodule\nmodule tb;\n\
             for (genvar g = 0; g < {count}; g++) cell_{count} c();\nendmodule\n",
            processes(1)
        )
    };
    let normalized = [8, 16].map(|count| generation_work(Work::NormalizedBytes, &instances(count)));
    assert!(normalized[0] > 0);
    assert!(
        normalized[1] <= 2 * normalized[0] + normalized[0] / 4,
        "{normalized:?}"
    );
}
