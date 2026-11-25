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

/// An escaped identifier (IEEE 1800-2009 5.6.1) is ranged from its backslash to
/// the white space after it in every answer, also where the declaration's name
/// leaves the backslash out (definitions of a use, module names).
#[test]
fn escaped_identifiers_are_ranged_from_their_backslash() {
    let _guards = analysis_guards();
    let file = hp("/x/escaped.sv");
    let text = "module \\esc!m (input logic \\a+b , output logic q);\n\
                \x20 logic \\w$x ;\n\
                \x20 assign q = \\a+b  & \\w$x ;\n\
                \x20 assign \\w$x  = 1'b0;\n\
                endmodule\n\
                \n\
                package \\p.k ;\n\
                \x20 function automatic int \\f() ();\n\
                \x20   return 0;\n\
                \x20 endfunction\n\
                endpackage\n\
                \n\
                class \\c#1 ;\n\
                \x20 int \\v:1 ;\n\
                endclass\n\
                \n\
                module top;\n\
                \x20 logic \\a+b , q;\n\
                \x20 \\esc!m  \\u-1 (.\\a+b (\\a+b ), .q(q));\n\
                endmodule\n";
    let analysis = analyze_sources(&[("/x/escaped.sv", text)]);
    let lines: Vec<&str> = text.lines().collect();
    // What `range` spans, with whether it starts with an escaped identifier,
    // which it must cover whole.
    let check = |what: &str, range: Range| -> bool {
        assert_eq!(range.start.line, range.end.line, "{what}: {range:?}");
        let line = lines[range.start.line as usize];
        let spelled = &line[range.start.character as usize..range.end.character as usize];
        let Some(escaped) = line[range.start.character as usize..]
            .starts_with('\\')
            .then(|| line[range.start.character as usize..].split(' ').next())
            .flatten()
        else {
            return false;
        };
        assert_eq!(spelled, escaped, "{what}: {range:?}");
        true
    };
    let mut escaped = std::collections::BTreeMap::<&str, usize>::new();
    let mut count = |what: &'static str, range: Range| {
        *escaped.entry(what).or_default() += usize::from(check(what, range));
    };
    for (line, l) in lines.iter().enumerate() {
        for (col, _) in l.match_indices('\\') {
            let (line, col) = (line as u32, col as u32 + 1);
            for location in references_at(&analysis, file, line, col) {
                count("references", location.range);
            }
            for target in definitions_at(&analysis, file, line, col) {
                count("definitions", target.location.range);
            }
            if let Some(hover) = hover_at(&analysis, file, line, col) {
                count("hover", hover.range.expect("hover range"));
            }
            if let Some((range, _)) = crate::rename::prepare_rename(&analysis, file, line, col) {
                count("prepareRename", range);
            }
            if let Ok(Some(edit)) = crate::rename::rename(&analysis, file, line, col, "renamed") {
                for edit in edit
                    .changes
                    .into_iter()
                    .flat_map(|c| c.into_values())
                    .flatten()
                {
                    count("rename", edit.range);
                }
            }
        }
    }
    fn symbols(found: &mut Vec<Range>, symbols: &[DocumentSymbol]) {
        for symbol in symbols {
            found.extend([symbol.range, symbol.selection_range]);
            symbols_of(found, symbol);
        }
    }
    fn symbols_of(found: &mut Vec<Range>, symbol: &DocumentSymbol) {
        symbols(found, symbol.children.as_deref().unwrap_or_default());
    }
    let mut outline = Vec::new();
    symbols(&mut outline, &document_symbols(&analysis, file));
    for range in outline {
        if range.start.line == range.end.line {
            count("documentSymbol", range);
        }
    }
    for symbol in workspace_symbols(&analysis, "") {
        count("workspace/symbol", symbol.location.range);
    }
    let mut snapshot =
        crate::module_explorer::snapshot_analysis("r", &analysis, |path| Some(path.to_path_buf()));
    let mut explorer = Vec::new();
    crate::module_explorer::visit_places(&mut snapshot, |_, range| explorer.extend(*range));
    for range in explorer {
        let range = Range::new(
            Position::new(range.start_line, range.start_character),
            Position::new(range.end_line, range.end_character),
        );
        count("moduleExplorer", range);
    }
    let (mut line, mut col) = (0, 0);
    for token in semantic_tokens_for(&analysis, file).data {
        line += token.delta_line;
        col = if token.delta_line == 0 {
            col + token.delta_start
        } else {
            token.delta_start
        };
        count(
            "semanticTokens",
            Range::new(
                Position::new(line, col),
                Position::new(line, col + token.length),
            ),
        );
    }
    for what in [
        "references",
        "definitions",
        "hover",
        "prepareRename",
        "rename",
        "documentSymbol",
        "workspace/symbol",
        "moduleExplorer",
        "semanticTokens",
    ] {
        assert!(
            escaped.get(what).is_some_and(|n| *n > 0),
            "{what}: {escaped:?}"
        );
    }
}
