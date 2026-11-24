//! Coordinates.

use super::*;

#[test]
fn navigation_ranges_use_utf16_after_supplementary_text() {
    let _guards = analysis_guards();
    let file = hp("/x/unicode.sv").to_owned();
    let source = "module top;\nlogic /* 😀 */ data;\nassign data = data;\nendmodule\n";
    let analysis = analyze(&CompileOpts {
        sources: vec![compile::OwnedSource::compilation_unit(&file, source)],
        editor: true,
        ..Default::default()
    });
    let declaration = analysis
        .tokens
        .iter()
        .flat_map(|file_tokens| file_tokens.nodes.iter())
        .find(|node| node.name.as_deref() == Some("data") && node.line == 2)
        .expect("normalized declaration token");
    assert_eq!(declaration.col, 16);
    assert_eq!(declaration.end_col, 20);
    assert_eq!(
        FeatureSourceMap::new("😀data\nwire \\escaped😀name ;\n".to_owned()).normalize_1based(
            1,
            2,
            Some("data")
        ),
        (1, 3),
        "a supplementary character before a name consumes two UTF-16 units"
    );
    assert_eq!(lsp_name_len("escaped😀name"), 13);

    let entry = analysis
        .index
        .decl_at(&file, 1, 15)
        .expect("data declaration at UTF-16 column");
    assert_eq!(
        entry_location(entry).range,
        Range::new(Position::new(1, 15), Position::new(1, 19),)
    );
    assert!(identifier_at(&analysis, &file, 1, 15).is_some());

    let hover = hover_at(&analysis, &file, 1, 15).expect("hover on data");
    assert_eq!(
        hover.range,
        Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
    );
    let definition = definition_at(&analysis, &file, 2, 7).expect("definition of data use");
    assert_eq!(definition.range.start, Position::new(1, 15));
    assert_eq!(definition.range.end, Position::new(1, 19));

    let references = references_at(&analysis, &file, 1, 15);
    let reference_starts: HashSet<_> = references
        .iter()
        .map(|location| (location.range.start.line, location.range.start.character))
        .collect();
    assert!(reference_starts.contains(&(1, 15)));
    assert!(reference_starts.contains(&(2, 7)));
    assert!(reference_starts.contains(&(2, 14)));

    let (rename_range, placeholder) =
        crate::rename::prepare_rename(&analysis, &file, 1, 15).expect("rename on data declaration");
    assert_eq!(placeholder, "data");
    assert_eq!(
        rename_range,
        Range::new(Position::new(1, 15), Position::new(1, 19))
    );

    let semantic = semantic_tokens_for(&analysis, &file);
    let mut line = 0u32;
    let mut col = 0u32;
    let mut found_data = false;
    for token in semantic.data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        if (line, col) == (1, 15) {
            assert_eq!(token.length, 4);
            found_data = true;
        }
    }
    assert!(
        found_data,
        "semantic token must use the UTF-16 declaration column"
    );
}
