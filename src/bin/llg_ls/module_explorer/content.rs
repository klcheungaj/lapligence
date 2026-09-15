//! Content.

use super::*;

pub(super) fn graph_module(
    _root_id: &str,
    definition: &ModuleGraphDefinition,
    id: String,
    representative: Option<&InstanceModel>,
    catalog: &GraphCatalog<'_>,
    budget: &mut InstanceBudget,
) -> ExplorerModule {
    if let Some(instance) = representative {
        let hierarchy = graph_hierarchy_name(&instance.full_name, &instance.name);
        let mut is_budget_truncated = false;
        let ports = collect_budgeted(
            instance.ports.iter(),
            budget,
            &mut is_budget_truncated,
            |item| {
                port_with_source(
                    item,
                    definition_port(definition, &item.name),
                    catalog.packed_ranges(&hierarchy, &item.name),
                )
            },
        );
        let params = collect_budgeted(
            instance.params.iter(),
            budget,
            &mut is_budget_truncated,
            |item| {
                parameter_with_source(
                    item,
                    definition_parameter(definition, &item.name),
                    catalog.packed_ranges(&hierarchy, &item.name),
                )
            },
        );
        let signals = instance_signals_with_source(
            instance,
            Some(definition),
            catalog,
            &hierarchy,
            budget,
            &mut is_budget_truncated,
        );
        return ExplorerModule {
            id,
            name: clean_name(&definition.name).to_owned(),
            uri: source_uri(definition.file.as_deref()),
            range: source_range(
                definition.line,
                definition.col,
                definition.end_line,
                definition.end_col,
            ),
            content_source: Some("elaborated".to_owned()),
            is_budget_truncated,
            ports,
            params,
            signals,
        };
    }
    let mut is_budget_truncated = false;
    let ports = collect_budgeted(
        definition.ports.iter(),
        budget,
        &mut is_budget_truncated,
        graph_port,
    );
    let params = collect_budgeted(
        definition.params.iter(),
        budget,
        &mut is_budget_truncated,
        graph_parameter,
    );
    let signals = graph_signals(definition, budget, &mut is_budget_truncated);
    ExplorerModule {
        id,
        name: clean_name(&definition.name).to_owned(),
        uri: source_uri(definition.file.as_deref()),
        range: source_range(
            definition.line,
            definition.col,
            definition.end_line,
            definition.end_col,
        ),
        content_source: Some("declaration".to_owned()),
        is_budget_truncated,
        ports,
        params,
        signals,
    }
}

pub(super) fn graph_port(port: &crate::features::ModuleGraphPort) -> ExplorerPort {
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type_with_context(
            &port.ty,
            port.display_type.as_deref(),
            Some(port.display_shape),
            None,
        ),
        detail: port.detail.clone(),
        location: port.location.as_ref().and_then(explorer_location),
    }
}

fn graph_signal(signal: &crate::features::ModuleGraphSignal) -> ExplorerSignal {
    ExplorerSignal {
        name: signal.name.clone(),
        kind: signal.kind.clone(),
        ty: explorer_type_with_context(
            &signal.ty,
            signal.display_type.as_deref(),
            Some(signal.display_shape),
            None,
        ),
        detail: signal.detail.clone(),
        location: signal.location.as_ref().and_then(explorer_location),
    }
}

pub(super) fn graph_parameter(
    parameter: &crate::features::ModuleGraphParameter,
) -> ExplorerParameter {
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type_with_context(
            &parameter.ty,
            parameter.display_type.as_deref(),
            Some(parameter.display_shape),
            None,
        ),
        value: None,
        local: parameter.local,
        detail: parameter.detail.clone(),
        location: parameter.location.as_ref().and_then(explorer_location),
    }
}

pub(super) fn instance_signals(
    instance: &InstanceModel,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = instance
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        instance
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        signal,
    )
}

pub(super) fn instance_signals_with_source(
    instance: &InstanceModel,
    definition: Option<&ModuleGraphDefinition>,
    catalog: &GraphCatalog<'_>,
    hierarchy: &str,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = instance
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        instance
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        |signal| {
            signal_with_source(
                signal,
                definition.and_then(|definition| definition_signal(definition, &signal.name)),
                catalog.packed_ranges(hierarchy, &signal.name),
            )
        },
    )
}

pub(super) fn graph_signals(
    definition: &ModuleGraphDefinition,
    budget: &mut InstanceBudget,
    truncated: &mut bool,
) -> Vec<ExplorerSignal> {
    let port_names: HashSet<&str> = definition
        .ports
        .iter()
        .map(|port| port.name.as_str())
        .collect();
    collect_budgeted(
        definition
            .signals
            .iter()
            .filter(|signal| !port_names.contains(signal.name.as_str())),
        budget,
        truncated,
        graph_signal,
    )
}

pub(super) fn definition_port<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphPort> {
    definition.ports.iter().find(|port| port.name == name)
}

fn definition_signal<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphSignal> {
    definition.signals.iter().find(|signal| signal.name == name)
}

pub(super) fn definition_parameter<'a>(
    definition: &'a ModuleGraphDefinition,
    name: &str,
) -> Option<&'a crate::features::ModuleGraphParameter> {
    definition
        .params
        .iter()
        .find(|parameter| parameter.name == name)
}
