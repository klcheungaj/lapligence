//! SYN-018 public-pipeline coverage for nested and extern module declarations.

use std::path::Path;

use llg::core::{compile, db, model};
use llg::sim::{codegen, opt, semantic};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn018_module_declarations";
const EXPECTED: &str = "extern=7 nested=13/8\n";

#[test]
fn nested_and_extern_modules_execute_in_both_compilation_unit_modes() {
    let expected_stderr = "llg: $finish at time 1000 at tb:73:5\n";
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "module_declarations",
            &["extern_child", "extern_child_body"],
            EXPECTED,
            expected_stderr,
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn enclosing_instance_specializations_keep_nested_defaults_and_port_widths() {
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_args(
            SUITE,
            "nested_specializations",
            "specialized=4/6 widths=4/5\n",
            "llg: $finish at time 1000 at tb:28:5\n",
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn matching_extern_header_and_body_specialize_parameterized_ports() {
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "extern_specializations",
            &["extern_child", "extern_child_body"],
            "extern_specialized=7/8 widths=4/5\n",
            "llg: $finish at time 1000 at tb:14:5\n",
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn extern_module_signature_and_body_failures_remain_frontend_diagnostics() {
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_mismatch",
        "extern module 'syn018_bad' does not match implementation",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_missing",
        "missing implementation for extern module 'syn018_missing'",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_2001",
        "expected a declaration name",
        &["--edition", "2001"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "nested_out_of_scope",
        "unknown module 'leaf'",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "nested_declaration_in_generate",
        "member not allowed in generate block",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "nested_2001",
        "nested module",
        &["--edition", "2001"],
    );
}

#[test]
fn owned_model_specializes_one_nested_definition_for_each_enclosing_instance() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("nested_specializations.sv");
    let output = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("nested specializations should compile");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let design = model::DesignModel::from_db(&database);

    for (parent_path, expected_width, expected_bias) in [("tb.p4", 4, 1), ("tb.p5", 5, 3)] {
        let parent = design.instance(parent_path).expect("parent instance");
        assert_eq!(parent.ports[0].ty.width, Some(expected_width));
        let leaf = design
            .instance(&format!("{parent_path}.u"))
            .expect("same-scope nested leaf instance");
        assert_eq!(leaf.def_name, "leaf");
        assert_eq!(leaf.full_name, format!("{parent_path}.u"));
        assert_eq!(
            leaf.params
                .iter()
                .find(|param| param.name == "EXTRA")
                .and_then(|param| param.value.as_ref())
                .and_then(|value| match value {
                    llg::core::elab::Val::Bits(bits) => bits.to_u64(),
                    _ => None,
                }),
            Some(expected_bias)
        );
        for port_name in ["leaf_a", "leaf_y"] {
            assert_eq!(
                leaf.ports
                    .iter()
                    .find(|port| port.name == port_name)
                    .expect("nested port")
                    .ty
                    .width,
                Some(expected_width)
            );
        }
    }
}

#[test]
fn owned_model_keeps_matching_extern_port_specializations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let output = compile::compile_checked(&compile::CompileOpts {
        files: [
            "extern_child.sv",
            "extern_child_body.sv",
            "extern_specializations.sv",
        ]
        .into_iter()
        .map(|name| root.join(name).to_string_lossy().into_owned())
        .collect(),
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("matching extern specializations should compile");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let design = model::DesignModel::from_db(&database);

    for (path, width) in [("tb.e4", 4), ("tb.e5", 5)] {
        let instance = design.instance(path).expect("extern instance");
        assert_eq!(instance.def_name, "syn018_extern_child");
        assert_eq!(
            instance
                .params
                .iter()
                .find(|param| param.name == "W")
                .and_then(|param| param.value.as_ref())
                .and_then(|value| match value {
                    llg::core::elab::Val::Bits(bits) => bits.to_u64(),
                    _ => None,
                }),
            Some(u64::from(width))
        );
        for port_name in ["a", "y"] {
            assert_eq!(
                instance
                    .ports
                    .iter()
                    .find(|port| port.name == port_name)
                    .expect("extern port")
                    .ty
                    .width,
                Some(width)
            );
        }
    }
}

#[test]
fn owned_model_keeps_nested_scope_and_extern_instance_identity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let output = compile::compile_checked(&compile::CompileOpts {
        files: [
            "extern_child.sv",
            "extern_child_body.sv",
            "module_declarations.sv",
        ]
        .into_iter()
        .map(|name| root.join(name).to_string_lossy().into_owned())
        .collect(),
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-018 module declaration matrix should compile");
    let definitions: Vec<_> = output
        .snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.kind == llg::ffi::slang::SemanticKind::Definition)
        .collect();
    assert_eq!(
        definitions
            .iter()
            .filter(|node| node.name == "leaf" && node.is_local)
            .count(),
        2
    );
    assert_eq!(
        definitions
            .iter()
            .filter(|node| node.name == "captured_base" && node.is_local)
            .count(),
        2
    );
    assert!(definitions
        .iter()
        .filter(|node| node.name != "leaf" && node.name != "captured_base")
        .all(|node| !node.is_local));
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    drop(output);
    let design = model::DesignModel::from_db(&database);

    let extern_instance = design.instance("tb.ext").expect("extern instance");
    assert_eq!(extern_instance.def_name, "syn018_extern_child");
    let width = extern_instance
        .params
        .iter()
        .find(|param| param.name == "W")
        .expect("extern W parameter");
    assert!(matches!(
        width.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(4)
    ));

    let left = design.instance("tb.left").expect("left parent");
    let right = design.instance("tb.right").expect("right parent");
    assert_eq!(
        design
            .instance("tb.left.captured")
            .expect("left same-scope nested instance")
            .def_name,
        "captured_base"
    );
    assert_eq!(
        design
            .instance("tb.right.captured")
            .expect("right same-scope nested instance")
            .def_name,
        "captured_base"
    );
    assert!(left
        .gen_scopes
        .iter()
        .any(|scope| scope.name == "generated"));
    assert!(right
        .gen_scopes
        .iter()
        .any(|scope| scope.name == "generated"));
    let left_generated = left
        .gen_scopes
        .iter()
        .find(|scope| scope.name == "generated")
        .expect("left generated scope");
    let right_generated = right
        .gen_scopes
        .iter()
        .find(|scope| scope.name == "generated")
        .expect("right generated scope");
    let left_leaf = left_generated.children.first().expect("left nested leaf");
    let right_leaf = right_generated.children.first().expect("right nested leaf");
    assert_eq!(left_leaf.def_name, "leaf");
    assert_eq!(right_leaf.def_name, "leaf");
    assert_ne!(
        left_leaf.full_name, right_leaf.full_name,
        "same-named nested definitions must retain distinct instance paths"
    );
    assert_eq!(left_leaf.full_name, "tb.left.generated.u");
    assert_eq!(right_leaf.full_name, "tb.right.generated.u");
    let left_extra = left_leaf
        .params
        .iter()
        .find(|param| param.name == "EXTRA")
        .expect("left EXTRA parameter");
    let right_extra = right_leaf
        .params
        .iter()
        .find(|param| param.name == "EXTRA")
        .expect("right EXTRA parameter");
    assert!(matches!(
        left_extra.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(2)
    ));
    assert!(matches!(
        right_extra.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(3)
    ));
    assert_eq!(
        left_leaf
            .ports
            .iter()
            .find(|port| port.name == "leaf_y")
            .expect("left output")
            .ty
            .width,
        Some(4)
    );
    assert_eq!(
        right_leaf
            .ports
            .iter()
            .find(|port| port.name == "leaf_y")
            .expect("right output")
            .ty
            .width,
        Some(4)
    );
}

#[test]
fn owned_nested_and_extern_declarations_lower_after_native_snapshot_drop() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    for (top, files) in [
        ("syn018_parent", vec!["nested_specializations.sv"]),
        ("syn018_left", vec!["module_declarations.sv"]),
        ("syn018_right", vec!["module_declarations.sv"]),
        (
            "syn018_extern_child",
            vec!["extern_child.sv", "extern_child_body.sv"],
        ),
    ] {
        let output = compile::compile_checked(&compile::CompileOpts {
            files: files
                .into_iter()
                .map(|name| root.join(name).to_string_lossy().into_owned())
                .collect(),
            top: Some(top.to_owned()),
            ..Default::default()
        })
        .expect("selected module should compile");
        let database = db::Db::from_slang(&output.snapshot).expect("owned database");
        drop(output);

        semantic::SemanticModel::from_db(&database)
            .validate_simulation()
            .unwrap_or_else(|issues| panic!("{top}: {issues:?}"));
        semantic::SemanticModel::from_db(&database)
            .validate_synthesizable(semantic::SynthesisProfile::PortableRtl)
            .unwrap_or_else(|issues| panic!("{top}: {issues:?}"));
        for options in [opt::OptConfig::none(), opt::OptConfig::default()] {
            let generated = codegen::generate_from_db_with_opts(&database, &options)
                .unwrap_or_else(|error| panic!("{top}: {error}"));
            assert!(!generated.model_c.is_empty(), "{top}: no generated model");
        }
    }
}
