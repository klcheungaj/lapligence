//! SYN-033 public-pipeline coverage for SystemVerilog-2009 structural bind.

use std::path::Path;

use llg::core::{compile, db, model};

use crate::sim_cli;

const SUITE: &str = "syn033_structural_bind";

#[test]
fn module_and_instance_bind_execute_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        SUITE,
        "syn_033_structural_bind",
        "bind=1/0 selected=x/0\nbind=0/1 selected=x/1\n",
        "llg: $finish at time 2000 at tb:38:5\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn interface_bind_executes_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        SUITE,
        "syn_033_interface_bind",
        "interface_bind=1\ninterface_bind=0\n",
        "llg: $finish at time 2000 at tb:28:5\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn generated_instance_binds_execute_in_both_optimizer_modes() {
    sim_cli::run_case_with_args(
        SUITE,
        "syn_033_generated_bind",
        "generated=100\ngenerated=011\n",
        "llg: $finish at time 2000 at tb:36:5\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn bind_diagnostics_keep_unknown_and_illegal_targets_single_fault() {
    sim_cli::reject_case_with_args(
        SUITE,
        "syn_033_unknown_target",
        "unknown module 'syn033_missing_target'",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "syn_033_illegal_target",
        "not a valid bind target; only modules and interfaces are allowed",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "syn_033_interface_module",
        "cannot instantiate a module in an interface",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "syn_033_duplicate_bind",
        "redefinition of 'repeated'",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "syn_033_outside_scope",
        "use of undeclared identifier 'only_in_tb'",
        &["--edition", "sv2009"],
    );
}

#[test]
fn owned_model_retains_bound_instance_identity_after_frontend_drop() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);

    let module_source = root.join("syn_033_structural_bind.sv");
    let module_output = compile::compile_checked(&compile::CompileOpts {
        files: vec![module_source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-033 module bind should compile");
    let module_db = db::Db::from_slang(&module_output.snapshot).expect("owned module bind DB");
    drop(module_output);
    let module_design = model::DesignModel::from_db(&module_db);

    let dut0 = module_design
        .instance("tb.dut0")
        .expect("first target instance");
    let dut1 = module_design
        .instance("tb.dut1")
        .expect("second target instance");
    assert_eq!(
        dut0.children
            .iter()
            .map(|child| child.name.as_str())
            .collect::<Vec<_>>(),
        vec!["by_type"]
    );
    assert_eq!(
        dut1.children
            .iter()
            .map(|child| child.name.as_str())
            .collect::<Vec<_>>(),
        vec!["by_instance", "by_type"]
    );
    assert_eq!(
        module_design
            .instance("tb.dut0.by_type")
            .expect("module-type binding path")
            .def_name,
        "syn033_observer"
    );
    assert_eq!(
        module_design
            .instance("tb.dut1.by_instance")
            .expect("instance binding path")
            .def_name,
        "syn033_observer"
    );
    for path in ["tb.dut0.by_type", "tb.dut1.by_type"] {
        let observer = module_design.instance(path).expect("module-type observer");
        let invert = observer
            .params
            .iter()
            .find(|param| param.name == "INVERT")
            .expect("module-type bind parameter");
        assert!(matches!(
            invert.value,
            Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(1)
        ));
    }
    let selected = module_design
        .instance("tb.dut1.by_instance")
        .expect("selected observer");
    let invert = selected
        .params
        .iter()
        .find(|param| param.name == "INVERT")
        .expect("selected bind parameter");
    assert!(matches!(
        invert.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(0)
    ));

    let interface_source = root.join("syn_033_interface_bind.sv");
    let interface_output = compile::compile_checked(&compile::CompileOpts {
        files: vec![interface_source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-033 interface bind should compile");
    let interface_db =
        db::Db::from_slang(&interface_output.snapshot).expect("owned interface bind DB");
    drop(interface_output);
    let interface_design = model::DesignModel::from_db(&interface_db);
    let bus = interface_design
        .instance("tb.bus")
        .expect("interface target instance");
    assert_eq!(bus.def_name, "syn033_if");
    assert_eq!(bus.children.len(), 1);
    let bound_interface = interface_design
        .instance("tb.bus.by_interface")
        .expect("interface binding path");
    assert_eq!(bound_interface.def_name, "syn033_if_observer");

    let generated_source = root.join("syn_033_generated_bind.sv");
    let generated_output = compile::compile_checked(&compile::CompileOpts {
        files: vec![generated_source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-033 generated instance binds should compile");
    let generated_db =
        db::Db::from_slang(&generated_output.snapshot).expect("owned generated bind DB");
    drop(generated_output);
    let generated_design = model::DesignModel::from_db(&generated_db);
    drop(generated_db);
    let top = generated_design.instance("tb").expect("generated bind top");
    for (path, seed, invert) in [
        ("tb.rows[0]", 0, 0),
        ("tb.rows[1]", 1, 1),
        ("tb.chosen", 1, 0),
    ] {
        let scope = top
            .gen_scopes
            .iter()
            .find(|scope| scope.full_name == path)
            .expect("generated scope path");
        let target = scope
            .children
            .iter()
            .find(|child| child.name == "dut")
            .expect("generated target instance");
        assert_eq!(target.full_name, format!("{path}.dut"));
        let observer = target
            .children
            .iter()
            .find(|child| child.name == "bound_probe")
            .expect("bound instance under generated target");
        assert_eq!(observer.full_name, format!("{path}.dut.bound_probe"));
        assert_eq!(observer.def_name, "syn033_generated_observer");
        let seed_param = target
            .params
            .iter()
            .find(|param| param.name == "SEED")
            .expect("generated target parameter");
        assert!(matches!(
            seed_param.value,
            Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(seed)
        ));
        let invert_param = observer
            .params
            .iter()
            .find(|param| param.name == "INVERT")
            .expect("generated bind parameter");
        assert!(matches!(
            invert_param.value,
            Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(invert)
        ));
    }
}
