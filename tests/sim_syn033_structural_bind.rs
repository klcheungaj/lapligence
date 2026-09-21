//! SYN-033 public-pipeline coverage for SystemVerilog-2009 structural bind.

use std::path::Path;

use llg::core::{compile, db, model};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn033_structural_bind";

#[test]
fn module_and_instance_bind_execute_in_both_optimizer_modes() {
    sim_cli::run_case(
        SUITE,
        "syn_033_structural_bind",
        "bind=1/0 selected=x/0\nbind=0/1 selected=x/1\n",
        "llg: $finish at time 2000 at tb:38:5\n",
        &[],
    );
}

#[test]
fn interface_bind_executes_in_both_optimizer_modes() {
    sim_cli::run_case(
        SUITE,
        "syn_033_interface_bind",
        "interface_bind=1\ninterface_bind=0\n",
        "llg: $finish at time 2000 at tb:28:5\n",
        &[],
    );
}

#[test]
fn bind_diagnostics_keep_unknown_and_illegal_targets_single_fault() {
    sim_cli::reject_case(
        SUITE,
        "syn_033_unknown_target",
        "unknown module 'syn033_missing_target'",
    );
    sim_cli::reject_case(
        SUITE,
        "syn_033_illegal_target",
        "not a valid bind target; only modules and interfaces are allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "syn_033_interface_module",
        "cannot instantiate a module in an interface",
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
}
