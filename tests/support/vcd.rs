//! Independent four-state VCD reader for waveform acceptance.
//!
//! The grammar is IEEE 1364-2001 Syntax 18-8 (IEEE 1800-2009 Syntax 21-20)
//! with the value formats of 18.2.2 / 21.7.2.2. The reader shares nothing
//! with the runtime writer: it tokenizes on white space, checks the
//! declaration and simulation commands, balances scopes, resolves every value
//! change to declared identifier codes and left-extends vectors by Table 83.
//! Checks are deliberately strict where the standard is permissive, so a
//! malformed llg file fails here rather than in a viewer:
//!
//! - timestamps strictly increase (llg never repeats one);
//! - a reference is declared once per scope, and every use of one identifier
//!   code has the same type and width (an alias);
//! - a `$dumpoff` checkpoint holds only `x` values (18.1.3), and a checkpoint
//!   never contains a timestamp.
#![allow(dead_code)]

use std::collections::BTreeMap;

/// A `$var` declaration with its dotted scope path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Var {
    pub(crate) kind: String,
    pub(crate) width: u32,
    pub(crate) code: String,
    pub(crate) path: String,
}

/// The simulation command a value change belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Plain,
    DumpVars,
    DumpAll,
    DumpOn,
    DumpOff,
}

/// One value change. `value` is the full declared width for a vector
/// (lowercase `0 1 x z`), one character for a scalar, or the real text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) time: u64,
    pub(crate) code: String,
    pub(crate) value: String,
    pub(crate) section: Section,
    /// Index into [`Vcd::sections`] for a checkpoint value.
    pub(crate) checkpoint: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vcd {
    pub(crate) date: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) timescale: Option<String>,
    pub(crate) vars: Vec<Var>,
    /// `$scope` paths in declaration order, with their scope type.
    pub(crate) scopes: Vec<(String, String)>,
    pub(crate) changes: Vec<Change>,
    /// Simulation commands in file order with their time.
    pub(crate) sections: Vec<(u64, Section)>,
    /// `$comment` texts after `$enddefinitions`, with the current time.
    pub(crate) comments: Vec<(u64, String)>,
    pub(crate) times: Vec<u64>,
}

const SCOPE_TYPES: &[&str] = &["module", "task", "function", "begin", "fork"];
const VAR_TYPES: &[&str] = &[
    "event",
    "integer",
    "parameter",
    "real",
    "realtime",
    "reg",
    "supply0",
    "supply1",
    "time",
    "tri",
    "triand",
    "trior",
    "trireg",
    "tri0",
    "tri1",
    "wand",
    "wire",
    "wor",
];

/// Collect the tokens up to the closing `$end` of a command.
fn until_end<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    command: &str,
) -> Result<Vec<&'a str>, String> {
    let mut body = Vec::new();
    for token in tokens.by_ref() {
        if token == "$end" {
            return Ok(body);
        }
        body.push(token);
    }
    Err(format!("{command} has no $end"))
}

fn valid_code(code: &str) -> bool {
    !code.is_empty() && code.bytes().all(|byte| (33..=126).contains(&byte))
}

/// Left-extend `bits` to `width` by IEEE 1364-2001 Table 83.
fn left_extend(bits: &str, width: u32) -> Result<String, String> {
    let width = width as usize;
    let bits = bits.to_ascii_lowercase();
    if bits.is_empty() || !bits.bytes().all(|bit| b"01xz".contains(&bit)) {
        return Err(format!("invalid binary value `{bits}`"));
    }
    if bits.len() > width {
        return Err(format!("value `{bits}` is wider than {width} bits"));
    }
    let fill = match bits.as_bytes()[0] {
        b'x' => 'x',
        b'z' => 'z',
        _ => '0',
    };
    Ok(std::iter::repeat_n(fill, width - bits.len())
        .chain(bits.chars())
        .collect())
}

pub(crate) fn parse(text: &str) -> Result<Vcd, String> {
    let mut vcd = Vcd::default();
    let mut tokens = text.split_ascii_whitespace();
    let mut scope: Vec<String> = Vec::new();
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    let mut codes: BTreeMap<String, (String, u32)> = BTreeMap::new();
    loop {
        let Some(token) = tokens.next() else {
            return Err("missing $enddefinitions".to_owned());
        };
        match token {
            "$comment" => {
                until_end(&mut tokens, token)?;
            }
            "$date" => vcd.date = Some(until_end(&mut tokens, token)?.join(" ")),
            "$version" => vcd.version = Some(until_end(&mut tokens, token)?.join(" ")),
            "$timescale" => {
                let body = until_end(&mut tokens, token)?.concat();
                let unit_at = body
                    .find(|c: char| !c.is_ascii_digit())
                    .ok_or_else(|| format!("$timescale `{body}` has no unit"))?;
                let (number, unit) = body.split_at(unit_at);
                if !["1", "10", "100"].contains(&number)
                    || !["s", "ms", "us", "ns", "ps", "fs"].contains(&unit)
                {
                    return Err(format!("invalid $timescale `{body}`"));
                }
                vcd.timescale = Some(body);
            }
            "$scope" => {
                let body = until_end(&mut tokens, token)?;
                let [kind, name] = body.as_slice() else {
                    return Err(format!("invalid $scope {body:?}"));
                };
                if !SCOPE_TYPES.contains(kind) {
                    return Err(format!("invalid scope type `{kind}`"));
                }
                scope.push((*name).to_owned());
                vcd.scopes.push((scope.join("."), (*kind).to_owned()));
            }
            "$upscope" => {
                if !until_end(&mut tokens, token)?.is_empty() {
                    return Err("$upscope takes no text".to_owned());
                }
                scope.pop().ok_or("$upscope without an open scope")?;
            }
            "$var" => {
                let body = until_end(&mut tokens, token)?;
                let [kind, width, code, reference] = body.as_slice() else {
                    return Err(format!("unsupported $var {body:?}"));
                };
                if !VAR_TYPES.contains(kind) {
                    return Err(format!("invalid var type `{kind}`"));
                }
                let width: u32 = width
                    .parse()
                    .ok()
                    .filter(|width| *width > 0)
                    .ok_or_else(|| format!("invalid $var size `{width}`"))?;
                if !valid_code(code) {
                    return Err(format!("invalid identifier code `{code}`"));
                }
                if scope.is_empty() {
                    return Err(format!("$var `{reference}` outside a scope"));
                }
                if let Some((first_kind, first_width)) = codes.get(*code) {
                    if first_kind != kind || *first_width != width {
                        return Err(format!("identifier code `{code}` reused with another type"));
                    }
                }
                codes.insert((*code).to_owned(), ((*kind).to_owned(), width));
                let path = format!("{}.{reference}", scope.join("."));
                if declared.insert(path.clone(), (*code).to_owned()).is_some() {
                    return Err(format!("`{path}` declared twice"));
                }
                vcd.vars.push(Var {
                    kind: (*kind).to_owned(),
                    width,
                    code: (*code).to_owned(),
                    path,
                });
            }
            "$enddefinitions" => {
                if !until_end(&mut tokens, token)?.is_empty() {
                    return Err("$enddefinitions takes no text".to_owned());
                }
                break;
            }
            other => return Err(format!("unexpected declaration token `{other}`")),
        }
    }
    if !scope.is_empty() {
        return Err(format!("unclosed scope {scope:?}"));
    }

    let mut time: Option<u64> = None;
    let mut section = Section::Plain;
    let mut tokens = tokens.peekable();
    while let Some(token) = tokens.next() {
        let now = || time.ok_or_else(|| format!("`{token}` before the first timestamp"));
        let opened = match token {
            "$dumpvars" => Some(Section::DumpVars),
            "$dumpall" => Some(Section::DumpAll),
            "$dumpon" => Some(Section::DumpOn),
            "$dumpoff" => Some(Section::DumpOff),
            _ => None,
        };
        if let Some(opened) = opened {
            if section != Section::Plain {
                return Err(format!("`{token}` inside another checkpoint"));
            }
            section = opened;
            vcd.sections.push((now()?, opened));
            continue;
        }
        if token == "$end" {
            if section == Section::Plain {
                return Err("`$end` outside a checkpoint".to_owned());
            }
            section = Section::Plain;
            continue;
        }
        if token == "$comment" {
            if section != Section::Plain {
                return Err("$comment inside a checkpoint".to_owned());
            }
            let text = until_end(&mut tokens, token)?.join(" ");
            vcd.comments.push((time.unwrap_or(0), text));
            continue;
        }
        if let Some(stamp) = token.strip_prefix('#') {
            if section != Section::Plain {
                return Err(format!("timestamp `{token}` inside a checkpoint"));
            }
            let stamp: u64 = stamp
                .parse()
                .map_err(|_| format!("invalid timestamp `{token}`"))?;
            if time.is_some_and(|previous| stamp <= previous) {
                return Err(format!("timestamp #{stamp} does not increase"));
            }
            time = Some(stamp);
            vcd.times.push(stamp);
            continue;
        }
        let first = token.as_bytes()[0];
        let (value, code) = match first {
            b'b' | b'B' | b'r' | b'R' => {
                let code = tokens
                    .next()
                    .ok_or_else(|| format!("`{token}` has no identifier code"))?;
                (&token[1..], code)
            }
            b'0' | b'1' | b'x' | b'X' | b'z' | b'Z' => (&token[..1], &token[1..]),
            _ => return Err(format!("unexpected simulation token `{token}`")),
        };
        let (kind, width) = codes
            .get(code)
            .ok_or_else(|| format!("value change for undeclared code `{code}`"))?;
        let value = match first {
            b'r' | b'R' => {
                if kind != "real" && kind != "realtime" {
                    return Err(format!("real value for {kind} code `{code}`"));
                }
                value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid real `{value}`"))?;
                value.to_owned()
            }
            b'b' | b'B' => {
                if kind == "real" || kind == "realtime" {
                    return Err(format!("binary value for real code `{code}`"));
                }
                left_extend(value, *width)?
            }
            _ => {
                if *width != 1 {
                    return Err(format!("scalar value for {width}-bit code `{code}`"));
                }
                value.to_ascii_lowercase()
            }
        };
        if section == Section::DumpOff
            && !(value.bytes().all(|bit| bit == b'x') || value.eq_ignore_ascii_case("nan"))
        {
            return Err(format!("$dumpoff value `{value}` for `{code}` is not x"));
        }
        vcd.changes.push(Change {
            time: now()?,
            code: code.to_owned(),
            value,
            section,
            checkpoint: (section != Section::Plain).then(|| vcd.sections.len() - 1),
        });
    }
    if section != Section::Plain {
        return Err("unterminated checkpoint at end of file".to_owned());
    }
    Ok(vcd)
}

impl Vcd {
    /// The declaration of `path` (dotted, with the writer's escapes).
    pub(crate) fn var(&self, path: &str) -> &Var {
        self.vars
            .iter()
            .find(|var| var.path == path)
            .unwrap_or_else(|| panic!("`{path}` is not declared; catalog: {:?}", self.paths()))
    }

    /// Every declared path, sorted.
    pub(crate) fn paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.vars.iter().map(|var| var.path.as_str()).collect();
        paths.sort_unstable();
        paths
    }

    /// Value changes of `path` in file order, as `(time, value)`.
    pub(crate) fn changes_of(&self, path: &str) -> Vec<(u64, String)> {
        let code = &self.var(path).code;
        self.changes
            .iter()
            .filter(|change| &change.code == code)
            .map(|change| (change.time, change.value.clone()))
            .collect()
    }

    /// The value of `path` at the end of each time step with a record: the
    /// last record of the step, which a reader displays.
    pub(crate) fn step_values(&self, path: &str) -> Vec<(u64, String)> {
        let mut values: Vec<(u64, String)> = Vec::new();
        for (time, value) in self.changes_of(path) {
            match values.last_mut() {
                Some((last, slot)) if *last == time => *slot = value,
                _ => values.push((time, value)),
            }
        }
        values
    }

    /// The largest number of records `path` has outside checkpoints within
    /// one time step.
    pub(crate) fn max_plain_records_per_step(&self, path: &str) -> usize {
        let code = &self.var(path).code;
        let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
        for change in &self.changes {
            if &change.code == code && change.section == Section::Plain {
                *counts.entry(change.time).or_default() += 1;
            }
        }
        counts.into_values().max().unwrap_or(0)
    }

    /// The checkpoints of one kind in file order, as `(time, [(code, value)])`.
    pub(crate) fn checkpoints(&self, kind: Section) -> Vec<(u64, Vec<(String, String)>)> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, (_, section))| *section == kind)
            .map(|(index, (time, _))| {
                let values = self
                    .changes
                    .iter()
                    .filter(|change| change.checkpoint == Some(index))
                    .map(|change| (change.code.clone(), change.value.clone()))
                    .collect();
                (*time, values)
            })
            .collect()
    }

    /// Declared paths grouped by identifier code (each group is one alias set).
    pub(crate) fn alias_groups(&self) -> BTreeMap<String, Vec<String>> {
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for var in &self.vars {
            groups
                .entry(var.code.clone())
                .or_default()
                .push(var.path.clone());
        }
        for paths in groups.values_mut() {
            paths.sort();
        }
        groups
    }
}
