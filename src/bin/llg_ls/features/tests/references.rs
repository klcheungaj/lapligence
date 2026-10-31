//! References.

use super::*;

#[test]
fn references_include_declaration() {
    let a = sample_analysis();
    let refs = references_at(&a, hp("/x/top.sv"), 0, 4);
    assert!(refs
        .iter()
        .any(|l| l.range.start.line == 0 && l.range.start.character == 4));
}

#[test]
fn references_options_include_declaration_preserves_existing_results() {
    let a = cross_file_analysis();
    let existing = references_at(&a, hp("/x/a.sv"), 0, 7);
    let with_option = references_at_with_options(&a, hp("/x/a.sv"), 0, 7, true);

    assert_eq!(with_option, existing);
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path(hp("/x/a.sv")).unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(with_option.iter().any(|location| {
        location.uri == Url::from_file_path(hp("/x/b.sv")).unwrap()
            && location.range.start == Position::new(0, 12)
    }));
}

#[test]
fn references_options_exclude_indexed_declaration() {
    let a = cross_file_analysis();
    let refs = references_at_with_options(&a, hp("/x/a.sv"), 0, 7, false);

    assert!(!refs.iter().any(|location| {
        location.uri == Url::from_file_path(hp("/x/a.sv")).unwrap()
            && location.range.start == Position::new(0, 7)
    }));
    assert!(refs.iter().any(|location| {
        location.uri == Url::from_file_path(hp("/x/b.sv")).unwrap()
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
        file: hp("/x/fallback.sv").to_owned(),
    };
    let a = Analysis::new(
        Vec::new(),
        empty_design(),
        vec![FileTokens {
            path: hp("/x/fallback.sv").to_owned(),
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
        .entry_at(hp("/x/fallback.sv"), 0, 0)
        .is_some_and(|entry| entry.is_decl));
    let with_declaration = references_at_with_options(&a, hp("/x/fallback.sv"), 0, 0, true);
    let without_declaration = references_at_with_options(&a, hp("/x/fallback.sv"), 0, 0, false);

    assert_eq!(with_declaration.len(), 2);
    assert_eq!(without_declaration.len(), 1);
    assert_eq!(without_declaration[0].range.start, Position::new(1, 0));
}

/// Same-named definitions in two files give their declarations the same
/// scope name: an unbound use resolves to the declaration in its own file,
/// and a declaration only another file holds (an included one) stays a
/// candidate.
#[test]
fn scoped_fallback_prefers_the_reference_file_among_same_named_definitions() {
    let entry = |name: &str, file: &'static str, line: u32, is_decl: bool| SymEntry {
        name: name.to_owned(),
        kind: SymKind::Var,
        file: hp(file).to_owned(),
        line,
        col: 6,
        end_line: line,
        end_col: 6 + name.len() as u32,
        is_decl,
        scope: Some("dup_top".to_owned()),
        detail: None,
    };
    let mut index = SymbolIndex::default();
    index.decls.push(entry("done_0", "/x/second.sv", 8, true));
    index.decls.push(entry("done_0", "/x/first.sv", 9, true));
    index.decls.push(entry("inc_sig", "/x/defs.svh", 0, true));
    index.refs.push(entry("done_0", "/x/first.sv", 5, false));
    index.refs.push(entry("inc_sig", "/x/first.sv", 6, false));
    index.rebuild_lookup_maps();

    let positions = |reference: &SymEntry| -> Vec<(String, u32)> {
        index
            .resolve(reference)
            .into_iter()
            .map(|declaration| (declaration.file.clone(), declaration.line))
            .collect()
    };
    assert_eq!(
        positions(&index.refs[0]),
        [(hp("/x/first.sv").to_owned(), 9)]
    );
    assert_eq!(
        positions(&index.refs[1]),
        [(hp("/x/defs.svh").to_owned(), 0)]
    );
    let references: Vec<(String, u32)> = index
        .all_references(&index.decls[1])
        .into_iter()
        .map(|entry| (entry.file, entry.line))
        .collect();
    assert_eq!(
        references,
        [
            (hp("/x/first.sv").to_owned(), 9),
            (hp("/x/first.sv").to_owned(), 5)
        ]
    );
}
