use super::*;
use crate::features::{
    Analysis, ModuleGraph, ModuleGraphDefinition, ModuleGraphElaboratedType,
    ModuleGraphGenerateScope, ModuleGraphInstance, ModuleGraphLocation, ModuleGraphPackedRange,
    ModuleGraphParameter, ModuleGraphPort, ModuleGraphSignal,
};
use llg::core::elab::Value;

fn ty(kind: &str, width: Option<u32>) -> TypeInfo {
    TypeInfo {
        kind: kind.to_owned(),
        width,
        signed: false,
        type_name: None,
    }
}

fn instance(name: &str, full_name: &str, def_name: &str) -> InstanceModel {
    InstanceModel {
        name: name.to_owned(),
        def_name: def_name.to_owned(),
        full_name: full_name.to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line: 1,
        col: 1,
        ports: Vec::new(),
        signals: Vec::new(),
        params: Vec::new(),
        gen_scopes: Vec::new(),
        funcs: Vec::new(),
        children: Vec::new(),
    }
}

fn source_instance(name: &str, module_type: &str, line: u32) -> ModuleGraphInstance {
    ModuleGraphInstance {
        name: name.to_owned(),
        module_type: module_type.to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line,
        col: 3,
    }
}

fn source_definition(
    name: &str,
    file: &str,
    line: u32,
    children: Vec<ModuleGraphInstance>,
) -> ModuleGraphDefinition {
    ModuleGraphDefinition {
        id: crate::features::module_graph_definition_id(name, Some(file), line, 1),
        name: name.to_owned(),
        file: Some(file.to_owned()),
        line,
        col: 1,
        end_line: line + 10,
        end_col: 1,
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        children,
        generated_scopes: Vec::new(),
    }
}

fn source_location(file: &str, line: u32, col: u32, width: u32) -> ModuleGraphLocation {
    ModuleGraphLocation {
        file: file.to_owned(),
        line,
        col,
        end_line: line,
        end_col: col + width,
    }
}

fn graph_analysis(
    definitions: Vec<ModuleGraphDefinition>,
    top_instances: Vec<InstanceModel>,
    configured_top: Option<&str>,
) -> Analysis {
    let model = DesignModel {
        design_name: "test".to_owned(),
        top_instances,
        modules: Vec::new(),
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let mut analysis = Analysis::new(Vec::new(), model, Vec::new(), Vec::new());
    analysis.module_graph = ModuleGraph {
        definitions,
        elaborated_types: Vec::new(),
    };
    analysis.configured_top = configured_top.map(str::to_owned);
    analysis
}

fn identity_source(path: &Path) -> Option<PathBuf> {
    Some(path.to_owned())
}

fn instance_stats(roots: &[ExplorerInstance]) -> (usize, usize, usize) {
    let mut instances = roots.iter().collect::<Vec<_>>();
    let mut scopes = Vec::new();
    let mut total = 0;
    let mut budget_markers = 0;
    let mut cycle_markers = 0;
    while let Some(instance) = instances.pop() {
        total += 1;
        budget_markers += usize::from(instance.is_budget_truncated);
        cycle_markers += usize::from(instance.is_cycle_truncated);
        instances.extend(instance.children.iter());
        scopes.extend(instance.generated_scopes.iter());
        while let Some(scope) = scopes.pop() {
            instances.extend(scope.children.iter());
            scopes.extend(scope.nested_scopes.iter());
        }
    }
    (total, budget_markers, cycle_markers)
}

fn serialized_hierarchy_stats(roots: &[ExplorerInstance]) -> (usize, usize, usize) {
    let mut instances = roots.iter().collect::<Vec<_>>();
    let mut scopes = Vec::new();
    let mut total = 0;
    let mut scope_count = 0;
    let mut budget_markers = 0;
    while let Some(instance) = instances.pop() {
        total += 1;
        budget_markers += usize::from(instance.is_budget_truncated);
        instances.extend(instance.children.iter());
        scopes.extend(instance.generated_scopes.iter());
    }
    while let Some(scope) = scopes.pop() {
        total += 1;
        scope_count += 1;
        instances.extend(scope.children.iter());
        scopes.extend(scope.nested_scopes.iter());
        while let Some(instance) = instances.pop() {
            total += 1;
            budget_markers += usize::from(instance.is_budget_truncated);
            instances.extend(instance.children.iter());
            scopes.extend(instance.generated_scopes.iter());
        }
    }
    (total, scope_count, budget_markers)
}

fn serialized_response_work(snapshot: &ExplorerSnapshot) -> usize {
    enum Work<'a> {
        Instance(&'a ExplorerInstance),
        Scope(&'a ExplorerGenerateScope),
    }

    let mut total = 0;
    for module in &snapshot.modules {
        total += 1 + module.ports.len() + module.params.len() + module.signals.len();
    }
    let mut work = snapshot
        .roots
        .iter()
        .map(Work::Instance)
        .collect::<Vec<_>>();
    while let Some(item) = work.pop() {
        match item {
            Work::Instance(instance) => {
                total += 1 + instance.ports.len() + instance.params.len() + instance.signals.len();
                work.extend(instance.children.iter().map(Work::Instance));
                work.extend(instance.generated_scopes.iter().map(Work::Scope));
            }
            Work::Scope(scope) => {
                total += 1 + scope.params.len();
                work.extend(scope.children.iter().map(Work::Instance));
                work.extend(scope.nested_scopes.iter().map(Work::Scope));
            }
        }
    }
    total
}

#[test]
fn snapshot_keeps_multiple_tops_and_recursive_stable_ids() {
    let mut top = instance("top", "work@top", "top");
    let mut child = instance("u_child", "work@top.u_child", "child");
    child.ports.push(PortModel {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(1)),
    });
    child.signals.push(SignalModel {
        name: "data".to_owned(),
        kind: "var".to_owned(),
        ty: ty("logic", Some(8)),
    });
    top.children.push(child);
    let second = instance("second", "work@second", "second");
    let model = DesignModel {
        design_name: "design".to_owned(),
        top_instances: vec![top, second],
        modules: vec![
            ModuleDef {
                name: "top".to_owned(),
                file: Some("/workspace/top.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 8,
                end_col: 1,
            },
            ModuleDef {
                name: "child".to_owned(),
                file: Some("/workspace/child.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 3,
                end_col: 1,
            },
            ModuleDef {
                name: "second".to_owned(),
                file: Some("/workspace/second.sv".to_owned()),
                line: 1,
                col: 8,
                end_line: 2,
                end_col: 1,
            },
        ],
        packages: Vec::new(),
        classes: Vec::new(),
    };

    let first = snapshot("/workspace", &model);
    let second = snapshot("/workspace", &model);
    assert_eq!(first, second);
    let reordered = DesignModel {
        top_instances: model.top_instances.iter().cloned().rev().collect(),
        modules: model.modules.iter().cloned().rev().collect(),
        ..model.clone()
    };
    assert_eq!(first, snapshot("/workspace", &reordered));
    assert_eq!(first.roots.len(), 2);
    let top_root = first
        .roots
        .iter()
        .find(|root| root.module_type == "top")
        .expect("top root");
    assert_eq!(top_root.children.len(), 1);
    assert_eq!(top_root.children[0].module_type, "child");
    assert_eq!(top_root.children[0].ports[0].ty.width, Some(1));
    assert_eq!(top_root.children[0].signals[0].ty.width, Some(8));
    assert!(top_root.id.contains("/workspace:top"));
}

#[test]
fn generated_scope_children_and_typed_parameters_are_preserved() {
    let mut top = instance("top", "top", "top");
    top.gen_scopes.push(GenScopeModel {
        name: "g[0]".to_owned(),
        full_name: "work@top.g[0]".to_owned(),
        params: vec![ParamModel {
            name: "i".to_owned(),
            value: Some(Val::Bits(Value::from_u64(3, 32, false))),
            ty: ty("int", Some(32)),
            local: true,
        }],
        children: vec![instance("u", "top.g[0].u", "leaf")],
    });
    let model = DesignModel {
        design_name: "design".to_owned(),
        top_instances: vec![top],
        modules: Vec::new(),
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let result = snapshot("root", &model);
    let generated = &result.roots[0].generated_scopes[0];
    assert_eq!(generated.name, "g[0]");
    assert_eq!(generated.params[0].value.as_deref(), Some("32'd3"));
    assert_eq!(generated.children[0].module_type, "leaf");
    assert_eq!(generated.children[0].id, "instance:root:top.g[0].u");
}

#[test]
fn wire_shape_keeps_typed_fields_and_camel_case_names() {
    let mut top = instance("top", "top", "top");
    top.ports.push(PortModel {
        name: "clock".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(1)),
    });
    let model = DesignModel {
        design_name: "design".to_owned(),
        top_instances: vec![top],
        modules: Vec::new(),
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let value = serde_json::to_value(snapshot("root", &model)).expect("serialize snapshot");
    assert_eq!(value["roots"][0]["instanceName"], "top");
    assert_eq!(value["roots"][0]["moduleType"], "top");
    assert_eq!(value["roots"][0]["ports"][0]["direction"], "input");
    assert_eq!(value["roots"][0]["ports"][0]["type"]["kind"], "logic");
    assert_eq!(value["roots"][0]["ports"][0]["type"]["width"], 1);
}

#[test]
fn syntax_fallback_modules_are_useful_without_instance_data() {
    let model = DesignModel {
        design_name: String::new(),
        top_instances: Vec::new(),
        modules: vec![ModuleDef {
            name: "broken".to_owned(),
            file: Some("/workspace/broken.sv".to_owned()),
            line: 2,
            col: 8,
            end_line: 2,
            end_col: 14,
        }],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let result = snapshot("root", &model);
    assert_eq!(result.roots.len(), 0);
    assert_eq!(result.modules[0].name, "broken");
    assert!(result.modules[0].ports.is_empty());
    assert!(result.modules[0]
        .uri
        .as_deref()
        .is_some_and(|uri| uri.starts_with("file:")));
}

#[test]
fn source_graph_excludes_instantiated_definitions_from_roots() {
    let top = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_child", "child", 4)],
    );
    let child = source_definition(
        "child",
        "/workspace/child.sv",
        1,
        vec![source_instance("u_leaf", "leaf", 5)],
    );
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let unrelated = source_definition("unrelated", "/workspace/unrelated.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![top, child, leaf, unrelated], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let mut root_names = result
        .roots
        .iter()
        .map(|root| root.module_type.as_str())
        .collect::<Vec<_>>();
    root_names.sort_unstable();
    assert_eq!(root_names, vec!["top", "unrelated"]);

    let top_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "top")
        .expect("top root");
    let child_node = top_root
        .children
        .iter()
        .find(|child| child.instance_name == "u_child")
        .expect("source child");
    assert_eq!(child_node.module_type, "child");
    assert_eq!(child_node.content_source.as_deref(), Some("declaration"));
    assert_eq!(child_node.children[0].module_type, "leaf");
    assert_eq!(result.modules.len(), 4);
}

#[test]
fn small_recursive_source_hierarchy_still_expands_normally() {
    let top = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_child", "child", 4)],
    );
    let child = source_definition(
        "child",
        "/workspace/child.sv",
        1,
        vec![source_instance("u_leaf", "leaf", 5)],
    );
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![top, child, leaf], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let top_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "top")
        .expect("top root");
    let child_node = &top_root.children[0];
    assert_eq!(child_node.instance_name, "u_child");
    assert_eq!(child_node.children[0].instance_name, "u_leaf");
    assert!(!top_root.is_budget_truncated);
    assert!(!child_node.is_budget_truncated);
    assert!(!child_node.is_cycle_truncated);
    assert_eq!(instance_stats(&result.roots), (3, 0, 0));
}

#[test]
fn graph_budget_counts_generate_wrappers_across_two_roots() {
    let mut first = source_definition("first", "/workspace/first.sv", 1, Vec::new());
    let mut second = source_definition("second", "/workspace/second.sv", 1, Vec::new());
    for index in 0..(MAX_INSTANCE_NODES / 2) {
        first.generated_scopes.push(ModuleGraphGenerateScope {
            name: format!("first_gen_{index}"),
            file: first.file.clone(),
            line: index as u32 + 2,
            col: 1,
            children: Vec::new(),
            nested: Vec::new(),
        });
        second.generated_scopes.push(ModuleGraphGenerateScope {
            name: format!("second_gen_{index}"),
            file: second.file.clone(),
            line: index as u32 + 2,
            col: 1,
            children: Vec::new(),
            nested: Vec::new(),
        });
    }
    let analysis = graph_analysis(vec![first, second], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let (total, scopes, budget_markers) = serialized_hierarchy_stats(&result.roots);
    assert_eq!(result.roots.len(), 2);
    assert!(scopes < MAX_INSTANCE_NODES);
    assert!(total <= MAX_INSTANCE_NODES);
    assert_eq!(budget_markers, 2);
    assert_eq!(instance_stats(&result.roots).1, 2);
}

#[test]
fn response_budget_is_shared_across_independent_analysis_roots() {
    let mut first = instance("first", "first", "first");
    first.children = (0..6_000)
        .map(|index| {
            instance(
                &format!("u_first_{index}"),
                &format!("first.u_first_{index}"),
                "leaf",
            )
        })
        .collect();
    let mut second = instance("second", "second", "second");
    second.children = (0..6_000)
        .map(|index| {
            instance(
                &format!("u_second_{index}"),
                &format!("second.u_second_{index}"),
                "leaf",
            )
        })
        .collect();

    let first_analysis = graph_analysis(Vec::new(), vec![first], None);
    let second_analysis = graph_analysis(Vec::new(), vec![second], None);
    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first_snapshot =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second_snapshot = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first_snapshot, second_snapshot]);

    let (total, budget_markers, _) = instance_stats(&merged.roots);
    assert!(total <= MAX_INSTANCE_NODES);
    assert_eq!(budget_markers, 1);
    assert!(merged.roots.iter().any(|root| root.module_type == "first"));
    assert!(merged.roots.iter().any(|root| root.module_type == "second"));
}

#[test]
fn deep_graph_and_compatibility_chains_stop_before_the_stack_grows_unbounded() {
    const CHAIN_LENGTH: usize = 5_100;

    let definitions = (0..CHAIN_LENGTH)
        .map(|index| {
            let children = (index + 1 < CHAIN_LENGTH).then(|| {
                vec![source_instance(
                    "u_next",
                    &format!("module_{}", index + 1),
                    index as u32 + 2,
                )]
            });
            source_definition(
                &format!("module_{index}"),
                &format!("/workspace/module_{index}.sv"),
                index as u32 + 1,
                children.unwrap_or_default(),
            )
        })
        .collect();
    let graph = graph_analysis(definitions, Vec::new(), None);
    let graph_result = snapshot_analysis("root", &graph, identity_source);
    let (graph_total, graph_budget_markers, graph_cycle_markers) =
        instance_stats(&graph_result.roots);
    assert_eq!(graph_result.roots.len(), 1);
    assert!(graph_total <= MAX_SAFE_HIERARCHY_DEPTH + 1);
    assert_eq!(graph_budget_markers, 1);
    assert_eq!(graph_cycle_markers, 0);

    let mut chain = instance("tail", "chain.tail", "leaf");
    for index in (0..CHAIN_LENGTH).rev() {
        let name = format!("u_{index}");
        let mut parent = instance(&name, &format!("chain.{name}"), "leaf");
        parent.children.push(chain);
        chain = parent;
    }
    let model = DesignModel {
        design_name: "design".to_owned(),
        top_instances: vec![chain],
        modules: Vec::new(),
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let compatibility_result = snapshot("root", &model);
    let (compatibility_total, compatibility_budget_markers, compatibility_cycle_markers) =
        instance_stats(&compatibility_result.roots);
    assert!(compatibility_total <= MAX_SAFE_HIERARCHY_DEPTH + 1);
    assert_eq!(compatibility_budget_markers, 1);
    assert_eq!(compatibility_cycle_markers, 0);
}

#[test]
fn configured_top_does_not_promote_nested_occurrence() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![source_instance("u_top", "top", 4)],
    );
    let top = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_leaf", "leaf", 4)],
    );
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let unrelated = source_definition("unrelated", "/workspace/unrelated.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![parent, top, leaf, unrelated], Vec::new(), Some("top"));

    let result = snapshot_analysis("root", &analysis, identity_source);
    assert!(!result.roots.iter().any(|root| root.module_type == "top"));
    assert!(result.roots.iter().any(|root| root.module_type == "parent"));
    assert!(result
        .roots
        .iter()
        .any(|root| root.module_type == "unrelated"));
    let parent_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "parent")
        .expect("parent root");
    assert_eq!(
        parent_root.children[0].module_type, "top",
        "configured top must remain nested under its source parent"
    );
    assert_eq!(result.roots.len(), 2);
}

#[test]
fn configured_nested_top_uses_unique_elaborated_source_occurrence() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![source_instance("u_child", "child", 4)],
    );
    let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
    child.ports.push(ModuleGraphPort {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", None),
        detail: None,
        location: Some(source_location("/workspace/child.sv", 1, 24, 4)),
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });
    child.params.push(ModuleGraphParameter {
        name: "WIDTH".to_owned(),
        ty: ty("int", Some(32)),
        local: false,
        detail: None,
        location: Some(source_location("/workspace/child.sv", 2, 10, 5)),
        display_type: Some("int".to_owned()),
        display_shape: ModuleGraphTypeShape::default(),
    });
    child.signals.push(ModuleGraphSignal {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", None),
        detail: None,
        location: Some(source_location("/workspace/child.sv", 3, 22, 7)),
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });

    let mut configured = instance("child", "work@child", "child");
    configured.file = Some("/workspace/child.sv".to_owned());
    configured.ports.push(PortModel {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(8)),
    });
    configured.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    configured.signals.push(SignalModel {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", Some(8)),
    });

    let mut analysis = graph_analysis(vec![parent, child], vec![configured], Some("child"));
    analysis.module_graph.elaborated_types.extend([
        ModuleGraphElaboratedType {
            instance: "child".to_owned(),
            name: "data".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
        },
        ModuleGraphElaboratedType {
            instance: "child".to_owned(),
            name: "payload".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
        },
    ]);

    let result = snapshot_analysis("root", &analysis, identity_source);
    assert_eq!(result.roots.len(), 1);
    assert_eq!(result.roots[0].module_type, "parent");
    let nested = result.roots[0]
        .children
        .iter()
        .find(|instance| instance.instance_name == "u_child")
        .expect("configured child source occurrence");
    assert_eq!(nested.content_source.as_deref(), Some("elaborated"));
    assert_eq!(nested.ports[0].ty.width, Some(8));
    assert_eq!(
        nested.ports[0].ty.display_type.as_deref(),
        Some("logic [7:0]")
    );
    assert_eq!(nested.params[0].value.as_deref(), Some("32'd8"));
    assert_eq!(nested.signals[0].ty.width, Some(8));
    assert_eq!(
        nested.signals[0].ty.display_type.as_deref(),
        Some("logic [7:0]")
    );
}

#[test]
fn configured_top_exact_match_beats_earlier_unmatched_same_type() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![source_instance("u_child", "child", 4)],
    );
    let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
    child.params.push(ModuleGraphParameter {
        name: "WIDTH".to_owned(),
        ty: ty("int", Some(32)),
        local: false,
        detail: None,
        location: None,
        display_type: Some("int".to_owned()),
        display_shape: ModuleGraphTypeShape::default(),
    });
    child.signals.push(ModuleGraphSignal {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", None),
        detail: None,
        location: None,
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });

    // This retained same-type instance has no matching source location or
    // name.  It appears first specifically to catch order-dependent
    // type-only fallback consuming the only source occurrence.
    let mut unmatched = instance("retained_child", "work@retained_child", "child");
    unmatched.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(16, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    unmatched.signals.push(SignalModel {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", Some(16)),
    });

    let mut exact = instance("u_child", "work@parent.u_child", "child");
    exact.line = 4;
    exact.col = 3;
    exact.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    exact.signals.push(SignalModel {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", Some(8)),
    });

    let mut analysis = graph_analysis(vec![parent, child], vec![unmatched, exact], Some("child"));
    analysis
        .module_graph
        .elaborated_types
        .push(ModuleGraphElaboratedType {
            instance: "parent.u_child".to_owned(),
            name: "payload".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
        });

    let result = snapshot_analysis("root", &analysis, identity_source);
    let parent_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "parent")
        .expect("parent root");
    let nested = parent_root
        .children
        .iter()
        .find(|child| child.instance_name == "u_child")
        .expect("exact source occurrence");

    assert_eq!(nested.content_source.as_deref(), Some("elaborated"));
    assert_eq!(nested.params[0].value.as_deref(), Some("32'd8"));
    assert_eq!(nested.signals[0].ty.width, Some(8));
    assert_eq!(
        nested.signals[0].ty.display_type.as_deref(),
        Some("logic [7:0]")
    );
}

#[test]
fn configured_top_does_not_guess_between_same_type_source_occurrences() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![
            source_instance("u_first", "child", 4),
            source_instance("u_second", "child", 8),
        ],
    );
    let mut child = source_definition("child", "/workspace/child.sv", 1, Vec::new());
    child.ports.push(ModuleGraphPort {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", None),
        detail: None,
        location: None,
        display_type: None,
        display_shape: ModuleGraphTypeShape::default(),
    });

    let mut exact = instance("u_first", "work@parent.u_first", "child");
    exact.file = Some("/workspace/top.sv".to_owned());
    exact.line = 4;
    exact.col = 3;
    exact.ports.push(PortModel {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(8)),
    });

    let mut unmatched = instance("retained_child", "work@retained_child", "child");
    unmatched.file = Some("/workspace/child.sv".to_owned());
    unmatched.ports.push(PortModel {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(16)),
    });

    let analysis = graph_analysis(vec![parent, child], vec![exact, unmatched], Some("child"));
    let result = snapshot_analysis("root", &analysis, identity_source);
    let parent_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "parent")
        .expect("parent root");
    let first = parent_root
        .children
        .iter()
        .find(|child| child.instance_name == "u_first")
        .expect("exact source occurrence");
    assert_eq!(first.content_source.as_deref(), Some("elaborated"));
    assert_eq!(first.ports[0].ty.width, Some(8));

    let second = parent_root
        .children
        .iter()
        .find(|child| child.instance_name == "u_second")
        .expect("unmatched source occurrence");
    assert_eq!(second.content_source.as_deref(), Some("declaration"));
    assert_ne!(
        second.ports[0].ty.width,
        Some(16),
        "an unmatched retained instance must not be assigned arbitrarily"
    );
}

#[test]
fn response_budget_preserves_a_hierarchy_root_before_large_module_content() {
    let mut definition = source_definition("large", "/workspace/large.sv", 1, Vec::new());
    definition.signals = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    let analysis = graph_analysis(vec![definition], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let root = result
        .roots
        .iter()
        .find(|root| root.module_type == "large")
        .expect("large module hierarchy root");
    assert!(root.definition_id.is_some());
    assert!(!root.is_ambiguous);
    assert!(root.is_budget_truncated);
    assert!(root.signals.len() < MAX_INSTANCE_NODES);
    assert!(
        result.modules.iter().any(|module| module.name == "large"),
        "hierarchy expansion must leave a usable module catalog prefix"
    );
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn fair_catalog_keeps_a_real_entry_after_hierarchy_content_exhaustion() {
    let mut definition = source_definition("large", "/workspace/large.sv", 1, Vec::new());
    definition.signals = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    let analysis = graph_analysis(vec![definition], Vec::new(), None);

    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(1);
    let result = snapshot_analysis_with_budget("root", &analysis, identity_source, &mut budget);

    let root = result.roots.first().expect("hierarchy root");
    assert!(root.is_budget_truncated);
    let module = result
        .modules
        .iter()
        .find(|module| module.name == "large")
        .expect("real module catalog entry");
    assert_eq!(module.content_source.as_deref(), Some("declaration"));
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn fair_catalog_keeps_later_workspace_entries_after_earlier_hierarchy_exhaustion() {
    let mut first_definition = source_definition("first", "/workspace/first.sv", 1, Vec::new());
    first_definition.signals = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    let first_analysis = graph_analysis(vec![first_definition], Vec::new(), None);
    let second_analysis = graph_analysis(
        vec![source_definition(
            "second",
            "/workspace/second.sv",
            1,
            Vec::new(),
        )],
        Vec::new(),
        None,
    );

    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(merged.modules.iter().any(|module| module.name == "first"));
    assert!(merged.modules.iter().any(|module| module.name == "second"));
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type == "first" && root.is_budget_truncated));
    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
}

#[test]
fn fair_budget_accounts_hierarchy_and_catalog_nodes_within_global_cap() {
    let mut first_definition = source_definition("first", "/workspace/first.sv", 1, Vec::new());
    first_definition.signals = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    let first_analysis = graph_analysis(vec![first_definition], Vec::new(), None);
    let second_definitions = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| {
            source_definition(
                &format!("second_{index}"),
                &format!("/workspace/second_{index}.sv"),
                index as u32 + 1,
                Vec::new(),
            )
        })
        .collect();
    let second_analysis = graph_analysis(second_definitions, Vec::new(), None);

    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
}

#[test]
fn response_budget_keeps_a_huge_root_catalog_bounded_and_visible() {
    let definitions = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| {
            source_definition(
                &format!("module_{index}"),
                &format!("/workspace/module_{index}.sv"),
                index as u32 + 1,
                Vec::new(),
            )
        })
        .collect();
    let analysis = graph_analysis(definitions, Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    assert!(result
        .modules
        .iter()
        .any(|module| module.is_budget_truncated));
    let root = result
        .roots
        .first()
        .expect("one source root survives the catalog budget");
    assert!(root.definition_id.is_some());
    assert!(result.modules.len() <= MAX_INSTANCE_NODES);
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn response_budget_keeps_later_workspace_roots_after_catalog_truncation() {
    let mut first_definition = source_definition("first", "/workspace/first.sv", 1, Vec::new());
    first_definition.signals = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    let first_analysis = graph_analysis(vec![first_definition], Vec::new(), None);
    let second_analysis = graph_analysis(
        vec![
            source_definition(
                "second",
                "/workspace/second.sv",
                1,
                vec![source_instance("u_leaf", "second_leaf", 2)],
            ),
            source_definition("second_leaf", "/workspace/second_leaf.sv", 1, Vec::new()),
        ],
        Vec::new(),
        None,
    );

    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
    let first_root = merged
        .roots
        .iter()
        .find(|root| root.module_type == "first")
        .expect("first workspace root");
    assert!(first_root.is_budget_truncated);
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type == "first" && root.definition_id.is_some()));
    let second_root = merged
        .roots
        .iter()
        .find(|root| root.module_type == "second")
        .expect("later workspace root");
    assert!(second_root.definition_id.is_some());
    assert!(second_root.children.is_empty());
    assert!(
        second_root.is_budget_truncated,
        "an omitted child must be represented as hierarchy truncation"
    );
}

#[test]
fn repeated_cycle_terminals_preserve_a_later_workspace_root_slot() {
    // Arrange
    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    assert!(budget.take_root(), "first workspace root");

    // Act: model an early workspace containing enough cycle leaves to
    // consume every non-root slot.
    while budget.take_terminal() {}

    // Assert: stopping cycle expansion must leave the reserved capacity
    // available to a later workspace hierarchy root.
    budget.begin_workspace(1);
    assert!(budget.take_root(), "later workspace root reservation");
}

#[test]
fn exhausted_cycle_leaves_stay_accounted_and_preserve_later_workspace_root() {
    let cycle_children = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| source_instance(&format!("u_cycle_{index}"), "cycle", index as u32 + 2))
        .collect();
    let first_analysis = graph_analysis(
        vec![source_definition(
            "cycle",
            "/workspace/cycle.sv",
            1,
            cycle_children,
        )],
        Vec::new(),
        None,
    );
    let second_analysis = graph_analysis(
        vec![source_definition(
            "second",
            "/workspace/second.sv",
            1,
            Vec::new(),
        )],
        Vec::new(),
        None,
    );
    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
    assert!(merged.roots.iter().any(|root| root.is_cycle_root));
    assert!(merged.roots.iter().any(|root| root.module_type == "second"));
}

#[test]
fn omitted_generate_scope_marks_parent_at_regular_budget_boundary() {
    let mut definition = source_definition("top", "/workspace/top.sv", 1, Vec::new());
    let ordinary_content_slots =
        MAX_INSTANCE_NODES - GRAPH_TERMINAL_SLOTS - GUARANTEED_HIERARCHY_ROOT_SLOTS - 2 - 1;
    definition.signals = (0..ordinary_content_slots)
        .map(|index| ModuleGraphSignal {
            name: format!("signal_{index}"),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        })
        .collect();
    definition.generated_scopes.push(ModuleGraphGenerateScope {
        name: "late_scope".to_owned(),
        file: definition.file.clone(),
        line: ordinary_content_slots as u32 + 2,
        col: 1,
        children: Vec::new(),
        nested: Vec::new(),
    });
    let analysis = graph_analysis(vec![definition], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);

    let root = result.roots.first().expect("top hierarchy root");
    assert!(root.generated_scopes.is_empty());
    assert!(
        root.is_budget_truncated,
        "omitting a generate scope must mark its containing instance"
    );
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn depth_limited_scope_merge_is_explicit_without_a_marker_slot() {
    let analysis = graph_analysis(
        vec![source_definition("top", "/workspace/top.sv", 1, Vec::new())],
        Vec::new(),
        None,
    );
    let catalog = GraphCatalog::new("root", &analysis.module_graph, &identity_source);
    let represented = HashSet::new();
    let lookup = SourceElaborationLookup::new(&catalog, &[], &represented);
    let source = ModuleGraphGenerateScope {
        name: "nested".to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line: 2,
        col: 1,
        children: Vec::new(),
        nested: Vec::new(),
    };
    let mut elaborated = ExplorerGenerateScope {
        id: "scope:root:top.nested".to_owned(),
        name: "nested".to_owned(),
        is_budget_truncated: false,
        params: Vec::new(),
        children: Vec::new(),
        nested_scopes: Vec::new(),
    };
    let mut budget = InstanceBudget::new(GRAPH_TERMINAL_SLOTS);
    budget.hierarchy_stopped = true;
    budget.budget_marker_emitted = true;

    merge_source_scope(
        "root",
        &catalog,
        &identity_source,
        "top",
        0,
        &mut elaborated,
        &source,
        &lookup,
        &mut Vec::new(),
        &mut budget,
        MAX_SAFE_HIERARCHY_DEPTH,
    );

    assert!(elaborated.is_budget_truncated);
    assert!(elaborated.children.is_empty());
}

#[test]
fn generated_scope_marks_omitted_siblings_without_an_available_marker() {
    let analysis = graph_analysis(
        vec![
            source_definition("top", "/workspace/top.sv", 1, Vec::new()),
            source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new()),
        ],
        Vec::new(),
        None,
    );
    let catalog = GraphCatalog::new("root", &analysis.module_graph, &identity_source);
    let lookup = SourceElaborationLookup::new(&catalog, &[], &HashSet::new());
    let mut first = instance("u0", "top.g[0].u0", "leaf");
    first.signals = (0..2)
        .map(|index| SignalModel {
            name: format!("payload_{index}"),
            kind: "var".to_owned(),
            ty: ty("logic", Some(1)),
        })
        .collect();
    let second = instance("u1", "top.g[0].u1", "leaf");
    let scope = GenScopeModel {
        name: "g[0]".to_owned(),
        full_name: "top.g[0]".to_owned(),
        params: Vec::new(),
        children: vec![first, second],
    };
    let mut budget = InstanceBudget::new(GRAPH_TERMINAL_SLOTS);
    // Leave enough regular capacity for the scope, u0, and one payload
    // signal. u0's next signal exhausts it; the hierarchy marker was
    // already spent, so u1 must remain omitted without a fake marker.
    budget.remaining = 8;
    budget.budget_marker_emitted = true;
    let generated = graph_elaborated_scope(
        "root",
        &catalog,
        &identity_source,
        "top",
        "instance:root:top",
        &scope,
        &lookup,
        &mut Vec::new(),
        &mut budget,
        0,
    )
    .expect("generated scope remains serializable");

    assert!(generated.is_budget_truncated);
    assert_eq!(generated.children.len(), 1);
    assert_eq!(generated.children[0].instance_name, "u0");
    assert_eq!(generated.children[0].signals.len(), 1);
    assert!(!generated
        .children
        .iter()
        .any(|child| child.instance_name == "<budget-truncated>"));
}

#[test]
fn merged_generated_scope_marks_siblings_without_an_available_marker() {
    let analysis = graph_analysis(
        vec![
            source_definition("top", "/workspace/top.sv", 1, Vec::new()),
            source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new()),
        ],
        Vec::new(),
        None,
    );
    let catalog = GraphCatalog::new("root", &analysis.module_graph, &identity_source);
    let lookup = SourceElaborationLookup::new(&catalog, &[], &HashSet::new());
    let source = ModuleGraphGenerateScope {
        name: "g".to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line: 2,
        col: 1,
        children: vec![
            source_instance("u0", "leaf", 3),
            source_instance("u1", "leaf", 4),
        ],
        nested: Vec::new(),
    };
    let mut elaborated = ExplorerGenerateScope {
        id: "generate:root:top.g".to_owned(),
        name: "g".to_owned(),
        is_budget_truncated: false,
        params: Vec::new(),
        children: Vec::new(),
        nested_scopes: Vec::new(),
    };
    let mut budget = InstanceBudget::with_reserved_slots(0, 0, 0);
    budget.remaining = 1;
    budget.budget_marker_emitted = true;
    merge_source_scope(
        "root",
        &catalog,
        &identity_source,
        "top",
        0,
        &mut elaborated,
        &source,
        &lookup,
        &mut Vec::new(),
        &mut budget,
        0,
    );

    assert!(elaborated.is_budget_truncated);
    assert_eq!(elaborated.children.len(), 1);
    assert_eq!(elaborated.children[0].instance_name, "u0");
    assert!(!elaborated
        .children
        .iter()
        .any(|child| child.instance_name == "<budget-truncated>"));
}

#[test]
fn tight_workspace_catalog_quotas_use_entries_before_markers() {
    let mut budget = InstanceBudget::with_reserved_slots(0, 0, 2);
    budget.prepare_workspaces();

    budget.begin_workspace(2);
    assert!(budget.take_module(true));
    assert!(!budget.take_module_marker());

    budget.begin_workspace(1);
    assert!(budget.take_module(true));
    assert!(!budget.take_module_marker());
    assert_eq!(budget.module_catalog_slots, 0);
    assert_eq!(budget.remaining, MAX_INSTANCE_NODES - 2);
}

#[test]
fn one_slot_workspace_catalog_marks_the_real_prefix_when_truncated() {
    // Arrange
    let analysis = graph_analysis(
        vec![
            source_definition("first", "/workspace/first.sv", 1, Vec::new()),
            source_definition("second", "/workspace/second.sv", 1, Vec::new()),
        ],
        Vec::new(),
        None,
    );
    let mut budget = InstanceBudget::with_reserved_slots(0, 0, 1);
    budget.prepare_workspaces();
    budget.begin_workspace(1);

    // Act
    let result = snapshot_analysis_with_budget("root", &analysis, identity_source, &mut budget);

    // Assert
    assert_eq!(result.modules.len(), 1);
    let module = result.modules.first().expect("one real module entry");
    assert_ne!(module.name, "<budget-truncated>");
    assert!(module.is_budget_truncated);
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn one_slot_workspace_catalog_leaves_a_complete_definition_unmarked() {
    // Arrange
    let analysis = graph_analysis(
        vec![source_definition(
            "only",
            "/workspace/only.sv",
            1,
            Vec::new(),
        )],
        Vec::new(),
        None,
    );
    let mut budget = InstanceBudget::with_reserved_slots(0, 0, 1);
    budget.prepare_workspaces();
    budget.begin_workspace(1);

    // Act
    let result = snapshot_analysis_with_budget("root", &analysis, identity_source, &mut budget);

    // Assert
    let module = result.modules.first().expect("one complete module entry");
    assert_eq!(result.modules.len(), 1);
    assert_eq!(module.name, "only");
    assert!(!module.is_budget_truncated);
    assert!(serialized_response_work(&result) <= MAX_INSTANCE_NODES);
}

#[test]
fn workspace_catalog_markers_are_reserved_per_workspace() {
    let mut budget = InstanceBudget::with_reserved_slots(0, 0, 4);
    budget.prepare_workspaces();

    budget.begin_workspace(2);
    assert!(budget.take_module(true));
    assert!(!budget.take_module(true));
    assert!(budget.take_module_marker());

    budget.begin_workspace(1);
    assert!(budget.take_module(true));
    assert!(!budget.take_module(true));
    assert!(budget.take_module_marker());
    assert_eq!(budget.module_catalog_slots, 0);
    assert_eq!(budget.remaining, MAX_INSTANCE_NODES - 4);
}

#[test]
fn oversized_first_workspace_cannot_consume_later_workspace_catalog_or_root_quota() {
    let first_definitions = (0..(MAX_INSTANCE_NODES * 2))
        .map(|index| {
            source_definition(
                &format!("first_{index}"),
                &format!("/workspace/first_{index}.sv"),
                index as u32 + 1,
                Vec::new(),
            )
        })
        .collect();
    let first_analysis = graph_analysis(first_definitions, Vec::new(), None);
    let second_analysis = graph_analysis(
        vec![source_definition(
            "second",
            "/workspace/second.sv",
            1,
            Vec::new(),
        )],
        Vec::new(),
        None,
    );
    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type == "second" && root.definition_id.is_some()));
    assert!(merged
        .modules
        .iter()
        .any(|module| module.name == "second" && !module.is_budget_truncated));
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type.starts_with("first_") && root.is_budget_truncated));
}

#[test]
fn response_budget_keeps_catalog_and_hierarchy_bounded_across_analysis_roots() {
    let make_analysis = |prefix: &str| {
        let definitions = (0..6_000)
            .map(|index| {
                source_definition(
                    &format!("{prefix}_{index}"),
                    &format!("/workspace/{prefix}_{index}.sv"),
                    index as u32 + 1,
                    Vec::new(),
                )
            })
            .collect();
        graph_analysis(definitions, Vec::new(), None)
    };
    let first_analysis = make_analysis("first");
    let second_analysis = make_analysis("second");
    let mut budget = new_response_budget();
    budget.prepare_workspaces();
    budget.begin_workspace(2);
    let first =
        snapshot_analysis_with_budget("first-root", &first_analysis, identity_source, &mut budget);
    budget.begin_workspace(1);
    let second = snapshot_analysis_with_budget(
        "second-root",
        &second_analysis,
        identity_source,
        &mut budget,
    );
    let merged = merge([first, second]);

    assert!(serialized_response_work(&merged) <= MAX_INSTANCE_NODES);
    assert!(merged.modules.len() < 12_000);
    assert!(merged.modules.len() <= MAX_INSTANCE_NODES);
    assert!(merged
        .modules
        .iter()
        .any(|module| module.is_budget_truncated));
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type.starts_with("first_") && root.definition_id.is_some()));
    assert!(merged
        .roots
        .iter()
        .any(|root| root.module_type.starts_with("second_") && root.definition_id.is_some()));
}

#[test]
fn rootless_source_cycle_has_one_bounded_cycle_root() {
    let first = source_definition(
        "first",
        "/workspace/first.sv",
        1,
        vec![source_instance("u_second", "second", 2)],
    );
    let second = source_definition(
        "second",
        "/workspace/second.sv",
        1,
        vec![source_instance("u_first", "first", 2)],
    );
    let analysis = graph_analysis(
        vec![first, second],
        vec![instance("first", "work@first", "first")],
        Some("first"),
    );

    let result = snapshot_analysis("root", &analysis, identity_source);
    assert_eq!(result.roots.len(), 1);
    let root = &result.roots[0];
    assert!(root.is_cycle_root);
    assert_eq!(root.module_type, "first");
    assert_eq!(root.children.len(), 1);
    assert_eq!(root.children[0].module_type, "second");
    let cycle_edge = root.children[0]
        .children
        .iter()
        .find(|child| child.module_type == "first")
        .expect("repeated first definition");
    assert!(cycle_edge.is_cycle_truncated);
    assert!(!cycle_edge.is_budget_truncated);
}

#[test]
fn self_cycle_is_visible_beside_an_ordinary_zero_incoming_root() {
    let ordinary = source_definition("ordinary", "/workspace/ordinary.sv", 1, Vec::new());
    let self_cycle = source_definition(
        "self_cycle",
        "/workspace/self_cycle.sv",
        1,
        vec![source_instance("self", "self_cycle", 2)],
    );
    let analysis = graph_analysis(vec![ordinary, self_cycle], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    assert_eq!(
        result
            .roots
            .iter()
            .map(|root| root.module_type.as_str())
            .collect::<Vec<_>>(),
        ["ordinary", "self_cycle"]
    );
    let ordinary_root = &result.roots[0];
    assert!(!ordinary_root.is_cycle_root);
    let cycle_root = &result.roots[1];
    assert!(cycle_root.is_cycle_root);
    assert_eq!(cycle_root.children.len(), 1);
    assert!(cycle_root.children[0].is_cycle_truncated);
}

#[test]
fn declaration_fallback_emits_typed_contents_and_filters_port_backing_signals() {
    let mut definition = source_definition("decl_top", "/workspace/decl.sv", 2, Vec::new());
    definition.ports.push(ModuleGraphPort {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(1)),
        detail: Some("input logic clk".to_owned()),
        location: Some(source_location("/workspace/decl.sv", 2, 25, 3)),
        display_type: None,
        display_shape: ModuleGraphTypeShape::default(),
    });
    definition.params.push(ModuleGraphParameter {
        name: "WIDTH".to_owned(),
        ty: ty("int", Some(32)),
        local: false,
        detail: Some("parameter int WIDTH = 8".to_owned()),
        location: Some(source_location("/workspace/decl.sv", 1, 24, 5)),
        display_type: None,
        display_shape: ModuleGraphTypeShape::default(),
    });
    definition.signals.extend([
        ModuleGraphSignal {
            name: "clk".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
            detail: None,
            location: None,
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        },
        ModuleGraphSignal {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(8)),
            detail: Some("wire logic [7:0] payload".to_owned()),
            location: Some(source_location("/workspace/decl.sv", 3, 23, 7)),
            display_type: Some("logic [7:0]".to_owned()),
            display_shape: ModuleGraphTypeShape::default(),
        },
        ModuleGraphSignal {
            name: "tri_bus".to_owned(),
            kind: "tri".to_owned(),
            ty: ty("logic", Some(4)),
            detail: Some("tri [3:0] tri_bus".to_owned()),
            location: Some(source_location("/workspace/decl.sv", 4, 14, 7)),
            display_type: None,
            display_shape: ModuleGraphTypeShape::default(),
        },
    ]);
    let analysis = graph_analysis(vec![definition], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let root = &result.roots[0];
    assert_eq!(root.content_source.as_deref(), Some("declaration"));
    assert_eq!(root.ports[0].detail.as_deref(), Some("input logic clk"));
    assert_eq!(
        root.ports[0]
            .location
            .as_ref()
            .map(|location| location.uri.as_str()),
        Some("file:///workspace/decl.sv")
    );
    assert_eq!(
        root.ports[0]
            .location
            .as_ref()
            .map(|location| (location.range.start_line, location.range.start_character)),
        Some((1, 24))
    );
    assert_eq!(root.params[0].value, None);
    assert_eq!(
        root.params[0].detail.as_deref(),
        Some("parameter int WIDTH = 8")
    );
    assert_eq!(
        root.params[0]
            .location
            .as_ref()
            .map(|location| location.range.start_line),
        Some(0)
    );
    assert_eq!(
        root.signals
            .iter()
            .map(|signal| signal.name.as_str())
            .collect::<Vec<_>>(),
        ["payload", "tri_bus"]
    );
    assert_eq!(root.signals[0].kind, "wire");
    assert_eq!(root.signals[1].kind, "tri");
    assert_eq!(
        root.signals[0]
            .location
            .as_ref()
            .map(|location| location.range.start_line),
        Some(2)
    );
    assert_eq!(
        result.modules[0].content_source.as_deref(),
        Some("declaration")
    );
}

#[test]
fn elaborated_contents_keep_exact_values_types_and_port_filtering() {
    let mut top = instance("top", "work@top", "top");
    top.file = Some("/workspace/top.sv".to_owned());
    let mut child = instance("u_child", "work@top.u_child", "child");
    child.file = Some("/workspace/child.sv".to_owned());
    child.line = 7;
    child.col = 5;
    child.ports.push(PortModel {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(1)),
    });
    child.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    child.signals.extend([
        SignalModel {
            name: "clk".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(1)),
        },
        SignalModel {
            name: "payload".to_owned(),
            kind: "wire".to_owned(),
            ty: ty("logic", Some(8)),
        },
    ]);
    top.children.push(child);
    let analysis = graph_analysis(
        vec![
            source_definition(
                "top",
                "/workspace/top.sv",
                1,
                vec![source_instance("u_child", "child", 7)],
            ),
            source_definition("child", "/workspace/child.sv", 1, Vec::new()),
        ],
        vec![top],
        None,
    );

    let result = snapshot_analysis("root", &analysis, identity_source);
    let top_root = &result.roots[0];
    let child_node = &top_root.children[0];
    assert_eq!(top_root.content_source.as_deref(), Some("elaborated"));
    assert_eq!(child_node.content_source.as_deref(), Some("elaborated"));
    assert_eq!(child_node.params[0].value.as_deref(), Some("32'd8"));
    assert_eq!(child_node.params[0].ty.width, Some(32));
    assert_eq!(child_node.signals.len(), 1);
    assert_eq!(child_node.signals[0].name, "payload");
    assert_eq!(child_node.signals[0].kind, "wire");
    assert_eq!(child_node.signals[0].ty.width, Some(8));
    let child_module = result
        .modules
        .iter()
        .find(|module| module.name == "child")
        .expect("child module entry");
    assert_eq!(child_module.content_source.as_deref(), Some("elaborated"));
    assert_eq!(child_module.params[0].value.as_deref(), Some("32'd8"));
}

#[test]
fn elaborated_types_resolve_parameters_for_nested_instances() {
    let mut top = instance("top", "work@top", "top");
    top.file = Some("/workspace/top.sv".to_owned());

    let mut child = instance("u_child", "work@top.u_child", "child");
    child.file = Some("/workspace/child.sv".to_owned());
    child.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    child.ports.push(PortModel {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(8)),
    });
    child.signals.push(SignalModel {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", Some(8)),
    });

    let mut leaf = instance("u_leaf", "work@top.u_child.u_leaf", "leaf");
    leaf.file = Some("/workspace/leaf.sv".to_owned());
    leaf.params.push(ParamModel {
        name: "WIDTH".to_owned(),
        value: Some(Val::Bits(Value::from_u64(3, 32, false))),
        ty: ty("int", Some(32)),
        local: false,
    });
    leaf.signals.push(SignalModel {
        name: "leaf_payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", Some(3)),
    });
    child.children.push(leaf);
    top.children.push(child);

    let source_top = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_child", "child", 3)],
    );
    let mut source_child = source_definition(
        "child",
        "/workspace/child.sv",
        1,
        vec![source_instance("u_leaf", "leaf", 3)],
    );
    source_child.ports.push(ModuleGraphPort {
        name: "data".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", None),
        detail: None,
        location: Some(source_location("/workspace/child.sv", 1, 45, 4)),
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });
    source_child.params.push(ModuleGraphParameter {
        name: "WIDTH".to_owned(),
        ty: ty("int", Some(32)),
        local: false,
        detail: None,
        location: Some(source_location("/workspace/child.sv", 1, 29, 5)),
        display_type: Some("int".to_owned()),
        display_shape: ModuleGraphTypeShape::default(),
    });
    source_child.signals.push(ModuleGraphSignal {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", None),
        detail: None,
        location: Some(source_location("/workspace/child.sv", 2, 22, 7)),
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });
    let mut source_leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    source_leaf.signals.push(ModuleGraphSignal {
        name: "leaf_payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", None),
        detail: None,
        location: Some(source_location("/workspace/leaf.sv", 2, 22, 12)),
        display_type: Some("logic [WIDTH-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });
    let mut analysis = graph_analysis(vec![source_top, source_child, source_leaf], vec![top], None);
    analysis.module_graph.elaborated_types = vec![
        ModuleGraphElaboratedType {
            instance: "top.u_child".to_owned(),
            name: "data".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
        },
        ModuleGraphElaboratedType {
            instance: "top.u_child".to_owned(),
            name: "payload".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 7, right: 0 })],
        },
        ModuleGraphElaboratedType {
            instance: "top.u_child.u_leaf".to_owned(),
            name: "leaf_payload".to_owned(),
            packed_ranges: vec![Some(ModuleGraphPackedRange { left: 2, right: 0 })],
        },
    ];

    let result = snapshot_analysis("root", &analysis, identity_source);
    let child_node = &result.roots[0].children[0];
    assert_eq!(
        child_node.signals[0].ty.display_type.as_deref(),
        Some("logic [7:0]")
    );
    assert_eq!(
        child_node.ports[0].ty.display_type.as_deref(),
        Some("logic [7:0]")
    );
    assert_eq!(
        child_node.params[0]
            .location
            .as_ref()
            .map(|location| location.uri.as_str()),
        Some("file:///workspace/child.sv")
    );
    let leaf_node = &child_node.children[0];
    assert_eq!(
        leaf_node.signals[0].ty.display_type.as_deref(),
        Some("logic [2:0]")
    );
}

#[test]
fn unresolved_symbolic_width_is_retained_without_guessing() {
    let mut definition = source_definition("symbolic", "/workspace/symbolic.sv", 1, Vec::new());
    definition.signals.push(ModuleGraphSignal {
        name: "payload".to_owned(),
        kind: "wire".to_owned(),
        ty: ty("logic", None),
        detail: None,
        location: None,
        display_type: Some("logic [UNKNOWN-1:0]".to_owned()),
        display_shape: ModuleGraphTypeShape {
            packed_dimensions: 1,
            unpacked_dimensions: 0,
        },
    });
    let analysis = graph_analysis(vec![definition], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let payload = &result.roots[0].signals[0];
    assert_eq!(payload.ty.width, None);
    assert_eq!(
        payload.ty.display_type.as_deref(),
        Some("logic [UNKNOWN-1:0]")
    );
    let json = serde_json::to_value(&result).expect("serialize symbolic snapshot");
    assert_eq!(
        json["roots"][0]["signals"][0]["type"]["displayType"],
        "logic [UNKNOWN-1:0]"
    );
}

#[test]
fn concrete_display_retains_symbolic_ranges_without_captured_bounds() {
    let ty = ty("logic", Some(8));
    let shape = ModuleGraphTypeShape {
        packed_dimensions: 1,
        unpacked_dimensions: 1,
    };
    let display = resolved_type_display(
        &ty,
        Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]"),
        Some(shape),
        None,
    );
    assert_eq!(
        display.as_deref(),
        Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]")
    );
    let missing = [None];
    assert_eq!(
        resolved_type_display(
            &ty,
            Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]"),
            Some(shape),
            Some(&missing),
        )
        .as_deref(),
        Some("logic [$clog2(WIDTH)-1:0] [DEPTH-1:0]")
    );
}

#[test]
fn symbolic_dimension_normalization_preserves_tokens_and_compacts_ranges() {
    assert_eq!(normalize_symbolic_expression(" P    +    +1 "), "P+ +1");
    assert_eq!(normalize_symbolic_expression(" P    -    -1 "), "P- -1");
    assert_eq!(normalize_symbolic_expression(r" \WIDTH + 1 "), r"\WIDTH +1");
    assert_eq!(
        normalize_symbolic_expression(" P    inside    { BASE , IDX } : 0 "),
        "P inside {BASE,IDX}:0"
    );
    assert_eq!(normalize_type_display("logic [ 1 : 0 ]"), "logic [1:0]");
    assert_eq!(
        normalize_type_display(r"logic [ \WIDTH + 1 : 0 ]"),
        r"logic [\WIDTH +1:0]"
    );
    assert_eq!(
        normalize_type_display("logic [ P inside { BASE , IDX } : 0 ]"),
        "logic [P inside {BASE,IDX}:0]"
    );
    assert_eq!(
        normalize_type_display("logic [ MODE == \"A  ] B\" : 0 ]"),
        "logic [MODE==\"A  ] B\":0]"
    );
}

#[test]
fn symbolic_dimension_comments_preserve_active_tokens_and_ranges() {
    let line_source = "logic [P // ignored ]\n + 1:0] payload;";
    let line_spans = bracket_spans(line_source);
    assert_eq!(line_spans.len(), 1);
    assert_eq!(line_spans[0].2, "P // ignored ]\n + 1:0");
    assert!(line_spans[0].2.contains("+ 1:0"));
    assert_eq!(normalize_type_display(line_source), line_source);

    let block_source = "logic [P /* ignored ] */ + 1:0] payload;";
    let block_spans = bracket_spans(block_source);
    assert_eq!(block_spans.len(), 1);
    assert_eq!(block_spans[0].2, "P /* ignored ] */ + 1:0");
    assert!(block_spans[0].2.contains("+ 1:0"));
    assert_eq!(normalize_type_display(block_source), block_source);

    let comments_between_operators = "P /* left */ + /* right */ + 1";
    assert_eq!(
        normalize_symbolic_expression(comments_between_operators),
        comments_between_operators
    );
    assert!(normalize_symbolic_expression(comments_between_operators).contains("+ 1"));
    assert_eq!(normalize_type_display("logic [ 1 : 0 ]"), "logic [1:0]");
}

#[test]
fn concrete_display_keeps_each_packed_dimension_separate() {
    let ty = ty("logic", Some(32));
    let shape = ModuleGraphTypeShape {
        packed_dimensions: 2,
        unpacked_dimensions: 0,
    };
    let ranges = [
        Some(ModuleGraphPackedRange { left: 3, right: 0 }),
        Some(ModuleGraphPackedRange { left: 7, right: 4 }),
    ];
    let display = resolved_type_display(
        &ty,
        Some("logic [ROWS-1:0][COLS+3:4]"),
        Some(shape),
        Some(&ranges),
    );
    assert_eq!(display.as_deref(), Some("logic [3:0] [7:4]"));
}

#[test]
fn scalar_source_display_preserves_typedef_and_net_qualifier() {
    let logic = ty("logic", Some(8));
    assert_eq!(
        resolved_type_display(&logic, Some("word_t"), None, None).as_deref(),
        Some("word_t")
    );
    assert_eq!(
        resolved_type_display(&logic, Some("wire"), None, None).as_deref(),
        Some("wire")
    );
}

#[test]
fn elaborated_and_source_generate_scopes_are_deduplicated_with_nested_wrappers() {
    let mut top = instance("top", "work@top", "top");
    top.gen_scopes.push(GenScopeModel {
        name: "g[0]".to_owned(),
        full_name: "work@top.g[0]".to_owned(),
        params: Vec::new(),
        children: vec![instance("u_leaf", "top.g[0].u_leaf", "leaf")],
    });

    let mut source_top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
    source_top.generated_scopes.push(ModuleGraphGenerateScope {
        name: "g".to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line: 4,
        col: 3,
        children: vec![source_instance("u_leaf", "leaf", 5)],
        nested: vec![ModuleGraphGenerateScope {
            name: "inner".to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 6,
            col: 5,
            children: vec![source_instance("u_nested", "leaf", 7)],
            nested: Vec::new(),
        }],
    });
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![source_top, leaf], vec![top], None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let generated = &result.roots[0].generated_scopes;
    assert_eq!(
        generated.len(),
        1,
        "source fallback duplicated an elaborated scope"
    );
    assert_eq!(generated[0].children.len(), 1);
    assert_eq!(generated[0].children[0].instance_name, "u_leaf");
    assert_eq!(generated[0].nested_scopes.len(), 1);
    assert_eq!(generated[0].nested_scopes[0].name, "inner");
    assert_eq!(
        generated[0].nested_scopes[0].children[0].instance_name,
        "u_nested"
    );
}

#[test]
fn duplicate_definition_name_is_an_explicit_unexpanded_ambiguous_leaf() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![source_instance("u_dup", "dup", 4)],
    );
    let first = source_definition("dup", "/workspace/one.sv", 1, Vec::new());
    let second = source_definition("dup", "/workspace/two.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![parent, first, second], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let parent_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "parent")
        .expect("parent root");
    let ambiguous = &parent_root.children[0];
    assert_eq!(ambiguous.module_type, "dup");
    assert!(ambiguous.is_ambiguous);
    assert_eq!(ambiguous.definition_id, None);
    assert!(ambiguous.children.is_empty());
    assert_eq!(
        result
            .modules
            .iter()
            .filter(|module| module.name == "dup")
            .count(),
        2
    );
}

#[test]
fn cycles_and_named_generate_boundaries_are_safe() {
    let mut top = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_a", "a", 3)],
    );
    top.generated_scopes.push(ModuleGraphGenerateScope {
        name: "gen_block".to_owned(),
        file: Some("/workspace/top.sv".to_owned()),
        line: 5,
        col: 3,
        children: vec![source_instance("u_leaf", "leaf", 6)],
        nested: vec![ModuleGraphGenerateScope {
            name: "inner_block".to_owned(),
            file: Some("/workspace/top.sv".to_owned()),
            line: 7,
            col: 5,
            children: vec![source_instance("u_nested", "leaf", 8)],
            nested: Vec::new(),
        }],
    });
    let a = source_definition(
        "a",
        "/workspace/a.sv",
        1,
        vec![source_instance("u_b", "b", 3)],
    );
    let b = source_definition(
        "b",
        "/workspace/b.sv",
        1,
        vec![source_instance("u_a_again", "a", 3)],
    );
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![top, a, b, leaf], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let top_root = &result.roots[0];
    assert_eq!(top_root.generated_scopes[0].name, "gen_block");
    assert_eq!(top_root.generated_scopes[0].children[0].module_type, "leaf");
    assert_eq!(
        top_root.generated_scopes[0].nested_scopes[0].name,
        "inner_block"
    );
    assert_eq!(
        top_root.generated_scopes[0].nested_scopes[0].children[0].instance_name,
        "u_nested"
    );
    let cycle_leaf = &top_root.children[0].children[0].children[0];
    assert_eq!(cycle_leaf.module_type, "a");
    assert!(
        cycle_leaf.is_cycle_truncated,
        "cycle must terminate as a cycle-marked leaf"
    );
    assert!(!cycle_leaf.is_budget_truncated);
    assert!(cycle_leaf.children.is_empty());
}

#[test]
fn expansion_budget_bounds_flat_source_fanout_to_one_marker() {
    let mut top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
    for index in 0..(MAX_INSTANCE_NODES * 2) {
        top.children.push(source_instance(
            &format!("u_{index}"),
            "leaf",
            index as u32 + 2,
        ));
    }
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![top, leaf], Vec::new(), None);

    let result = snapshot_analysis("root", &analysis, identity_source);
    let top_root = &result.roots[0];
    let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
    assert!(total <= MAX_INSTANCE_NODES);
    assert_eq!(budget_markers, 1);
    assert_eq!(cycle_markers, 0);
    assert!(top_root
        .children
        .iter()
        .any(|child| child.is_budget_truncated));
}

#[test]
fn compatibility_snapshot_bounds_flat_fanout_to_one_marker() {
    let mut top = instance("top", "top", "top");
    for index in 0..(MAX_INSTANCE_NODES * 2) {
        top.children.push(instance(
            &format!("u_{index}"),
            &format!("top.u_{index}"),
            "leaf",
        ));
    }
    let model = DesignModel {
        design_name: "design".to_owned(),
        top_instances: vec![top],
        modules: vec![
            ModuleDef {
                name: "top".to_owned(),
                file: Some("/workspace/top.sv".to_owned()),
                line: 1,
                col: 1,
                end_line: 2,
                end_col: 1,
            },
            ModuleDef {
                name: "leaf".to_owned(),
                file: Some("/workspace/leaf.sv".to_owned()),
                line: 1,
                col: 1,
                end_line: 2,
                end_col: 1,
            },
        ],
        packages: Vec::new(),
        classes: Vec::new(),
    };

    let result = snapshot("root", &model);
    let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
    assert!(total <= MAX_INSTANCE_NODES);
    assert_eq!(budget_markers, 1);
    assert_eq!(cycle_markers, 0);
}

#[test]
fn cycle_at_budget_boundary_keeps_cycle_flag_and_one_remainder_marker() {
    let parent = source_definition(
        "parent",
        "/workspace/parent.sv",
        1,
        vec![source_instance("u_top", "top", 2)],
    );
    let mut top = source_definition("top", "/workspace/top.sv", 1, Vec::new());
    // Hierarchy roots are serialized before module records. Leave enough
    // ordinary slots to place the cycle at the boundary, then include two
    // trailing siblings so the third becomes the one explicit remainder
    // marker.
    for index in 0..(MAX_INSTANCE_NODES - 7) {
        top.children.push(source_instance(
            &format!("u_leaf_{index}"),
            "leaf",
            index as u32 + 2,
        ));
    }
    top.children.push(source_instance(
        "u_cycle",
        "top",
        MAX_INSTANCE_NODES as u32 + 2,
    ));
    top.children.push(source_instance(
        "u_after",
        "leaf",
        MAX_INSTANCE_NODES as u32 + 3,
    ));
    top.children.push(source_instance(
        "u_after_second",
        "leaf",
        MAX_INSTANCE_NODES as u32 + 4,
    ));
    top.children.push(source_instance(
        "u_after_third",
        "leaf",
        MAX_INSTANCE_NODES as u32 + 5,
    ));
    let leaf = source_definition("leaf", "/workspace/leaf.sv", 1, Vec::new());
    let analysis = graph_analysis(vec![parent, top, leaf], Vec::new(), Some("top"));

    let result = snapshot_analysis("root", &analysis, identity_source);
    let parent_root = result
        .roots
        .iter()
        .find(|root| root.module_type == "parent")
        .expect("source parent root");
    let top_root = &parent_root.children[0];
    let cycle = top_root
        .children
        .iter()
        .find(|child| child.instance_name == "u_cycle")
        .expect("cycle child");
    assert!(cycle.is_cycle_truncated);
    assert!(!cycle.is_budget_truncated);
    let (total, budget_markers, cycle_markers) = instance_stats(&result.roots);
    assert!(total <= MAX_INSTANCE_NODES);
    assert_eq!(budget_markers, 1);
    assert_eq!(cycle_markers, 1);
}

#[test]
fn graph_snapshot_json_has_deterministic_ids_and_camel_case_optional_fields() {
    let mut first_definition = source_definition(
        "top",
        "/workspace/top.sv",
        1,
        vec![source_instance("u_child", "child", 4)],
    );
    first_definition.ports.push(ModuleGraphPort {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: ty("logic", Some(1)),
        detail: Some("input logic clk".to_owned()),
        location: None,
        display_type: None,
        display_shape: ModuleGraphTypeShape::default(),
    });
    let second_definition = source_definition("child", "/workspace/child.sv", 1, Vec::new());
    let first = graph_analysis(
        vec![first_definition.clone(), second_definition.clone()],
        Vec::new(),
        None,
    );
    let second = graph_analysis(vec![second_definition, first_definition], Vec::new(), None);
    let first_json = serde_json::to_value(snapshot_analysis("root", &first, identity_source))
        .expect("serialize first graph snapshot");
    let second_json = serde_json::to_value(snapshot_analysis("root", &second, identity_source))
        .expect("serialize second graph snapshot");
    assert_eq!(first_json, second_json);
    let root = &first_json["roots"][0];
    assert!(root["id"]
        .as_str()
        .is_some_and(|id| id.contains("/workspace/top.sv")));
    assert_eq!(root["contentSource"], "declaration");
    assert!(root.get("content_source").is_none());
    assert!(root.get("isAmbiguous").is_none());
    assert_eq!(root["ports"][0]["detail"], "input logic clk");
}
