use super::*;
use crate::sim::emit_c::names::{
    identifier_spans, runtime_identifiers, scoped_name, MAX_C_IDENTIFIER_LEN,
};

#[test]
fn generate_instance_model_bounds_all_internal_c_identifiers() {
    let mut model = numeric_model();
    let deep = "hierarchy_".repeat(MAX_C_IDENTIFIER_LEN);
    for index in 0..16 {
        let generate = format!("sites[{index}]");
        let components = ["pca_sites", deep.as_str(), generate.as_str(), "v"];
        let name = scoped_name("G", &components);
        model.signals.push(
            IrSignal::new(
                name,
                Some(format!("pca_sites\u{1f}{deep}\u{1f}{generate}\u{1f}v")),
                model.signals[0].ty,
                None,
            )
            .unwrap(),
        );
        let mut process = model.processes[0].clone();
        process.c_name = scoped_name("p", &components);
        process.label = format!("pca_sites.{deep}.{generate}");
        process.body.insert(
            0,
            IrStmt::Delay {
                ticks: crate::sim::ir::IrDelay::Constant(1),
            },
        );
        model.spawns.push(process.c_name.clone());
        model.processes.push(process);
    }
    let execution = ExecutionModel::lower(model).unwrap();
    let rendered = crate::sim::emit_c::render_with_symbols(&execution).unwrap();
    let repeated = crate::sim::emit_c::render_with_symbols(&execution).unwrap();
    assert_eq!(rendered.symbols_tsv, repeated.symbols_tsv);
    let source = rendered.source;
    assert_eq!(
        source,
        super::super::super::model::render(&execution).unwrap()
    );
    assert!(identifier_spans(&source).all(|span| {
        span.len() <= MAX_C_IDENTIFIER_LEN || runtime_identifiers().contains(&source[span])
    }));
    let identifiers = identifier_spans(&source)
        .map(|span| &source[span])
        .collect::<std::collections::BTreeSet<_>>();
    let rows = rendered
        .symbols_tsv
        .lines()
        .map(|line| line.split_once('\t').unwrap())
        .collect::<Vec<_>>();
    assert!(rows.len() >= 32);
    assert!(rows.windows(2).all(|rows| rows[0].0 < rows[1].0));
    for (short, original) in rows {
        assert!(identifiers.contains(short));
        assert!(!identifiers.contains(original));
        assert!(original.len() > MAX_C_IDENTIFIER_LEN);
    }
    assert!(source.contains(&format!("pca_sites.{deep}.sites[0]")));
    assert!(source.contains(&format!("pca_sites.{deep}.sites[15].v")));
    assert!(!source.contains("__llg_ident_"));
}

#[test]
fn complete_model_keeps_long_foreign_dpi_names_and_diagnostic_labels() {
    let mut model = numeric_model();
    model.funcs[0].diagnostic_name = Some("original diagnostic spelling".to_owned());
    model.funcs[0].body.insert(
        0,
        IrStmt::Repeat {
            count: number(1, 32),
            body: Vec::new(),
        },
    );
    let foreign = format!("foreign_{}", "x".repeat(MAX_C_IDENTIFIER_LEN));
    let mut function = IrFunc::new(
        format!("fn_{}", "internal_".repeat(MAX_C_IDENTIFIER_LEN)),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    function.diagnostic_name = Some("original diagnostic spelling".to_owned());
    function.is_task = false;
    function.dpi = Some(crate::sim::ir::IrDpiImport::new(
        foreign.clone(),
        false,
        false,
        Vec::new(),
        None,
    ));
    model.funcs.push(function);
    let execution = ExecutionModel::lower(model).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains(&format!("extern void {foreign}(void);")));
    assert!(source.contains(&format!("{foreign}();")));
    assert!(source.contains("\"original diagnostic spelling\""));
    assert!(identifier_spans(&source).all(|span| {
        span.len() <= MAX_C_IDENTIFIER_LEN || {
            let name = &source[span];
            name == foreign || runtime_identifiers().contains(name)
        }
    }));
}

#[test]
fn coroutine_branch_descriptors_and_unowned_functions_do_not_label_c_symbols() {
    let mut model = numeric_model();
    model.funcs[0].diagnostic_name = None;
    model.funcs[0].body.insert(
        0,
        IrStmt::Repeat {
            count: number(1, 32),
            body: Vec::new(),
        },
    );
    let process = &mut model.processes[0];
    process.label = "tb.initial".to_owned();
    let branch = "p_cI_encoded_branch".to_owned();
    process.pre_fns.push(crate::sim::ir::IrPreFn::Branch {
        c_name: branch.clone(),
        body: vec![IrStmt::Delay {
            ticks: crate::sim::ir::IrDelay::Constant(1),
        }],
    });
    process.body.insert(
        0,
        IrStmt::Fork {
            join_kind: crate::sim::ir::IrJoinKind::Join,
            branches: vec![(branch.clone(), "tb.fork[0]".to_owned())],
            target: None,
        },
    );
    let execution = ExecutionModel::lower(model).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains("\"tb.initial.fork\""));
    assert!(source.contains("llg_budget_point(\"unnamed function\")"));
    assert!(!source.contains(&format!("\"{branch}\"")));
}
