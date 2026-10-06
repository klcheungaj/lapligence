//! Read-only eligibility is decided on all target leaves and operation flags.
use super::*;

#[test]
fn arithmetic_assignment_flags_are_not_mistaken_for_reads() {
    use crate::core::db::{ConstantSource, ConstantType, Node};
    use crate::core::model::TypeInfo;
    for assignment in [false, true] {
        let kinds = vec![
            (
                NodeKind::Var {
                    ty: TypeInfo {
                        kind: "logic".into(),
                        width: Some(8),
                        signed: false,
                        type_name: None,
                    },
                },
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(NodeId(0)),
                }),
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::Binary,
                    value: ValueData::Bin("1".into()),
                    size: 1,
                    source: ConstantSource::NotCaptured,
                    time_scale: None,
                }),
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Add,
                    reordered: false,
                    assignment,
                    operands: vec![NodeId(1), NodeId(2)],
                }),
                vec![NodeId(1), NodeId(2)],
            ),
        ];
        let nodes = kinds
            .into_iter()
            .map(|(kind, children)| Node {
                kind,
                children,
                parent: None,
                name: String::new(),
                full_name: "".into(),
                file: None,
                line: 0,
                col: 0,
                end_line: 0,
                end_col: 0,
            })
            .collect();
        let database =
            Db::from_test_nodes("callback-flags", nodes, vec![], HashMap::new()).unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let cg = Codegen::new(&semantic);
        let result = cg.check_event_expression_effects(NodeId(3), "tb");
        if assignment {
            assert!(result
                .unwrap_err()
                .contains("writes external or persistent storage"));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn composite_helper_targets_are_all_private_or_rejected_at_source() {
    for (name, source, allowed, callee_name) in [
        ("helper_private.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_private.sv"), true, "computed"),
        ("helper_external_concat.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_external_concat.sv"), false, "bad"),
        ("helper_external_compound.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_external_compound.sv"), false, "bad"),
    ] {
        let database = {
            let output = crate::core::compile::compile_sources_checked(
                &[crate::core::compile::OwnedSource::compilation_unit(name, source)],
                &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).unwrap();
            Db::from_slang(&output.snapshot).unwrap()
        };
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        let tops = cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        for top in tops { cg.emit_func_prototypes(top).unwrap(); }
        let call = database.node_ids().find(|node| {
            matches!(cg.kind(*node), NodeKind::FuncCall { name, .. } if name == callee_name)
        }).unwrap();
        if let Some(instance) = cg.owning_inst(call) { cg.inst = instance; }
        let result = cg.check_event_expression_effects(call, "tb");
        if allowed { result.unwrap(); }
        else { assert!(result.unwrap_err().contains("writes external or persistent storage")); }
        // Visible writes rule out only the read-only callback; the waiting
        // process can still evaluate the legal helper.
        match cg.classify_event_expression(call, "tb").unwrap() {
            EventEvaluation::Callback => assert!(allowed, "{name}"),
            EventEvaluation::Process(reason) => {
                assert!(!allowed, "{name}");
                assert!(reason.contains("writes external or persistent storage"), "{reason}");
            }
        }
    }
}

#[test]
fn stateful_and_descriptor_helpers_select_process_evaluation() {
    let source = r#"
module tb;
    localparam int N = 65537;
    typedef logic [16:0] big_t [0:N-1];
    big_t big;
    logic [7:0] a;
    function logic [7:0] retained(input logic [7:0] v);
        if (v[0]) retained = v;
    endfunction
    function automatic logic [16:0] lead(input big_t v);
        return v[0];
    endfunction
    function automatic logic [7:0] noisy(input logic [7:0] v);
        $display("noisy");
        return v;
    endfunction
    function automatic logic [7:0] clean(input logic [7:0] v);
        logic [7:0] t;
        t = v + 1;
        return t;
    endfunction
    initial begin
        @(retained(a));
        @(lead(big));
        @(clean(a));
        @(noisy(a));
    end
endmodule
"#;
    let database = {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "classify.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    };
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let tops = cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    for top in tops {
        cg.emit_func_prototypes(top).unwrap();
    }
    let call = |cg: &Codegen<'_>, name: &str| {
        database.node_ids().find(|node| {
        matches!(cg.kind(*node), NodeKind::FuncCall { name: callee, .. } if callee == name)
    }).unwrap()
    };
    let (retained, lead, clean, noisy) = (
        call(&cg, "retained"),
        call(&cg, "lead"),
        call(&cg, "clean"),
        call(&cg, "noisy"),
    );
    if let Some(instance) = cg.owning_inst(retained) {
        cg.inst = instance;
    }
    assert!(matches!(
        cg.classify_event_expression(retained, "tb").unwrap(),
        EventEvaluation::Process(reason) if reason.contains("static function return")
    ));
    assert!(matches!(
        cg.classify_event_expression(lead, "tb").unwrap(),
        EventEvaluation::Process(reason) if reason.contains("descriptor-transported")
    ));
    assert_eq!(
        cg.classify_event_expression(clean, "tb").unwrap(),
        EventEvaluation::Callback
    );
    // A helper form without an effect summary still rejects outright.
    assert!(cg
        .classify_event_expression(noisy, "tb")
        .unwrap_err()
        .contains("has no pure effect summary"));
}

#[test]
fn postponed_helpers_may_only_store_to_their_own_storage() {
    let source = r#"
module tb;
    int a, calls;
    function int counted(input int v);
        static int seen = 0;
        seen++;
        return v;
    endfunction
    function int visible(input int v);
        calls++;
        return v;
    endfunction
    function automatic int clean(input int v);
        int t;
        t = v + 1;
        return t;
    endfunction
    function int nested(input int v);
        return counted(v) + clean(v);
    endfunction
    initial begin
        $strobe("%0d", counted(a));
        $strobe("%0d", visible(a));
        $strobe("%0d", clean(a));
        $strobe("%0d", nested(a));
        $strobe("%0d", $time);
    end
endmodule
"#;
    let database = {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "postponed.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    };
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let tops = cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    for top in tops {
        cg.emit_func_prototypes(top).unwrap();
    }
    // The call site in the initial block, not a nested call in a body.
    let call = |cg: &Codegen<'_>, name: &str| {
        let in_function = |mut node: NodeId| {
            while let Some(parent) = cg.node(node).parent() {
                if matches!(cg.kind(parent), NodeKind::FuncTask { .. }) {
                    return true;
                }
                node = parent;
            }
            false
        };
        database
            .node_ids()
            .find(|node| {
                matches!(cg.kind(*node), NodeKind::FuncCall { name: callee, .. } if callee == name)
                    && !in_function(*node)
            })
            .unwrap()
    };
    let counted = call(&cg, "counted");
    if let Some(instance) = cg.owning_inst(counted) {
        cg.inst = instance;
    }
    assert_eq!(
        cg.classify_postponed_expression(counted, "tb").unwrap(),
        PostponedEvaluation::PrivateEffects
    );
    assert!(cg
        .classify_postponed_expression(call(&cg, "visible"), "tb")
        .unwrap_err()
        .contains("4.4.2.9"));
    assert_eq!(
        cg.classify_postponed_expression(call(&cg, "clean"), "tb")
            .unwrap(),
        PostponedEvaluation::Callback
    );
    assert_eq!(
        cg.classify_postponed_expression(call(&cg, "nested"), "tb")
            .unwrap(),
        PostponedEvaluation::PrivateEffects
    );
    // A helper's own formal, static local and result never enter the
    // sensitivity of a process-evaluated expression; the actual does.
    let sensitivity = cg
        .collect_evaluator_sensitivity("tb", call(&cg, "nested"))
        .unwrap();
    assert_eq!(sensitivity.len(), 1, "{sensitivity:?}");
    assert!(sensitivity[0].scalar_name().unwrap().contains('a'));
}

#[test]
fn only_subroutine_scoped_event_reads_force_expansion() {
    let source = r#"
module tb;
    event ev;
    logic clk;
    logic [3:0] sig;
    task automatic module_edge(); @(posedge clk); endtask
    task automatic module_expression(); @(posedge (clk & sig[0])); endtask
    task automatic input_event(input event e); @(e); endtask
    task automatic input_event_or(input event e); @(e or posedge clk); endtask
    task automatic delay_disable(); #1; disable delay_disable; endtask
    task automatic calls_typed(); input_event(ev); module_edge(); endtask
    task automatic output_event(output event e); e = ev; endtask
    task automatic formal_expression(input logic [3:0] v); @(posedge (clk & v[0])); endtask
    task automatic ref_edge(ref logic r); @(posedge r); endtask
    task automatic local_expression(); automatic logic l = 0; @(posedge (l | clk)); endtask
    task automatic calls_expanded(); formal_expression(4'd1); endtask
    task automatic forwards_ref(ref logic r); ref_edge(r); endtask
    task automatic forwards_local(); automatic logic l = 0; ref_edge(l); endtask
    task automatic forwards_module(); ref_edge(clk); endtask
    task automatic ref_select(ref logic [3:0] r); @(posedge r[0]); endtask
    task automatic ref_level(ref logic [3:0] r); wait (r[1] == 1'b1); endtask
    task automatic forwards_select(); automatic logic [3:0] l = 0; ref_select(l); endtask
    initial begin end
endmodule
"#;
    let database = {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "expansion.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    };
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let tops = cg.collect_design().unwrap();
    let top = tops[0];
    for (name, expanded) in [
        ("module_edge", false),
        ("module_expression", false),
        ("input_event", false),
        ("input_event_or", false),
        ("delay_disable", false),
        ("calls_typed", false),
        // Event formals of every direction take the typed call path.
        ("output_event", false),
        ("formal_expression", false),
        // A whole `ref` formal waited on directly (edge, any change or a
        // level wait) is followed through its descriptor by the typed body.
        ("ref_edge", false),
        ("ref_level", false),
        // Locals are read through their (fork-shared) cells.
        ("local_expression", false),
        ("calls_expanded", false),
        ("forwards_ref", false),
        ("forwards_local", false),
        ("forwards_module", false),
        // An evaluated expression over a `ref` formal needs its actual's
        // dependencies: a specialization or the call-site expansion.
        ("ref_select", true),
        ("forwards_select", true),
    ] {
        let task = database
            .node_ids()
            .find(|node| {
                matches!(cg.kind(*node), NodeKind::FuncTask { .. }) && cg.node(*node).name == name
            })
            .unwrap_or_else(|| panic!("task `{name}` is missing"));
        assert_eq!(
            cg.subroutine_requires_inline(task, top),
            expanded,
            "task `{name}`"
        );
    }
    // A `ref` formal read by an event control is bound by a specialization
    // when it (or a task it is forwarded to) has a whole-signal actual. A
    // caller local is followed through its descriptor when every read of the
    // formal is a direct wait (`bound_refs` empty), and forces the expansion
    // otherwise.
    for (name, inline_only, static_refs, bound_refs) in [
        ("ref_edge", false, vec![0], vec![]),
        ("ref_level", false, vec![0], vec![]),
        ("ref_select", false, vec![0], vec![0]),
        ("forwards_ref", false, vec![0], vec![]),
        ("forwards_module", false, vec![], vec![]),
        ("forwards_local", false, vec![], vec![]),
        ("forwards_select", true, vec![], vec![]),
        ("formal_expression", false, vec![], vec![]),
        ("input_event", false, vec![], vec![]),
    ] {
        let task = database
            .node_ids()
            .find(|node| {
                matches!(cg.kind(*node), NodeKind::FuncTask { .. }) && cg.node(*node).name == name
            })
            .unwrap();
        let shape = cg.call_shape(task, top);
        assert_eq!(shape.inline_only, inline_only, "task `{name}`");
        assert_eq!(shape.static_refs, static_refs, "task `{name}`");
        assert_eq!(shape.bound_refs, bound_refs, "task `{name}`");
    }
}
