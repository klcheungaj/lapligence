//! rename — `textDocument/prepareRename` + `textDocument/rename` feature
//! logic.
//!
//! Pure functions over [`Analysis`] like the rest of `features.rs`: no
//! `Client`, no async, no filesystem access.  The reference set is computed by
//! exactly the same machinery find-references uses
//! ([`references_at_with_options`] with `include_declaration = true`), so
//! rename and find-references can never disagree about what an occurrence is.
//!
//! Known v1 limitation (inherited from the symbol index): resolution away
//! from binding-precise positions is name-based + scope-aware, so a reference
//! set can conflate same-named declarations across scopes (inner-scope
//! shadowing).  Rename inherits that approximation verbatim from
//! find-references.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};

use crate::features::{references_at_with_options, Analysis, SymEntry};
use llg::core::model::SymKind;

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

/// The indexed symbol under the 0-based `(line, col)` cursor when it may be
/// renamed.
///
/// Instance-name declarations are excluded on purpose: the index resolves an
/// instance to its module *definition* (goto-definition semantics), so a
/// rename there would retarget every occurrence of the type family instead of
/// the instance name.  Keyword positions never hit the index and are therefore
/// not renamable either.
fn rename_target<'a>(a: &'a Analysis, file: &str, line: u32, col: u32) -> Option<&'a SymEntry> {
    let e = a.index.entry_at(file, line, col)?;
    if e.is_decl && e.kind == SymKind::Instance {
        return None;
    }
    Some(e)
}

/// The identifier range plus current name at the 0-based `(line, col)` cursor,
/// when a renamable symbol sits there.  `None` means "not renamable" and maps
/// to a null/error prepareRename response client-side.
pub fn prepare_rename(a: &Analysis, file: &str, line: u32, col: u32) -> Option<(Range, String)> {
    let e = rename_target(a, file, line, col)?;
    let len = e.name.chars().count() as u32;
    let range = Range::new(
        Position::new(e.line, e.col),
        Position::new(e.line, e.col + len),
    );
    Some((range, e.name.clone()))
}

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
) -> Result<Option<WorkspaceEdit>, String> {
    if rename_target(a, file, line, col).is_none() {
        return Ok(None);
    }
    if !is_valid_identifier(new_name) {
        let reason = if is_sv_keyword(new_name) {
            format!("`{new_name}` is a SystemVerilog keyword")
        } else {
            "expected a plain identifier ([A-Za-z_][A-Za-z0-9_$]*)".to_owned()
        };
        return Err(format!("cannot rename to `{new_name}`: {reason}"));
    }
    // Same computation as textDocument/references with includeDeclaration —
    // spans come straight from the indexed entries (start column + name
    // width), so longer identifiers sharing a prefix are never touched.
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
    use llg::core::model::{DesignModel, Direction, InstanceModel, ModuleDef, PortModel, TypeInfo};
    use llg::core::tokens::FileTokens;
    use llg::ffi::surelog::VObjectInfo;
    use llg::ffi::vpi;

    /// Hand-built single-file analysis mirroring the real pipeline's token
    /// shapes for:
    ///
    /// ```text
    /// /x/ren.sv
    /// 1: module m(input logic data, output logic data_out);
    /// 2:   assign data_out = data;
    /// 3: endmodule
    /// ```
    ///
    /// `/x/top.sv` instantiates `m` so the module type-name ref exercises
    /// cross-file edits:
    ///
    /// ```text
    /// /x/top.sv
    /// 4: module top;
    /// 5:   m u0();
    /// 6: endmodule
    /// ```
    fn prefix_analysis() -> Analysis {
        let node = |line: u32, col: u32, t: i32, name: &str| VObjectInfo {
            line,
            col,
            end_line: line,
            end_col: col + name.len() as u32,
            vpi_type: t,
            name: Some(name.to_owned()),
            file: String::new(), // filled per file below
        };
        let mk = |nodes: Vec<(u32, u32, i32, &str)>, path: &str| -> FileTokens {
            FileTokens {
                path: path.to_owned(),
                nodes: nodes
                    .into_iter()
                    .map(|(l, c, t, n)| {
                        let mut v = node(l, c, t, n);
                        v.file = path.to_owned();
                        v
                    })
                    .collect(),
            }
        };

        let ren_file = mk(
            vec![
                (1, 8, vpi::vpiModule, "m"),
                (1, 22, vpi::TOKEN_PORT_INPUT, "data"),
                (1, 22, vpi::vpiNet, "data"),
                (1, 22, vpi::vpiPort, "data"),
                (1, 41, vpi::TOKEN_PORT_OUTPUT, "data_out"),
                (1, 41, vpi::vpiNet, "data_out"),
                (1, 41, vpi::vpiPort, "data_out"),
                (2, 10, vpi::vpiRefObj, "data_out"),
                (2, 21, vpi::vpiRefObj, "data"),
                // A parse-tree keyword token: named but never classified into
                // the index, so it must not become a rename target.
                (3, 1, 900_001, "module"),
            ],
            "/x/ren.sv",
        );
        let top_file = mk(
            vec![
                (4, 8, vpi::vpiModule, "top"),
                (5, 3, vpi::uhdmclass_defn, "m"),
                (5, 5, vpi::uhdmlogic_var, "u0"),
            ],
            "/x/top.sv",
        );

        let port = |name: &str, dir: Direction| PortModel {
            name: name.to_owned(),
            direction: dir,
            ty: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(1),
                signed: false,
                type_name: None,
            },
        };
        let model = DesignModel {
            design_name: "top".to_owned(),
            top_instances: vec![InstanceModel {
                name: "tb".to_owned(),
                def_name: "m".to_owned(),
                full_name: "tb".to_owned(),
                file: Some("/x/ren.sv".to_owned()),
                line: 9,
                col: 1,
                ports: vec![
                    port("data", Direction::Input),
                    port("data_out", Direction::Output),
                ],
                signals: Vec::new(),
                params: Vec::new(),
                gen_scopes: Vec::new(),
                funcs: Vec::new(),
                children: Vec::new(),
            }],
            modules: vec![
                ModuleDef {
                    name: "m".to_owned(),
                    file: Some("/x/ren.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 3,
                    end_col: 10,
                },
                ModuleDef {
                    name: "top".to_owned(),
                    file: Some("/x/top.sv".to_owned()),
                    line: 4,
                    col: 8,
                    end_line: 6,
                    end_col: 10,
                },
            ],
            packages: Vec::new(),
            classes: Vec::new(),
        };
        Analysis::new(Vec::new(), model, vec![ren_file, top_file], Vec::new())
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
            prepare_rename(&a, "/x/ren.sv", 0, 23).expect("port decl is renamable");
        assert_eq!(placeholder, "data");
        assert_eq!(range.start, Position::new(0, 21));
        assert_eq!(range.end, Position::new(0, 25));
    }

    #[test]
    fn prepare_rename_works_from_a_reference_site() {
        let a = prefix_analysis();
        let (_, placeholder) =
            prepare_rename(&a, "/x/ren.sv", 1, 22).expect("ref site is renamable");
        assert_eq!(placeholder, "data");
    }

    #[test]
    fn prepare_rename_rejects_keywords_and_unindexed_positions() {
        let a = prefix_analysis();
        // The keyword token itself.
        assert!(prepare_rename(&a, "/x/ren.sv", 2, 3).is_none());
        // Anywhere off-symbol.
        assert!(prepare_rename(&a, "/x/ren.sv", 20, 0).is_none());
    }

    #[test]
    fn rename_replaces_exactly_the_identifier_spans_across_files() {
        let a = prefix_analysis();
        // Rename the PORT `data` from its declaration.
        let edit = rename(&a, "/x/ren.sv", 0, 21, "din")
            .expect("valid name")
            .expect("occurrences exist");
        let changes = edit.changes.expect("plain changes map");

        let ren_edits = changes
            .get(&Url::from_file_path("/x/ren.sv").unwrap())
            .expect("edits in /x/ren.sv");
        assert_eq!(ren_edits.len(), 2, "decl + one use in ren.sv");
        assert_eq!(
            ren_edits[0].range,
            Range::new(Position::new(0, 21), Position::new(0, 25))
        );
        assert_eq!(
            ren_edits[1].range,
            Range::new(Position::new(1, 20), Position::new(1, 24))
        );
        for edit in ren_edits {
            assert_eq!(edit.new_text, "din");
        }

        // Prefix-collision safety: `data_out` keeps its own occurrences.
        let top_edits = changes.get(&Url::from_file_path("/x/top.sv").unwrap());
        assert!(top_edits.is_none(), "renaming `data` must not touch top.sv");
    }

    #[test]
    fn rename_is_prefix_collision_safe_in_the_same_file() {
        let a = prefix_analysis();
        // Same request from the USE site of `data` (line 1): the `data_out`
        // use on the very same line must stay untouched.
        let edit = rename(&a, "/x/ren.sv", 1, 22, "din")
            .expect("valid name")
            .expect("occurrences exist");
        let changes = edit.changes.unwrap();
        let edits = changes
            .get(&Url::from_file_path("/x/ren.sv").unwrap())
            .unwrap();
        for edit in edits {
            let start = edit.range.start;
            assert!(
                !(start.line == 1 && start.character >= 9 && start.character < 17),
                "the `data_out` use at 1:9 must not be edited: {edit:?}"
            );
        }
        assert_eq!(edits.len(), 2);
    }

    #[test]
    fn rename_of_module_covers_instantiation_type_names() {
        let a = prefix_analysis();
        let edit = rename(&a, "/x/ren.sv", 0, 7, "m2")
            .expect("valid name")
            .expect("module occurrences exist");
        let changes = edit.changes.unwrap();

        let ren_edits = changes
            .get(&Url::from_file_path("/x/ren.sv").unwrap())
            .expect("module decl edit");
        assert_eq!(ren_edits.len(), 1);
        assert_eq!(
            ren_edits[0].range,
            Range::new(Position::new(0, 7), Position::new(0, 8))
        );

        let top_edits = changes
            .get(&Url::from_file_path("/x/top.sv").unwrap())
            .expect("type-name ref edit");
        assert_eq!(top_edits.len(), 1);
        assert_eq!(
            top_edits[0].range,
            Range::new(Position::new(4, 2), Position::new(4, 3))
        );
    }

    #[test]
    fn rename_rejects_invalid_new_names() {
        let a = prefix_analysis();
        for bad in ["1abc", "a-b", "", "clk#"] {
            let err = rename(&a, "/x/ren.sv", 0, 21, bad).expect_err(bad);
            assert!(err.contains(bad), "error should name the offender: {err}");
        }
    }

    #[test]
    fn rename_rejects_keywords_with_a_clear_error() {
        let a = prefix_analysis();
        for keyword in ["module", "always_comb", "wire", "default"] {
            let err = rename(&a, "/x/ren.sv", 0, 21, keyword).expect_err(keyword);
            assert!(
                err.contains("is a SystemVerilog keyword"),
                "error should say why: {err}"
            );
            assert!(
                err.contains(keyword),
                "error should name the offender: {err}"
            );
        }
    }

    #[test]
    fn rename_without_a_renamable_target_is_null_not_an_error() {
        let a = prefix_analysis();
        // Keyword position: Ok(None).
        assert!(matches!(
            rename(&a, "/x/ren.sv", 2, 3, "whatever"),
            Ok(None)
        ));
    }
}
