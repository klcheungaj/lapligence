//! Reader of the VPI object model catalog (`src/sim/rt/vpi_catalog.txt`) and
//! of the constants of the runtime VPI headers, shared by the catalog gate
//! (`runtime_vpi_catalog`) and the ADV-019 catalog walk. The file format is
//! documented in the catalog's own header comment.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;

/// The catalog path, relative to the repository root.
pub(crate) const CATALOG: &str = "src/sim/rt/vpi_catalog.txt";

/// The rows every object has (37.3.2, 37.3.3), which the catalog does not list.
pub(crate) const IMPLICIT: &[(&str, &str)] = &[
    ("get", "vpiType"),
    ("str", "vpiType"),
    ("get", "vpiLineNo"),
    ("str", "vpiFile"),
];

/// The row kinds: the routine family a row is queried with.
pub(crate) const KINDS: &[&str] = &["get", "str", "handle", "iterate", "func"];

/// The row statuses.
pub(crate) const STATUSES: &[&str] = &["implemented", "extension", "error", "unsupported", "n/a"];

/// A row's status and its detail (a test alias and, for some statuses, a reason).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) word: String,
    pub(crate) detail: String,
}

impl Status {
    /// Whether the runtime answers the row.
    pub(crate) fn answers(&self) -> bool {
        self.word == "implemented" || self.word == "extension"
    }

    /// The test alias of an implemented, extension or error row.
    pub(crate) fn test(&self) -> Option<&str> {
        matches!(self.word.as_str(), "implemented" | "extension" | "error")
            .then(|| self.detail.split_whitespace().next())
            .flatten()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) status: Status,
    pub(crate) line: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) clause: String,
    /// The object type of a type node (`NULL` for the NULL reference); None
    /// for a class node.
    pub(crate) type_name: Option<String>,
    pub(crate) members: Vec<String>,
    pub(crate) rows: Vec<Row>,
    pub(crate) line: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    /// Test alias to `<suite>::<test function>`.
    pub(crate) tests: BTreeMap<String, String>,
    pub(crate) nodes: Vec<Node>,
    index: HashMap<String, usize>,
}

fn status_of(words: &[&str]) -> Option<Status> {
    let (word, detail) = words.split_first()?;
    Some(Status {
        word: (*word).to_owned(),
        detail: detail.join(" "),
    })
}

impl Catalog {
    /// Parse catalog text; the error names the offending line.
    pub(crate) fn parse(text: &str) -> Result<Catalog, String> {
        let mut catalog = Catalog::default();
        let mut default: Option<Status> = None;
        for (number, line) in text.lines().enumerate() {
            let number = number + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            if line.starts_with("  ") {
                let node = catalog
                    .nodes
                    .last_mut()
                    .ok_or_else(|| format!("line {number}: row before any node"))?;
                if words.len() < 2 {
                    return Err(format!("line {number}: a row needs a kind and a name"));
                }
                let status = match status_of(&words[2..]).or_else(|| default.clone()) {
                    Some(status) => status,
                    None => {
                        return Err(format!(
                            "line {number}: row without a status in a node without a default"
                        ));
                    }
                };
                node.rows.push(Row {
                    kind: words[0].to_owned(),
                    name: words[1].to_owned(),
                    status,
                    line: number,
                });
                continue;
            }
            match words.first().copied() {
                Some("test") if words.len() == 3 => {
                    if catalog
                        .tests
                        .insert(words[1].to_owned(), words[2].to_owned())
                        .is_some()
                    {
                        return Err(format!("line {number}: duplicate test alias {}", words[1]));
                    }
                }
                Some("node") if words.len() >= 4 => {
                    let (type_name, members) = if let Some(t) = words[3].strip_prefix("type=") {
                        (Some(t.to_owned()), Vec::new())
                    } else if let Some(m) = words[3].strip_prefix("members=") {
                        (None, m.split(',').map(str::to_owned).collect())
                    } else {
                        return Err(format!("line {number}: node needs type= or members="));
                    };
                    default = match words.get(4) {
                        None => None,
                        Some(first) => {
                            let Some(word) = first.strip_prefix("default=") else {
                                return Err(format!("line {number}: unexpected {first}"));
                            };
                            let mut rest = vec![word];
                            rest.extend_from_slice(&words[5..]);
                            status_of(&rest)
                        }
                    };
                    let id = words[1].to_owned();
                    if catalog
                        .index
                        .insert(id.clone(), catalog.nodes.len())
                        .is_some()
                    {
                        return Err(format!("line {number}: duplicate node {id}"));
                    }
                    catalog.nodes.push(Node {
                        id,
                        clause: words[2].to_owned(),
                        type_name,
                        members,
                        rows: Vec::new(),
                        line: number,
                    });
                }
                _ => return Err(format!("line {number}: cannot parse `{line}`")),
            }
        }
        Ok(catalog)
    }

    /// Read and parse the checked-in catalog.
    pub(crate) fn load() -> Catalog {
        let text = read_source(CATALOG);
        Catalog::parse(&text).unwrap_or_else(|error| panic!("{CATALOG}: {error}"))
    }

    pub(crate) fn node(&self, id: &str) -> Option<&Node> {
        self.index.get(id).map(|&i| &self.nodes[i])
    }

    /// The rows an object of type node `id` has: its own and those of every
    /// class that contains it, transitively, the nearest declaration
    /// winning. Two classes at the same distance that give one row
    /// different statuses are an error.
    pub(crate) fn effective_rows(
        &self,
        id: &str,
    ) -> Result<BTreeMap<(String, String), Status>, String> {
        let mut parents: HashMap<&str, Vec<&str>> = HashMap::new();
        for node in &self.nodes {
            for member in &node.members {
                parents.entry(member).or_default().push(&node.id);
            }
        }
        let mut rows: BTreeMap<(String, String), (Status, usize)> = BTreeMap::new();
        let mut queue = VecDeque::from([(id, 0usize)]);
        let mut seen = Vec::new();
        while let Some((current, distance)) = queue.pop_front() {
            if seen.contains(&current) {
                continue;
            }
            seen.push(current);
            let node = self
                .node(current)
                .ok_or_else(|| format!("unknown node {current}"))?;
            for row in &node.rows {
                let key = (row.kind.clone(), row.name.clone());
                match rows.get(&key) {
                    None => {
                        rows.insert(key, (row.status.clone(), distance));
                    }
                    Some((status, d)) if *d == distance && status.word != row.status.word => {
                        return Err(format!(
                            "{id}: {} {} is {} and {} at the same distance",
                            row.kind, row.name, status.word, row.status.word
                        ));
                    }
                    Some(_) => {}
                }
            }
            for parent in parents.get(current).into_iter().flatten() {
                queue.push_back((parent, distance + 1));
            }
        }
        Ok(rows
            .into_iter()
            .map(|(key, (status, _))| (key, status))
            .collect())
    }
}

/// Read a repository file as text.
pub(crate) fn read_source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

/// C text with its comments replaced by spaces (newlines kept).
pub(crate) fn strip_c_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            let end = source[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |e| i + 2 + e + 2);
            out.extend(
                source[i..end]
                    .chars()
                    .map(|c| if c == '\n' { '\n' } else { ' ' }),
            );
            i = end;
        } else if bytes[i..].starts_with(b"//") {
            let end = source[i..].find('\n').map_or(bytes.len(), |e| i + e);
            i = end;
        } else if bytes[i] == b'"' || bytes[i] == b'\'' {
            // A string or character literal, which may hold `"`, `'` or `/*`.
            let quote = bytes[i];
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != quote {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(bytes.len());
            out.push_str(&source[i..end]);
            i = end;
        } else {
            let c = source[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// The object-like `#define`s of C text: name to value with whitespace removed.
pub(crate) fn defines(source: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in strip_c_comments(source).lines() {
        let Some(rest) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix("define") else {
            continue;
        };
        if !rest.starts_with([' ', '\t']) {
            continue;
        }
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if rest[end..].starts_with('(') {
            continue;
        }
        let value: String = rest[end..].chars().filter(|c| !c.is_whitespace()).collect();
        out.insert(rest[..end].to_owned(), value);
    }
    out
}

/// Evaluate a define value: integers, names of other defines, parentheses
/// and `|`, the only forms the VPI headers use.
pub(crate) fn evaluate(value: &str, table: &BTreeMap<String, String>) -> Option<i64> {
    fn go(value: &str, table: &BTreeMap<String, String>, depth: u32) -> Option<i64> {
        if depth > 16 {
            return None;
        }
        let mut v = value;
        while v.starts_with('(') && v.ends_with(')') && balanced(&v[1..v.len() - 1]) {
            v = &v[1..v.len() - 1];
        }
        let mut level = 0i32;
        for (i, c) in v.char_indices() {
            match c {
                '(' => level += 1,
                ')' => level -= 1,
                '|' if level == 0 => {
                    return Some(
                        go(&v[..i], table, depth + 1)? | go(&v[i + 1..], table, depth + 1)?,
                    );
                }
                _ => {}
            }
        }
        let (negative, digits) = match v.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, v),
        };
        let number = if let Some(hex) = digits
            .strip_prefix("0x")
            .or_else(|| digits.strip_prefix("0X"))
        {
            i64::from_str_radix(hex, 16).ok()
        } else if digits.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            digits.parse::<i64>().ok()
        } else {
            None
        };
        match number {
            Some(n) => Some(if negative { -n } else { n }),
            None if !negative => go(table.get(v)?, table, depth + 1),
            None => None,
        }
    }
    fn balanced(v: &str) -> bool {
        let mut level = 0i32;
        for c in v.chars() {
            match c {
                '(' => level += 1,
                ')' => {
                    level -= 1;
                    if level < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }
        level == 0
    }
    go(value, table, 0)
}

/// The defines of `vpi_user.h` and `sv_vpi_user.h`.
pub(crate) fn header_defines() -> BTreeMap<String, String> {
    let mut table = defines(&read_source("src/sim/rt/vpi_user.h"));
    table.extend(defines(&read_source("src/sim/rt/sv_vpi_user.h")));
    table
}

/// Every header constant that evaluates to an integer.
pub(crate) fn header_values() -> BTreeMap<String, i64> {
    let table = header_defines();
    table
        .keys()
        .filter_map(|name| Some((name.clone(), evaluate(&table[name], &table)?)))
        .collect()
}
