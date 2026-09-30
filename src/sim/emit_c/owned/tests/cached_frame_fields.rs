//! Resume-stable frame fields are mirrored by C locals that are reloaded after
//! every suspension point (design §13.2 rule 2).
use super::*;

const DISPATCH: &str = "LLG_CO_DISPATCH_BEGIN(co)";

fn render(model: IrModel) -> String {
    let execution = ExecutionModel::lower(model).expect("validate cached-field model");
    super::super::super::model::render(&execution).expect("emit cached-field model")
}

fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!(
            "static llg_co_status_t {name}(llg_co_frame_t* co, llg_co_chain_t* ch) {{"
        ))
        .unwrap_or_else(|| panic!("coroutine {name} in {source}"));
    let end = source[start..].find("\n}\n").expect("function end") + start;
    &source[start..end]
}

fn assign(rhs: IrExpr) -> IrStmt {
    IrStmt::Assign {
        lhs: IrLhs::Whole(0),
        rhs,
        nba: false,
    }
}

fn delay() -> IrStmt {
    IrStmt::Delay {
        ticks: IrDelay::Constant(1),
    }
}

fn local(name: &str) -> IrStmt {
    IrStmt::DeclLocal {
        name: name.to_owned(),
        width: 65,
        signed: false,
        two_state: false,
        init: Some(Box::new(number(23, 65))),
    }
}

fn read_local(name: &str) -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead(name.to_owned()), 65, false, None)
}

fn two_resume_points(mut model: IrModel, mut prefix: Vec<IrStmt>) -> IrModel {
    let sum = add(read_local_or_constant(&prefix), number(1, 65), 65);
    prefix.extend([
        assign(sum.clone()),
        delay(),
        assign(sum.clone()),
        delay(),
        assign(sum),
    ]);
    model.processes[0].body = prefix;
    model
}

fn read_local_or_constant(prefix: &[IrStmt]) -> IrExpr {
    if prefix.is_empty() {
        number(5, 65)
    } else {
        read_local("kept")
    }
}

fn lines_after<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    let lines = text.lines().collect::<Vec<_>>();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .filter_map(|(index, _)| lines.get(index + 1).copied())
        .collect()
}

#[test]
fn temporaries_are_a_local_declared_before_the_dispatch_and_reloaded_after_each_resume_point() {
    let source = render(two_resume_points(numeric_model(), Vec::new()));
    let process = function(&source, "p_numeric");
    let declaration = process
        .find("    sv4_t* _llg_t;\n")
        .expect("uninitialized local");
    assert!(
        declaration < process.find(DISPATCH).expect("dispatch"),
        "{process}"
    );
    assert_eq!(process.matches("LLG_CO_AWAIT(").count(), 2, "{process}");
    let reloads = lines_after(process, "LLG_CO_AWAIT(");
    assert_eq!(reloads, ["    _llg_t = F->_llg_t;"; 2], "{process}");
    assert!(process.contains("    F->_llg_t = llg_value_scope_values(F->_llg_temp_scope);\n"));
    assert!(process.contains("    (void)F->_llg_t;\n    _llg_t = F->_llg_t;\n"));
    // Every other read goes through the local.
    assert_eq!(process.matches("F->_llg_t =").count(), 1, "{process}");
    assert_eq!(
        process.matches("F->_llg_t;").count(),
        4,
        "keep-alive cast, copy after the prologue and two reloads: {process}"
    );
    assert!(process.contains("&_llg_t["), "{process}");
    assert!(
        !process.contains("F->_llg_frame_base;"),
        "read once: {process}"
    );
    assert!(
        !process.contains("_llg_temp_scope;\n    _llg_t"),
        "{process}"
    );
}

#[test]
fn a_process_without_cell_pointers_has_no_cell_reloads() {
    let source = render(two_resume_points(numeric_model(), Vec::new()));
    let process = function(&source, "p_numeric");
    assert!(!process.contains("_llg_local"), "{process}");
}

#[test]
fn a_cell_pointer_is_assigned_with_its_field_and_reloaded_after_resume() {
    let source = render(two_resume_points(numeric_model(), vec![local("kept")]));
    let process = function(&source, "p_numeric");
    let declaration = process
        .find("    sv4_t* _llg_local_1;\n")
        .unwrap_or_else(|| panic!("uninitialized cell pointer local: {process}"));
    assert!(
        declaration < process.find(DISPATCH).expect("dispatch"),
        "{process}"
    );
    assert!(
        process.contains("_llg_local_1 = F->_llg_local_1 = llg_value_scope_values("),
        "{process}"
    );
    let reloads = lines_after(process, "LLG_CO_AWAIT(");
    assert_eq!(
        reloads, ["    _llg_t = F->_llg_t; _llg_local_1 = F->_llg_local_1;"; 2],
        "{process}"
    );
    let uses = process.matches("_llg_local_1").count();
    let frame_uses = process.matches("F->_llg_local_1").count();
    assert_eq!(frame_uses, 3, "assignment and two reloads only: {process}");
    assert!(uses > frame_uses + 2, "{process}");
}

#[test]
fn a_field_read_less_often_than_it_is_reloaded_stays_in_the_frame() {
    let mut model = numeric_model();
    model.processes[0].body = vec![local("kept"), delay(), delay(), delay(), delay()];
    let source = render(model);
    let process = function(&source, "p_numeric");
    assert!(
        !process.contains("_llg_local_1 = F->_llg_local_1;"),
        "{process}"
    );
    assert!(!process.contains("sv4_t* _llg_local_1;"), "{process}");
}

#[test]
fn functions_without_a_resume_point_keep_plain_c_locals() {
    let source = render(numeric_model());
    assert!(!source.contains("_llg_t = F->"), "{source}");
    assert!(
        source.contains("sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);"),
        "{source}"
    );
}
