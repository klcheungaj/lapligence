//! Presentation.

use super::*;

/// What [`visit_instance_places`] and its companions call with each URI of a
/// snapshot and the range that is in it.
pub(super) trait Visit: FnMut(&mut Option<String>, &mut Option<ExplorerRange>) {}

impl<F: FnMut(&mut Option<String>, &mut Option<ExplorerRange>)> Visit for F {}

pub(super) fn visit_instance_places(instance: &mut ExplorerInstance, visit: &mut impl Visit) {
    visit(&mut instance.uri, &mut instance.range);
    visit_content_places(
        &mut instance.ports,
        &mut instance.params,
        &mut instance.signals,
        visit,
    );
    for generated in &mut instance.generated_scopes {
        visit_scope_places(generated, visit);
    }
    for child in &mut instance.children {
        visit_instance_places(child, visit);
    }
}

fn visit_scope_places(scope: &mut ExplorerGenerateScope, visit: &mut impl Visit) {
    for parameter in &mut scope.params {
        visit_location(&mut parameter.location, visit);
    }
    for child in &mut scope.children {
        visit_instance_places(child, visit);
    }
    for nested in &mut scope.nested_scopes {
        visit_scope_places(nested, visit);
    }
}

pub(super) fn visit_content_places(
    ports: &mut [ExplorerPort],
    params: &mut [ExplorerParameter],
    signals: &mut [ExplorerSignal],
    visit: &mut impl Visit,
) {
    for port in ports {
        visit_location(&mut port.location, visit);
    }
    for parameter in params {
        visit_location(&mut parameter.location, visit);
    }
    for signal in signals {
        visit_location(&mut signal.location, visit);
    }
}

fn visit_location(location: &mut Option<ExplorerLocation>, visit: &mut impl Visit) {
    if let Some(ExplorerLocation { uri, range }) = location.take() {
        let (mut uri, mut range) = (Some(uri), Some(range));
        visit(&mut uri, &mut range);
        *location = uri
            .zip(range)
            .map(|(uri, range)| ExplorerLocation { uri, range });
    }
}
