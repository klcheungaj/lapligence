//! Package and definition units for the isolated open-buffer token parse.
//!
//! An open buffer is compiled alone, so a package or a module, interface,
//! program, checker or primitive declared in another admitted project file
//! is unknown to it unless that file joins the parse. The project compile
//! sees every file: packages and definitions are design-wide names (IEEE
//! 1800-2009 3.13 a and b, 26.2), never scoped to a compilation unit. This
//! module selects, per request, the admitted compilation units that declare
//! the packages and definitions the buffer names, transitively through the
//! names those units use, so both serving paths resolve the same package
//! items, instances, interface ports and hierarchical names (`dut.sig`).
//!
//! Every package reference is written `name ::` (import and export items,
//! scoped names; 26.3, 26.6); a definition is named by a plain identifier
//! (an instantiation, an interface port type, `virtual ifc`, `bind`). One
//! conservative lexical pass per text finds the candidate names: comments,
//! strings and directives other than `` `include `` are skipped,
//! `` `include "name" `` operands are followed through the admitted headers,
//! and the text of macro bodies counts as written. A spurious name (a class
//! scope, an inactive branch, a signal spelled like a module) can only add a
//! unit to the parse; it never removes one. The facts of every admitted unit
//! and header are computed once per analysis commit; a request scans only
//! its own buffer.
//!
//! The selected units are library units, so only what the buffer
//! instantiates is elaborated; a testbench that instantiates the design
//! elaborates the design's hierarchy, as the project compile does.

use super::*;

/// Package and definition facts of one text from the lexical pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PackageScan {
    /// Names declared by `package [lifetime] name`.
    pub declares: Vec<String>,
    /// Names written directly before `::`.
    pub references: Vec<String>,
    /// Operands of `` `include "name" `` / `` `include <name> ``.
    pub includes: Vec<String>,
    /// Names declared by a `module`, `macromodule`, `interface`, `program`,
    /// `checker` or `primitive` header (`virtual interface` and `extern`
    /// prototypes declare nothing).
    pub definitions: Vec<String>,
    /// Identifiers that name a definition some admitted unit declares
    /// ([`definition_uses`]); empty until a [`PackageContext`] fills it.
    pub uses: Vec<String>,
}

/// Header words that declare a definition (A.1.2, A.1.3, A.5.1; 17.2).
const DEFINITION_KEYWORDS: &[&str] = &[
    "module",
    "macromodule",
    "interface",
    "program",
    "checker",
    "primitive",
];

#[derive(Debug, PartialEq, Eq)]
enum Lexeme<'a> {
    Identifier(&'a str),
    DoubleColon,
    Include(&'a str),
    Other,
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// The significant lexemes of a text, without comments, white space, string
/// contents and directive names other than `` `include ``. Streaming, so a
/// scan holds no per-token storage.
struct Lexemes<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Iterator for Lexemes<'a> {
    type Item = Lexeme<'a>;

    fn next(&mut self) -> Option<Lexeme<'a>> {
        let text = self.text;
        let bytes = text.as_bytes();
        let mut at = self.at;
        let lexeme = loop {
            let &byte = bytes.get(at)?;
            match byte {
                b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' => at += 1,
                b'/' if bytes.get(at + 1) == Some(&b'/') => {
                    while at < bytes.len() && bytes[at] != b'\n' {
                        at += 1;
                    }
                }
                b'/' if bytes.get(at + 1) == Some(&b'*') => {
                    at += 2;
                    while at < bytes.len()
                        && !(bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/'))
                    {
                        at += 1;
                    }
                    at = (at + 2).min(bytes.len());
                }
                b'"' => {
                    at += 1;
                    while at < bytes.len() && bytes[at] != b'"' && bytes[at] != b'\n' {
                        at += if bytes[at] == b'\\' { 2 } else { 1 };
                    }
                    at = (at + 1).min(bytes.len());
                    break Lexeme::Other;
                }
                b'\\' => {
                    // An escaped identifier runs to the next white space and
                    // names the text after the backslash (5.6.1).
                    let start = at + 1;
                    at = start;
                    while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
                        at += 1;
                    }
                    break if at > start {
                        Lexeme::Identifier(&text[start..at])
                    } else {
                        Lexeme::Other
                    };
                }
                b'`' => {
                    let start = at + 1;
                    at = start;
                    while at < bytes.len() && is_identifier_part(bytes[at]) {
                        at += 1;
                    }
                    if &text[start..at] != "include" {
                        break Lexeme::Other;
                    }
                    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t') {
                        at += 1;
                    }
                    let close = match bytes.get(at) {
                        Some(b'"') => b'"',
                        Some(b'<') => b'>',
                        _ => break Lexeme::Other,
                    };
                    let start = at + 1;
                    at = start;
                    while at < bytes.len() && bytes[at] != close && bytes[at] != b'\n' {
                        at += 1;
                    }
                    if bytes.get(at) == Some(&close) && at > start {
                        at += 1;
                        break Lexeme::Include(&text[start..at - 1]);
                    }
                    break Lexeme::Other;
                }
                b':' if bytes.get(at + 1) == Some(&b':') => {
                    at += 2;
                    break Lexeme::DoubleColon;
                }
                _ if is_identifier_start(byte) => {
                    let start = at;
                    while at < bytes.len() && is_identifier_part(bytes[at]) {
                        at += 1;
                    }
                    break Lexeme::Identifier(&text[start..at]);
                }
                b'$' => {
                    // System names and `$unit` never name a package.
                    at += 1;
                    while at < bytes.len() && is_identifier_part(bytes[at]) {
                        at += 1;
                    }
                    break Lexeme::Other;
                }
                _ => {
                    // Skip a whole UTF-8 sequence so slicing stays on boundaries.
                    at += text[at..].chars().next().map_or(1, char::len_utf8);
                    break Lexeme::Other;
                }
            }
        };
        self.at = at;
        Some(lexeme)
    }
}

fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|known| known == name) {
        names.push(name.to_owned());
    }
}

/// Scan one text for package and definition declarations, `name ::`
/// references and include operands.
pub(crate) fn scan_packages(text: &str) -> PackageScan {
    let mut declares = HashSet::new();
    let mut references = HashSet::new();
    let mut includes = HashSet::new();
    let mut definitions = HashSet::new();
    // The identifier just before the current lexeme, and how far a
    // `package [lifetime] name` or definition header has been read.
    let mut previous: Option<&str> = None;
    let mut package_header = Header::None;
    let mut definition_header = Header::None;
    for lexeme in (Lexemes { text, at: 0 }) {
        package_header = match (package_header, &lexeme) {
            (_, Lexeme::Identifier("package")) => Header::Keyword,
            (Header::Keyword, Lexeme::Identifier("automatic" | "static")) => Header::Lifetime,
            (Header::Keyword | Header::Lifetime, Lexeme::Identifier(name)) => {
                declares.insert(*name);
                Header::None
            }
            _ => Header::None,
        };
        definition_header = match (definition_header, &lexeme) {
            // `virtual interface ifc` names an interface (25.9) and an
            // `extern module` prototype (23.2.1) declares none.
            (_, Lexeme::Identifier(word)) if DEFINITION_KEYWORDS.contains(word) => {
                if matches!(previous, Some("virtual" | "extern")) {
                    Header::None
                } else {
                    Header::Keyword
                }
            }
            (Header::Keyword, Lexeme::Identifier("automatic" | "static")) => Header::Lifetime,
            (Header::Keyword | Header::Lifetime, Lexeme::Identifier(name)) => {
                definitions.insert(*name);
                Header::None
            }
            _ => Header::None,
        };
        match lexeme {
            Lexeme::DoubleColon => {
                if let Some(name) = previous {
                    references.insert(name);
                }
            }
            Lexeme::Include(name) => {
                includes.insert(name);
            }
            _ => {}
        }
        previous = match lexeme {
            Lexeme::Identifier(name) => Some(name),
            _ => None,
        };
    }
    PackageScan {
        declares: sorted_names(declares),
        references: sorted_names(references),
        includes: sorted_names(includes),
        definitions: sorted_names(definitions),
        uses: Vec::new(),
    }
}

fn sorted_names(names: HashSet<&str>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().map(str::to_owned).collect();
    names.sort_unstable();
    names
}

/// The identifiers of `text` that name one of the `known` definitions, from
/// the same lexical pass as [`scan_packages`]: every identifier counts, so a
/// signal spelled like a module only adds that module's unit.
fn definition_uses(text: &str, known: &HashMap<String, Vec<usize>>) -> Vec<String> {
    if known.is_empty() {
        return Vec::new();
    }
    let mut uses = HashSet::new();
    for lexeme in (Lexemes { text, at: 0 }) {
        if let Lexeme::Identifier(name) = lexeme {
            if known.contains_key(name) {
                uses.insert(name);
            }
        }
    }
    sorted_names(uses)
}

#[derive(Debug, Clone, Copy)]
enum Header {
    None,
    Keyword,
    Lifetime,
}

/// One admitted compilation unit that declares at least one package or
/// definition, with the facts of the headers it includes folded in.
#[derive(Debug)]
struct ContextUnit {
    name: String,
    text: Arc<String>,
    /// Packages the unit names (`name ::`).
    references: Vec<String>,
    /// Definitions of other units the unit names ([`definition_uses`]).
    uses: Vec<String>,
}

/// Package- and definition-declaring units and header facts of one admitted
/// project compile.
#[derive(Debug, Default)]
pub struct PackageContext {
    units: Vec<ContextUnit>,
    by_package: HashMap<String, Vec<usize>>,
    by_definition: HashMap<String, Vec<usize>>,
    headers: Vec<(String, PackageScan)>,
}

/// Whether the admitted name `path` is what `` `include "include" `` names:
/// the operand matches whole trailing path components. A looser match only
/// follows one header too many.
fn include_names(path: &str, include: &str) -> bool {
    let path = path.replace('\\', "/");
    let include = include.replace('\\', "/");
    let include = include.trim_start_matches("./");
    path == include
        || path
            .strip_suffix(include)
            .is_some_and(|prefix| prefix.ends_with('/'))
}

impl PackageContext {
    /// Scan the admitted compilation units and include-only headers of one
    /// project compile. Only units that declare a package or a definition
    /// (directly or in a header they include) are kept; their texts are the
    /// shared admission snapshots. Each unit and header keeps only the
    /// identifiers that name a definition some kept unit declares.
    pub fn new(
        units: impl IntoIterator<Item = (String, Arc<String>)>,
        headers: &[(String, Arc<String>)],
    ) -> Self {
        let mut context = PackageContext {
            headers: headers
                .iter()
                .map(|(name, text)| (name.clone(), scan_packages(text)))
                .collect(),
            ..PackageContext::default()
        };
        let mut kept = Vec::new();
        for (name, text) in units {
            let scan = context.with_headers(scan_packages(&text));
            if scan.declares.is_empty() && scan.definitions.is_empty() {
                continue;
            }
            let index = kept.len();
            for package in &scan.declares {
                context
                    .by_package
                    .entry(package.clone())
                    .or_default()
                    .push(index);
            }
            for definition in &scan.definitions {
                context
                    .by_definition
                    .entry(definition.clone())
                    .or_default()
                    .push(index);
            }
            kept.push((name, text, scan));
        }
        // Definition uses need every declaration first.
        for (header, (_, text)) in context.headers.iter_mut().zip(headers) {
            header.1.uses = definition_uses(text, &context.by_definition);
        }
        for (name, text, scan) in kept {
            let mut uses = definition_uses(&text, &context.by_definition);
            for header in context.included_headers(&scan.includes) {
                for definition in &context.headers[header].1.uses {
                    push_unique(&mut uses, definition);
                }
            }
            context.units.push(ContextUnit {
                name,
                text,
                references: scan.references,
                uses,
            });
        }
        context
    }

    /// Whether `other` holds the same package- and definition-declaring
    /// units, in the same order and with the same texts, so every selection
    /// from either is the same. Header facts derive from the include
    /// buffers, which [`IncludeContext::same_inputs`] compares.
    pub fn same_units(&self, other: &PackageContext) -> bool {
        self.units.len() == other.units.len()
            && self.units.iter().zip(&other.units).all(|(a, b)| {
                a.name == b.name && (Arc::ptr_eq(&a.text, &b.text) || a.text == b.text)
            })
    }

    /// The indices of the admitted headers `includes` names, transitively.
    fn included_headers(&self, includes: &[String]) -> Vec<usize> {
        let mut visited = vec![false; self.headers.len()];
        let mut found = Vec::new();
        let mut pending = includes.to_vec();
        while let Some(include) = pending.pop() {
            for (index, (path, header)) in self.headers.iter().enumerate() {
                if visited[index] || !include_names(path, &include) {
                    continue;
                }
                visited[index] = true;
                found.push(index);
                pending.extend(header.includes.iter().cloned());
            }
        }
        found
    }

    /// `scan` with the declarations, references and definition uses of
    /// every admitted header it includes, transitively.
    fn with_headers(&self, mut scan: PackageScan) -> PackageScan {
        for index in self.included_headers(&scan.includes) {
            let header = &self.headers[index].1;
            for name in &header.declares {
                push_unique(&mut scan.declares, name);
            }
            for name in &header.references {
                push_unique(&mut scan.references, name);
            }
            for name in &header.definitions {
                push_unique(&mut scan.definitions, name);
            }
            for name in &header.uses {
                push_unique(&mut scan.uses, name);
            }
        }
        scan
    }

    /// The units an isolated parse of `name` (with `text`) needs, in
    /// admission order: the declarers of every package and definition the
    /// buffer names, transitively through the names the selected units use.
    /// The buffer's own unit and the packages and definitions it declares
    /// itself are never added, so its unsaved text stays the only version of
    /// them.
    pub fn sources_for(&self, name: &str, text: &str) -> Vec<compile::OwnedSource> {
        if self.units.is_empty() {
            return Vec::new();
        }
        let mut scan = scan_packages(text);
        scan.uses = definition_uses(text, &self.by_definition);
        let scan = self.with_headers(scan);
        let mut selected = vec![false; self.units.len()];
        let mut seen_packages: HashSet<&str> = scan.declares.iter().map(String::as_str).collect();
        let mut seen_definitions: HashSet<&str> =
            scan.definitions.iter().map(String::as_str).collect();
        // `(name, names a definition)`.
        let mut pending: Vec<(&str, bool)> = scan
            .references
            .iter()
            .map(|package| (package.as_str(), false))
            .chain(
                scan.uses
                    .iter()
                    .map(|definition| (definition.as_str(), true)),
            )
            .collect();
        while let Some((wanted, definition)) = pending.pop() {
            let (seen, declarers) = if definition {
                (&mut seen_definitions, &self.by_definition)
            } else {
                (&mut seen_packages, &self.by_package)
            };
            if !seen.insert(wanted) {
                continue;
            }
            for &index in declarers.get(wanted).into_iter().flatten() {
                let unit = &self.units[index];
                if selected[index] || unit.name == name {
                    continue;
                }
                selected[index] = true;
                pending.extend(
                    unit.references
                        .iter()
                        .map(|package| (package.as_str(), false)),
                );
                pending.extend(
                    unit.uses
                        .iter()
                        .map(|definition| (definition.as_str(), true)),
                );
            }
        }
        self.units
            .iter()
            .zip(selected)
            .filter(|(_, selected)| *selected)
            .map(|(unit, _)| {
                compile::OwnedSource::compilation_unit(unit.name.clone(), unit.text.as_str())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(values: &[String]) -> Vec<&str> {
        values.iter().map(String::as_str).collect()
    }

    #[test]
    fn scan_finds_declarations_references_and_includes_outside_comments_and_strings() {
        let scan = scan_packages(
            "`include \"defs.svh\"\n`include <sys.svh>\n\
             package automatic a_pkg; import b_pkg::*; endpackage\n\
             package c_pkg; endpackage : c_pkg\n\
             module m import d_pkg::X; #(int W = e_pkg :: W) ();\n\
             // f_pkg::x\n/* g_pkg::y */ string s = \"h_pkg::z\";\n\
             localparam int Q = $unit::Q + \\esc_pkg ::V;\n\
             `define USE i_pkg::T\nendmodule\n",
        );
        assert_eq!(names(&scan.declares), ["a_pkg", "c_pkg"]);
        assert_eq!(
            names(&scan.references),
            ["b_pkg", "d_pkg", "e_pkg", "esc_pkg", "i_pkg"]
        );
        assert_eq!(names(&scan.includes), ["defs.svh", "sys.svh"]);
    }

    #[test]
    fn scan_survives_unterminated_text_and_non_ascii() {
        for text in ["/* open", "\"open", "`include \"open", "pkg:", "é::x", "\\"] {
            let _ = scan_packages(text);
        }
        assert_eq!(names(&scan_packages("ü a::b").references), ["a"]);
    }

    fn unit(name: &str, text: &str) -> (String, Arc<String>) {
        (name.to_owned(), Arc::new(text.to_owned()))
    }

    fn selected(context: &PackageContext, name: &str, text: &str) -> Vec<String> {
        context
            .sources_for(name, text)
            .into_iter()
            .map(|source| source.name)
            .collect()
    }

    #[test]
    fn sources_for_selects_declarers_transitively_and_skips_the_buffer() {
        let headers = [unit(
            "/ws/inc/deep_pkg.svh",
            "package deep_pkg; endpackage\n",
        )];
        let context = PackageContext::new(
            [
                unit("/ws/a.sv", "package a_pkg; import b_pkg::*; endpackage\n"),
                unit("/ws/b.sv", "package b_pkg; endpackage\n"),
                unit("/ws/c.sv", "package c_pkg; endpackage\n"),
                unit("/ws/d.sv", "`include \"inc/deep_pkg.svh\"\n"),
                unit("/ws/plain.sv", "module plain; endmodule\n"),
                unit("/ws/self.sv", "package self_pkg; endpackage\n"),
            ],
            &headers,
        );
        assert_eq!(
            selected(
                &context,
                "/ws/top.sv",
                "module t; import a_pkg::*; endmodule"
            ),
            ["/ws/a.sv", "/ws/b.sv"]
        );
        assert_eq!(
            selected(
                &context,
                "/ws/top.sv",
                "module t; int x = deep_pkg::V; endmodule"
            ),
            ["/ws/d.sv"]
        );
        assert!(selected(&context, "/ws/top.sv", "module t; endmodule").is_empty());
        // The buffer's own unit and a package the buffer declares itself are
        // never added.
        assert!(selected(&context, "/ws/self.sv", "package self_pkg; endpackage").is_empty());
        assert!(selected(
            &context,
            "/ws/top.sv",
            "package c_pkg; endpackage module t; import c_pkg::*; endmodule"
        )
        .is_empty());
    }

    #[test]
    fn buffer_includes_are_followed_through_admitted_headers() {
        let headers = [
            unit("/ws/inc/uses.svh", "`include \"more.svh\"\n"),
            unit("/ws/inc/more.svh", "`define M x_pkg::V\n"),
        ];
        let context =
            PackageContext::new([unit("/ws/x.sv", "package x_pkg; endpackage\n")], &headers);
        assert_eq!(
            selected(
                &context,
                "/ws/top.sv",
                "`include \"uses.svh\"\nmodule t; endmodule"
            ),
            ["/ws/x.sv"]
        );
        assert!(include_names("/ws/inc/uses.svh", "./uses.svh"));
        assert!(!include_names("/ws/inc/xuses.svh", "uses.svh"));
    }

    #[test]
    fn scan_finds_definition_headers_but_not_virtual_interfaces_or_prototypes() {
        let scan = scan_packages(
            "module automatic m_a; endmodule\nmacromodule m_b; endmodule\n\
             interface i_a(input logic c); endinterface\nprogram static p_a; endprogram\n\
             checker c_a; endchecker\nprimitive u_a(o, i); endprimitive\n\
             module m_c; virtual interface i_x vif; virtual i_y v2; endmodule\n\
             extern module m_x(input a);\n// module m_comment;\n",
        );
        assert_eq!(
            names(&scan.definitions),
            ["c_a", "i_a", "m_a", "m_b", "m_c", "p_a", "u_a"]
        );
        assert!(scan.declares.is_empty());
    }

    #[test]
    fn sources_for_selects_definition_declarers_transitively() {
        let headers = [unit("/ws/inc/body.svh", "leaf_m u_leaf();\n")];
        let context = PackageContext::new(
            [
                unit("/ws/dut.sv", "module dut_m; mid_m u_mid(); endmodule\n"),
                unit(
                    "/ws/mid.sv",
                    "module mid_m;\n`include \"inc/body.svh\"\nendmodule\n",
                ),
                unit("/ws/leaf.sv", "module leaf_m; import l_pkg::*; endmodule\n"),
                unit("/ws/l_pkg.sv", "package l_pkg; endpackage\n"),
                unit("/ws/ifc.sv", "interface bus_if; logic req; endinterface\n"),
                unit("/ws/other.sv", "module other_m; endmodule\n"),
                unit("/ws/tb.sv", "module tb; dut_m dut(); endmodule\n"),
            ],
            &headers,
        );
        // An instance pulls in its definition and, through the included
        // header and the package import, everything below it.
        assert_eq!(
            selected(
                &context,
                "/ws/tb.sv",
                "module tb; dut_m dut(); initial $display(dut.u_mid.u_leaf); endmodule"
            ),
            ["/ws/dut.sv", "/ws/mid.sv", "/ws/leaf.sv", "/ws/l_pkg.sv"]
        );
        // Interface port types and `virtual` interface types name the
        // interface; unrelated definitions stay out.
        assert_eq!(
            selected(
                &context,
                "/ws/top.sv",
                "module top(bus_if.mp b); virtual bus_if v; endmodule"
            ),
            ["/ws/ifc.sv"]
        );
        // A definition the buffer declares itself is never taken from the
        // committed units, and the buffer's own unit is never added.
        assert!(selected(
            &context,
            "/ws/x.sv",
            "module dut_m; endmodule\nmodule t; dut_m d(); endmodule"
        )
        .is_empty());
        assert!(
            selected(&context, "/ws/dut.sv", "module tb2; dut_m d(); endmodule")
                .iter()
                .all(|n| n != "/ws/dut.sv")
        );
    }
}
