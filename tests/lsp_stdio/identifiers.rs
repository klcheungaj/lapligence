//! Highlighting and go-to-definition agree on every written name.
//
// Contract (`docs/semantic_tokens.md`, "Names and definitions"):
// * a name has a semantic token exactly when it has a definition, in the
//   project table (an unopened file, and an open one that is as compiled);
// * the only tokens without a definition are the exceptions that
//   `core::tokens::TokenRule` lists: keywords, operators and literals, names
//   the language declares, attribute names, and macro names that no source
//   defines;
// * a macro name leads to the `` `define `` in effect where it is written and
//   the path of a resolved `` `include `` to the included file.

use super::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/semantic_tokens/identifiers"
);

const SOURCES: &[&str] = &[
    "names.sv",
    "macros.sv",
    "shared_a.sv",
    "shared_b.sv",
    "classes.sv",
    "dup_a.sv",
    "dup_b.sv",
    "inc/defs.svh",
    "inc/shared.svh",
];

struct Project {
    _guard: TempDirCleanup,
    root: PathBuf,
    client: LspProcess,
    types: Vec<String>,
    modifiers: Vec<String>,
}

impl Project {
    /// The fixture, analyzed.
    fn start(tag: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("llg-lsp-identifiers-{}-{tag}", std::process::id()));
        let root = base.join("project");
        copy_tree(Path::new(FIXTURE), &root).expect("copy identifier fixture");
        // No directory is searched for an angle-bracketed name, so write one
        // that needs no search.
        let header = root.join("inc/defs.svh");
        let header = header.to_str().expect("UTF-8 path").replace('\\', "/");
        fs::write(root.join("angle.sv"), format!("`include <{header}>\n")).expect("write angle.sv");
        let mut client = LspProcess::spawn(&root);
        let initialize = client
            .initialize(&[("identifiers", &root)], default_init_options())
            .expect("initialize identifier workspace");
        let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
        let project = Self {
            types: legend_names(legend, "tokenTypes"),
            modifiers: legend_names(legend, "tokenModifiers"),
            _guard: TempDirCleanup(base),
            root,
            client,
        };
        project.ready()
    }

    fn ready(mut self) -> Self {
        let uri = self.uri("names.sv");
        wait_for_diagnostics(&mut self.client, &uri, has_severity_1);
        self
    }

    fn uri(&self, file: &str) -> String {
        file_uri(&self.root.join(file))
    }

    fn text(&self, file: &str) -> String {
        fs::read_to_string(self.root.join(file)).expect("read fixture source")
    }

    fn rows(&mut self, file: &str) -> Vec<SemanticRow> {
        let uri = self.uri(file);
        let result = self
            .client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("semantic token response");
        semantic_token_rows(&result, &self.types, &self.modifiers)
    }

    fn at(&mut self, method: &str, file: &str, position: (u64, u64)) -> Value {
        let uri = self.uri(file);
        self.client
            .request(
                method,
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": position.0, "character": position.1 },
                    "context": { "includeDeclaration": true }
                }),
            )
            .expect("navigation response")
    }

    /// The definitions of the name at `position`, sorted.
    fn definitions(&mut self, file: &str, position: (u64, u64)) -> Vec<(String, (u64, u64))> {
        let mut definitions = locations(&self.at("textDocument/definition", file, position));
        definitions.sort();
        definitions
    }

    /// The definitions of the name at the `|` of `marked` in `file` are those
    /// at the marks in `expected`.
    fn expect(&mut self, file: &str, marked: &str, expected: &[(&str, &str)]) {
        let position = cursor(&self.text(file), marked);
        let mut want: Vec<_> = expected
            .iter()
            .map(|(target, marked)| (self.uri(target), cursor(&self.text(target), marked)))
            .collect();
        want.sort();
        assert_eq!(self.definitions(file, position), want, "{file}: {marked}");
    }
}

fn locations(result: &Value) -> Vec<(String, (u64, u64))> {
    let location = |value: &Value| {
        (
            value["uri"].as_str().expect("location URI").to_owned(),
            start_key(&value["range"]["start"]),
        )
    };
    match result {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().map(location).collect(),
        single => vec![location(single)],
    }
}

/// The position of the `|` in `marked`, which without it occurs once.
fn cursor(text: &str, marked: &str) -> (u64, u64) {
    let offset = marked.find('|').expect("cursor mark");
    let plain = marked.replacen('|', "", 1);
    assert_eq!(text.matches(&plain).count(), 1, "{plain:?}");
    let (line, character) = position_of(text, &plain, 0);
    (line, character + offset as u64)
}

/// The start of every word outside comments and string literals, with the
/// word: identifiers, keywords and, with their grave accent, macro uses and
/// directives. The fixture is ASCII.
fn words(text: &str) -> Vec<((u64, u64), String)> {
    let bytes = text.as_bytes();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let rest = &text[at..];
        if rest.starts_with("//") {
            at += rest.find('\n').unwrap_or(rest.len());
        } else if let Some(literal) = rest.strip_prefix('"') {
            at += 2 + literal.find('"').expect("closing quote");
        } else if is_word(bytes[at]) || bytes[at] == b'`' {
            let length = 1 + bytes[at + 1..]
                .iter()
                .take_while(|byte| is_word(**byte))
                .count();
            let word = &text[at..at + length];
            // A number (`4'd3`) is no name.
            if !bytes[at].is_ascii_digit() && bytes.get(at.wrapping_sub(1)) != Some(&b'\'') {
                let line = text[..at].matches('\n').count();
                let start = text[..at].rfind('\n').map_or(0, |index| index + 1);
                out.push(((line as u64, (at - start) as u64), word.to_owned()));
            }
            at += length;
        } else {
            at += 1;
        }
    }
    out
}

/// Every word of `file` has a token exactly when it has a definition, except
/// for the tokens that name no declaration.
fn check_agreement(project: &mut Project, file: &str, what: &str) {
    let text = project.text(file);
    let rows = project.rows(file);
    for (position, word) in words(&text) {
        let row = rows
            .iter()
            .find(|row| (row.line, row.character) == position);
        let definitions = project.definitions(file, position);
        let at = format!(
            "{what} {file}:{}:{}: {word}",
            position.0 + 1,
            position.1 + 1
        );
        match row {
            None => assert!(definitions.is_empty(), "{at}: no token, {definitions:?}"),
            Some(row) if definitions.is_empty() => assert!(
                ["keyword", "modifier", "operator", "string", "decorator"]
                    .contains(&row.token_type.as_str())
                    || row.modifiers.iter().any(|name| name == "defaultLibrary"),
                "{at}: no definition, {row:?}"
            ),
            Some(_) => {}
        }
    }
}

#[test]
fn lsp_stdio_every_name_has_a_token_exactly_when_it_has_a_definition() {
    let mut project = Project::start("agreement");
    for file in SOURCES {
        check_agreement(&mut project, file, "closed");
    }
    for file in SOURCES {
        let text = project.text(file);
        let path = project.root.join(file);
        project.client.open(&path, &text).expect("open source");
        check_agreement(&mut project, file, "open");
    }
    project.client.shutdown();
}

/// One use of each kind of name, with where it leads.
#[test]
fn lsp_stdio_names_lead_to_their_declarations() {
    let mut project = Project::start("names");
    let names = "names.sv";
    let classes = "classes.sv";
    for (marked, expected) in [
        // Before its declaration, unknown, or in an unknown module.
        ("before_wire = |late;", &[] as &[(&str, &str)]),
        ("<= |missing_name", &[]),
        ("<= |missing_call", &[]),
        ("pair.|nothing", &[]),
        (".|UNKNOWN(1)", &[]),
        (".|absent(done)", &[]),
        ("|ghost_module", &[]),
        ("ghost_module #(.|WIDTH", &[]),
        ("u_ghost (.|data", &[]),
        // An implicit net declares itself.
        ("assign |before_wire", &[(names, "assign |before_wire")]),
        // The rest of the same statements resolves.
        ("assign |late", &[(names, "logic |late;")]),
        (".absent(|done)", &[(names, "ready, |done;")]),
        ("missing_name + |done", &[(names, "ready, |done;")]),
        ("missing_call(|word", &[(names, "[DEPTH-1:0] |word;")]),
        ("pair.|lo);", &[(names, "[3:0] |lo;}")]),
        ("|pair.nothing", &[(names, "  pair_t |pair, pairs")]),
        // Scope qualifiers, end labels, named and typed patterns.
        ("|names_pkg::IDLE", &[(names, "package |names_pkg;")]),
        ("endpackage : |names_pkg", &[(names, "package |names_pkg;")]),
        ("endfunction : |twice", &[(names, "int |twice(input")]),
        ("endmodule : |leaf", &[(names, "module |leaf #")]),
        ("end : |lanes", &[(names, "begin : |lanes")]),
        ("disable |main", &[(names, "begin : |main")]),
        ("RESET = '{|hi", &[(names, "[3:0] |hi;")]),
        ("<= |pair_t'{hi: 4'd3", &[(names, "[3:0] lo;} |pair_t;")]),
        ("pair_t'{|hi: 4'd3", &[(names, "[3:0] |hi;")]),
        ("'{|pair_t: RESET}", &[(names, "[3:0] lo;} |pair_t;")]),
        ("(.|value(DEPTH))", &[(names, "input int |value);")]),
        ("if ((|twice(1))", &[(names, "int |twice(input")]),
        // Constant expressions the frontend evaluates and discards.
        ("(int'(|RESET.lo)", &[(names, "pair_t |RESET =")]),
        ("(int'(RESET.|lo)", &[(names, "[3:0] |lo;}")]),
        ("lanes[|DEPTH-2]", &[(names, "int |DEPTH = 4;")]),
        ("|lanes[DEPTH-2]", &[(names, "begin : |lanes")]),
        ("u_array[|DEPTH-4]", &[(names, "int |DEPTH = 4;")]),
        ("if (|DEPTH > 2)", &[(names, "int |DEPTH = 4;")]),
        // Instances, hierarchical names, overrides and binds.
        (
            "  |leaf #(.WIDTH(DEPTH), .UNKNOWN",
            &[(names, "module |leaf #")],
        ),
        (".UNKNOWN(1)) |u_leaf", &[(names, "module |leaf #")]),
        ("u_leaf (.|data(word)", &[(names, "[WIDTH-1:0] |data,")]),
        ("defparam u_leaf.|EXTRA", &[(names, "parameter int |EXTRA")]),
        ("u_leaf.|held[DEPTH-1]", &[(names, "[WIDTH-1:0] |held;")]),
        ("u_untaken.|held;", &[(names, "[WIDTH-1:0] |held;")]),
        ("|u_untaken.held", &[(names, "leaf |u_untaken (")]),
        ("bind |leaf", &[(names, "module |leaf #")]),
        ("cross |point_a", &[(names, "  |point_a: coverpoint")]),
        // Attribute names declare nothing; their values are expressions.
        ("(* |keep,", &[]),
        ("|async_reg = \"true\"", &[]),
        ("depth = |DEPTH *", &[(names, "int |DEPTH = 4;")]),
        ("style = |DEPTH *", &[(names, "int |DEPTH = 4;")]),
    ] {
        project.expect(names, marked, expected);
    }
    for (marked, expected) in [
        (
            "class item extends |base_item;",
            &[(classes, "class |base_item;")] as &[(&str, &str)],
        ),
        (
            "#(type ELEMENT = |base_item)",
            &[(classes, "class |base_item;")],
        ),
        ("typedef |registry #", &[(classes, "class |registry #")]),
        (
            "super.new(.|total(",
            &[(classes, "new(int |total = 0);\n      this")],
        ),
        ("made = |item::type_id", &[(classes, "class |item extends")]),
        (
            "item::|type_id::create",
            &[(classes, "\"item\") |type_id;")],
        ),
        (
            "type_id::|create(\"made\")",
            &[(classes, "static function T |create(")],
        ),
        (
            "function void |item::run",
            &[(classes, "class |item extends")],
        ),
        ("|wrapper::count", &[(classes, "class |wrapper;")]),
        (
            "endclass : |item\n  class wrapper",
            &[(classes, "class |item extends")],
        ),
        ("{|level == 2'd1;}", &[(classes, "bit [1:0] |level;")]),
        // A class that nothing specializes.
        (
            "void add(|ELEMENT member",
            &[(classes, "#(type |ELEMENT =")],
        ),
        ("push_back(|member)", &[(classes, "add(ELEMENT |member);")]),
        ("|members.push_back", &[(classes, "ELEMENT |members [$];")]),
        ("members.|push_back", &[]),
    ] {
        project.expect(classes, marked, expected);
    }
    // Two bodies, only one of which is elaborated.
    for file in ["dup_a.sv", "dup_b.sv"] {
        for (marked, expected) in [
            (
                "= |u_inner.held;",
                &[(file, "leaf |u_inner (")] as &[(&str, &str)],
            ),
            ("u_inner.|held;", &[(names, "[WIDTH-1:0] |held;")]),
            ("(.|data(clk)", &[(names, "[WIDTH-1:0] |data,")]),
            ("ready(|held_here)", &[(file, "logic |held_here;")]),
            ("defparam |u_inner", &[(file, "leaf |u_inner (")]),
            ("u_inner.|EXTRA", &[(names, "parameter int |EXTRA")]),
            ("|scope.in_scope", &[(file, "begin : |scope")]),
            ("scope.|in_scope", &[(file, "logic |in_scope;")]),
        ] {
            project.expect(file, marked, expected);
        }
    }
    project.client.shutdown();
}

#[test]
fn lsp_stdio_macro_names_lead_to_the_define_in_effect() {
    let mut project = Project::start("macros");
    let macros = "macros.sv";
    let defs = "inc/defs.svh";
    for (marked, expected) in [
        (
            "[`|LOCAL_W-1:0] first",
            &[(macros, "`define |LOCAL_W 8")] as &[(&str, &str)],
        ),
        ("[|`LOCAL_W-1:0] first", &[(macros, "`define |LOCAL_W 8")]),
        ("[`LOCAL_W|-1:0] first", &[(macros, "`define |LOCAL_W 8")]),
        ("[`LOCAL|_W-1:0] second", &[(macros, "`define |LOCAL_W 16")]),
        ("`undef |LOCAL_W", &[(macros, "`define |LOCAL_W 16")]),
        // Undefined there, or defined by the configuration or the language.
        ("[`|LOCAL_W-1:0] third", &[]),
        ("`|CONFIG_W", &[]),
        ("`|__LINE__", &[]),
        ("`ifdef |UNDEFINED_A", &[]),
        ("`ifndef |UNDEFINED_GUARD", &[]),
        // Other directive operands, headers, arguments, inactive branches.
        ("`define |LOCAL_W 8", &[(macros, "`define |LOCAL_W 8")]),
        ("`ifdef |LEVEL_A", &[(macros, "`define |LEVEL_A")]),
        ("`elsif |LEVEL_A", &[(macros, "`define |LEVEL_A")]),
        ("`|HDR_W", &[(defs, "`define |HDR_W 4")]),
        ("= `|SUM(first", &[(defs, "`define |SUM(lhs")]),
        (", `|SUM(second", &[(defs, "`define |SUM(lhs")]),
        ("SUM(|first[0]", &[(macros, "] |first;")]),
        ("SUM(|second[1]", &[(macros, "] |second;")]),
        ("|sum_late))", &[]),
        ("= `|PICKED", &[(macros, "`define |PICKED 1")]),
        ("`define |PICKED 2", &[]),
    ] {
        project.expect(macros, marked, expected);
    }
    for (marked, expected) in [
        // The names in a macro body: formal arguments, and what every
        // expansion resolves alike.
        ("((|lhs)", &[(defs, "SUM(|lhs,")] as &[(&str, &str)]),
        ("|object.step", &[(defs, "CALL(|object)")]),
        ("object.|step", &[("classes.sv", "function int |step(")]),
        ("step(|total)", &[("classes.sv", "int |total;")]),
    ] {
        project.expect(defs, marked, expected);
    }
    // Every unit that includes the header defines the macro.
    project.expect(
        "inc/shared.svh",
        "`|SHARED_W",
        &[
            ("shared_a.sv", "`define |SHARED_W"),
            ("shared_b.sv", "`define |SHARED_W"),
        ],
    );
    project.client.shutdown();
}

/// The path of an include directive, with its quotes or brackets, leads to
/// the start of the included file; an unresolved one leads nowhere.
#[test]
fn lsp_stdio_include_paths_lead_to_the_included_file() {
    let mut project = Project::start("includes");
    let header = (project.uri("inc/defs.svh"), (0, 0));
    for opened in [false, true] {
        for (file, opening, target) in [
            ("macros.sv", "\"defs.svh\"", Some(&header)),
            ("macros.sv", "\"missing.svh\"", None),
            ("angle.sv", "<", Some(&header)),
        ] {
            let text = project.text(file);
            if opened {
                let path = project.root.join(file);
                project.client.open(&path, &text).expect("open source");
            }
            let (line, start) = position_of(&text, opening, 0);
            let end = text
                .lines()
                .nth(line as usize)
                .expect("directive line")
                .len() as u64;
            for character in start..=end {
                assert_eq!(
                    project.definitions(file, (line, character)),
                    target.into_iter().cloned().collect::<Vec<_>>(),
                    "opened={opened} {opening}@{character}"
                );
            }
            assert_eq!(
                project.definitions(file, (line, 3)).len(),
                0,
                "the directive itself"
            );
        }
    }
    project.client.shutdown();
}

/// An attribute name is a `decorator` without a definition, in the project
/// table and in an edited buffer; its value keeps its own tokens.
#[test]
fn lsp_stdio_attribute_names_are_decorators() {
    let mut project = Project::start("attributes");
    let text = project.text("names.sv");
    let check = |project: &mut Project, text: &str, what: &str| {
        let rows = project.rows("names.sv");
        let token = |marked: &str| {
            let position = cursor(text, marked);
            let found: Vec<_> = rows
                .iter()
                .filter(|row| (row.line, row.character) == position)
                .map(|row| (row.token_type.as_str(), row.modifiers.len()))
                .collect();
            assert_eq!(found.len(), 1, "{what}: {marked}");
            found[0]
        };
        for marked in [
            "(* |keep,",
            "|async_reg = \"true\"",
            "|depth = DEPTH",
            "(* |full_case,",
            "|style = DEPTH",
        ] {
            assert_eq!(token(marked), ("decorator", 0), "{what}: {marked}");
        }
        assert_eq!(token("async_reg = |\"true\"").0, "string", "{what}");
        for marked in ["depth = |DEPTH *", "style = |DEPTH *"] {
            assert_eq!(token(marked), ("variable", 1), "{what}: {marked}");
        }
    };
    check(&mut project, &text, "closed");
    let path = project.root.join("names.sv");
    project.client.open(&path, &text).expect("open names.sv");
    check(&mut project, &text, "open");
    let edited = format!("// edited\n{text}");
    project
        .client
        .change(&path, 2, &edited)
        .expect("edit names.sv");
    check(&mut project, &edited, "edited");
    project.client.shutdown();
}

/// Hover, references and rename read the bindings that tokens and
/// definitions do: a name that resolves to nothing has none of them, and an
/// end label is a reference.
#[test]
fn lsp_stdio_hover_references_and_rename_follow_the_same_bindings() {
    let mut project = Project::start("references");
    let names = "names.sv";
    let text = project.text(names);
    for marked in ["before_wire = |late;", "<= |missing_name", "|ghost_module"] {
        let position = cursor(&text, marked);
        for method in ["textDocument/hover", "textDocument/references"] {
            let result = project.at(method, names, position);
            assert!(
                result.is_null() || result.as_array().is_some_and(Vec::is_empty),
                "{marked}: {method}: {result}"
            );
        }
    }
    let declaration = cursor(&text, "logic |late;");
    let references = project.at("textDocument/references", names, declaration);
    let mut lines: Vec<_> = locations(&references)
        .into_iter()
        .map(|(_, start)| start)
        .collect();
    lines.sort();
    assert_eq!(lines, [declaration, cursor(&text, "assign |late")]);

    let uri = project.uri(names);
    let declaration = cursor(&text, "module |leaf #");
    let rename = project
        .client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": declaration.0, "character": declaration.1 },
                "newName": "renamed_leaf"
            }),
        )
        .expect("rename response");
    let starts: Vec<_> = edits_for(&rename, &uri)
        .iter()
        .map(|(start, _, _)| start_key(start))
        .collect();
    for marked in ["endmodule : |leaf", "bind |leaf", "  |leaf u_untaken"] {
        assert!(
            starts.contains(&cursor(&text, marked)),
            "{marked}: {starts:?}"
        );
    }
    for file in ["dup_a.sv", "dup_b.sv"] {
        let duplicate = project.uri(file);
        assert_eq!(edits_for(&rename, &duplicate).len(), 1, "{rename}");
    }
    project.client.shutdown();
}
