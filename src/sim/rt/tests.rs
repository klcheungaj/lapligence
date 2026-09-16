use super::{container_sources, runtime_sources, value_sources};
use std::path::Path;

fn facade_source(facade: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim/rt");
    let facade_path = root.join(facade);
    let text = std::fs::read_to_string(&facade_path)
        .unwrap_or_else(|error| panic!("{}: {error}", facade_path.display()));
    let mut source = String::new();
    let mut fragments = 0;
    for line in text.lines() {
        let Some(name) = line
            .trim()
            .strip_prefix("#include \"")
            .and_then(|name| name.strip_suffix('"'))
        else {
            continue;
        };
        assert!(name.ends_with(".c"), "unexpected facade include: {name}");
        let path = root.join(name);
        source.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        );
        fragments += 1;
    }
    assert!(fragments > 1, "{facade} must assemble its domain fragments");
    source
}

#[test]
fn scheduler_embedding_matches_private_facade_order() {
    assert_eq!(runtime_sources().1, facade_source("llg_rt.c"));
}

#[test]
fn container_embedding_matches_private_facade_order() {
    assert_eq!(container_sources().1, facade_source("llg_container.c"));
}

#[test]
fn value_embedding_matches_private_facade_order() {
    assert_eq!(value_sources().1, facade_source("llg_value.c"));
}
