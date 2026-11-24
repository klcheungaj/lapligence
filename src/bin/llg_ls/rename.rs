//! rename — `textDocument/prepareRename` + `textDocument/rename` feature
//! logic.
//!
//! Pure functions over [`Analysis`] like the rest of `features.rs`: no
//! `Client`, no async, no filesystem access.  The reference set is computed by
//! exactly the same machinery find-references uses
//! ([`references_at_with_options`] with `include_declaration = true`), so
//! rename and find-references can never disagree about what an occurrence is.
//!
//! An occurrence is a name the frontend bound to the declaration, so names in
//! macro text and names that do not resolve are left unchanged.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};

use crate::features::{declaration_at, identifier_at, references_at_with_options, Analysis};
use llg::core::tokens::TokenInfo;

/// Reserved SystemVerilog keywords (curated IEEE 1800 subset covering the
/// common declaration, process, control, type, class/constraint, config and
/// specify families).  Renaming to any of these would produce edits that no
/// longer compile, so they are rejected exactly like malformed spellings.
///
/// Kept sorted for [`is_sv_keyword`]'s binary search;
/// [`tests::sv_keyword_table_stays_sorted`] pins that.
const SYSTEMVERILOG_KEYWORDS: [&str; 127] = [
    "always",
    "always_comb",
    "always_ff",
    "always_latch",
    "assert",
    "assign",
    "assume",
    "automatic",
    "before",
    "begin",
    "bind",
    "bit",
    "break",
    "byte",
    "case",
    "casex",
    "casez",
    "cell",
    "class",
    "config",
    "const",
    "constraint",
    "context",
    "continue",
    "cover",
    "deassign",
    "default",
    "design",
    "disable",
    "do",
    "edge",
    "else",
    "end",
    "endcase",
    "endclass",
    "endconfig",
    "endfunction",
    "endgenerate",
    "endinterface",
    "endmodule",
    "endpackage",
    "endprimitive",
    "endprogram",
    "endproperty",
    "endsequence",
    "endspecify",
    "endtable",
    "endtask",
    "enum",
    "expect",
    "export",
    "extends",
    "extern",
    "final",
    "for",
    "force",
    "forever",
    "function",
    "generate",
    "genvar",
    "if",
    "ifnone",
    "implements",
    "import",
    "initial",
    "inout",
    "input",
    "instance",
    "int",
    "integer",
    "interface",
    "liblist",
    "library",
    "localparam",
    "logic",
    "longint",
    "macromodule",
    "module",
    "negedge",
    "new",
    "noshowcancelled",
    "null",
    "output",
    "package",
    "parameter",
    "posedge",
    "primitive",
    "priority",
    "program",
    "property",
    "pulsestyle_ondetect",
    "pulsestyle_onevent",
    "pure",
    "rand",
    "randc",
    "randcase",
    "randproperty",
    "real",
    "realtime",
    "reg",
    "release",
    "repeat",
    "return",
    "sequence",
    "shortint",
    "shortreal",
    "showcancelled",
    "signed",
    "solve",
    "specify",
    "static",
    "struct",
    "super",
    "table",
    "task",
    "this",
    "time",
    "typedef",
    "union",
    "unique",
    "unsigned",
    "use",
    "virtual",
    "void",
    "wait",
    "while",
    "wire",
];

/// Whether `name` is a reserved SystemVerilog keyword.
pub fn is_sv_keyword(name: &str) -> bool {
    SYSTEMVERILOG_KEYWORDS.binary_search(&name).is_ok()
}

/// Whether `name` is a legal plain SystemVerilog identifier
/// (`[A-Za-z_][A-Za-z0-9_$]*`) that is not a reserved keyword.
///
/// Escaped identifiers (`\foo `, leading digits/symbols) are deliberately
/// rejected: rename never produces spellings it cannot resolve afterwards.
pub fn is_valid_identifier(name: &str) -> bool {
    is_plain_identifier(name) && !is_sv_keyword(name)
}

fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// The name under the 0-based `(line, col)` cursor when it may be renamed.
///
/// Instance-name declarations are excluded on purpose: they lead to the
/// definition they instantiate, so a client would offer to rename one name and
/// show the other.
fn rename_target<'a>(a: &'a Analysis, file: &str, line: u32, col: u32) -> Option<&'a TokenInfo> {
    declaration_at(a, file, line, col)?;
    let token = identifier_at(a, file, line, col)?;
    let instance =
        a.bindings
            .navigation
            .contains_key(&(token.file.clone(), token.line - 1, token.col - 1));
    (!instance).then_some(token)
}

/// The identifier range plus current name at the 0-based `(line, col)` cursor,
/// when a renamable symbol sits there.  `None` means "not renamable" and maps
/// to a null/error prepareRename response client-side.
pub fn prepare_rename(a: &Analysis, file: &str, line: u32, col: u32) -> Option<(Range, String)> {
    let token = rename_target(a, file, line, col)?;
    let name = token.name.clone()?;
    let start = Position::new(token.line - 1, token.col - 1);
    let end = Position::new(token.line - 1, token.col - 1 + name.chars().count() as u32);
    Some((Range::new(start, end), name))
}

/// An invalid replacement identifier in a rename request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    Keyword(String),
    InvalidIdentifier(String),
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keyword(name) => write!(
                f,
                "cannot rename to `{name}`: `{name}` is a SystemVerilog keyword"
            ),
            Self::InvalidIdentifier(name) => write!(
                f,
                "cannot rename to `{name}`: expected a plain identifier ([A-Za-z_][A-Za-z0-9_$]*)"
            ),
        }
    }
}

impl std::error::Error for RenameError {}

/// Compute the workspace edit renaming the symbol at the 0-based `(line, col)`
/// cursor to `new_name`.
///
/// Occurrences are the find-references set for that position (declaration
/// included); each edit replaces ONLY the identifier span.  Returns:
/// * `Ok(None)` when nothing renamable sits at the position or the occurrence
///   set is empty — the LSP answer is then a null edit, not an error;
/// * `Err(message)` when `new_name` is not a plain identifier or is a
///   reserved SystemVerilog keyword — surfaced as an invalid-params JSON-RPC
///   error.
pub fn rename(
    a: &Analysis,
    file: &str,
    line: u32,
    col: u32,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, RenameError> {
    if rename_target(a, file, line, col).is_none() {
        return Ok(None);
    }
    if !is_valid_identifier(new_name) {
        return Err(if is_sv_keyword(new_name) {
            RenameError::Keyword(new_name.to_owned())
        } else {
            RenameError::InvalidIdentifier(new_name.to_owned())
        });
    }
    // Same computation as textDocument/references with includeDeclaration —
    // spans are the captured name tokens, so longer identifiers sharing a
    // prefix are never touched.
    let locations = references_at_with_options(a, file, line, col, true);
    if locations.is_empty() {
        return Ok(None);
    }
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for loc in locations {
        changes.entry(loc.uri).or_default().push(TextEdit {
            range: loc.range,
            new_text: new_name.to_owned(),
        });
    }
    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.character,
            )
        });
        edits.dedup();
    }
    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_paths::host_path as hp;
    use llg::core::compile::{CompileOpts, OwnedSource};

    const REN: &str = concat!(
        "module m(input logic data, output logic data_out);\n",
        "  assign data_out = data;\n",
        "endmodule : m\n",
    );
    const TOP: &str = concat!(
        "`define DATA data\n",
        "module top;\n",
        "  logic w, o;\n",
        "  m u0(.data(w), .data_out(o));\n",
        "  wire data;\n",
        "  assign data = `DATA;\n",
        "  assign o = late;\n",
        "  logic late;\n",
        "endmodule\n",
    );

    /// Two ports whose names share a prefix in `/x/ren.sv`, and an instance of
    /// their module in `/x/top.sv`.
    fn prefix_analysis() -> Analysis {
        analysis_of(&[("/x/ren.sv", REN), ("/x/top.sv", TOP)])
    }

    fn analysis_of(sources: &[(&'static str, &str)]) -> Analysis {
        let _shadow = crate::features::TEST_PROCESS_SHADOW_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::features::analyze(&CompileOpts {
            sources: sources
                .iter()
                .map(|(file, text)| OwnedSource::compilation_unit(hp(file), *text))
                .collect(),
            editor: true,
            ..Default::default()
        })
    }

    /// The edits of renaming the name at the position in [`prefix_analysis`].
    fn edits(file: &'static str, line: u32, col: u32) -> Vec<(String, u32, u32, u32)> {
        edits_in(&prefix_analysis(), file, line, col)
    }

    /// The edits of renaming the name at the position, as file, line, columns.
    fn edits_in(
        analysis: &Analysis,
        file: &'static str,
        line: u32,
        col: u32,
    ) -> Vec<(String, u32, u32, u32)> {
        let edit = rename(analysis, hp(file), line, col, "din")
            .expect("valid name")
            .expect("occurrences exist");
        let mut edits: Vec<_> = edit
            .changes
            .expect("plain changes map")
            .into_iter()
            .flat_map(|(uri, edits)| {
                let path = uri.to_file_path().unwrap().to_string_lossy().into_owned();
                edits.into_iter().map(move |edit| {
                    assert_eq!(edit.new_text, "din");
                    let Range { start, end } = edit.range;
                    (path.clone(), start.line, start.character, end.character)
                })
            })
            .collect();
        edits.sort();
        edits
    }

    #[test]
    fn identifier_validation_accepts_plain_spellings_only() {
        for name in ["a", "_x", "Z_9", "clk$", "A_b$9"] {
            assert!(is_valid_identifier(name), "`{name}` must be accepted");
        }
        for name in [
            "",
            "9lives",
            "a-b",
            "a b",
            "clk#",
            "\\escaped ",
            "$x",
            "d.out",
            // Non-ASCII spellings are not legal plain SV identifiers
            // (escaped-identifier territory, which rename never emits).
            "clé",
            "信号",
            // Reserved keywords are lexically fine but never valid targets.
            "module",
            "always_ff",
            "wire",
            "default",
        ] {
            assert!(!is_valid_identifier(name), "`{name}` must be rejected");
        }
    }

    #[test]
    fn sv_keyword_table_stays_sorted() {
        for pair in SYSTEMVERILOG_KEYWORDS.windows(2) {
            assert!(
                pair[0] < pair[1],
                "keyword table must stay sorted: `{}` >= `{}`",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn prepare_rename_returns_range_and_placeholder_on_declaration() {
        let a = prefix_analysis();
        let (range, placeholder) =
            prepare_rename(&a, hp("/x/ren.sv"), 0, 23).expect("port decl is renamable");
        assert_eq!(placeholder, "data");
        assert_eq!(range.start, Position::new(0, 21));
        assert_eq!(range.end, Position::new(0, 25));
    }

    #[test]
    fn prepare_rename_works_from_a_reference_site() {
        let a = prefix_analysis();
        let (_, placeholder) =
            prepare_rename(&a, hp("/x/ren.sv"), 1, 22).expect("ref site is renamable");
        assert_eq!(placeholder, "data");
    }

    #[test]
    fn prepare_rename_rejects_keywords_and_unindexed_positions() {
        let a = prefix_analysis();
        // The keyword token itself.
        assert!(prepare_rename(&a, hp("/x/ren.sv"), 2, 3).is_none());
        // Anywhere off-symbol.
        assert!(prepare_rename(&a, hp("/x/ren.sv"), 20, 0).is_none());
        // An instance name, a macro name at its definition and its use, a name
        // in macro text, a use before the declaration.
        for (line, col) in [(3, 4), (0, 8), (5, 17), (0, 13), (6, 13)] {
            assert!(
                prepare_rename(&a, hp("/x/top.sv"), line, col).is_none(),
                "{line}:{col}"
            );
        }
    }

    /// `data_out` shares a prefix with the port `data` and `top` declares a
    /// `data` of its own; neither is an occurrence.
    #[test]
    fn rename_replaces_exactly_the_bound_names_across_files() {
        let expected = [
            (hp("/x/ren.sv").to_owned(), 0, 21, 25),
            (hp("/x/ren.sv").to_owned(), 1, 20, 24),
            (hp("/x/top.sv").to_owned(), 3, 8, 12),
        ];
        for (file, line, col) in [
            ("/x/ren.sv", 0, 21),
            ("/x/ren.sv", 1, 22),
            ("/x/top.sv", 3, 9),
        ] {
            assert_eq!(edits(file, line, col), expected, "{file}:{line}:{col}");
        }
    }

    #[test]
    fn rename_of_module_covers_instantiation_type_names_and_the_end_label() {
        let expected = [
            (hp("/x/ren.sv").to_owned(), 0, 7, 8),
            (hp("/x/ren.sv").to_owned(), 2, 12, 13),
            (hp("/x/top.sv").to_owned(), 3, 2, 3),
        ];
        for (file, line, col) in [
            ("/x/ren.sv", 0, 7),
            ("/x/ren.sv", 2, 12),
            ("/x/top.sv", 3, 2),
        ] {
            assert_eq!(edits(file, line, col), expected, "{file}:{line}:{col}");
        }
    }

    /// A name that a macro argument declares is renamed there and at its uses.
    #[test]
    fn rename_covers_a_name_declared_in_a_macro_argument() {
        let text = concat!(
            "`define PORT(name) input logic name\n",
            "module m(`PORT(data));\n",
            "  wire o = data;\n",
            "endmodule\n",
        );
        let a = analysis_of(&[("/x/m.sv", text)]);
        let expected = [
            (hp("/x/m.sv").to_owned(), 1, 15, 19),
            (hp("/x/m.sv").to_owned(), 2, 11, 15),
        ];
        for (line, col) in [(1, 15), (2, 12)] {
            assert_eq!(edits_in(&a, "/x/m.sv", line, col), expected, "{line}:{col}");
        }
    }

    #[test]
    fn rename_rejects_invalid_new_names() {
        let a = prefix_analysis();
        for bad in ["1abc", "a-b", "", "clk#"] {
            let err = rename(&a, hp("/x/ren.sv"), 0, 21, bad).expect_err(bad);
            assert!(
                err.to_string().contains(bad),
                "error should name the offender: {err}"
            );
        }
    }

    #[test]
    fn rename_rejects_keywords_with_a_clear_error() {
        let a = prefix_analysis();
        for keyword in ["module", "always_comb", "wire", "default"] {
            let err = rename(&a, hp("/x/ren.sv"), 0, 21, keyword).expect_err(keyword);
            assert!(
                err.to_string().contains("is a SystemVerilog keyword"),
                "error should say why: {err}"
            );
            assert!(
                err.to_string().contains(keyword),
                "error should name the offender: {err}"
            );
        }
    }

    #[test]
    fn rename_without_a_renamable_target_is_null_not_an_error() {
        let a = prefix_analysis();
        // Keyword position: Ok(None).
        assert!(matches!(
            rename(&a, hp("/x/ren.sv"), 2, 3, "whatever"),
            Ok(None)
        ));
    }
}
