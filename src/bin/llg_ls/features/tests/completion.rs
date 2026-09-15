//! Completion.

use super::*;

#[test]
fn completion_filters_by_prefix() {
    let a = sample_analysis();
    let items = completion_at(&a, "/x/top.sv", 0, 3, "mod");
    assert!(
        items.iter().any(|i| i.label == "module"),
        "items: {items:?}"
    );
    let all = completion_at(&a, "/x/top.sv", 0, 0, "");
    assert!(
        all.iter()
            .any(|i| i.label == "m" && i.kind == Some(CompletionItemKind::MODULE)),
        "items: {all:?}"
    );
}

#[test]
fn completion_includes_function_and_task_names() {
    let a = sample_analysis();
    let all = completion_at(&a, "/x/top.sv", 0, 0, "");
    assert!(
        all.iter()
            .any(|i| i.label == "add" && i.kind == Some(CompletionItemKind::FUNCTION)),
        "items: {all:?}"
    );
    assert!(
        all.iter()
            .any(|i| i.label == "run" && i.kind == Some(CompletionItemKind::FUNCTION)),
        "items: {all:?}"
    );
    // Prefix filtering applies to function candidates too.
    let pre = completion_at(&a, "/x/top.sv", 0, 2, "ad");
    assert!(pre.iter().any(|i| i.label == "add"), "items: {pre:?}");
    // The model and index-backed sources must not double-list a function.
    assert_eq!(all.iter().filter(|i| i.label == "add").count(), 1);
}
