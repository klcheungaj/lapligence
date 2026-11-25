//! Definitions read from the bindings of an analysis.

use super::*;

const BOUND: &str = "/x/bound.sv";

/// [`sample_parts`] tokens plus `used` at 0-based (6, 2), as a parameter
/// reference.
fn with_reference() -> (Vec<FileTokens>, TokenInfo) {
    let (_, mut token_files) = sample_parts();
    let reference = TokenInfo {
        line: 7,
        col: 3,
        end_line: 7,
        end_col: 4,
        kind: tokens::TOKEN_SLANG_PARAMETER,
        name: Some("W".to_owned()),
        file: hp("/x/top.sv").to_owned(),
    };
    token_files[0].nodes.push(reference.clone());
    (token_files, reference)
}

fn target(name: &str, line0: u32, col0: u32) -> DeclTarget {
    DeclTarget {
        name: name.to_owned(),
        kind: "parameter".to_owned(),
        file: hp(BOUND).to_owned(),
        line0,
        col0,
        via_label: false,
        via_connection: false,
    }
}

#[test]
fn a_declaration_is_its_own_definition() {
    let a = sample_analysis();
    // Every column of `add` at 1-based (4, 8), and the one after it.
    for col in 7..=10 {
        assert_eq!(
            definition_at(&a, hp("/x/top.sv"), 3, col),
            Some(Location {
                uri: Url::from_file_path(hp("/x/top.sv")).unwrap(),
                range: Range::new(Position::new(3, 7), Position::new(3, 10)),
            })
        );
    }
    for col in [6, 11] {
        assert_eq!(definition_at(&a, hp("/x/top.sv"), 3, col), None);
    }
}

#[test]
fn a_reference_is_defined_by_the_target_it_is_bound_to() {
    let (token_files, _) = with_reference();
    let mut bindings = tokens::Bindings::default();
    bindings
        .references
        .insert((hp("/x/top.sv").to_owned(), 6, 2), target("W_long", 4, 2));
    let a = sample_analysis_with_bindings(token_files, bindings);
    let location = definition_at(&a, hp("/x/top.sv"), 6, 2).expect("bound reference");
    assert_eq!(location.uri, Url::from_file_path(hp(BOUND)).unwrap());
    // The range spans the target's name.
    assert_eq!(
        location.range,
        Range::new(Position::new(4, 2), Position::new(4, 8))
    );
}

#[test]
fn a_name_several_declarations_resolve_has_all_of_them() {
    let (token_files, _) = with_reference();
    let mut bindings = tokens::Bindings::default();
    bindings.navigation.insert(
        (hp("/x/top.sv").to_owned(), 6, 2),
        vec![target("W", 4, 2), target("W", 9, 1)],
    );
    let a = sample_analysis_with_bindings(token_files, bindings);
    let starts: Vec<Position> = definitions_at(&a, hp("/x/top.sv"), 6, 2)
        .iter()
        .map(|target| target.location.range.start)
        .collect();
    assert_eq!(starts, [Position::new(4, 2), Position::new(9, 1)]);
    assert!(hover_at(&a, hp("/x/top.sv"), 6, 2).is_none());
    assert!(references_at(&a, hp("/x/top.sv"), 6, 2).is_empty());
}

/// The declaration `W` of the same file is no candidate for a name that is
/// not bound to it.
#[test]
fn an_unbound_name_is_not_resolved_by_its_spelling() {
    let (mut token_files, _) = with_reference();
    token_files[0].nodes.last_mut().unwrap().kind = tokens::TOKEN_SLANG_IDENTIFIER;
    let a = sample_analysis_with_bindings(token_files, tokens::Bindings::default());
    assert!(definitions_at(&a, hp("/x/top.sv"), 6, 2).is_empty());
    assert!(hover_at(&a, hp("/x/top.sv"), 6, 2).is_none());
    assert!(references_at(&a, hp("/x/top.sv"), 6, 2).is_empty());
    let tokens = semantic_tokens_for(&a, hp("/x/top.sv")).data;
    let last_line: u32 = tokens.iter().map(|token| token.delta_line).sum();
    assert_eq!(last_line, 4, "{tokens:?}");
}

#[test]
fn index_is_built_by_analyze() {
    let a = empty_analysis();
    assert!(a.index.decls.is_empty());
    assert!(a.index.refs.is_empty());
    assert!(a.index.decl_at(hp("/nope.sv"), 0, 0).is_none());
    assert!(a.index.decls_in_file(hp("/nope.sv")).is_empty());
}
