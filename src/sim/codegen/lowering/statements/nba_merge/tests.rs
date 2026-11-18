use super::*;
use crate::core::db::Db;

/// Lower `source` (top `tb`) and run the merge on the lowered model.
fn merged(source: &str) -> (IrModel, usize) {
    let compiled = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "nba_merge.sv",
            source,
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    let database = Db::from_slang(&compiled.snapshot).unwrap();
    let mut lowered = super::super::super::lower_model(&database, &Default::default()).unwrap();
    let count = merge_overridden_nbas(&mut lowered.model, &lowered.monitor_reads);
    (lowered.model, count)
}

/// The nonblocking updates of every process, in statement order, as
/// `(signal name, branch path)`; the branch path names each enclosing arm.
fn updates(model: &IrModel) -> Vec<(String, String)> {
    fn walk(model: &IrModel, list: &[IrStmt], path: &str, out: &mut Vec<(String, String)>) {
        for statement in list {
            if let Some((lhs, _)) = nba(statement) {
                if let Some(signal) = target_signal(lhs) {
                    let name = model.signals[signal].hdl_name.clone().unwrap_or_default();
                    out.push((name.replace('\u{1f}', "."), path.to_owned()));
                }
                continue;
            }
            match statement.unlocated() {
                IrStmt::If { then_, els, .. } => {
                    walk(model, then_, &format!("{path}T"), out);
                    if let Some(els) = els {
                        walk(model, els, &format!("{path}E"), out);
                    }
                }
                IrStmt::Case { items, .. } => {
                    for (index, item) in items.iter().enumerate() {
                        let arm = if item.exprs.is_empty() {
                            "D".to_owned()
                        } else {
                            index.to_string()
                        };
                        walk(model, &item.body, &format!("{path}{arm}"), out);
                    }
                }
                other => {
                    let mut other = other.clone();
                    for child in child_lists(&mut other) {
                        walk(model, child, path, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for process in &model.processes {
        walk(model, &process.body, "", &mut out);
    }
    out
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(name, path)| (name.to_string(), path.to_string()))
        .collect()
}

#[test]
fn default_then_conditional_override_queues_one_update_per_path() {
    let (model, count) = merged(
        "module tb; logic clk, c, d, q;
           always @(posedge clk) begin q <= 1'b0; if (c) q <= d; end
         endmodule",
    );
    assert_eq!(count, 1);
    assert_eq!(updates(&model), pairs(&[("tb.q", "T"), ("tb.q", "E")]));
}

#[test]
fn adjacent_updates_keep_the_last_one() {
    let (model, count) = merged(
        "module tb; logic clk; logic [3:0] q;
           always @(posedge clk) begin q <= 4'd1; q <= 4'd2; q <= 4'd3; end
         endmodule",
    );
    assert_eq!(count, 2);
    assert_eq!(updates(&model), pairs(&[("tb.q", "")]));
}

#[test]
fn an_update_of_another_target_keeps_its_place_before_the_override() {
    let (model, count) = merged(
        "module tb; logic clk; logic [3:0] q, r;
           always @(posedge clk) begin q <= 4'd1; r <= q; q <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 1);
    assert_eq!(updates(&model), pairs(&[("tb.r", ""), ("tb.q", "")]));
}

#[test]
fn a_run_of_defaults_moves_into_the_branch_in_source_order() {
    let (model, count) = merged(
        "module tb; logic clk, c; logic [3:0] q, r, d, e;
           always @(posedge clk) begin
             q <= 4'd0; r <= 4'd0;
             if (c) begin q <= d; r <= e; end
           end
         endmodule",
    );
    assert_eq!(count, 2);
    assert_eq!(
        updates(&model),
        pairs(&[("tb.q", "T"), ("tb.r", "T"), ("tb.q", "E"), ("tb.r", "E")])
    );
}

#[test]
fn nested_branches_and_case_items_take_the_default() {
    let (model, count) = merged(
        "module tb; logic clk, a, b; logic [1:0] s; logic [3:0] q, w;
           always @(posedge clk) begin
             q <= 4'd1;
             if (a) begin if (b) q <= 4'd2; else q <= 4'd3; end
             w <= 4'd9;
             case (s) 2'd0: w <= 4'd10; 2'd1: ; 2'd2: w <= 4'd12; endcase
           end
         endmodule",
    );
    // Two arms override each default.
    assert_eq!(count, 4);
    assert_eq!(
        updates(&model),
        pairs(&[
            ("tb.q", "TT"),
            ("tb.q", "TE"),
            ("tb.q", "E"),
            ("tb.w", "0"),
            ("tb.w", "1"),
            ("tb.w", "2"),
            ("tb.w", "D"),
        ])
    );
}

#[test]
fn nothing_merges_across_other_statements() {
    let (model, count) = merged(
        "module tb; logic clk, c; logic [3:0] q, x;
           always @(posedge clk) begin q <= 4'd1; x = 4'd2; q <= 4'd2; end
           always @(posedge clk) begin q <= 4'd1; $display(\"%0d\", q); q <= 4'd2; end
           always @(posedge clk) begin q <= 4'd1; #1 q <= 4'd2; end
           always @(posedge clk) begin q <= #1 4'd1; q <= 4'd2; end
           always @(posedge clk) begin q <= 4'd1; unique if (c) q <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 0, "{:?}", updates(&model));
}

#[test]
fn effects_runtime_selectors_and_partial_overrides_stay() {
    let (_, count) = merged(
        "module tb; logic clk; logic [3:0] q, v; int i;
           function automatic logic [3:0] f(); i++; return 4'd1; endfunction
           always @(posedge clk) begin q <= f(); q <= 4'd2; end
           always @(posedge clk) begin q <= $random; q <= 4'd2; end
           always @(posedge clk) begin v[i] <= 1'b0; v[i] <= 1'b1; end
           always @(posedge clk) begin q <= 4'd1; q[0] <= 1'b0; end
         endmodule",
    );
    assert_eq!(count, 0);
}

#[test]
fn a_monitored_target_keeps_each_update() {
    let (_, count) = merged(
        "module tb; logic clk; logic [3:0] q, r;
           initial $monitor(\"%0d\", q);
           always @(posedge clk) begin q <= 4'd1; q <= 4'd2; end
           always @(posedge clk) begin r <= 4'd1; r <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 1);
}

#[test]
fn a_strobe_does_not_observe_writes() {
    let (_, count) = merged(
        "module tb; logic clk; logic [3:0] q;
           always @(posedge clk) $strobe(\"%0d\", q);
           always @(posedge clk) begin q <= 4'd1; q <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 1);
}

#[test]
fn a_model_with_waveforms_keeps_each_update() {
    let (_, count) = merged(
        "module tb; logic clk; logic [3:0] q;
           initial $dumpvars;
           always @(posedge clk) begin q <= 4'd1; q <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 0);
}

#[test]
fn a_clocking_input_keeps_each_update() {
    let (_, count) = merged(
        "module tb; logic clk; logic [3:0] q, r;
           clocking cb @(posedge clk); input #1 q; endclocking
           always @(posedge clk) begin q <= 4'd1; q <= 4'd2; end
           always @(posedge clk) begin r <= 4'd1; r <= 4'd2; end
           initial begin @(cb); $display(\"%0d\", cb.q); end
         endmodule",
    );
    assert_eq!(count, 1);
}

#[test]
fn a_branch_value_reading_a_local_does_not_move() {
    let (_, count) = merged(
        "module tb; logic clk, c; logic [3:0] q;
           always @(posedge clk)
             for (int i = 0; i < 2; i++) begin q <= i[3:0]; if (c) q <= 4'd2; end
         endmodule",
    );
    assert_eq!(count, 0);
}

/// Fixtures that check the NBA batch at run time keep both updates of a
/// turn: their overridden update has an impure value or a run-time-indexed
/// target. The same sources with plain values and constant targets merge,
/// so a widened rule that dropped these updates fails here.
/// A fixture name, its source, the rewrites that make it mergeable, and how
/// many merges the rewritten source must make.
type BatchCase<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)], usize);

#[test]
fn run_time_batch_fixtures_keep_each_update() {
    let cases: [BatchCase; 5] = [
        (
            "g1_19/nba_ordered_updates",
            include_str!(
                "../../../../../../tests/fixtures/sim/feature_completion/g1_19/nba_ordered_updates.sv"
            ),
            &[
                ("x[lo+:4]", "x[3:0]"),
                ("counted(8'h1)", "8'h1"),
                ("counted(8'h2)", "8'h2"),
            ],
            3,
        ),
        (
            "sim_043/value_changes",
            include_str!(
                "../../../../../../tests/fixtures/sim/feature_completion/sim_043/value_changes.sv"
            ),
            &[("counted(1'b1)", "1'b1")],
            1,
        ),
        (
            "T1-D11_changes_at_check",
            include_str!(
                "../../../../../../tests/fixtures/sim/lrm_decisions/T1-D11_changes_at_check.sv"
            ),
            &[("counted(0)", "0")],
            2,
        ),
        (
            "T1-D11_disable_iff_at_check",
            include_str!(
                "../../../../../../tests/fixtures/sim/lrm_decisions/T1-D11_disable_iff_at_check.sv"
            ),
            &[("counted(1'b1)", "1'b1")],
            1,
        ),
        (
            "T1-D11_switch_control_at_check",
            include_str!(
                "../../../../../../tests/fixtures/sim/lrm_decisions/T1-D11_switch_control_at_check.sv"
            ),
            &[("counted(1'b1)", "1'b1")],
            1,
        ),
    ];
    for (name, source, plain, plain_count) in cases {
        assert_eq!(merged(source).1, 0, "{name}");
        let mut plain_source = source.to_owned();
        for (from, to) in plain {
            assert!(plain_source.contains(from), "{name}: {from}");
            plain_source = plain_source.replace(from, to);
        }
        assert_eq!(
            merged(&plain_source).1,
            plain_count,
            "{name} with plain values"
        );
    }
}
