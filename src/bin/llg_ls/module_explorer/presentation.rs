//! Presentation.

use super::*;

pub(super) fn remap_instance_uris(instance: &mut ExplorerInstance, map: &impl Fn(&Path) -> Option<PathBuf>) {
    remap_uri(&mut instance.uri, map);
    remap_content_uris(
        &mut instance.ports,
        &mut instance.params,
        &mut instance.signals,
        map,
    );
    for generated in &mut instance.generated_scopes {
        remap_scope_uris(generated, map);
    }
    for child in &mut instance.children {
        remap_instance_uris(child, map);
    }
}

fn remap_scope_uris(scope: &mut ExplorerGenerateScope, map: &impl Fn(&Path) -> Option<PathBuf>) {
    for parameter in &mut scope.params {
        remap_location(&mut parameter.location, map);
    }
    for child in &mut scope.children {
        remap_instance_uris(child, map);
    }
    for nested in &mut scope.nested_scopes {
        remap_scope_uris(nested, map);
    }
}

pub(super) fn remap_content_uris(
    ports: &mut [ExplorerPort],
    params: &mut [ExplorerParameter],
    signals: &mut [ExplorerSignal],
    map: &impl Fn(&Path) -> Option<PathBuf>,
) {
    for port in ports {
        remap_location(&mut port.location, map);
    }
    for parameter in params {
        remap_location(&mut parameter.location, map);
    }
    for signal in signals {
        remap_location(&mut signal.location, map);
    }
}

fn remap_location(
    location: &mut Option<ExplorerLocation>,
    map: &impl Fn(&Path) -> Option<PathBuf>,
) {
    let Some(location) = location else {
        return;
    };
    let Ok(parsed) = Url::parse(&location.uri) else {
        return;
    };
    let Ok(path) = parsed.to_file_path() else {
        return;
    };
    let Some(mapped) = map(&path) else {
        return;
    };
    if let Ok(uri) = Url::from_file_path(mapped) {
        location.uri = uri.to_string();
    }
}

pub(super) fn remap_uri(uri: &mut Option<String>, map: &impl Fn(&Path) -> Option<PathBuf>) {
    let Some(value) = uri.as_deref() else {
        return;
    };
    let Ok(parsed) = Url::parse(value) else {
        return;
    };
    let Ok(path) = parsed.to_file_path() else {
        return;
    };
    if let Some(mapped) = map(&path).and_then(|path| Url::from_file_path(path).ok()) {
        *uri = Some(mapped.to_string());
    }
}
