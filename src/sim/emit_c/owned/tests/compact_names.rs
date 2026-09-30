use super::*;
use crate::sim::emit_c::names::{identifier_spans, scoped_name, MAX_C_IDENTIFIER_LEN};

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
    let source = super::super::super::model::render(&execution).unwrap();
    assert_eq!(
        source,
        super::super::super::model::render(&execution).unwrap()
    );
    assert!(identifier_spans(&source).all(|span| span.len() <= MAX_C_IDENTIFIER_LEN));
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
    function.dpi = Some(crate::sim::ir::IrDpiImport {
        c_name: foreign.clone(),
        context: false,
        pure: false,
    });
    model.funcs.push(function);
    let execution = ExecutionModel::lower(model).unwrap();
    let source = super::super::super::model::render(&execution).unwrap();
    assert!(source.contains(&format!("extern void {foreign}(void);")));
    assert!(source.contains(&format!("{foreign}();")));
    assert!(source.contains("\"original diagnostic spelling\""));
    assert!(identifier_spans(&source)
        .all(|span| { span.len() <= MAX_C_IDENTIFIER_LEN || source[span] == foreign }));
}
