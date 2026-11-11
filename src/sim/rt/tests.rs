use super::{constraint_sources, container_sources, runtime_sources, value_sources};
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
fn constraint_embedding_matches_private_facade_order() {
    assert_eq!(constraint_sources().1, facade_source("llg_constraint.c"));
}

#[test]
fn container_embedding_matches_private_facade_order() {
    assert_eq!(container_sources().1, facade_source("llg_container.c"));
}

#[test]
fn value_embedding_matches_private_facade_order() {
    assert_eq!(value_sources().1, facade_source("llg_value.c"));
}

#[test]
fn compact_embedding_contains_every_private_include() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim/rt");
    let sources = super::value_backend_sources(crate::sim::value_backend::ValueBackend::Compact);
    let mut exported = sources
        .iter()
        .map(|(name, _)| root.join(name).canonicalize().unwrap())
        .collect::<std::collections::HashSet<_>>();
    exported.insert(root.join("llg_value.h").canonicalize().unwrap());
    // The platform layer ships beside every model (`platform_headers`);
    // `llg_compiler.h` is the part generated models and backends may use.
    exported.insert(root.join("llg_compiler.h").canonicalize().unwrap());
    // The message reporter ships beside every model (`message_headers`).
    exported.insert(root.join("llg_msg.h").canonicalize().unwrap());
    for (name, text) in sources {
        if !name.starts_with("value_gmp/") {
            continue;
        }
        for line in text.lines() {
            let Some(include) = line
                .trim()
                .strip_prefix("#include \"")
                .and_then(|name| name.strip_suffix('"'))
            else {
                continue;
            };
            if include.ends_with("llg_value_build.h") {
                continue;
            }
            let dependency = root
                .join(name)
                .parent()
                .unwrap()
                .join(include)
                .canonicalize()
                .unwrap();
            assert!(
                exported.contains(&dependency),
                "{name} includes unexported {include}"
            );
        }
    }
}

/// Every runtime diagnostic reaches stderr in one stdio call (assembled with
/// `llg_report_t`), so reports from different threads never interleave. A
/// message that ends with a separate newline write, or streams its body with
/// `vfprintf`, would be split.
#[test]
fn runtime_reports_write_stderr_in_one_call() {
    let (_, scheduler) = runtime_sources();
    for split in [
        "fputc('\\n', stderr)",
        "vfprintf(stderr",
        "fputs(\"\\n\", stderr)",
    ] {
        assert!(
            !scheduler.contains(split),
            "runtime writes a stderr message in pieces: `{split}`"
        );
    }
}

/// The record marker entry points (S07-D18) run only in a model that observes
/// a record. They stay `LLG_OUTLYING`, so the functions every other model
/// runs keep the placement, and with it the timing, they had before these
/// were added; the commit of a located NBA and a store through a record `ref`
/// formal reach them through one out-of-line call.
#[test]
fn record_marker_entry_points_are_outlying() {
    let (_, scheduler) = runtime_sources();
    let (_, container) = container_sources();
    for (unit, definition) in [
        (scheduler, "void llg_record_changed(sv4_t* dependency) {"),
        (scheduler, "void llg_nba_locate_marker(sv4_t* marker) {"),
        (
            scheduler,
            "void nba_located_committed(const llg_nba_locator_t* locator) {",
        ),
        (
            container,
            "void llg_record_watch_begin(llg_record_watch_t* watch,",
        ),
        (container, "void llg_record_watch_end(void* object) {"),
        (
            container,
            "void llg_record_watch_stored(const llg_value_t* value) {",
        ),
    ] {
        let at = unit
            .find(definition)
            .unwrap_or_else(|| panic!("missing `{definition}`"));
        let line = unit[..at].rfind('\n').map_or(0, |start| start + 1);
        assert!(
            unit[line..at].contains("LLG_OUTLYING "),
            "`{definition}` is not LLG_OUTLYING"
        );
    }
    for (unit, inline_work) in [
        (scheduler, "if (locator->marker) llg_record_changed"),
        (container, "watch->changed(watch->marker)"),
    ] {
        assert_eq!(
            unit.matches(inline_work).count(),
            1,
            "`{inline_work}` belongs to one outlying function"
        );
    }
}
