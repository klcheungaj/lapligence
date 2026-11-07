//! One message format for every `llg` report.
//!
//! The layout (severity words, brackets, separators, field order and how the
//! design context and the selection hint attach) is read from
//! `src/sim/rt/llg_msg_layout.def`, the file the generated-model runtime
//! (`llg_msg.h`) renders from as well, so a layout change is one edit there.
//! The message catalog (every id, its default severity, meaning and how to
//! disable it) is [`catalog`]; `docs/messages.md` is generated from it.
//!
//! A message is `severity`, an optional `location`, an `id` (`TOOL` plus a
//! three-digit index, or a frontend diagnostic name), an optional design
//! `context` (scope and simulation time), the `text` and an optional `hint`.
//! Everything is rendered on stderr by the drivers; this module only builds
//! and parses lines and counts what was reported.

pub mod catalog;

use std::sync::OnceLock;

pub use catalog::{MessageDef, Tool};

const LAYOUT_DEF: &str = include_str!("../sim/rt/llg_msg_layout.def");

/// Message severity, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
}

impl Severity {
    /// Every severity, least severe first.
    pub const ALL: [Severity; 4] = [
        Severity::Info,
        Severity::Warning,
        Severity::Error,
        Severity::Fatal,
    ];

    /// The word the layout prints for this severity.
    pub fn word(self) -> &'static str {
        let layout = layout();
        match self {
            Severity::Info => &layout.severity_info,
            Severity::Warning => &layout.severity_warning,
            Severity::Error => &layout.severity_error,
            Severity::Fatal => &layout.severity_fatal,
        }
    }

    /// The catalog spelling (`info`, `warning`, `error`, `fatal`).
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }

    fn from_word(word: &str) -> Option<Severity> {
        Severity::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// The parsed `llg_msg_layout.def`.
#[derive(Debug)]
pub struct Layout {
    pub template: String,
    pub severity_fatal: String,
    pub severity_error: String,
    pub severity_warning: String,
    pub severity_info: String,
    pub id_join: String,
    pub context_join: String,
    pub summary: String,
}

/// The layout every renderer uses.
pub fn layout() -> &'static Layout {
    static LAYOUT: OnceLock<Layout> = OnceLock::new();
    LAYOUT.get_or_init(|| {
        parse_layout(LAYOUT_DEF).expect("src/sim/rt/llg_msg_layout.def is malformed")
    })
}

/// Parse the `LLG_MSG_LAYOUT(NAME, "value")` entries of a layout file.
pub fn parse_layout(text: &str) -> Result<Layout, String> {
    let mut entries = std::collections::HashMap::new();
    for (name, args) in macro_calls(text, "LLG_MSG_LAYOUT") {
        let value = match args.as_slice() {
            [value] => value.clone(),
            _ => return Err(format!("{name}: expected one string")),
        };
        entries.insert(name, value);
    }
    let mut take = |name: &str| {
        entries
            .remove(name)
            .ok_or_else(|| format!("missing LLG_MSG_LAYOUT({name}, ...)"))
    };
    let layout = Layout {
        template: take("TEMPLATE")?,
        severity_fatal: take("SEVERITY_FATAL")?,
        severity_error: take("SEVERITY_ERROR")?,
        severity_warning: take("SEVERITY_WARNING")?,
        severity_info: take("SEVERITY_INFO")?,
        id_join: take("ID_JOIN")?,
        context_join: take("CONTEXT_JOIN")?,
        summary: take("SUMMARY")?,
    };
    if let Some(name) = entries.keys().next() {
        return Err(format!("unknown LLG_MSG_LAYOUT entry {name}"));
    }
    tokens(&layout.template)?;
    tokens(&layout.summary)?;
    Ok(layout)
}

/// The `NAME` and string arguments of each `MACRO(NAME, ...)` line. Integer
/// and bare-word arguments are returned as written.
pub(crate) fn macro_calls(text: &str, macro_name: &str) -> Vec<(String, Vec<String>)> {
    let prefix = format!("{macro_name}(");
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(body) = line.strip_prefix(&prefix).and_then(|b| b.strip_suffix(')')) else {
            continue;
        };
        let mut args = Vec::new();
        let mut rest = body.trim();
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('"') {
                let mut value = String::new();
                let mut chars = after.char_indices();
                let mut end = after.len();
                while let Some((i, c)) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some((_, escaped)) = chars.next() {
                                value.push(escaped);
                            }
                        }
                        '"' => {
                            end = i + 1;
                            break;
                        }
                        c => value.push(c),
                    }
                }
                args.push(value);
                rest = after[end..].trim_start();
            } else {
                let end = rest.find(',').unwrap_or(rest.len());
                args.push(rest[..end].trim().to_owned());
                rest = &rest[end..];
            }
            rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();
        }
        if args.is_empty() {
            continue;
        }
        let name = args.remove(0);
        out.push((name, args));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(String),
    Field(String),
    Group(Vec<Token>),
}

/// Split a template into literals, `{field}`s and `<...>` groups.
fn tokens(template: &str) -> Result<Vec<Token>, String> {
    fn flush(literal: &mut String, out: &mut Vec<Token>) {
        if !literal.is_empty() {
            out.push(Token::Literal(std::mem::take(literal)));
        }
    }
    let mut top = Vec::new();
    let mut group: Option<Vec<Token>> = None;
    let mut literal = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        let out = group.as_mut().unwrap_or(&mut top);
        match c {
            '{' => {
                flush(&mut literal, out);
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => name.push(c),
                        None => return Err(format!("unclosed field in `{template}`")),
                    }
                }
                out.push(Token::Field(name));
            }
            '<' => {
                if group.is_some() {
                    return Err(format!("nested group in `{template}`"));
                }
                flush(&mut literal, &mut top);
                group = Some(Vec::new());
            }
            '>' => {
                let Some(mut inner) = group.take() else {
                    return Err(format!("unopened group in `{template}`"));
                };
                flush(&mut literal, &mut inner);
                top.push(Token::Group(inner));
            }
            c => literal.push(c),
        }
    }
    if group.is_some() {
        return Err(format!("unclosed group in `{template}`"));
    }
    flush(&mut literal, &mut top);
    Ok(top)
}

/// Render `template` with `lookup`; a group whose fields are not all
/// non-empty is left out.
fn render_template<'a>(template: &str, lookup: impl Fn(&str) -> Option<&'a str>) -> String {
    let tokens = tokens(template).unwrap_or_default();
    let mut out = String::new();
    for token in &tokens {
        match token {
            Token::Literal(text) => out.push_str(text),
            Token::Field(name) => out.push_str(lookup(name).unwrap_or("")),
            Token::Group(inner) => {
                let complete = inner.iter().all(|t| match t {
                    Token::Field(name) => lookup(name).is_some_and(|v| !v.is_empty()),
                    _ => true,
                });
                if complete {
                    for t in inner {
                        match t {
                            Token::Literal(text) => out.push_str(text),
                            Token::Field(name) => out.push_str(lookup(name).unwrap_or("")),
                            Token::Group(_) => {}
                        }
                    }
                }
            }
        }
    }
    out
}

/// One message to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub severity: Severity,
    /// `file:line:col`, optionally followed by a `` (`line file:line)`` suffix.
    pub location: Option<String>,
    /// The rendered id, e.g. `LINT-001` (see [`id`]).
    pub id: String,
    /// Design scope and simulation time (see [`context`]).
    pub context: Option<String>,
    pub text: String,
    /// How to select or disable the message (a lint rule name, a frontend
    /// warning option).
    pub hint: Option<String>,
}

impl Message {
    /// A message with the catalog default severity of `def`.
    pub fn new(def: &MessageDef, text: impl Into<String>) -> Self {
        Message {
            severity: def.severity,
            location: None,
            id: def.id(),
            context: None,
            text: text.into(),
            hint: None,
        }
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn location(mut self, location: impl Into<String>) -> Self {
        let location = location.into();
        self.location = (!location.is_empty()).then_some(location);
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        let hint = hint.into();
        self.hint = (!hint.is_empty()).then_some(hint);
        self
    }

    /// The message as one line (multi-line text keeps its line breaks).
    pub fn render(&self) -> String {
        let severity = self.severity.word();
        render_template(&layout().template, |field| match field {
            "severity" => Some(severity),
            "location" => self.location.as_deref(),
            "id" => Some(self.id.as_str()),
            "context" => self.context.as_deref(),
            "text" => Some(self.text.as_str()),
            "hint" => self.hint.as_deref(),
            _ => None,
        })
    }
}

/// `TOOL` joined to a three-digit `index`, e.g. `LINT-001`.
pub fn id(tool: &str, index: u16) -> String {
    format!("{tool}{}{index:03}", layout().id_join)
}

/// `TOOL` joined to a name, for ids that come from another tool's own
/// catalog (frontend diagnostics: `SLANG-WidthTruncate`).
pub fn named_id(tool: &str, name: &str) -> String {
    format!("{tool}{}{name}", layout().id_join)
}

/// Design context: `scope` and `time` joined by the layout's separator;
/// either may be empty.
pub fn context(scope: &str, time: &str) -> String {
    match (scope.is_empty(), time.is_empty()) {
        (false, false) => format!("{scope}{}{time}", layout().context_join),
        (false, true) => scope.to_owned(),
        (true, false) => time.to_owned(),
        (true, true) => String::new(),
    }
}

/// A message line split into its fields by the layout template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub severity: Severity,
    pub location: Option<String>,
    pub id: String,
    pub context: Option<String>,
    /// The text plus anything the template places after it.
    pub text: String,
}

impl ParsedMessage {
    /// The tool part of the id (`LINT` for `LINT-001`).
    pub fn tool(&self) -> &str {
        let join = &layout().id_join;
        self.id
            .split_once(join.as_str())
            .map_or(&self.id, |(t, _)| t)
    }
}

/// Split `line` into fields by the layout template, or `None` when it is not
/// a message line (design output, tool output passed through).
pub fn parse(line: &str) -> Option<ParsedMessage> {
    parse_with(&layout().template, line)
}

/// [`parse`] by `template` instead of the shipped layout.
fn parse_with(template: &str, line: &str) -> Option<ParsedMessage> {
    let tokens = tokens(template).ok()?;
    let mut fields = Vec::new();
    if !match_tokens(&tokens, line, &mut fields) {
        return None;
    }
    let get = |name: &str| {
        fields
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v): &(String, String)| v.clone())
    };
    let severity = Severity::from_word(&get("severity")?)?;
    let id = get("id")?;
    if id.is_empty() || id.contains(' ') {
        return None;
    }
    Some(ParsedMessage {
        severity,
        location: get("location").filter(|v| !v.is_empty()),
        id,
        context: get("context").filter(|v| !v.is_empty()),
        text: get("text").unwrap_or_default(),
    })
}

/// Match `tokens` against the whole of `input`, backtracking over optional
/// groups. `{text}` takes the rest of the line; any other field ends at the
/// first occurrence of the literal that follows it.
fn match_tokens(tokens: &[Token], input: &str, fields: &mut Vec<(String, String)>) -> bool {
    let Some((first, rest)) = tokens.split_first() else {
        return input.is_empty();
    };
    match first {
        Token::Literal(text) => input
            .strip_prefix(text.as_str())
            .is_some_and(|after| match_tokens(rest, after, fields)),
        Token::Field(name) if name == "text" => {
            fields.push((name.clone(), input.to_owned()));
            true
        }
        Token::Field(name) => {
            // The field ends where what follows can start: the first literal
            // of each optional group up to the next required literal (a
            // skipped group exposes the token after it), or the end of the
            // line when nothing required follows.
            let mut literals: Vec<&str> = Vec::new();
            let mut open_end = true;
            for token in rest {
                match token {
                    Token::Literal(text) => {
                        literals.push(text.as_str());
                        open_end = false;
                        break;
                    }
                    Token::Group(inner) => {
                        if let Some(Token::Literal(text)) = inner.first() {
                            literals.push(text.as_str());
                        }
                    }
                    Token::Field(_) => break,
                }
            }
            let mut candidates: Vec<usize> = literals
                .iter()
                .flat_map(|literal| input.match_indices(literal).map(|(i, _)| i))
                .collect();
            if open_end || literals.is_empty() {
                candidates.push(input.len());
            }
            candidates.sort_unstable();
            candidates.dedup();
            for end in candidates {
                if !field_value_fits(name, &input[..end]) {
                    continue;
                }
                let mark = fields.len();
                fields.push((name.clone(), input[..end].to_owned()));
                if match_tokens(rest, &input[end..], fields) {
                    return true;
                }
                fields.truncate(mark);
            }
            false
        }
        Token::Group(inner) => {
            let mut joined: Vec<Token> = inner.clone();
            joined.extend_from_slice(rest);
            let mark = fields.len();
            if match_tokens(&joined, input, fields) {
                return true;
            }
            fields.truncate(mark);
            match_tokens(rest, input, fields)
        }
    }
}

/// Whether `value` can fill field `name`, so a field never swallows the
/// neighbouring field of a layout that separates them with the same literal:
/// a severity is a severity word, and a location ends in `:line[:column]`,
/// optionally followed by a `` (`line ...)`` origin.
fn field_value_fits(name: &str, value: &str) -> bool {
    match name {
        "severity" => Severity::from_word(value).is_some(),
        "location" => {
            let site = match value.find(" (`line ") {
                Some(at) if value.ends_with(')') => &value[..at],
                _ => value,
            };
            let mut parts = site.rsplit(':');
            let last = parts.next().unwrap_or("");
            let numeric = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            numeric(last) && parts.next().is_some_and(|before| !before.is_empty())
        }
        _ => !value.is_empty(),
    }
}

/// Counts of reported messages per severity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub info: u64,
    pub warning: u64,
    pub error: u64,
    pub fatal: u64,
}

impl Counts {
    pub fn add(&mut self, severity: Severity) {
        let slot = match severity {
            Severity::Info => &mut self.info,
            Severity::Warning => &mut self.warning,
            Severity::Error => &mut self.error,
            Severity::Fatal => &mut self.fatal,
        };
        *slot = slot.saturating_add(1);
    }

    /// Whether a warning or worse was counted, the condition for printing a
    /// summary.
    pub fn warning_or_worse(&self) -> bool {
        self.warning != 0 || self.error != 0 || self.fatal != 0
    }

    /// The summary text from the layout's `SUMMARY` template.
    pub fn summary_text(&self) -> String {
        let fatal = (self.fatal != 0).then(|| self.fatal.to_string());
        let info = (self.info != 0).then(|| self.info.to_string());
        let error = self.error.to_string();
        let warning = self.warning.to_string();
        render_template(&layout().summary, |field| match field {
            "fatal" => fatal.as_deref(),
            "error" => Some(error.as_str()),
            "warning" => Some(warning.as_str()),
            "info" => info.as_deref(),
            _ => None,
        })
    }
}

/// Counts per tool in first-report order, for the end-of-run summary.
#[derive(Debug, Default)]
pub struct Tally {
    tools: Vec<(String, Counts)>,
}

impl Tally {
    /// Count `message` under the tool of its id.
    pub fn count(&mut self, message: &Message) {
        let join = &layout().id_join;
        let tool = message
            .id
            .split_once(join.as_str())
            .map_or(message.id.as_str(), |(t, _)| t);
        self.count_tool(tool, message.severity);
    }

    pub fn count_tool(&mut self, tool: &str, severity: Severity) {
        match self.tools.iter_mut().find(|(t, _)| t == tool) {
            Some((_, counts)) => counts.add(severity),
            None => {
                let mut counts = Counts::default();
                counts.add(severity);
                self.tools.push((tool.to_owned(), counts));
            }
        }
    }

    pub fn counts(&self, tool: &str) -> Counts {
        self.tools
            .iter()
            .find(|(t, _)| t == tool)
            .map(|(_, c)| *c)
            .unwrap_or_default()
    }

    /// One `TOOL-000` summary message per tool that reported a warning or
    /// worse, in first-report order.
    pub fn summaries(&self) -> Vec<Message> {
        self.tools
            .iter()
            .filter(|(_, counts)| counts.warning_or_worse())
            .map(|(tool, counts)| summary(tool, counts))
            .collect()
    }
}

/// The summary message of `tool` (index 000, severity info).
pub fn summary(tool: &str, counts: &Counts) -> Message {
    Message {
        severity: Severity::Info,
        location: None,
        id: id(tool, 0),
        context: None,
        text: counts.summary_text(),
        hint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_file_parses() {
        let layout = layout();
        assert_eq!(layout.severity_error, "ERROR");
        assert!(layout.template.contains("{text}"));
    }

    #[test]
    fn renders_located_and_unlocated_messages() {
        let message = Message::new(
            &catalog::LINT_RULES[0].def(),
            "signal `x` in `tb` is never used",
        )
        .location("f.sv:7:9")
        .hint("unused-signal");
        assert_eq!(
            message.render(),
            "WARNING: f.sv:7:9: [LINT-001] signal `x` in `tb` is never used (unused-signal)"
        );
        let message = Message::new(&catalog::CLI_OPTION_VALUE, "--top requires a module name");
        assert_eq!(
            message.render(),
            "ERROR: [CLI-003] --top requires a module name"
        );
        let mut message = Message::new(&catalog::CLI_OPTION_VALUE, "$finish called")
            .severity(Severity::Info)
            .location("f.sv:3:35");
        message.id = id("SIM", 1);
        message.context = Some(context("tb", "5 ns"));
        assert_eq!(
            message.render(),
            "INFO: f.sv:3:35: [SIM-001] (tb @ 5 ns) $finish called"
        );
    }

    #[test]
    fn parse_round_trips_rendered_lines() {
        for line in [
            "WARNING: f.sv:7:9: [LINT-001] signal `x` is never used (unused-signal)",
            "ERROR: [CLI-003] --top requires: a module name",
            "INFO: /a b/f.sv:3:35 (`line g.sv:9): [SIM-001] (tb @ 5 ns) $finish called",
            "ERROR: C:\\w\\f.sv:3:35: [SLANG-WidthTruncate] x [y]",
            "INFO: [SIM-000] 0 error(s), 1 warning(s)",
        ] {
            let parsed = parse(line).unwrap_or_else(|| panic!("{line}"));
            let message = Message {
                severity: parsed.severity,
                location: parsed.location.clone(),
                id: parsed.id.clone(),
                context: parsed.context.clone(),
                text: parsed.text.clone(),
                hint: None,
            };
            assert_eq!(message.render(), line);
        }
        let parsed = parse("INFO: f.sv:3:35: [SIM-001] (tb @ 5 ns) done").unwrap();
        assert_eq!(parsed.location.as_deref(), Some("f.sv:3:35"));
        assert_eq!(parsed.context.as_deref(), Some("tb @ 5 ns"));
        assert_eq!(parsed.tool(), "SIM");
        assert!(parse("hello world").is_none());
        assert!(parse("PASS: 3").is_none());
    }

    #[test]
    fn summary_lists_nonzero_optional_counts() {
        let mut counts = Counts::default();
        counts.add(Severity::Warning);
        assert_eq!(counts.summary_text(), "0 error(s), 1 warning(s)");
        counts.add(Severity::Fatal);
        counts.add(Severity::Info);
        assert_eq!(
            counts.summary_text(),
            "1 fatal, 0 error(s), 1 warning(s), 1 info(s)"
        );
        let mut tally = Tally::default();
        tally.count_tool("LINT", Severity::Info);
        tally.count_tool("SLANG", Severity::Warning);
        let summaries = tally.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].render(),
            "INFO: [SLANG-000] 0 error(s), 1 warning(s)"
        );
    }

    #[test]
    fn a_changed_layout_moves_every_field() {
        let layout = parse_layout(&LAYOUT_DEF.replace(
            "{severity}: <{location}: >[{id}] <({context}) >{text}< ({hint})>",
            "<{location}: >{severity} {id}: <{context}: >{text}",
        ))
        .unwrap();
        assert_eq!(
            layout.template,
            "<{location}: >{severity} {id}: <{context}: >{text}"
        );
    }

    /// A field followed by optional groups ends where any of them, or the
    /// required literal after them, starts.
    #[test]
    fn parse_follows_a_layout_with_trailing_optional_groups() {
        let template = "[{id}] {severity}< {location}>< ({context})>: {text}< [{hint}]>";
        let parsed = parse_with(
            template,
            "[LINT-016] ERROR /a/f.sv:5:1: process mixes (`=`) and (`<=`) [mixed-assignments]",
        )
        .unwrap();
        assert_eq!(parsed.location.as_deref(), Some("/a/f.sv:5:1"));
        assert_eq!(parsed.context, None);
        let parsed = parse_with(
            template,
            "[SIM-001] INFO f.sv:6:8 (tb @ 8 ns): $finish called",
        )
        .unwrap();
        assert_eq!(parsed.location.as_deref(), Some("f.sv:6:8"));
        assert_eq!(parsed.context.as_deref(), Some("tb @ 8 ns"));
        let parsed = parse_with(
            template,
            "[SIM-009] FATAL (method call at tb:15:12 @ 1 ns): null class handle access",
        )
        .unwrap();
        assert_eq!(parsed.location, None);
        assert_eq!(
            parsed.context.as_deref(),
            Some("method call at tb:15:12 @ 1 ns")
        );
        let parsed = parse_with(template, "[SIM-000] INFO: 1 error(s), 0 warning(s)").unwrap();
        assert_eq!(parsed.location, None);
        assert_eq!(parsed.text, "1 error(s), 0 warning(s)");
    }
}
