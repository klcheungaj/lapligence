//! Package units for the isolated open-buffer token parse.
//!
//! An open buffer is compiled alone, so a package declared in another
//! admitted project file is unknown to it unless that file joins the parse.
//! The project compile sees every file: a package is a design-wide name
//! (IEEE 1800-2009 3.13 b, 26.2), never scoped to a compilation unit. This
//! module selects, per request, the admitted compilation units that declare
//! the packages the buffer names, transitively through the packages those
//! units name, so both serving paths resolve the same package items.
//!
//! Every package reference is written `name ::` (import and export items,
//! scoped names; 26.3, 26.6), so one conservative lexical pass per text finds
//! the candidate names: comments, strings and directives other than
//! `` `include `` are skipped, `` `include "name" `` operands are followed
//! through the admitted headers, and the text of macro bodies counts as
//! written. A spurious name (a class scope, an inactive branch) can only add
//! a unit to the parse; it never removes one. The facts of every admitted
//! unit and header are computed once per analysis commit; a request scans
//! only its own buffer.

use super::*;

/// Package facts of one text from the lexical pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PackageScan {
    /// Names declared by `package [lifetime] name`.
    pub declares: Vec<String>,
    /// Names written directly before `::`.
    pub references: Vec<String>,
    /// Operands of `` `include "name" `` / `` `include <name> ``.
    pub includes: Vec<String>,
}

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

/// Scan one text for package declarations, `name ::` references and
/// include operands.
pub(crate) fn scan_packages(text: &str) -> PackageScan {
    let mut declares = HashSet::new();
    let mut references = HashSet::new();
    let mut includes = HashSet::new();
    // The identifier just before the current lexeme, and how far a
    // `package [lifetime] name` header has been read.
    let mut previous: Option<&str> = None;
    let mut package_header = PackageHeader::None;
    for lexeme in (Lexemes { text, at: 0 }) {
        package_header = match (package_header, &lexeme) {
            (_, Lexeme::Identifier("package")) => PackageHeader::Keyword,
            (PackageHeader::Keyword, Lexeme::Identifier("automatic" | "static")) => {
                PackageHeader::Lifetime
            }
            (PackageHeader::Keyword | PackageHeader::Lifetime, Lexeme::Identifier(name)) => {
                declares.insert(*name);
                PackageHeader::None
            }
            _ => PackageHeader::None,
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
    let sorted = |names: HashSet<&str>| {
        let mut names: Vec<String> = names.into_iter().map(str::to_owned).collect();
        names.sort_unstable();
        names
    };
    PackageScan {
        declares: sorted(declares),
        references: sorted(references),
        includes: sorted(includes),
    }
}

#[derive(Debug, Clone, Copy)]
enum PackageHeader {
    None,
    Keyword,
    Lifetime,
}

/// One admitted compilation unit that declares at least one package, with
/// the facts of the headers it includes folded in.
#[derive(Debug)]
struct PackageUnit {
    name: String,
    text: Arc<String>,
    references: Vec<String>,
}

/// Package-declaring units and header facts of one admitted project compile.
#[derive(Debug, Default)]
pub struct PackageContext {
    units: Vec<PackageUnit>,
    by_package: HashMap<String, Vec<usize>>,
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
    /// project compile. Only units that declare a package (directly or in a
    /// header they include) are kept; their texts are the shared admission
    /// snapshots.
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
        for (name, text) in units {
            let scan = context.with_headers(scan_packages(&text));
            if scan.declares.is_empty() {
                continue;
            }
            let index = context.units.len();
            for package in &scan.declares {
                context
                    .by_package
                    .entry(package.clone())
                    .or_default()
                    .push(index);
            }
            context.units.push(PackageUnit {
                name,
                text,
                references: scan.references,
            });
        }
        context
    }

    /// `scan` with the declarations and references of every admitted header
    /// it includes, transitively.
    fn with_headers(&self, mut scan: PackageScan) -> PackageScan {
        let mut visited = vec![false; self.headers.len()];
        let mut pending = std::mem::take(&mut scan.includes);
        while let Some(include) = pending.pop() {
            for (index, (path, header)) in self.headers.iter().enumerate() {
                if visited[index] || !include_names(path, &include) {
                    continue;
                }
                visited[index] = true;
                for name in &header.declares {
                    push_unique(&mut scan.declares, name);
                }
                for name in &header.references {
                    push_unique(&mut scan.references, name);
                }
                pending.extend(header.includes.iter().cloned());
            }
        }
        scan
    }

    /// The package-declaring units an isolated parse of `name` (with `text`)
    /// needs, in admission order: the declarers of every package the buffer
    /// names, transitively. The buffer's own unit and the packages it
    /// declares itself are never added, so its unsaved text stays the only
    /// version of them.
    pub fn sources_for(&self, name: &str, text: &str) -> Vec<compile::OwnedSource> {
        if self.units.is_empty() {
            return Vec::new();
        }
        let scan = self.with_headers(scan_packages(text));
        let mut selected = vec![false; self.units.len()];
        let mut seen: HashSet<&str> = scan.declares.iter().map(String::as_str).collect();
        let mut pending: Vec<&str> = scan.references.iter().map(String::as_str).collect();
        while let Some(package) = pending.pop() {
            if !seen.insert(package) {
                continue;
            }
            for &index in self.by_package.get(package).into_iter().flatten() {
                let unit = &self.units[index];
                if selected[index] || unit.name == name {
                    continue;
                }
                selected[index] = true;
                pending.extend(unit.references.iter().map(String::as_str));
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
}
