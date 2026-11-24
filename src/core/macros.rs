//! macros — the text macros of one compilation, as Slang's preprocessor
//! resolved them.
//!
//! [`MacroTable`] holds every `` `define `` that took effect and every written
//! macro name with the definitions it stands for. Macro hover and
//! go-to-definition on a macro name read this one table. The capture resolves
//! each name in the state of the preprocessor at that position
//! (`LlgSlangMacroReference` in `src/wrapper/slang_c_api.h`): headers count
//! where they are included, inactive conditional text does not count,
//! redefinition and `` `undef `` take effect from their position on, and a
//! use written in a `` `define `` body is resolved where the macro is
//! expanded. A `[compile] defines` entry takes precedence over a source
//! `` `define `` of the same name until the source undefines it.

use std::collections::HashMap;

use crate::core::tokens::{DeclTarget, SnapshotSources, TARGET_MACRO};
use crate::ffi::slang::{LexicalKind, LexicalRole, MacroReferenceKind, Snapshot, SourceRange};

/// One macro definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    /// Macro name (without the leading backtick).
    pub name: String,
    /// Formal arguments of a function-like macro as written (`None` for a
    /// simple macro). Empty parentheses yield `Some(Vec::new())`.
    pub args: Option<Vec<String>>,
    /// Replacement text with each white-space run and line continuation as
    /// one space. Empty for a bare `` `define NAME ``.
    pub body: String,
    /// `None` for a `[compile] defines` entry.
    pub source: Option<MacroSource>,
}

/// Where a `` `define `` names its macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSource {
    pub file: String,
    /// 1-based line and UTF-16 column of the name.
    pub line: u32,
    pub col: u32,
}

/// How a macro name is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroSiteKind {
    /// The name in a `` `define ``.
    Definition,
    /// `` `NAME ``.
    Use,
    /// The operand of a conditional directive or `` `undef ``.
    Operand,
}

/// One written macro name with its resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSite {
    /// 0-based line.
    pub line0: u32,
    /// 0-based UTF-16 columns of the name (with the grave accent of a use)
    /// and one past it.
    pub col_start0: u32,
    pub col_end0: u32,
    /// Macro name (without the leading backtick).
    pub name: String,
    pub kind: MacroSiteKind,
    /// Indices of the source definitions: none for a macro that is not
    /// defined there or is predefined, several when the syntax trees that
    /// include the file, or the expansions of the body that holds the use,
    /// resolve the name differently.
    definitions: Vec<usize>,
    /// A `[compile] defines` entry or the language defines the macro.
    pub predefined: bool,
}

/// The macros of one compilation.
#[derive(Debug, Default)]
pub struct MacroTable {
    /// The source definitions, then the `[compile] defines` entries.
    definitions: Vec<MacroDefinition>,
    /// Index of the first `[compile] defines` entry.
    config_start: usize,
    /// Written names by file, sorted by position.
    files: HashMap<String, Vec<MacroSite>>,
    /// Display note naming the configuration source for undefined-macro
    /// messages (e.g. the effective `llg.toml` path); optional.
    config_note: Option<String>,
}

impl MacroTable {
    /// The macros of `snapshot`, compiled with `defines`.
    pub(crate) fn from_slang(
        snapshot: &Snapshot,
        sources: &SnapshotSources<'_>,
        defines: &[String],
    ) -> Self {
        let mut definitions = Vec::with_capacity(snapshot.macro_definitions.len());
        let mut files: HashMap<String, Vec<MacroSite>> = HashMap::new();
        let mut add = |range: SourceRange, kind, definition: Option<usize>, predefined| {
            let Some(source) = sources.file(range.file_id) else {
                return;
            };
            let (line, col) = source.positions.position(range.start);
            let (_, end_col) = source.positions.position(range.end);
            let sites = match files.get_mut(source.name) {
                Some(sites) => sites,
                None => files.entry(source.name.to_owned()).or_default(),
            };
            let text = source.positions.text(range);
            let text = match kind {
                MacroSiteKind::Use => text.strip_prefix('`').unwrap_or(text),
                _ => text,
            };
            sites.push(MacroSite {
                line0: line - 1,
                col_start0: col - 1,
                col_end0: end_col.saturating_sub(1).max(col),
                name: text
                    .strip_prefix('\\')
                    .unwrap_or(text)
                    .trim_end()
                    .to_owned(),
                kind,
                definitions: definition.into_iter().collect(),
                predefined,
            });
        };
        for definition in &snapshot.macro_definitions {
            let Some(source) = sources.file(definition.name.file_id) else {
                // Keeps the indices of the macro references.
                definitions.push(MacroDefinition::config("", ""));
                continue;
            };
            let name = source.positions.text(definition.name);
            let (line, col) = source.positions.position(definition.name.start);
            let formals = definition.function_like.then(|| {
                formal_names(source.positions.text(SourceRange {
                    start: definition.name.end,
                    end: definition.body.start,
                    ..definition.range
                }))
            });
            add(
                definition.name,
                MacroSiteKind::Definition,
                Some(definitions.len()),
                false,
            );
            definitions.push(MacroDefinition {
                name: name
                    .strip_prefix('\\')
                    .unwrap_or(name)
                    .trim_end()
                    .to_owned(),
                args: formals,
                body: one_line(source.positions.text(definition.body)),
                source: Some(MacroSource {
                    file: source.name.to_owned(),
                    line,
                    col,
                }),
            });
        }
        for reference in &snapshot.macro_references {
            let kind = match reference.kind {
                MacroReferenceKind::Use => MacroSiteKind::Use,
                MacroReferenceKind::Test | MacroReferenceKind::Undef => MacroSiteKind::Operand,
            };
            add(
                reference.range,
                kind,
                reference.definition,
                reference.definition.is_none(),
            );
        }
        // Uses of macros that are not defined have no reference record.
        for token in &snapshot.lexical_tokens {
            if let (LexicalKind::Macro, LexicalRole::Reference, false, false, Some(range)) = (
                token.kind,
                token.role,
                token.is_skipped,
                token.is_macro_expansion && !token.is_macro_argument,
                token.range,
            ) {
                add(range, MacroSiteKind::Use, None, false);
            }
        }
        for sites in files.values_mut() {
            sites.sort_by_key(|site| (site.line0, site.col_start0));
            sites.dedup_by(|next, kept| {
                let same = (next.line0, next.col_start0) == (kept.line0, kept.col_start0);
                if same {
                    kept.predefined |= next.predefined;
                    for definition in &next.definitions {
                        if !kept.definitions.contains(definition) {
                            kept.definitions.push(*definition);
                        }
                    }
                }
                same
            });
        }
        let config_start = definitions.len();
        definitions.extend(parse_config_defines(defines));
        Self {
            definitions,
            config_start,
            files,
            config_note: None,
        }
    }

    /// The macro name whose span covers the 0-based position.
    pub fn site_at(&self, file: &str, line0: u32, col0: u32) -> Option<&MacroSite> {
        let sites = self.files.get(file)?;
        let after = sites.partition_point(|site| (site.line0, site.col_start0) <= (line0, col0));
        sites[..after]
            .last()
            .filter(|site| site.line0 == line0 && col0 < site.col_end0)
    }

    /// The definitions a written name stands for. A predefined macro of the
    /// language has none.
    pub fn definitions<'a>(
        &'a self,
        site: &'a MacroSite,
    ) -> impl Iterator<Item = &'a MacroDefinition> + 'a {
        let config = &self.definitions[self.config_start..];
        site.definitions
            .iter()
            .map(|index| &self.definitions[*index])
            .chain(
                config
                    .iter()
                    .filter(|_| site.predefined)
                    // The last entry of a name wins, as in the compile request.
                    .rfind(|definition| definition.name == site.name),
            )
    }

    /// The `` `define `` names that a written name leads to.
    pub fn targets<'a>(&'a self, site: &'a MacroSite) -> impl Iterator<Item = DeclTarget> + 'a {
        self.definitions(site).filter_map(|definition| {
            let source = definition.source.as_ref()?;
            DeclTarget::from_decl_coords(
                &definition.name,
                TARGET_MACRO,
                &source.file,
                source.line,
                source.col,
            )
        })
    }

    /// Every written name of `file`, sorted by position.
    pub fn sites(&self, file: &str) -> &[MacroSite] {
        self.files.get(file).map_or(&[], Vec::as_slice)
    }

    /// Display note used by undefined-macro messages (the config file path
    /// when the caller supplied one).
    pub fn config_note(&self) -> Option<&str> {
        self.config_note.as_deref()
    }

    /// Attach the config display note (called once per analysis commit).
    pub fn set_config_note(&mut self, note: impl Into<String>) {
        self.config_note = Some(note.into());
    }
}

impl MacroDefinition {
    fn config(name: &str, body: &str) -> Self {
        Self {
            name: name.to_owned(),
            args: None,
            body: body.to_owned(),
            source: None,
        }
    }
}

/// The formal arguments in the text between a macro's name and its body, as
/// written: the top-level commas of the parenthesized list separate them.
fn formal_names(text: &str) -> Vec<String> {
    let list = text.trim();
    let list = list
        .strip_prefix('(')
        .and_then(|list| list.rfind(')').map(|end| &list[..end]))
        .unwrap_or(list);
    let mut formals = Vec::new();
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut start = 0;
    for (offset, c) in list.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' | '[' | '{' if !quoted => depth += 1,
            ')' | ']' | '}' if !quoted => depth = depth.saturating_sub(1),
            ',' if !quoted && depth == 0 => {
                formals.push(one_line(&list[start..offset]));
                start = offset + 1;
            }
            _ => {}
        }
    }
    let last = one_line(&list[start..]);
    if !last.is_empty() || !formals.is_empty() {
        formals.push(last);
    }
    formals
}

/// `text` with each white-space run and line continuation as one space.
fn one_line(text: &str) -> String {
    let joined = text.replace("\\\r\n", " ").replace("\\\n", " ");
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_identifier_start(byte: Option<u8>) -> bool {
    matches!(byte, Some(b) if b.is_ascii_alphabetic() || b == b'_')
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Parse validated compile definitions (`NAME`, `NAME=VALUE`) into
/// config-origin macro definitions. Entries that do not reduce to a plain
/// identifier name are dropped defensively (the config layer already
/// validates them).
fn parse_config_defines(args: &[String]) -> Vec<MacroDefinition> {
    let mut out = Vec::new();
    for arg in args {
        let (name, value) = match arg.split_once('=') {
            Some((name, value)) => (name, value),
            None => (arg.as_str(), ""),
        };
        if !is_identifier_start(name.as_bytes().first().copied())
            || !name.bytes().skip(1).all(is_identifier_continue)
        {
            continue;
        }
        out.push(MacroDefinition::config(name, value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tokens;
    use crate::ffi::slang;

    /// The macros of one source unit, `t.sv`, compiled with `defines`.
    fn table(defines: &[&str], text: &str) -> MacroTable {
        unit_tables(defines, &[("t.sv", text)])
    }

    /// The macros of source units compiled with `defines`.
    fn unit_tables(defines: &[&str], units: &[(&str, &str)]) -> MacroTable {
        let defines: Vec<String> = defines.iter().map(|define| (*define).to_owned()).collect();
        let sources: Vec<_> = units
            .iter()
            .map(|(name, text)| slang::Source::compilation_unit(name, text))
            .collect();
        let options = slang::CompileOptions {
            defines: defines
                .iter()
                .map(|define| {
                    let (name, value) = match define.split_once('=') {
                        Some((name, value)) => (name, Some(value)),
                        None => (define.as_str(), None),
                    };
                    slang::Define {
                        name: name.to_owned(),
                        value: value.map(str::to_owned),
                    }
                })
                .collect(),
            library_units: true,
            editor: true,
            ..Default::default()
        };
        let snapshot = slang::compile(&slang::CompileRequest {
            sources: &sources,
            library_sources: &[],
            options: &options,
        })
        .expect("compile");
        tokens::project_slang(&snapshot, None, &defines).macros
    }

    /// The written name at the `|` of `marked`, which without it occurs once
    /// in `text`.
    fn site<'a>(table: &'a MacroTable, text: &str, marked: &str) -> Option<&'a MacroSite> {
        let plain = marked.replacen('|', "", 1);
        assert_eq!(text.matches(&plain).count(), 1, "{plain:?}");
        let at = text.find(&plain).expect("text") + marked.find('|').expect("mark");
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rfind('\n').map_or(at, |start| at - start - 1) as u32;
        table.site_at("t.sv", line, col)
    }

    /// The bodies of the definitions that the name at `marked` stands for.
    fn bodies(table: &MacroTable, text: &str, marked: &str) -> Vec<String> {
        site(table, text, marked).map_or_else(Vec::new, |site| {
            table
                .definitions(site)
                .map(|definition| definition.body.clone())
                .collect()
        })
    }

    #[test]
    fn the_definition_in_effect_at_a_use_wins() {
        let text = "\
module m;
  int early = `W;
`define W 4
  int first = `W;
`define W 8
  int second = `W;
`undef W
  int third = `W;
`define W 16
`undefineall
  int fourth = `W;
endmodule
";
        let macros = table(&[], text);
        for (marked, expected) in [
            ("early = `|W", &[] as &[&str]),
            ("first = `|W", &["4"]),
            ("second = `|W", &["8"]),
            ("`undef |W", &["8"]),
            ("third = `|W", &[]),
            ("fourth = `|W", &[]),
            ("`define |W 8", &["8"]),
        ] {
            assert_eq!(bodies(&macros, text, marked), expected, "{marked}");
        }
        let early = site(&macros, text, "early = `|W").expect("undefined use");
        assert_eq!(early.kind, MacroSiteKind::Use);
        assert!(early.definitions.is_empty() && !early.predefined);
    }

    #[test]
    fn a_config_define_wins_over_a_source_definition() {
        let text = "\
module m;
  int first = `W;
  int other = `FLAG;
`define W 8
  int second = `W;
endmodule
";
        let macros = table(&["W=4", "FLAG"], text);
        for marked in ["first = `|W", "second = `|W"] {
            let used = site(&macros, text, marked).expect("use");
            assert!(used.predefined, "{marked}");
            assert_eq!(bodies(&macros, text, marked), ["4"], "{marked}");
            assert_eq!(macros.targets(used).count(), 0, "{marked}");
        }
        assert_eq!(bodies(&macros, text, "other = `|FLAG"), [""]);
        // The frontend ignores the directive, so it defines nothing.
        assert!(site(&macros, text, "`define |W").is_none());
    }

    #[test]
    fn a_macro_the_language_defines_has_no_definition() {
        let text = "module m;\n  int line = `__LINE__;\nendmodule\n";
        let macros = table(&[], text);
        let site = site(&macros, text, "`|__LINE__").expect("use");
        assert!(site.predefined);
        assert_eq!(macros.definitions(site).count(), 0);
    }

    #[test]
    fn inactive_text_comments_and_strings_hold_no_names() {
        let text = "\
// `IN_COMMENT
module m;
`define USED 1
`ifdef NOT_DEFINED
`define IN_INACTIVE 2
  int inactive = `USED;
`elsif USED
  string text = \"`IN_STRING\";
`else
`undef USED
`endif
  int active = `USED;
endmodule
";
        let macros = table(&[], text);
        for marked in [
            "`|IN_COMMENT",
            "`define |IN_INACTIVE",
            "inactive = `|USED",
            "`|IN_STRING",
            "`undef |USED",
        ] {
            assert!(site(&macros, text, marked).is_none(), "{marked}");
        }
        assert_eq!(bodies(&macros, text, "active = `|USED;\nendmodule"), ["1"]);
        assert_eq!(bodies(&macros, text, "`elsif |USED"), ["1"]);
        let operand = site(&macros, text, "`elsif |USED").expect("operand");
        assert_eq!(operand.kind, MacroSiteKind::Operand);
        // An operand that is not defined there is no macro.
        assert!(site(&macros, text, "`ifdef |NOT_DEFINED").is_none());
    }

    /// The branch the frontend takes is the one that defines, uses and names
    /// macros, at every depth.
    #[test]
    fn only_the_taken_branches_of_nested_conditions_hold_names() {
        let text = "\
`define OUTER 1
`ifdef OUTER
`ifndef INNER
`define FROM_IFNDEF 2
`else
`define FROM_INNER 3
`endif
`else
`ifdef OUTER
`define FROM_DEAD 4
`endif
`endif
`ifdef ABSENT
`define FROM_ABSENT 5
`elsif ALSO_ABSENT
`define FROM_ALSO_ABSENT 6
`else
`define FROM_ELSE 7
`endif
`ifdef (OUTER)
`define FROM_PARENTHESES 8
`endif
module m;
  int a = `FROM_IFNDEF, b = `FROM_INNER, c = `FROM_DEAD, d = `FROM_ABSENT;
  int e = `FROM_ALSO_ABSENT, f = `FROM_ELSE, g = `FROM_PARENTHESES;
endmodule
";
        let macros = table(&[], text);
        for (marked, expected) in [
            ("a = `|FROM_IFNDEF", &["2"] as &[&str]),
            ("b = `|FROM_INNER", &[]),
            ("c = `|FROM_DEAD", &[]),
            ("d = `|FROM_ABSENT", &[]),
            ("e = `|FROM_ALSO_ABSENT", &[]),
            ("f = `|FROM_ELSE", &["7"]),
            ("g = `|FROM_PARENTHESES", &["8"]),
            ("`ifdef (|OUTER)", &["1"]),
            // The frontend reads a condition nested in a branch it skips.
            ("`else\n`ifdef |OUTER", &["1"]),
        ] {
            assert_eq!(bodies(&macros, text, marked), expected, "{marked}");
        }
        for marked in [
            "`define |FROM_INNER",
            "`define |FROM_DEAD",
            "`define |FROM_ABSENT",
            "`define |FROM_ALSO_ABSENT",
        ] {
            assert!(site(&macros, text, marked).is_none(), "{marked}");
        }
    }

    /// A condition without an operand is an error, and the names are those of
    /// the text the frontend kept: it reads the directives up to the next token
    /// in place of the operand and skips the rest of the file.
    #[test]
    fn a_condition_without_an_operand_leaves_the_names_the_frontend_kept() {
        let text = "\
`define BEFORE 1
module m;
  int a = `BEFORE;
endmodule
`ifdef
`define AFTER 2
module n;
  int b = `BEFORE, c = `AFTER;
endmodule
";
        let macros = table(&[], text);
        assert_eq!(bodies(&macros, text, "a = `|BEFORE"), ["1"]);
        assert_eq!(bodies(&macros, text, "`define |AFTER"), ["2"]);
        for marked in ["b = `|BEFORE", "c = `|AFTER"] {
            assert!(site(&macros, text, marked).is_none(), "{marked}");
        }
    }

    /// `` `undefineall `` defines the configured macros again.
    #[test]
    fn undef_and_undefineall_on_a_config_define() {
        let text = "\
module m;
  int first = `W;
`undef W
  int second = `W;
`undefineall
  int third = `W;
endmodule
";
        let macros = table(&["W=4"], text);
        for (marked, expected) in [
            ("first = `|W", &["4"] as &[&str]),
            ("`undef |W", &["4"]),
            ("second = `|W", &[]),
            ("third = `|W", &["4"]),
        ] {
            assert_eq!(bodies(&macros, text, marked), expected, "{marked}");
        }
    }

    #[test]
    fn directives_are_no_macro_names() {
        let text = "\
`timescale 1ns/1ps
`default_nettype none
`celldefine
module m;
endmodule
`endcelldefine
`resetall
`define OK 1
";
        let macros = table(&[], text);
        assert_eq!(macros.sites("t.sv").len(), 1);
        assert_eq!(bodies(&macros, text, "`define |OK"), ["1"]);
    }

    #[test]
    fn a_unit_does_not_see_the_macros_of_another() {
        let first = "`define SHARED local\nmodule a; string s = \"`SHARED\"; endmodule\n";
        let second = "module b; int x = `SHARED, y = `CONFIGURED; endmodule\n";
        let macros = unit_tables(&["CONFIGURED=9"], &[("a.sv", first), ("b.sv", second)]);
        let at = |marked: &str| {
            let column = second.find(marked).expect("text") as u32;
            macros.site_at("b.sv", 0, column).expect("use")
        };
        let shared = at("`SHARED");
        assert!(shared.definitions.is_empty() && !shared.predefined);
        let configured = at("`CONFIGURED");
        assert_eq!(
            macros
                .definitions(configured)
                .next()
                .map(|d| d.body.as_str()),
            Some("9")
        );
        assert_eq!(macros.sites("a.sv").len(), 1);
    }

    #[test]
    fn uses_in_actual_arguments_are_names_of_their_own() {
        let text = "\
`define W 4
`define ADD(a, b) ((a) + (b))
module m;
  int x = `ADD(`W, `ADD(1, `GONE));
endmodule
";
        let macros = table(&[], text);
        assert_eq!(bodies(&macros, text, "x = `|ADD"), ["((a) + (b))"]);
        assert_eq!(bodies(&macros, text, "ADD(`|W"), ["4"]);
        assert_eq!(bodies(&macros, text, "W, `|ADD(1"), ["((a) + (b))"]);
        let gone = site(&macros, text, "`|GONE").expect("undefined use");
        assert!(gone.definitions.is_empty() && !gone.predefined);
    }

    #[test]
    fn a_definition_keeps_its_formals_and_its_body_on_one_line() {
        let text = "\
`define SUM(a, b = `W, c = {1, 2}) \\
  ((a) + \\
   (b))
`define EMPTY() 7
`define BARE
`define SIMPLE (1)
module m;
  int x = `SUM(1);
endmodule
";
        let macros = table(&[], text);
        let definition = |marked: &str| {
            let site = site(&macros, text, marked).expect("definition");
            assert_eq!(site.kind, MacroSiteKind::Definition, "{marked}");
            macros.definitions(site).next().expect("definition")
        };
        let sum = definition("`define |SUM");
        assert_eq!(
            sum.args.as_deref(),
            Some(&["a".to_owned(), "b = `W".to_owned(), "c = {1, 2}".to_owned()][..])
        );
        assert_eq!(sum.body, "((a) + (b))");
        assert_eq!(definition("`define |EMPTY").args, Some(Vec::new()));
        let bare = definition("`define |BARE");
        assert_eq!((bare.args.as_deref(), bare.body.as_str()), (None, ""));
        let simple = definition("`define |SIMPLE");
        assert_eq!(
            (simple.args.as_deref(), simple.body.as_str()),
            (None, "(1)")
        );
        assert_eq!(bodies(&macros, text, "x = `|SUM(1)"), ["((a) + (b))"]);
    }

    #[test]
    fn a_use_in_a_body_resolves_where_the_body_is_expanded() {
        let text = "\
`define INNER 1
`define OUTER `INNER + 1
module m;
  int first = `OUTER;
`define INNER 2
  int second = `OUTER;
endmodule
";
        let macros = table(&[], text);
        let mut found = bodies(&macros, text, "`define OUTER `|INNER");
        found.sort();
        assert_eq!(found, ["1", "2"]);
    }

    #[test]
    fn a_site_covers_the_grave_accent_and_the_name() {
        let text = "`define W 4\nmodule m;\n  int x = `W;\nendmodule\n";
        let macros = table(&[], text);
        let site = macros.site_at("t.sv", 2, 10).expect("grave accent");
        assert_eq!((site.col_start0, site.col_end0), (10, 12));
        assert!(macros.site_at("t.sv", 2, 11).is_some());
        assert!(macros.site_at("t.sv", 2, 12).is_none());
        assert!(macros.site_at("t.sv", 2, 9).is_none());
        assert_eq!(macros.sites("t.sv").len(), 2);
        assert!(macros.sites("other.sv").is_empty());
    }

    #[test]
    fn config_defines_that_are_no_identifier_are_dropped() {
        let defines = ["A", "B=1", "C=", "1BAD", "BAD-NAME=2", "", "=3"].map(str::to_owned);
        let parsed: Vec<_> = parse_config_defines(&defines)
            .into_iter()
            .map(|definition| (definition.name, definition.body))
            .collect();
        let expected = [("A", ""), ("B", "1"), ("C", "")]
            .map(|(name, body)| (name.to_owned(), body.to_owned()));
        assert_eq!(parsed, expected);
    }
}
