//! Compatibility.

use super::*;

pub(super) struct InstanceWalkFrame<'a> {
    pub(super) instance: &'a InstanceModel,
    pub(super) entered: bool,
    pub(super) next_child: usize,
    pub(super) next_scope: usize,
    pub(super) next_scope_child: usize,
}

impl<'a> InstanceWalkFrame<'a> {
    pub(super) fn new(instance: &'a InstanceModel) -> Self {
        Self {
            instance,
            entered: false,
            next_child: 0,
            next_scope: 0,
            next_scope_child: 0,
        }
    }
}

/// Collect representative instances without recursive calls or an
/// unbounded pending list.  The traversal order matches the old pre-order:
/// direct children come before generated-scope children.
pub(super) fn collect_instances_bounded(roots: &[InstanceModel]) -> Vec<&InstanceModel> {
    let mut out = Vec::new();
    let mut root_index = 0;
    let mut stack = Vec::new();

    while out.len() < MAX_INSTANCE_NODES {
        if stack.is_empty() {
            let Some(root) = roots.get(root_index) else {
                break;
            };
            root_index += 1;
            stack.push(InstanceWalkFrame::new(root));
        }

        let next_instance = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            if frame.entered {
                None
            } else {
                frame.entered = true;
                Some(frame.instance)
            }
        };
        if let Some(instance) = next_instance {
            out.push(instance);
            continue;
        }

        let next_child = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            if frame.next_child < frame.instance.children.len() {
                let child = &frame.instance.children[frame.next_child];
                frame.next_child += 1;
                Some(child)
            } else {
                None
            }
        };
        if let Some(child) = next_child {
            stack.push(InstanceWalkFrame::new(child));
            continue;
        }

        let next_generated_child = {
            let frame = stack.last_mut().expect("non-empty instance walk");
            let mut next = None;
            while frame.next_scope < frame.instance.gen_scopes.len() {
                let scope = &frame.instance.gen_scopes[frame.next_scope];
                if frame.next_scope_child < scope.children.len() {
                    let child = &scope.children[frame.next_scope_child];
                    frame.next_scope_child += 1;
                    next = Some(child);
                    break;
                }
                frame.next_scope += 1;
                frame.next_scope_child = 0;
            }
            next
        };
        if let Some(child) = next_generated_child {
            stack.push(InstanceWalkFrame::new(child));
            continue;
        }

        stack.pop();
    }

    out
}

pub(super) fn instance_node(
    root_id: &str,
    instance: &InstanceModel,
    definition_ids: &[(String, String)],
    budget: &mut InstanceBudget,
    depth: usize,
    root: bool,
) -> Option<ExplorerInstance> {
    let id = instance_id(root_id, instance);
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        let node = compatibility_budget_marker(root_id, instance, definition_ids);
        return budget.take_budget_marker().then_some(node);
    }
    let budget_slot = if root {
        budget.take_root()
    } else {
        budget.take_regular()
    };
    if !budget_slot {
        let node = compatibility_budget_marker(root_id, instance, definition_ids);
        return budget.take_budget_marker().then_some(node);
    }

    let mut children = Vec::new();
    let mut omitted_descendants =
        budget.should_stop() && (!instance.children.is_empty() || !instance.gen_scopes.is_empty());
    for child in &instance.children {
        if budget.should_stop() {
            break;
        }
        let Some(child_node) = instance_node(
            root_id,
            child,
            definition_ids,
            budget,
            depth.saturating_add(1),
            false,
        ) else {
            omitted_descendants = true;
            break;
        };
        children.push(child_node);
        if budget.should_stop() {
            break;
        }
    }
    let mut generated_scopes = Vec::new();
    if !budget.should_stop() {
        for scope in &instance.gen_scopes {
            match generate_scope(root_id, &id, scope, definition_ids, budget, depth) {
                Some(generated) => generated_scopes.push(generated),
                None => {
                    append_compatibility_budget_marker(&id, scope, &mut children, budget);
                    break;
                }
            }
            if budget.should_stop() {
                break;
            }
        }
    }
    children.sort_by(|left, right| left.id.cmp(&right.id));
    generated_scopes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut content_truncated = false;
    let ports = collect_budgeted(instance.ports.iter(), budget, &mut content_truncated, port);
    let params = collect_budgeted(
        instance.params.iter(),
        budget,
        &mut content_truncated,
        parameter,
    );
    let signals = instance_signals(instance, budget, &mut content_truncated);

    Some(ExplorerInstance {
        id,
        instance_name: instance.name.clone(),
        module_type: clean_name(&instance.def_name).to_owned(),
        definition_id: find_definition_id(definition_ids, &instance.def_name),
        is_budget_truncated: content_truncated || omitted_descendants,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: source_uri(instance.file.as_deref()),
        range: instance_range(instance),
        ports,
        params,
        signals,
        generated_scopes,
        children,
    })
}

fn generate_scope(
    root_id: &str,
    parent_id: &str,
    scope: &GenScopeModel,
    definition_ids: &[(String, String)],
    budget: &mut InstanceBudget,
    depth: usize,
) -> Option<ExplorerGenerateScope> {
    if !budget.take_regular() {
        return None;
    }
    let id = generate_scope_id(root_id, parent_id, scope);
    let mut children = Vec::new();
    let mut is_budget_truncated = false;
    let params = collect_budgeted(
        scope.params.iter(),
        budget,
        &mut is_budget_truncated,
        parameter,
    );
    if depth >= MAX_SAFE_HIERARCHY_DEPTH {
        is_budget_truncated = true;
        append_compatibility_budget_marker(parent_id, scope, &mut children, budget);
        return Some(ExplorerGenerateScope {
            id,
            name: scope.name.clone(),
            is_budget_truncated,
            params,
            children,
            nested_scopes: Vec::new(),
        });
    }
    for child in &scope.children {
        if budget.should_stop() {
            break;
        }
        let Some(child_node) = instance_node(
            root_id,
            child,
            definition_ids,
            budget,
            depth.saturating_add(1),
            false,
        ) else {
            is_budget_truncated = true;
            break;
        };
        children.push(child_node);
        if budget.should_stop() {
            break;
        }
    }
    if children.len() < scope.children.len() {
        is_budget_truncated = true;
        append_compatibility_budget_marker(parent_id, scope, &mut children, budget);
    }
    children.sort_by(|left, right| left.id.cmp(&right.id));
    Some(ExplorerGenerateScope {
        id,
        name: scope.name.clone(),
        is_budget_truncated,
        params,
        children,
        nested_scopes: Vec::new(),
    })
}

fn append_compatibility_budget_marker(
    parent_id: &str,
    scope: &GenScopeModel,
    children: &mut Vec<ExplorerInstance>,
    budget: &mut InstanceBudget,
) {
    if !budget.take_budget_marker() {
        return;
    }
    children.push(ExplorerInstance {
        id: format!("{parent_id}:generate:{}:budget-truncated", scope.name),
        instance_name: "<budget-truncated>".to_owned(),
        module_type: "<budget-truncated>".to_owned(),
        definition_id: None,
        is_budget_truncated: true,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: None,
        range: None,
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        generated_scopes: Vec::new(),
        children: Vec::new(),
    });
}

fn compatibility_budget_marker(
    root_id: &str,
    instance: &InstanceModel,
    definition_ids: &[(String, String)],
) -> ExplorerInstance {
    ExplorerInstance {
        id: instance_id(root_id, instance),
        instance_name: instance.name.clone(),
        module_type: clean_name(&instance.def_name).to_owned(),
        definition_id: find_definition_id(definition_ids, &instance.def_name),
        is_budget_truncated: true,
        is_cycle_truncated: false,
        is_ambiguous: false,
        is_cycle_root: false,
        content_source: None,
        uri: source_uri(instance.file.as_deref()),
        range: instance_range(instance),
        ports: Vec::new(),
        params: Vec::new(),
        signals: Vec::new(),
        generated_scopes: Vec::new(),
        children: Vec::new(),
    }
}
