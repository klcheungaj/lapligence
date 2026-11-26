//! Completion.

use super::*;

#[test]
fn completion_filters_by_prefix() {
    let a = sample_analysis();
    let items = completion_at(&a, &[], 3, "mod");
    assert!(
        items.iter().any(|i| i.label == "module"),
        "items: {items:?}"
    );
    let all = completion_at(&a, &[], 0, "");
    assert!(
        all.iter()
            .any(|i| i.label == "m" && i.kind == Some(CompletionItemKind::MODULE)),
        "items: {all:?}"
    );
}

#[test]
fn completion_includes_function_and_task_names() {
    let a = sample_analysis();
    let all = completion_at(&a, &[], 0, "");
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
    let pre = completion_at(&a, &[], 2, "ad");
    assert!(pre.iter().any(|i| i.label == "add"), "items: {pre:?}");
    // The model and index-backed sources must not double-list a function.
    assert_eq!(all.iter().filter(|i| i.label == "add").count(), 1);
}

/// The names that an unsaved text declares, once each and without what it uses.
#[test]
fn completion_offers_unsaved_declarations() {
    let text = "`define W 8\n\
                typedef enum logic { IDLE, BUSY } phase_t;\n\
                module fresh(input logic clk);\n\
                \x20 logic [`W-1:0] added;\n\
                \x20 phase_t phase;\n\
                \x20 function automatic int twice(int added); return 2 * added; endfunction\n\
                \x20 always_ff @(posedge clk) missing <= added;\n\
                endmodule\n";
    let unsaved = open_document_identifiers(
        hp("/virtual/fresh.sv"),
        &[],
        text,
        &Default::default(),
        &CancelToken::default(),
    )
    .expect("parsed")
    .declared_names();
    let names: Vec<_> = unsaved.iter().map(|name| name.name.as_str()).collect();
    assert_eq!(
        names,
        ["IDLE", "BUSY", "phase_t", "fresh", "clk", "added", "phase", "twice"]
    );
    let a = sample_analysis();
    let offered = completion_at(&a, &unsaved, 1, "a");
    let labels = |label: &str| offered.iter().filter(|item| item.label == label).count();
    assert_eq!((labels("added"), labels("add"), labels("phase")), (1, 1, 0));
    // A scope has the members that the analysis knows.
    assert!(completion_at(&a, &unsaved, 10, "some_pkg::").is_empty());
}
