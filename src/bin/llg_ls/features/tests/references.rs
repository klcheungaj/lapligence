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
fn references_options_include_or_exclude_the_declaration() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    let (line, col) = cursor(A_SV, "module |m(");
    let declaration = Location {
        uri: Url::from_file_path(hp("/x/a.sv")).unwrap(),
        range: Range::new(Position::new(line, col), Position::new(line, col + 1)),
    };
    let (use_line, use_col) = cursor(B_SV, "  |m u0(");
    let reference = Location {
        uri: Url::from_file_path(hp("/x/b.sv")).unwrap(),
        range: Range::new(
            Position::new(use_line, use_col),
            Position::new(use_line, use_col + 1),
        ),
    };
    for (file, line, col) in [("/x/a.sv", line, col), ("/x/b.sv", use_line, use_col)] {
        let locations = |include| references_at_with_options(&a, hp(file), line, col, include);
        assert_eq!(locations(true), [declaration.clone(), reference.clone()]);
        assert_eq!(locations(true), references_at(&a, hp(file), line, col));
        assert_eq!(locations(false), std::slice::from_ref(&reference));
    }
}

/// Definitions of one name in two files keep their references apart.
#[test]
fn references_stay_in_the_definition_they_are_bound_to() {
    let _guards = analysis_guards();
    const FIRST: &str = "module dup;\n  logic done;\n  assign done = 1'b0;\nendmodule\n";
    const SECOND: &str =
        "module dup;\n  int pad;\n  logic done;\n  assign done = 1'b1;\nendmodule\n";
    let a = analyze(&CompileOpts {
        library_units: true,
        sources: vec![
            compile::OwnedSource::compilation_unit(hp("/x/first.sv"), FIRST),
            compile::OwnedSource::compilation_unit(hp("/x/second.sv"), SECOND),
        ],
        editor: true,
        ..Default::default()
    });
    for (file, text) in [("/x/first.sv", FIRST), ("/x/second.sv", SECOND)] {
        let (line, col) = cursor(text, "assign |done");
        let references = references_at(&a, hp(file), line, col);
        assert_eq!(references.len(), 2, "{file}: {references:?}");
        assert!(references
            .iter()
            .all(|location| uri_file_path(&location.uri) == std::path::Path::new(hp(file))));
    }
}
