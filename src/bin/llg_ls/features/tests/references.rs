//! References.

use super::*;

#[test]
fn references_include_declaration() {
    let a = sample_analysis();
    let refs = references_at(&a, "/x/top.sv", 0, 4);
    assert!(refs
        .iter()
        .any(|l| l.range.start.line == 0 && l.range.start.character == 4));
}

#[test]
fn references_options_include_declaration_preserves_existing_results() {
    let a = cross_file_analysis();
    let existing = references_at(&a, "/x/a.sv", 0, 7);
    let with_option = references_at_with_options(&a, "/x/a.sv", 0, 7, true);

    assert_eq!(with_option, existing);
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path("/x/a.sv").unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path("/x/b.sv").unwrap()
            && location.range.start == Position::new(0, 12)
    }));
}

#[test]
fn references_options_exclude_indexed_declaration() {
    let a = cross_file_analysis();
    let refs = references_at_with_options(&a, "/x/a.sv", 0, 7, false);

    assert!(!refs.iter().any(|location| {
        location.uri == Url::from_file_path("/x/a.sv").unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(refs.iter().any(|location| {
        location.uri == Url::from_file_path("/x/b.sv").unwrap()
            && location.range.start == Position::new(0, 12)
    }));
}

#[test]
fn references_options_filter_explicit_slang_declaration() {
    let node = |line: u32, ty: i32| TokenInfo {
        line,
        col: 1,
        end_line: line,
        end_col: 7,
        kind: ty,
        name: Some("thing".to_owned()),
        file: "/x/fallback.sv".to_owned(),
    };
    let a = Analysis::new(
        Vec::new(),
        empty_design(),
        vec![FileTokens {
            path: "/x/fallback.sv".to_owned(),
            nodes: vec![
                node(
                    1,
                    tokens::TOKEN_SLANG_MODULE + tokens::TOKEN_DECLARATION_OFFSET,
                ),
                node(2, tokens::TOKEN_SLANG_IDENTIFIER),
            ],
        }],
        Vec::new(),
    );

    assert!(a
        .index
        .entry_at("/x/fallback.sv", 0, 0)
        .is_some_and(|entry| entry.is_decl));
    let with_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, true);
    let without_declaration = references_at_with_options(&a, "/x/fallback.sv", 0, 0, false);

    assert_eq!(with_declaration.len(), 2);
    assert_eq!(without_declaration.len(), 1);
    assert_eq!(without_declaration[0].range.start, Position::new(1, 0));
}
