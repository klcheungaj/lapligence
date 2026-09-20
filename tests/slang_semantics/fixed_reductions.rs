//! R03: owned method result types and lexical iterator identities reach lowering.

use llg::core::compile;
use llg::core::db::{Db, NodeKind};
use std::collections::HashSet;

fn capture(source: &str) -> Db {
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit("fixed-reductions.sv", source)],
        &compile::CompileOpts {
            top: Some("tb".into()),
            ..Default::default()
        },
    )
    .expect("valid fixed-array reduction source");
    Db::from_slang(&compiled.snapshot).expect("owned reduction import")
}

#[test]
fn fixed_array_reduction_import_retains_self_determined_types_and_distinct_iterators() {
    let database = capture(
        r#"// llg-test-fixture: tests/slang_semantics/fixed_reductions.rs/types-and-iterators
module tb;
    logic [7:0] values [-1:0];
    logic [7:0] matrix [1:0][-1:0];
    int result;
    initial begin
        values[-1] = 200; values[0] = 56;
        matrix[1][-1] = 5; matrix[1][0] = 10;
        matrix[0][-1] = 15; matrix[0][0] = 20;
        result = values.sum();
        result = values.sum() with (int'(item));
        result = matrix.sum() with (item.sum() with (int'(item)));
        $display("%0d", result);
    end
endmodule
"#,
    );
    database.validate().expect("valid imported database");
    let mut widths = Vec::new();
    let mut iterators = HashSet::new();
    for id in database.node_ids() {
        let NodeKind::MethodCall { name, receiver, .. } = database.node_kind(id) else {
            continue;
        };
        if name != "sum" {
            continue;
        }
        assert!(receiver.is_some());
        widths.push(
            database
                .type_descriptor(id)
                .expect("method result descriptor")
                .info
                .width,
        );
        if database.method_call_has_with_clause(id) {
            let iterator = database
                .method_call_iterator(id)
                .expect("owned iterator identity");
            assert!(
                iterators.insert(iterator),
                "nested maps must not share iterator declarations"
            );
        }
    }
    widths.sort();
    assert_eq!(widths, [Some(8), Some(32), Some(32), Some(32)]);
    assert_eq!(iterators.len(), 3);
    let generated = llg::sim::codegen::generate(&database)
        .expect("owned reductions lower after capture is dropped");
    assert!(generated.model_c.contains("reduction_ordinal"));
}

#[test]
fn fixed_array_reduction_maps_keep_enclosing_automatic_values_during_lowering() {
    let database = capture(include_str!("../fixtures/sim/fixed_array_reductions/functions.sv"));
    database.validate().expect("valid captured activation graph");
    let generated = llg::sim::codegen::generate(&database)
        .expect("lexical maps lower in the caller frame");
    assert!(generated.model_c.contains("reduction_ordinal"));
}
