//! Coordinates.

use super::*;

#[test]
fn navigation_ranges_use_utf16_after_supplementary_text() {
    let dir = std::env::temp_dir().join(format!(
        "llg-features-utf16-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temporary source directory");
    let _guard = TempDirGuard {
        dir: dir.clone(),
        orig: std::env::current_dir().expect("current directory"),
    };
    let path = dir.join("unicode.sv");
    let file = path.to_string_lossy().into_owned();
    let source = "module top;\nlogic /* 😀 */ data;\nassign data = data;\nendmodule\n";
    std::fs::write(&path, source).expect("write temporary source");

    let ty = TypeInfo {
        kind: "logic".to_owned(),
        width: Some(1),
        signed: false,
        type_name: None,
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![InstanceModel {
            name: "top".to_owned(),
            def_name: "top".to_owned(),
            full_name: "top".to_owned(),
            file: Some(file.clone()),
            line: 1,
            col: 1,
            ports: Vec::new(),
            signals: vec![SignalModel {
                name: "data".to_owned(),
                kind: "wire".to_owned(),
                ty: ty.clone(),
            }],
            params: Vec::new(),
            gen_scopes: Vec::new(),
            funcs: Vec::new(),
            children: Vec::new(),
        }],
        modules: vec![ModuleDef {
            name: "top".to_owned(),
            file: Some(file.clone()),
            line: 1,
            col: 1,
            end_line: 4,
            end_col: 1,
        }],
        packages: Vec::new(),
        classes: Vec::new(),
    };
    let token = |line: u32, col: u32, kind: i32| TokenInfo {
        line,
        col,
        end_line: line,
        end_col: col + 4,
        kind,
        name: Some("data".to_owned()),
        file: file.clone(),
    };
    let tokens = vec![FileTokens {
        path: file.clone(),
        nodes: vec![
            TokenInfo {
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 12,
                kind: tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("top".to_owned()),
                file: file.clone(),
            },
            // Token coordinates already use one-based UTF-16; the emoji in
            // the comment consumes two code units before `data`.
            token(
                2,
                16,
                tokens::TOKEN_SLANG_NET + tokens::TOKEN_DECLARATION_OFFSET,
            ),
            token(
                2,
                16,
                tokens::TOKEN_SLANG_NET + tokens::TOKEN_DECLARATION_OFFSET,
            ),
            token(3, 8, tokens::TOKEN_SLANG_IDENTIFIER),
            token(3, 15, tokens::TOKEN_SLANG_IDENTIFIER),
        ],
    }];
    let analysis = Analysis::new(Vec::new(), model, tokens, Vec::new());

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
        .entry_at(&file, 1, 15)
        .expect("data declaration at UTF-16 column");
    assert_eq!(
        entry_location(entry).range,
        Range::new(Position::new(1, 15), Position::new(1, 19),)
    );
    assert!(token_at(&analysis, &file, 1, 15).is_some());

    let hover = hover_at(&analysis, &file, 1, 15).expect("hover on data");
    assert_eq!(
        hover.range,
        Some(Range::new(Position::new(1, 15), Position::new(1, 19)))
    );
    let fallback_hover = hover_fallback(&analysis, &file, 1, 16).expect("fallback hover");
    assert_eq!(
        fallback_hover.range,
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
