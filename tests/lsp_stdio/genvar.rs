use super::*;

struct GenvarCase {
    name: &'static str,
    sites: Vec<Value>,
}

fn symbol_positions(value: &Value, name: &str, positions: &mut Vec<(u64, u64)>) {
    match value {
        Value::Array(values) => {
            for value in values {
                symbol_positions(value, name, positions);
            }
        }
        Value::Object(fields) => {
            if fields.get("name").and_then(Value::as_str) == Some(name) {
                let start = value
                    .pointer("/selectionRange/start")
                    .or_else(|| value.pointer("/location/range/start"))
                    .expect("symbol declaration location");
                positions.push(start_key(start));
            }
            if let Some(children) = fields.get("children") {
                symbol_positions(children, name, positions);
            }
        }
        _ => {}
    }
}

fn genvar_case(text: &str, name: &'static str, begin: &str, end: &str) -> GenvarCase {
    let start = text.find(begin).expect("scope start in fixture");
    let stop = start + text[start..].find(end).expect("scope end in fixture");
    let is_identifier = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$');
    let sites = text[start..stop]
        .match_indices(name)
        .filter_map(|(relative, _)| {
            let at = start + relative;
            let bytes = text.as_bytes();
            if at > 0 && is_identifier(bytes[at - 1])
                || bytes
                    .get(at + name.len())
                    .copied()
                    .is_some_and(is_identifier)
            {
                return None;
            }
            let prefix = &text[..at];
            let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
            let line_start = prefix.rfind('\n').map_or(0, |position| position + 1);
            Some(json!({
                "line": line,
                "character": text[line_start..at].encode_utf16().count()
            }))
        })
        .collect::<Vec<_>>();
    assert!(!sites.is_empty(), "fixture must contain {name}");
    GenvarCase { name, sites }
}

fn assert_genvar_navigation(client: &mut LspProcess, uri: &str, case: &GenvarCase) {
    let declaration = &case.sites[0];
    let hover = param_hover_markup(client, uri, declaration);
    assert!(
        hover.contains(&format!("genvar {}", case.name)),
        "{hover:?}"
    );
    assert!(
        !hover.contains("localparam"),
        "genvar declaration hover: {hover}"
    );
    assert!(
        value_line(&hover).is_none(),
        "no single elaborated genvar value: {hover}"
    );

    for site in &case.sites {
        let definition = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri }, "position": site
                }),
            )
            .expect("genvar definition");
        assert_eq!(
            definition.get("uri").and_then(Value::as_str),
            Some(uri),
            "{definition}"
        );
        assert_eq!(
            definition.pointer("/range/start"),
            Some(declaration),
            "at {site}: {definition}"
        );
    }

    for query in [declaration, case.sites.last().expect("genvar site")] {
        for include_declaration in [false, true] {
            let references = client
                .request(
                    "textDocument/references",
                    json!({
                        "textDocument": { "uri": uri }, "position": query,
                        "context": { "includeDeclaration": include_declaration }
                    }),
                )
                .expect("genvar references");
            let mut actual = references
                .as_array()
                .expect("reference array")
                .iter()
                .map(|location| {
                    assert_eq!(location.get("uri").and_then(Value::as_str), Some(uri));
                    start_key(&location["range"]["start"])
                })
                .collect::<Vec<_>>();
            let mut expected = case
                .sites
                .iter()
                .skip(usize::from(!include_declaration))
                .map(start_key)
                .collect::<Vec<_>>();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(
                actual,
                expected,
                "{name} references at {query}: {references}",
                name = case.name
            );
        }
    }

    let query = case.sites.last().expect("genvar query");
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri }, "position": query
            }),
        )
        .expect("prepare genvar rename");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some(case.name)
    );
    assert_eq!(prepared.pointer("/range/start"), Some(query));
    let renamed = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri }, "position": query, "newName": "renamed_index"
            }),
        )
        .expect("rename genvar");
    assert_no_shadow_uris(&renamed);
    assert_eq!(
        renamed["changes"]
            .as_object()
            .expect("rename changes")
            .len(),
        1
    );
    let edits = edits_for(&renamed, uri);
    let mut actual = edits
        .iter()
        .map(|(start, end, replacement)| {
            assert_eq!(replacement, "renamed_index");
            assert_eq!(end["line"], start["line"]);
            assert_eq!(
                end["character"].as_u64(),
                Some(start["character"].as_u64().expect("column") + case.name.len() as u64)
            );
            start_key(start)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = case.sites.iter().map(start_key).collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "rename must stay in the lexical genvar scope"
    );
}

fn assert_genvar_tokens(
    client: &mut LspProcess,
    uri: &str,
    source: &str,
    legend: &Value,
    cases: &[GenvarCase],
) {
    let result = client
        .request(
            "textDocument/semanticTokens/full",
            json!({
                "textDocument": { "uri": uri }
            }),
        )
        .expect("genvar semantic tokens");
    let rows = semantic_token_rows(
        &result,
        &legend_names(legend, "tokenTypes"),
        &legend_names(legend, "tokenModifiers"),
    );
    for case in cases {
        for site in &case.sites {
            let (line, column) = start_key(site);
            assert_eq!(
                row_at(&rows, line, column).token_type,
                "variable",
                "{} at {site}",
                case.name
            );
        }
    }
    for (byte_index, _) in source.match_indices("genvar ") {
        let prefix = &source[..byte_index];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64;
        let line_start = prefix.rfind('\n').map_or(0, |position| position + 1);
        let column = source[line_start..byte_index].encode_utf16().count() as u64;
        assert_eq!(row_at(&rows, line, column).token_type, "type");
    }
}

#[test]
fn lsp_stdio_genvar_explicit_declarations_and_loop_uses() {
    let fixture = FixtureTree::new();
    let root = fixture.root("genvar");
    let path = root.join("explicit.v");
    let source = fs::read_to_string(&path).expect("read explicit genvar fixture");
    let uri = file_uri(&path);
    let cases = [
        genvar_case(
            &source,
            "count",
            "module GenvarExplicit;",
            "function integer",
        ),
        genvar_case(&source, "spare", "module GenvarExplicit;", "endmodule"),
        genvar_case(&source, "extra", "module GenvarExplicit;", "endmodule"),
        genvar_case(&source, "last", "module GenvarExplicit;", "endmodule"),
    ];
    assert_eq!(cases[0].sites.len(), 6);
    assert_eq!(cases[1].sites.len(), 1);
    let mut client = LspProcess::spawn(fixture.base());
    let initialized = client
        .initialize(&[("genvar", &root)], default_init_options())
        .expect("initialize genvar workspace");
    let legend = &initialized["capabilities"]["semanticTokensProvider"]["legend"];
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    for case in &cases {
        assert_genvar_navigation(&mut client, &uri, case);
    }
    assert_genvar_tokens(&mut client, &uri, &source, legend, &cases);
    client
        .open(&path, &source)
        .expect("open explicit genvar buffer");
    assert_genvar_tokens(&mut client, &uri, &source, legend, &cases);
    let ordinary = position_at(&source, "initial count", "initial ".len());
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": ordinary
            }),
        )
        .expect("ordinary same-name variable definition");
    assert_eq!(
        definition.pointer("/range/start"),
        Some(&position_at(
            &source,
            "  integer count;",
            "  integer ".len()
        ))
    );
    let function_use = position_at(&source, "passthrough = count", "passthrough = ".len());
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": function_use
            }),
        )
        .expect("function formal shadows module genvar");
    assert_eq!(
        definition.pointer("/range/start"),
        Some(&position_at(
            &source,
            "input integer count",
            "input integer ".len()
        ))
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_genvar_inline_scopes_and_pruned_loops() {
    let fixture = FixtureTree::new();
    let root = fixture.root("genvar");
    let path = root.join("inline.sv");
    let source = fs::read_to_string(&path).expect("read inline genvar fixture");
    let uri = file_uri(&path);
    let cases = [
        genvar_case(
            &source,
            "idx",
            "for (genvar idx = 0; idx < 2",
            "for (genvar idx = 0; idx < 0",
        ),
        genvar_case(
            &source,
            "inner",
            "for (genvar inner",
            "for (genvar idx = 0; idx < 0",
        ),
        genvar_case(
            &source,
            "idx",
            "for (genvar idx = 0; idx < 0",
            "wire [3:0] after_loop",
        ),
    ];
    assert_eq!(cases[0].sites.len(), 5);
    assert_eq!(cases[1].sites.len(), 4);
    assert_eq!(cases[2].sites.len(), 4);
    let mut client = LspProcess::spawn(fixture.base());
    let initialized = client
        .initialize(&[("genvar", &root)], default_init_options())
        .expect("initialize inline genvar workspace");
    let legend = &initialized["capabilities"]["semanticTokensProvider"]["legend"];
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    for case in &cases {
        assert_genvar_navigation(&mut client, &uri, case);
    }
    assert_genvar_tokens(&mut client, &uri, &source, legend, &cases);
    client
        .open(&path, &source)
        .expect("open inline genvar buffer");
    assert_genvar_tokens(&mut client, &uri, &source, legend, &cases);
    let ordinary = position_at(&source, "after_loop = idx", "after_loop = ".len());
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": ordinary
            }),
        )
        .expect("outer parameter definition after loop");
    assert_eq!(
        definition.pointer("/range/start"),
        Some(&position_at(&source, "integer idx", "integer ".len()))
    );
    for (method, params) in [
        (
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        ),
        ("workspace/symbol", json!({ "query": "idx" })),
    ] {
        let symbols = client.request(method, params).expect("genvar symbols");
        assert_no_shadow_uris(&symbols);
        let mut actual = Vec::new();
        symbol_positions(&symbols, "idx", &mut actual);
        actual.sort_unstable();
        let mut expected = vec![
            start_key(&position_at(&source, "integer idx", "integer ".len())),
            start_key(&cases[0].sites[0]),
            start_key(&cases[2].sites[0]),
        ];
        expected.sort_unstable();
        assert_eq!(
            actual, expected,
            "{method} must expose each lexical declaration once: {symbols}"
        );
    }
    client.shutdown();
}

#[test]
fn lsp_stdio_genvar_navigation_survives_syntax_fallback() {
    let fixture = FixtureTree::new();
    let root = fixture.root("genvar-fallback");
    let path = root.join("broken.sv");
    let source = fs::read_to_string(&path).expect("read fallback genvar fixture");
    let uri = file_uri(&path);
    let mut cases = [
        genvar_case(&source, "index", "module GenvarFallback;", "endmodule"),
        genvar_case(&source, "row", "module GenvarFallback;", "endmodule"),
    ];
    let shadow_start = position_at(&source, "if (1) begin : wire_scope", 0)["line"]
        .as_u64()
        .expect("wire scope start");
    let shadow_end = position_at(&source, "wire [1:0] data = index", 0)["line"]
        .as_u64()
        .expect("wire scope end");
    cases[0].sites.retain(|site| {
        let line = site["line"].as_u64().expect("site line");
        line < shadow_start || line >= shadow_end
    });
    assert_eq!(cases[0].sites.len(), 6);
    assert_eq!(cases[1].sites.len(), 4);
    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("genvar-fallback", &root)], default_init_options())
        .expect("initialize syntax-fallback genvar workspace");
    wait_for_diagnostics(&mut client, &uri, has_severity_1);
    for case in &cases {
        assert_genvar_navigation(&mut client, &uri, case);
    }
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&source, "scoped_value = index", "scoped_value = ".len())
            }),
        )
        .expect("fallback wire shadows genvar");
    assert_eq!(
        definition.pointer("/range/start"),
        Some(&position_at(&source, "wire index;", "wire ".len()))
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_genvar_rename_excludes_labels_members_and_shadowed_formals() {
    let fixture = FixtureTree::new();
    let root = fixture.root("genvar");
    let path = root.join("shadows.sv");
    let source = fs::read_to_string(&path).expect("read genvar shadow fixture");
    let uri = file_uri(&path);
    let outer = GenvarCase {
        name: "index",
        sites: vec![
            position_at(&source, "genvar index;", "genvar ".len()),
            position_at(&source, "for (index = 0; index < 2", "for (".len()),
            position_at(&source, "index < 2", 0),
            position_at(&source, "index < 2; index++", "index < 2; ".len()),
            position_at(&source, "(.index(index))", "(.index(".len()),
            position_at(&source, "for (index = 0; index < 3", "for (".len()),
            position_at(&source, "index < 3", 0),
            position_at(&source, "index < 3; index++", "index < 3; ".len()),
            position_at(&source, "outside_value = index", "outside_value = ".len()),
        ],
    };
    let local = genvar_case(
        &source,
        "index",
        "if (1) begin : local_scope",
        "for (index = 0; index < 3",
    );
    assert_eq!(local.sites.len(), 5);
    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("genvar", &root)], default_init_options())
        .expect("initialize genvar shadow workspace");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    assert_genvar_navigation(&mut client, &uri, &outer);
    assert_genvar_navigation(&mut client, &uri, &local);
    for (site, declaration) in [
        (
            position_at(&source, "return index", "return ".len()),
            position_at(&source, "input int index", "input int ".len()),
        ),
        (
            position_at(&source, "(.index(index))", "(.".len()),
            position_at(&source, "input wire index", "input wire ".len()),
        ),
        (
            position_at(
                &source,
                "parameter_value = index",
                "parameter_value = ".len(),
            ),
            position_at(
                &source,
                "localparam integer index",
                "localparam integer ".len(),
            ),
        ),
    ] {
        let definition = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri }, "position": site
                }),
            )
            .expect("ordinary scoped symbol definition");
        assert_eq!(
            definition.pointer("/range/start"),
            Some(&declaration),
            "{definition}"
        );
    }
    client.shutdown();
}
