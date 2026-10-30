//! VPI header parity and catalog gate (ADV-019 A03, ADV-020 A01).
//!
//! - Header parity: `tests/fixtures/runtime/vpi_annex_kml.tsv` lists every
//!   constant, structure and routine of IEEE 1800-2009 Annex K
//!   (`vpi_user.h`), Annex L (`vpi_compatibility.h`) and Annex M
//!   (`sv_vpi_user.h`) with the line of the standard text it comes from. The
//!   runtime headers must define each Annex K and M constant with its value,
//!   declare each structure with its members and each routine with its
//!   parameter types, and define none of the Annex L compatibility renames.
//! - Catalog gate: every row of `src/sim/rt/vpi_catalog.txt` has a status, a
//!   test that exists for implemented, extension and error rows and a reason
//!   for the others; the catalog names every property, relation and
//!   iteration the runtime dispatch (`llg_vpi.c`, `llg_vpi_dynamic.c`)
//!   handles, and every row the runtime answers is in that dispatch; the
//!   ADV-019 walker probes every one of those names. The walk itself is
//!   `adv_019::catalog_walk_matches_the_vpi_catalog`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::vpi_catalog::{
    self, evaluate, header_defines, header_values, read_source, strip_c_comments, Catalog,
};

const ANNEX: &str = "tests/fixtures/runtime/vpi_annex_kml.tsv";
const WALKER: &str = "tests/fixtures/sim/feature_completion/adv_019/catalog.c";

/// Header names no annex defines, with the reason each is there.
const HEADER_EXTRAS: &[(&str, &str)] = &[
    ("LLG_VPI_USER_H", "include guard"),
    ("SV_VPI_USER_H", "include guard"),
    (
        "VPI_TIME",
        "guard of s_vpi_time, shared with other PLI headers",
    ),
    (
        "VPI_VECVAL",
        "guard of s_vpi_vecval, shared with other PLI headers",
    ),
    ("SV_COV_OVERFLOW", "40.3.1 coverage control results"),
    ("SV_COV_ERROR", "40.3.1 coverage control results"),
    ("SV_COV_NOCOV", "40.3.1 coverage control results"),
    ("SV_COV_OK", "40.3.1 coverage control results"),
    ("SV_COV_PARTIAL", "40.3.1 coverage control results"),
    (
        "vpiCollection",
        "P1800-2005 30.4.2 collections for vpi_create",
    ),
];

struct AnnexRow {
    annex: String,
    line: String,
    kind: String,
    class: String,
    name: String,
    value: String,
}

fn annex_rows() -> Vec<AnnexRow> {
    let text = read_source(ANNEX);
    let rows: Vec<AnnexRow> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(fields.len() >= 6, "{ANNEX}: malformed row `{line}`");
            AnnexRow {
                annex: fields[0].to_owned(),
                line: fields[1].to_owned(),
                kind: fields[2].to_owned(),
                class: fields[3].to_owned(),
                name: fields[4].to_owned(),
                value: fields[5].to_owned(),
            }
        })
        .collect();
    let count = |annex: &str, kind: &str| {
        rows.iter()
            .filter(|row| row.annex == annex && row.kind == kind)
            .count()
    };
    // The annexes as extracted: a truncated table must not pass.
    assert_eq!(count("K", "define"), 460, "Annex K constants");
    assert_eq!(count("L", "define"), 70, "Annex L renames");
    assert_eq!(count("M", "define"), 364, "Annex M constants");
    assert_eq!(count("K", "routine"), 41, "Annex K routines");
    rows
}

fn headers_text() -> String {
    strip_c_comments(&format!(
        "{}\n{}",
        read_source("src/sim/rt/vpi_user.h"),
        read_source("src/sim/rt/sv_vpi_user.h")
    ))
}

#[test]
fn headers_define_the_annex_k_and_m_constants_with_their_values() {
    let rows = annex_rows();
    let header = header_defines();
    let annex: BTreeMap<String, String> = rows
        .iter()
        .filter(|row| row.kind == "define")
        .map(|row| (row.name.clone(), row.value.clone()))
        .collect();
    let mut errors = Vec::new();
    for row in rows.iter().filter(|row| row.kind == "define") {
        let at = format!("Annex {} line {}", row.annex, row.line);
        match row.class.as_str() {
            "std" | "alias" | "deprecated" => match header.get(&row.name) {
                None => errors.push(format!("{} ({at}) is not defined", row.name)),
                Some(value) => {
                    let expected = evaluate(&row.value, &annex);
                    let actual = evaluate(value, &header);
                    if expected.is_none() || expected != actual {
                        errors.push(format!(
                            "{} ({at}) is {value} ({actual:?}), the annex gives {} ({expected:?})",
                            row.name, row.value
                        ));
                    }
                }
            },
            "compat" => {
                if header.contains_key(&row.name) {
                    errors.push(format!(
                        "{} ({at}) is an Annex L compatibility rename, which llg does not provide",
                        row.name
                    ));
                }
            }
            "platform" => {}
            other => errors.push(format!("{} ({at}): unknown class {other}", row.name)),
        }
    }
    for name in header.keys() {
        if !annex.contains_key(name) && !HEADER_EXTRAS.iter().any(|(extra, _)| extra == name) {
            errors.push(format!(
                "{name} is in no annex and not a listed llg addition"
            ));
        }
    }
    for (extra, _) in HEADER_EXTRAS {
        if !header.contains_key(*extra) {
            errors.push(format!("listed addition {extra} is not defined"));
        }
    }
    assert!(errors.is_empty(), "header parity:\n{}", errors.join("\n"));
}

/// The identifiers and `*` tokens of a declaration.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if c == '*' {
                out.push("*".to_owned());
            }
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// A parameter's type without its name: `PLI_BYTE8* name` is `PLI_BYTE8*`.
fn parameter_type(parameter: &str) -> String {
    let mut parts = tokens(parameter);
    if parts.len() > 1 && parts.last().is_some_and(|last| last != "*") {
        parts.pop();
    }
    parts.concat()
}

/// The normalized prototype `ret(type,...)` of routine `name`, from the
/// first declaration `ret name(params);` in `text`.
fn prototype(text: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = text[from..].find(&format!("{name}(")) {
        let at = from + found;
        from = at + name.len();
        let before = text[..at].chars().next_back();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let open = at + name.len();
        let close = open + text[open..].find(')')?;
        if !text[close + 1..].trim_start().starts_with(';') {
            continue;
        }
        let line_start = text[..at].rfind(['\n', ';', '}']).map_or(0, |i| i + 1);
        let ret = tokens(&text[line_start..at]).concat();
        let params: Vec<String> = text[open + 1..close]
            .split(',')
            .map(parameter_type)
            .collect();
        return Some(format!("{ret}({})", params.join(",")));
    }
    None
}

/// The parameter types of `typedef ret(name)(params);`.
fn function_type(text: &str, name: &str) -> Option<String> {
    let at = text.find(&format!("({name})("))?;
    let line_start = text[..at].rfind(['\n', ';'])? + 1;
    let ret = tokens(&text[line_start..at]);
    let ret = ret
        .strip_prefix(&["typedef".to_owned()])
        .unwrap_or(&ret)
        .concat();
    let open = at + name.len() + 2;
    let close = open + text[open..].find(')')?;
    let params: Vec<String> = text[open + 1..close]
        .split(',')
        .map(parameter_type)
        .collect();
    Some(format!("{ret}({})", params.join(",")))
}

/// The member names of `typedef struct tag { ... }`, nested unions and
/// structures flattened as the annex lists them.
fn struct_members(text: &str, tag: &str) -> Option<String> {
    let start = text.find(&format!("typedef struct {tag} {{"))?;
    let body_start = start + text[start..].find('{')? + 1;
    let mut depth = 1;
    let mut end = body_start;
    for (i, c) in text[body_start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            end = body_start + i;
            break;
        }
    }
    let mut declarations = Vec::new();
    let mut current = String::new();
    let mut chars = text[body_start..end].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => current.clear(),
            '}' => {
                // `} name;` closes a nested union: the annex lists its members.
                while chars.peek().is_some_and(|&c| c != ';') {
                    chars.next();
                }
            }
            ';' => declarations.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    let mut members = Vec::new();
    for declaration in declarations {
        let mut cleaned = String::new();
        let mut in_brackets = false;
        for c in declaration.chars() {
            match c {
                '[' => in_brackets = true,
                ']' => in_brackets = false,
                _ if !in_brackets => cleaned.push(c),
                _ => {}
            }
        }
        if let Some(pointer) = cleaned.find("(*") {
            let rest = &cleaned[pointer + 2..];
            members.push(rest[..rest.find(')')?].trim().to_owned());
            continue;
        }
        for (i, part) in cleaned.split(',').enumerate() {
            let ids: Vec<String> = tokens(part).into_iter().filter(|t| t != "*").collect();
            if !ids.is_empty() && (ids.len() > 1 || i > 0) {
                members.push(ids.last()?.clone());
            }
        }
    }
    Some(members.join(","))
}

#[test]
fn headers_declare_the_annex_structures_and_routines() {
    let text = headers_text();
    let mut errors = Vec::new();
    let mut checked = 0;
    for row in annex_rows() {
        if row.annex == "L" || row.kind == "define" {
            continue;
        }
        let at = format!("Annex {} line {}", row.annex, row.line);
        let actual = match row.kind.as_str() {
            "struct" => struct_members(&text, &row.name),
            "routine" => prototype(&text, &row.name),
            "functype" => function_type(&text, &row.name),
            other => {
                errors.push(format!("{} ({at}): unknown kind {other}", row.name));
                continue;
            }
        };
        checked += 1;
        if actual.as_deref() != Some(row.value.as_str()) {
            errors.push(format!(
                "{} {} ({at}): the header gives {actual:?}, the annex {}",
                row.kind, row.name, row.value
            ));
        }
    }
    assert!(checked >= 55, "only {checked} declarations checked");
    assert!(
        errors.is_empty(),
        "declaration parity:\n{}",
        errors.join("\n")
    );
}

/// Every `.rs` file under `tests/`.
fn test_sources(directory: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("read tests directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "fixtures") {
                test_sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Whether `<suite>.rs` under `tests/` has a `#[test] fn <name>()`.
fn test_exists(sources: &[PathBuf], qualified: &str) -> bool {
    let Some((suite, name)) = qualified.split_once("::") else {
        return false;
    };
    sources
        .iter()
        .filter(|path| path.file_stem().is_some_and(|stem| stem == suite))
        .any(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            text.match_indices(&format!("fn {name}()")).any(|(at, _)| {
                let head = &text[..at];
                let attribute = head.rfind("#[test]");
                attribute.is_some_and(|a| !head[a..].contains("\n}\n"))
            })
        })
}

#[test]
fn catalog_rows_are_well_formed_and_name_existing_tests() {
    let catalog = Catalog::load();
    let values = header_values();
    let routines: BTreeSet<String> = annex_rows()
        .into_iter()
        .filter(|row| row.kind == "routine" && row.annex != "L")
        .map(|row| row.name)
        .collect();
    let mut sources = Vec::new();
    test_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"),
        &mut sources,
    );
    let mut errors = Vec::new();
    for (alias, qualified) in &catalog.tests {
        if !test_exists(&sources, qualified) {
            errors.push(format!("test {alias}: no #[test] fn for {qualified}"));
        }
    }
    let mut used = BTreeSet::new();
    let mut types = BTreeMap::new();
    for node in &catalog.nodes {
        let at = format!("line {} (node {})", node.line, node.id);
        match &node.type_name {
            Some(name) if name == "NULL" => {}
            Some(name) => match values.get(name) {
                None => errors.push(format!("{at}: {name} is not a header constant")),
                Some(value) => {
                    if let Some(other) = types.insert(*value, node.id.clone()) {
                        errors.push(format!("{at}: type {name} is also node {other}"));
                    }
                }
            },
            None => {
                if node.members.is_empty() {
                    errors.push(format!("{at}: a class needs members"));
                }
                for member in &node.members {
                    if catalog.node(member).is_none() {
                        errors.push(format!("{at}: unknown member {member}"));
                    }
                }
            }
        }
        let mut keys = BTreeSet::new();
        for row in &node.rows {
            let at = format!("line {} ({} {})", row.line, row.kind, row.name);
            if !vpi_catalog::KINDS.contains(&row.kind.as_str()) {
                errors.push(format!("{at}: unknown kind"));
            }
            if !keys.insert((row.kind.clone(), row.name.clone())) {
                errors.push(format!("{at}: duplicate row in node {}", node.id));
            }
            let status = &row.status;
            if !vpi_catalog::STATUSES.contains(&status.word.as_str()) {
                errors.push(format!("{at}: unknown status {}", status.word));
            }
            let known = if row.kind == "func" {
                routines.contains(&row.name)
            } else {
                values.contains_key(&row.name)
            };
            // A diagram name without a header constant cannot be served.
            if !known && status.word != "n/a" {
                errors.push(format!("{at}: not a header constant or routine"));
            }
            match status.test() {
                Some(alias) => {
                    used.insert(alias.to_owned());
                    if !catalog.tests.contains_key(alias) {
                        errors.push(format!("{at}: unknown test alias {alias}"));
                    }
                    let reason = status.detail.split_whitespace().count() > 1;
                    if status.word != "implemented" && !reason {
                        errors.push(format!("{at}: {} needs a reason", status.word));
                    }
                }
                None if status.answers() || status.word == "error" => {
                    errors.push(format!("{at}: {} needs a test alias", status.word));
                }
                None if status.detail.is_empty() => {
                    errors.push(format!("{at}: {} needs a reason", status.word));
                }
                None => {}
            }
        }
        if node.type_name.is_some() {
            if let Err(error) = catalog.effective_rows(&node.id) {
                errors.push(error);
            }
        }
    }
    for alias in catalog.tests.keys() {
        if !used.contains(alias) {
            errors.push(format!("test {alias} is used by no row"));
        }
    }
    assert!(
        errors.is_empty(),
        "{}:\n{}",
        vpi_catalog::CATALOG,
        errors.join("\n")
    );
}

/// How a dispatch function names the values it handles.
enum Match {
    /// `case vpiX:` labels of a `switch (var)`.
    Switch,
    /// `var == vpiX`, `var != vpiX`, `var >= vpiX` and `var <= vpiX`.
    Compare,
}

/// The dispatch of each row kind: the functions of `llg_vpi.c` and
/// `llg_vpi_dynamic.c` that decide which properties, relations and
/// iterations an object answers, and the variable holding the query.
/// One dispatch rule: a function, how it names values, and its query variable.
type Rule = (&'static str, Match, &'static str);

const DISPATCH: &[(&str, &[Rule])] = &[
    (
        "get",
        &[
            ("vpi_get", Match::Switch, "property"),
            ("vpi_get", Match::Compare, "property"),
            ("vpi_get64", Match::Compare, "property"),
            ("typespec_property", Match::Switch, "property"),
            ("assertion_property", Match::Switch, "property"),
            ("code_property", Match::Switch, "property"),
            ("dyn_get", Match::Switch, "property"),
            ("dyn_get", Match::Compare, "property"),
            ("dyn_item_get", Match::Switch, "property"),
            ("dyn_item_get", Match::Compare, "property"),
            ("dyn_random_property", Match::Compare, "property"),
            ("is_coverage_type_property", Match::Compare, "property"),
            ("fsm_property", Match::Switch, "property"),
            ("rts_get", Match::Switch, "property"),
        ],
    ),
    (
        "str",
        &[
            ("vpi_get_str", Match::Switch, "property"),
            ("vpi_get_str", Match::Compare, "property"),
            ("code_string", Match::Switch, "property"),
            ("code_string", Match::Compare, "property"),
            ("dyn_get_str", Match::Switch, "property"),
            ("dyn_get_str", Match::Compare, "property"),
            ("dyn_item_get_str", Match::Compare, "property"),
        ],
    ),
    (
        "handle",
        &[
            ("vpi_handle", Match::Switch, "type"),
            ("vpi_handle", Match::Compare, "type"),
            ("statement_relation", Match::Compare, "type"),
            ("clocking_relation", Match::Compare, "type"),
            ("declaration_relation", Match::Switch, "type"),
            ("declaration_relation", Match::Compare, "type"),
            ("assertion_relation", Match::Compare, "type"),
            ("assertion_relation_applies", Match::Compare, "relation"),
            ("code_relation_applies", Match::Compare, "relation"),
            ("code_relation", Match::Compare, "type"),
            ("dyn_handle", Match::Compare, "type"),
            ("dyn_item_handle", Match::Switch, "type"),
            ("fsm_relation", Match::Compare, "type"),
            ("rts_handle", Match::Compare, "type"),
        ],
    ),
    (
        "iterate",
        &[
            ("vpi_iterate", Match::Compare, "type"),
            ("iteration_type_supported", Match::Switch, "type"),
            ("child_matches", Match::Switch, "type"),
            ("code_iteration_applies", Match::Compare, "relation"),
            ("iterate_code", Match::Compare, "relation"),
            ("iterate_typespec", Match::Compare, "type"),
            ("is_connectivity_iteration", Match::Compare, "type"),
            ("dyn_iterate", Match::Compare, "type"),
            ("dyn_class_iteration", Match::Compare, "type"),
            ("dyn_item_iterate", Match::Compare, "type"),
            ("rts_iterate", Match::Compare, "type"),
        ],
    ),
];

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The text from `open` (just after a `{`) to its matching `}`.
fn braced(text: &str, open: usize) -> &str {
    let mut depth = 1;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open..open + i];
                }
            }
            _ => {}
        }
    }
    &text[open..]
}

/// The body of the function definition `name` (a line that starts with a
/// declaration and reaches `name(...) {`).
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let start = line_start;
        line_start += line.len();
        if !line.starts_with(|c: char| c.is_ascii_alphabetic()) {
            continue;
        }
        let Some(at) = line.find(&format!("{name}(")) else {
            continue;
        };
        if line[..at].ends_with(is_ident) || line[..at].contains([';', '{', '}']) {
            continue;
        }
        let after = start + at + name.len();
        let Some(stop) = source[after..].find([';', '{']) else {
            continue;
        };
        if source.as_bytes()[after + stop] == b'{' {
            return braced(source, after + stop + 1);
        }
    }
    panic!("no definition of {name} in the VPI runtime");
}

/// The `vpi...` identifier that starts `text` after any whitespace.
fn vpi_name_after(text: &str) -> Option<&str> {
    let text = text.trim_start();
    let end = text.find(|c: char| !is_ident(c)).unwrap_or(text.len());
    let name = &text[..end];
    name.starts_with("vpi").then_some(name)
}

/// The names `body` compares `var` with or switches on into `out`, and each
/// `var >= lo ... var <= hi` range into `ranges`.
fn dispatch_names(
    body: &str,
    how: &Match,
    var: &str,
    out: &mut BTreeSet<String>,
    ranges: &mut Vec<(String, String)>,
) {
    let mut low: Option<String> = None;
    let mut from = 0;
    while let Some(found) = body[from..].find(var) {
        let at = from + found;
        from = at + var.len();
        let before = body[..at].chars().next_back();
        if before.is_some_and(|c| is_ident(c) || c == '.' || c == '>') {
            continue;
        }
        let rest = &body[from..];
        if rest.starts_with(is_ident) {
            continue;
        }
        let rest = rest.trim_start();
        match how {
            Match::Compare => {
                for op in ["==", "!=", ">=", "<="] {
                    if let Some(name) = rest.strip_prefix(op).and_then(vpi_name_after) {
                        out.insert(name.to_owned());
                        match op {
                            ">=" => low = Some(name.to_owned()),
                            "<=" => {
                                if let Some(lo) = low.take() {
                                    ranges.push((lo, name.to_owned()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Match::Switch => {
                let Some(rest) = rest.strip_prefix(')') else {
                    continue;
                };
                if !body[..at]
                    .trim_end()
                    .strip_suffix('(')
                    .is_some_and(|head| head.trim_end().ends_with("switch"))
                {
                    continue;
                }
                let Some(open) = rest.trim_start().strip_prefix('{') else {
                    continue;
                };
                let open = body.len() - open.len();
                let block = braced(body, open);
                for (case, _) in block.match_indices("case") {
                    if block[..case].ends_with(is_ident) {
                        continue;
                    }
                    let label = &block[case + 4..];
                    if let Some(name) = vpi_name_after(label) {
                        let tail = label.trim_start()[name.len()..].trim_start();
                        if tail.starts_with(':') {
                            out.insert(name.to_owned());
                        }
                    }
                }
            }
        }
    }
}

/// The names each row kind's dispatch handles, and its `>=`/`<=` ranges.
type Dispatch = BTreeMap<&'static str, (BTreeSet<String>, Vec<(String, String)>)>;

fn runtime_dispatch() -> Dispatch {
    let source = strip_c_comments(&format!(
        "{}\n{}\n{}",
        read_source("src/sim/rt/llg_vpi.c"),
        read_source("src/sim/rt/llg_vpi_dynamic.c"),
        read_source("src/sim/rt/llg_vpi_runtime.c")
    ));
    DISPATCH
        .iter()
        .map(|(kind, rules)| {
            let mut names = BTreeSet::new();
            let mut ranges = Vec::new();
            for (function, how, var) in rules.iter() {
                let body = function_body(&source, function);
                dispatch_names(body, how, var, &mut names, &mut ranges);
            }
            (*kind, (names, ranges))
        })
        .collect()
}

#[test]
fn catalog_lists_every_case_of_the_runtime_dispatch() {
    let catalog = Catalog::load();
    let values = header_values();
    let dispatch = runtime_dispatch();
    let mut errors = Vec::new();
    for (kind, (names, ranges)) in &dispatch {
        assert!(
            names.len() >= 5,
            "{kind}: dispatch extraction found only {names:?}"
        );
        let mut listed = BTreeSet::new();
        let mut answered = BTreeMap::new();
        for node in &catalog.nodes {
            for row in node.rows.iter().filter(|row| row.kind == *kind) {
                if let Some(value) = values.get(&row.name) {
                    listed.insert(*value);
                    if row.status.answers() {
                        answered.insert(*value, row.line);
                    }
                }
            }
        }
        for (implicit_kind, name) in vpi_catalog::IMPLICIT {
            if implicit_kind == kind {
                listed.insert(values[*name]);
            }
        }
        let mut handled = BTreeSet::new();
        for name in names {
            match values.get(name) {
                None => errors.push(format!(
                    "{kind} {name}: dispatch name is not a header constant"
                )),
                Some(value) => {
                    handled.insert(*value);
                    if !listed.contains(value) {
                        errors.push(format!(
                            "{kind} {name}: handled by the runtime, not in the catalog"
                        ));
                    }
                }
            }
        }
        for (lo, hi) in ranges {
            let (lo, hi) = (values[lo], values[hi]);
            handled.extend(listed.iter().filter(|v| (lo..=hi).contains(*v)));
        }
        for (value, line) in answered {
            if !handled.contains(&value) {
                errors.push(format!(
                    "{kind} at catalog line {line}: answered, but no dispatch case handles it"
                ));
            }
        }
    }
    assert!(
        errors.is_empty(),
        "catalog and dispatch:\n{}",
        errors.join("\n")
    );
}

/// The `{"name", name}` entries of `static const probe_t <table>[]`.
fn probe_table(source: &str, table: &str) -> BTreeSet<String> {
    let start = source
        .find(&format!("static const probe_t {table}[] = {{"))
        .unwrap_or_else(|| panic!("{WALKER}: no {table}"));
    let body = &source[start..start + source[start..].find("};").expect("table end")];
    body.lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("{\"")?;
            let (quoted, rest) = line.split_once("\",")?;
            let constant = rest.trim().strip_suffix("},")?.trim();
            assert_eq!(
                quoted, constant,
                "{WALKER}: probe {quoted} queries {constant}"
            );
            Some(quoted.to_owned())
        })
        .collect()
}

#[test]
fn catalog_walker_probes_every_catalog_and_dispatch_name() {
    let catalog = Catalog::load();
    let values = header_values();
    let dispatch = runtime_dispatch();
    let source = read_source(WALKER);
    for (table, kinds) in [
        ("property_probes", ["get", "str"]),
        ("relation_probes", ["handle", "iterate"]),
    ] {
        let mut expected: BTreeSet<String> = BTreeSet::new();
        for kind in kinds {
            expected.extend(dispatch[kind].0.iter().cloned());
            for node in &catalog.nodes {
                expected.extend(
                    node.rows
                        .iter()
                        .filter(|row| row.kind == kind && values.contains_key(&row.name))
                        .map(|row| row.name.clone()),
                );
            }
            expected.extend(
                vpi_catalog::IMPLICIT
                    .iter()
                    .filter(|(k, _)| *k == kind)
                    .map(|(_, name)| (*name).to_owned()),
            );
        }
        let actual = probe_table(&source, table);
        let missing: Vec<_> = expected.difference(&actual).collect();
        let extra: Vec<_> = actual.difference(&expected).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{WALKER} {table}: missing {missing:?}, not catalogued {extra:?}"
        );
    }
}

#[test]
fn catalog_reader_resolves_rows_through_classes_and_rejects_bad_rows() {
    let text = "test t suite::case\n\
                node all 1 members=mid\n\
                \x20 get vpiSize implemented t\n\
                \x20 get vpiSigned unsupported not captured\n\
                node mid 2 members=leaf default=n/a not modelled\n\
                \x20 get vpiSigned\n\
                node leaf 3 type=vpiNet\n\
                \x20 get vpiSize unsupported refused\n";
    let catalog = Catalog::parse(text).expect("parse");
    let rows = catalog.effective_rows("leaf").expect("rows");
    let word = |name: &str| rows[&("get".to_owned(), name.to_owned())].word.clone();
    assert_eq!(word("vpiSize"), "unsupported", "the node's own row wins");
    assert_eq!(word("vpiSigned"), "n/a", "the nearer class wins");
    assert!(Catalog::parse("node x 1 type=vpiNet\n  get vpiSize\n").is_err());
    assert!(Catalog::parse("  get vpiSize implemented t\n").is_err());
    assert!(Catalog::parse("node x 1 type=vpiNet\nnode x 1 type=vpiReg\n").is_err());
    let tie = "node a 1 members=leaf\n  get vpiSize implemented t\n\
               node b 1 members=leaf\n  get vpiSize unsupported no\n\
               node leaf 2 type=vpiNet\n";
    let tie = Catalog::parse(tie).expect("parse");
    assert!(
        tie.effective_rows("leaf").is_err(),
        "equal-distance conflict"
    );
}
