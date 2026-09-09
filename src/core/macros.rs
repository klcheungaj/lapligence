//! macros — preprocessor macro tables backing macro-usage hover.
//!
//! Slang expands directives before semantic capture, so this module builds a
//! conservative macro view from the exact source buffers the LSP commits:
//!
//! 1. **Config defines seed every file.**  The root's `[compile] defines`
//!    ([`parse_config_defines`] over `CompileOpts::defines`, i.e. the
//!    validated `NAME[=VALUE]` entries) act as if defined on the command
//!    line before any source text of EVERY analyzed file.  They are the
//!    authoritative base table and hot-reload through config changes.
//! 2. **In-source directives resolve per file, positionally.**  Each
//!    analyzed file is scanned in ONE conservative pass over its exact
//!    compiled text, honoring `` `define `` / `` `undef `` /
//!    `` `undefineall `` and conditional compilation (`` `ifdef `` /
//!    `` `ifndef `` / `` `elsif `` / `` `else `` / `` `endif``, evaluated
//!    against the evolving in-file table).  Last definition wins (LRM); a
//!    usage resolves against the table state at its own position, so an
//!    in-source redefinition overrides the config value only from its
//!    definition point onward, and `` `undef `` removes even a config-seeded
//!    name.  Comments and string literals are skipped: directives or usages
//!    inside them do not exist.
//!
//! Documented approximations (deliberate, conservative — they can lose a
//! value but never invent a wrong one):
//!
//! * Include files are NOT followed: a macro defined only in a header
//!   resolves as undefined here.
//! * In-source definitions do not leak across files (per-file scoping instead
//!   of the classic compilation-order-dependent global scope).
//! * Usages inside a `` `define `` body resolve at the DEFINITION point
//!   rather than at expansion time.
//! * Escaped identifiers (`\foo` up to the next white space) are not tracked:
//!   a `` `NAME `` sequence inside an escaped identifier (e.g. `` \a`WIDTH b ``)
//!   still records a usage even though a real preprocessor would fold the
//!   whole token into the identifier.
//!
//! Conditional-directive grammar and failure semantics are shared with the
//! inactive-region dimmer (`bin/llg_ls/inactive_ranges.rs`) so macro hover can
//! never contradict the editor's dimming: both accept ONE optional
//! surrounding paren pair around a bare identifier (`` `ifdef (NAME) ``), and
//! an unparseable condition fails safe as an always-active branch that can
//! hide nothing. A corpus test in the bin pins the two scanners together.

use std::collections::{BTreeMap, HashMap};

/// One known macro definition, snapshotted at resolution time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    /// Macro name (without the leading backtick).
    pub name: String,
    /// Formal argument names for function-like macros (`None` for simple
    /// macros).  Empty parentheses yield `Some(Vec::new())`.
    pub args: Option<Vec<String>>,
    /// Replacement text.  Backslash-newline continuations are joined into a
    /// single space; surrounding whitespace is trimmed.  Empty for a bare
    /// `` `define NAME ``.
    pub body: String,
    /// `true` when seeded from `[compile] defines` rather
    /// than from a source `` `define `` directive.
    pub from_config: bool,
    /// Defining source file (`None` for config defines).
    pub file: Option<String>,
    /// 1-based defining line within [`MacroDefinition::file`] (0 for config
    /// defines).
    pub line1: u32,
}

impl MacroDefinition {
    fn config(name: &str, body: &str) -> MacroDefinition {
        MacroDefinition {
            name: name.to_owned(),
            args: None,
            body: body.to_owned(),
            from_config: true,
            file: None,
            line1: 0,
        }
    }
}

/// One recorded macro usage (a `` `NAME `` occurrence) with its resolution.
#[derive(Debug, Clone)]
pub struct MacroUsage {
    /// Usage line (0-based).
    pub line0: u32,
    /// Column of the backtick (0-based, UTF-16 units).
    pub col_start0: u32,
    /// One past the NAME's last character (the span covers `` `NAME ``;
    /// 0-based, UTF-16 units).
    pub col_end0: u32,
    /// Macro name without the leading backtick (kept even when unresolved,
    /// so the undefined-macro message can name what was clicked).
    pub name: String,
    /// The resolved definition at this position, or `None` when the macro is
    /// not defined under the current configuration.
    pub definition: Option<MacroDefinition>,
}

impl MacroUsage {
    /// Macro name without the leading backtick.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// One recorded `` `define `` site (the NAME identifier after the keyword),
/// so hovering a definition renders the same text as hovering a usage.
#[derive(Debug, Clone)]
pub struct MacroDecl {
    /// Directive line (0-based).
    pub line0: u32,
    /// First column of the NAME identifier (0-based, UTF-16 units).
    pub col_start0: u32,
    /// One past the NAME's last character (0-based, UTF-16 units).
    pub col_end0: u32,
    /// The defined macro.
    pub definition: MacroDefinition,
}

/// Per-file scan result: usages and definition sites, sorted by position.
#[derive(Debug, Default)]
pub struct FileMacros {
    usages: Vec<MacroUsage>,
    decls: Vec<MacroDecl>,
}

/// Macro tables for one analysis root, keyed by compiled file path.
///
/// Built once per analysis commit (see [`build_table`]); hover requests only
/// look positions up, so serving stays a pure read over committed data.
#[derive(Debug, Default)]
pub struct MacroTable {
    files: BTreeMap<String, FileMacros>,
    config_count: usize,
    /// Display note naming the configuration source for undefined-macro
    /// messages (e.g. the effective `llg.toml` path); optional.
    config_note: Option<String>,
}

impl MacroTable {
    /// Whether neither config defines nor scanned files contributed anything.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.config_count == 0
    }

    /// The usage whose `` `NAME `` span covers `(line0, col0)`, if any.
    pub fn usage_at(&self, file: &str, line0: u32, col0: u32) -> Option<&MacroUsage> {
        self.files
            .get(file)?
            .usages
            .iter()
            .find(|u| u.line0 == line0 && u.col_start0 <= col0 && col0 < u.col_end0)
    }

    /// The `` `define `` NAME identifier covering `(line0, col0)`, if any.
    pub fn decl_at(&self, file: &str, line0: u32, col0: u32) -> Option<&MacroDecl> {
        self.files
            .get(file)?
            .decls
            .iter()
            .find(|d| d.line0 == line0 && d.col_start0 <= col0 && col0 < d.col_end0)
    }

    /// Number of config defines seeding every file's table.
    pub fn config_defines(&self) -> usize {
        self.config_count
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

fn is_identifier_start(byte: Option<u8>) -> bool {
    matches!(byte, Some(b) if b.is_ascii_alphabetic() || b == b'_')
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Parse validated compile definitions (`NAME`, `NAME=VALUE`) into
/// config-origin macro definitions.  Entries that do not reduce to a plain
/// identifier name are dropped defensively (the config layer already
/// validates them).
pub fn parse_config_defines(args: &[String]) -> Vec<MacroDefinition> {
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

/// Build the whole table: seed every source with the config defines, then run
/// the single-pass scanner per file.  `sources` pairs absolute paths with the
/// exact text each file was compiled from (shadow-staged buffer text wins
/// exactly as it was admitted to Slang).
pub fn build_table(
    config_defines_args: &[String],
    sources: &[(&str, &str)],
    config_note: Option<&str>,
) -> MacroTable {
    let seed = parse_config_defines(config_defines_args);
    let mut files = BTreeMap::new();
    for (path, text) in sources {
        files.insert((*path).to_owned(), scan_file(path, text, &seed));
    }
    MacroTable {
        config_count: seed.len(),
        config_note: config_note.map(str::to_owned),
        files,
    }
}

// ── Single-pass scanner ───────────────────────────────────────────────────────

/// Compiler directive keywords following a backtick.  These never count as
/// macro usages; any other backtick-identifier does.
const DIRECTIVE_KEYWORDS: &[&str] = &[
    "define",
    "undef",
    "undefineall",
    "ifdef",
    "ifndef",
    "elsif",
    "else",
    "endif",
    "include",
    "timescale",
    "resetall",
    "default_nettype",
    "line",
    "begin_keywords",
    "end_keywords",
    "celldefine",
    "endcelldefine",
    "pragma",
    "unconnected_drive",
    "nounconnected_drive",
    "accelerate",
    "noaccelerate",
    "default_decay_time",
    "default_trireg_strength",
    "delay_mode_distributed",
    "delay_mode_path",
    "delay_mode_unit",
    "delay_mode_zero",
    "__FILE__",
    "__LINE__",
];

#[derive(Debug, Clone, Copy)]
struct CondFrame {
    /// Whether all enclosing conditionals were active when this one opened.
    parent_active: bool,
    /// Whether some branch of THIS conditional already matched.
    taken: bool,
    /// Whether the current branch is active.
    active: bool,
}

struct Scan<'a> {
    src: &'a str,
    bytes: &'a [u8],
    path: &'a str,
    idx: usize,
    /// 1-based line of `idx`.
    line: u32,
    /// 1-based UTF-16 column of `idx`.
    col: u32,
    table: HashMap<String, MacroDefinition>,
    conds: Vec<CondFrame>,
    usages: Vec<MacroUsage>,
    decls: Vec<MacroDecl>,
}

impl<'a> Scan<'a> {
    fn new(src: &'a str, path: &'a str) -> Scan<'a> {
        Scan {
            src,
            bytes: src.as_bytes(),
            path,
            idx: 0,
            line: 1,
            col: 1,
            table: HashMap::new(),
            conds: Vec::new(),
            usages: Vec::new(),
            decls: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.idx..].chars().next()
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.idx + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.idx += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += ch.len_utf16() as u32;
        }
        Some(ch)
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.advance();
        }
    }

    fn active(&self) -> bool {
        self.conds.last().is_none_or(|frame| frame.active)
    }

    /// Consume up to and including the next newline, honoring backslash-newline
    /// continuations.
    fn skip_to_line_end(&mut self) {
        loop {
            match self.advance() {
                None | Some('\n') => return,
                Some('\\') => {
                    if self.byte_at(0) == Some(b'\r') {
                        self.advance();
                    }
                    if self.byte_at(0) == Some(b'\n') {
                        self.advance();
                    }
                }
                Some(_) => {}
            }
        }
    }

    fn read_identifier(&mut self) -> Option<String> {
        if !is_identifier_start(self.byte_at(0)) {
            return None;
        }
        let start = self.idx;
        self.idx += 1;
        self.col += 1;
        while self
            .bytes
            .get(self.idx)
            .copied()
            .is_some_and(is_identifier_continue)
        {
            self.idx += 1;
            self.col += 1;
        }
        Some(self.src[start..self.idx].to_owned())
    }

    /// Whole-line-directive heuristic: only whitespace precedes the cursor on
    /// this line.
    fn at_line_start(&self) -> bool {
        let mut back = self.idx;
        while back > 0 {
            match self.bytes[back - 1] {
                b'\n' => return true,
                b' ' | b'\t' | b'\r' => back -= 1,
                _ => return false,
            }
        }
        true
    }

    fn record_usage(&mut self, tick_line: u32, tick_col: u32, name: String) {
        let definition = self.table.get(&name).cloned();
        self.usages.push(MacroUsage {
            line0: tick_line - 1,
            col_start0: tick_col - 1,
            col_end0: tick_col + name.chars().count() as u32,
            name,
            definition,
        });
    }

    /// Dispatch after the backtick was consumed at `(tick_line, tick_col)`.
    fn handle_backtick(&mut self, tick_line: u32, tick_col: u32) {
        match self.peek() {
            // SV string literals (`"…`) and escaped sequences (`\…) are not
            // macro references.
            Some('"') | Some('\\') => {
                self.skip_sv_quoted_or_escaped();
                return;
            }
            _ => {}
        }
        let Some(name) = self.read_identifier() else {
            return; // lone backtick or punctuation: not a reference
        };
        if !DIRECTIVE_KEYWORDS.contains(&name.as_str()) {
            if self.active() {
                self.record_usage(tick_line, tick_col, name);
            }
            return;
        }
        match name.as_str() {
            // Conditional bookkeeping runs in every branch state so nesting
            // stays correct; the frame logic itself consults parent_active.
            "ifdef" | "ifndef" => self.handle_ifdef(name == "ifndef"),
            "elsif" => self.handle_elsif(),
            "else" => self.handle_else(),
            "endif" => {
                self.conds.pop();
                self.skip_to_line_end();
            }
            _ if !self.active() => {
                // Definitions/mutations inside dead regions never take
                // effect, but their lines must still be consumed.
                self.skip_to_line_end();
            }
            "define" => self.handle_define(),
            "undef" => self.handle_undef(),
            "undefineall" => {
                self.table.clear();
                self.skip_to_line_end();
            }
            _ => {
                if self.at_line_start() {
                    self.skip_to_line_end();
                }
                // Mid-line informational directives leave the remainder of
                // the line scannable; only the keyword was consumed.
            }
        }
    }

    /// `` `" `` opens an SV string literal; `` `\ `` escapes whatever
    /// follows.  Both end at the closing quote/backslash or the newline.
    fn skip_sv_quoted_or_escaped(&mut self) {
        self.advance(); // the `"` or `\
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '"' || ch == '\n' {
                return;
            }
        }
    }

    /// Skip one optional opening paren plus surrounding spaces, the shared
    /// conditional-directive grammar with the inactive-region dimmer
    /// (`` `ifdef (NAME) ``).  Callers read the identifier right after.
    fn skip_optional_condition_paren(&mut self) {
        self.skip_spaces();
        if self.peek() == Some('(') {
            self.advance();
            self.skip_spaces();
        }
    }

    fn handle_ifdef(&mut self, inverted: bool) {
        self.skip_optional_condition_paren();
        let parent_active = self.active();
        // Unparseable conditions fail safe exactly like the inactive-region
        // scanner: the branch is treated as ACTIVE (nothing inside can be
        // hidden) but counts as untaken, so sibling `elsif`/`else` branches
        // stay live too.
        let (target_known, holds) = match self.read_identifier() {
            Some(target) => (true, self.table.contains_key(&target) != inverted),
            None => (false, true),
        };
        self.conds.push(CondFrame {
            parent_active,
            taken: parent_active && target_known && holds,
            active: parent_active && (!target_known || holds),
        });
        self.skip_to_line_end();
    }

    fn handle_elsif(&mut self) {
        self.skip_optional_condition_paren();
        // Same fail-safe shape as `handle_ifdef`: an unparseable condition
        // matches whenever no real branch has matched yet.
        let eligible = self
            .conds
            .last()
            .is_some_and(|frame| frame.parent_active && !frame.taken);
        let (target_known, holds) = match self.read_identifier() {
            Some(target) => (true, self.table.contains_key(&target)),
            None => (false, true),
        };
        if let Some(frame) = self.conds.last_mut() {
            if eligible && (!target_known || holds) {
                frame.active = true;
                frame.taken = true;
            } else {
                frame.active = false;
            }
        }
        self.skip_to_line_end();
    }

    fn handle_else(&mut self) {
        if let Some(frame) = self.conds.last_mut() {
            frame.active = frame.parent_active && !frame.taken;
            frame.taken = true;
        }
        self.skip_to_line_end();
    }

    fn handle_undef(&mut self) {
        self.skip_spaces();
        if let Some(target) = self.read_identifier() {
            self.table.remove(&target);
        }
        self.skip_to_line_end();
    }

    /// `` `define NAME[(args)] BODY`` — capture the raw body (with
    /// continuations), update the table, and record the NAME span plus the
    /// usages inside the body (resolved at definition time).
    fn handle_define(&mut self) {
        self.skip_spaces();
        let def_line = self.line;
        let Some(name) = self.read_identifier() else {
            // Malformed directive: swallow the line so nothing leaks.
            self.skip_to_line_end();
            return;
        };
        let name_start_col = self.col - name.chars().count() as u32;
        let name_end_col = self.col;
        // An IMMEDIATE '(' introduces formal arguments (no space allowed).
        let args = if self.peek() == Some('(') {
            self.advance();
            let mut names = Vec::new();
            loop {
                self.skip_spaces();
                if self.peek() == Some(')') {
                    self.advance();
                    break;
                }
                match self.read_identifier() {
                    Some(arg) => names.push(arg),
                    None => break, // malformed list: stop consuming args
                }
                self.skip_spaces();
                match self.peek() {
                    Some(',') => {
                        self.advance();
                    }
                    Some(')') => {
                        self.advance();
                        break;
                    }
                    // Default argument value (`b=5`): consume it so the body
                    // display starts after the formal-argument list instead
                    // of at a stray `= 5) …`.  Nesting is tracked so defaults
                    // containing commas/brackets stay intact.
                    Some('=') => {
                        self.advance();
                        let mut depth = 0usize;
                        loop {
                            match self.peek() {
                                Some('(') | Some('[') | Some('{') => depth += 1,
                                Some(')') | Some(',') if depth == 0 => break,
                                Some(')') | Some(']') | Some('}') => {
                                    depth = depth.saturating_sub(1)
                                }
                                None => break,
                                _ => {}
                            }
                            self.advance();
                        }
                        // Consume the separating comma so the next loop
                        // iteration starts cleanly at the next argument name.
                        if self.peek() == Some(',') {
                            self.advance();
                        }
                    }
                    _ => break,
                }
            }
            Some(names)
        } else {
            None
        };
        // Body: the rest of the LOGICAL line, honoring backslash-newline
        // continuations.  `raw` keeps the verbatim text (real newlines) for
        // the position-accurate nested-usage scan; `logical` joins
        // continuations into single spaces for display.
        let body_line = self.line;
        let body_col = self.col;
        let mut raw = String::new();
        let mut logical = String::new();
        loop {
            match self.peek() {
                Some('\n') => {
                    if logical.trim_end_matches('\r').ends_with('\\') {
                        while logical.ends_with('\\') || logical.ends_with('\r') {
                            logical.pop();
                        }
                        logical.push(' ');
                        raw.push('\n');
                        self.advance();
                        continue;
                    }
                    break;
                }
                Some(ch) => {
                    logical.push(ch);
                    raw.push(ch);
                    self.advance();
                }
                None => break,
            }
        }
        // Display form: collapse every whitespace run (continuation joins and
        // indentation) into one space.
        let body = logical.split_whitespace().collect::<Vec<_>>().join(" ");

        if self.active() {
            let definition = MacroDefinition {
                name: name.clone(),
                args,
                body,
                from_config: false,
                file: Some(self.path.to_owned()),
                line1: def_line,
            };
            self.decls.push(MacroDecl {
                line0: def_line - 1,
                col_start0: name_start_col - 1,
                col_end0: name_end_col - 1,
                definition: definition.clone(),
            });
            self.table.insert(name, definition);
            // Usages inside the body resolve against the table state AT THE
            // DEFINITION point (documented approximation of expansion-time
            // resolution); positions map back into the source via the raw
            // capture.
            scan_body_usages(&raw, body_line, body_col, &self.table, &mut self.usages);
        }
        // The capture consumed through end-of-line; nothing further on this
        // line can leak.
    }

    fn run(&mut self) {
        while self.idx < self.src.len() {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('/') if self.byte_at(1) == Some(b'/') => {
                    self.skip_to_line_end();
                }
                Some('/') if self.byte_at(1) == Some(b'*') => {
                    self.advance();
                    self.advance();
                    while self.idx < self.src.len() {
                        if self.peek() == Some('*') && self.byte_at(1) == Some(b'/') {
                            self.advance();
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                Some('"') => self.skip_verilog_string(),
                Some('`') => {
                    let (tick_line, tick_col) = (self.line, self.col);
                    self.advance();
                    self.handle_backtick(tick_line, tick_col);
                }
                Some(_) => {
                    self.advance();
                }
                None => break,
            }
        }
    }

    /// A plain double-quoted literal; Verilog strings never span lines, so an
    /// unterminated quote is confined to its own line.
    fn skip_verilog_string(&mut self) {
        self.advance(); // opening quote
        while let Some(ch) = self.peek() {
            if ch == '\\' {
                self.advance();
                self.advance();
                continue;
            }
            self.advance();
            if ch == '"' || ch == '\n' {
                return;
            }
        }
    }
}

fn scan_file(path: &str, text: &str, seed: &[MacroDefinition]) -> FileMacros {
    let mut scan = Scan::new(text, path);
    for def in seed {
        scan.table.insert(def.name.clone(), def.clone());
    }
    scan.run();
    scan.usages.sort_by_key(|u| (u.line0, u.col_start0));
    scan.decls.sort_by_key(|d| (d.line0, d.col_start0));
    FileMacros {
        usages: scan.usages,
        decls: scan.decls,
    }
}

/// Record the `` `NAME `` usages inside a `` `define `` body, resolved against
/// the table state at the definition point.  `(line, col)` is the 1-based
/// source position of the body's first character; the raw capture keeps real
/// newlines so continuation lines map back exactly.
fn scan_body_usages(
    raw: &str,
    line: u32,
    col: u32,
    table: &HashMap<String, MacroDefinition>,
    out: &mut Vec<MacroUsage>,
) {
    let mut idx = 0usize;
    let mut cur_line = line;
    let mut cur_col = col;
    while idx < raw.len() {
        let ch = match raw[idx..].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        match ch {
            c if c.is_whitespace() => {
                if c == '\n' {
                    cur_line += 1;
                    cur_col = 1;
                } else {
                    cur_col += c.len_utf16() as u32;
                }
                idx += c.len_utf8();
            }
            '/' if raw.as_bytes().get(idx + 1) == Some(&b'/') => {
                while idx < raw.len() && !raw[idx..].starts_with('\n') {
                    idx += 1;
                }
            }
            '/' if raw.as_bytes().get(idx + 1) == Some(&b'*') => {
                idx += 2;
                while idx < raw.len() && !raw[idx..].starts_with("*/") {
                    idx += 1;
                }
                idx = (idx + 2).min(raw.len());
            }
            '"' => {
                idx += 1;
                cur_col += 1;
                while idx < raw.len() {
                    let esc = raw.as_bytes()[idx] == b'\\';
                    let inner = raw[idx..].chars().next().unwrap_or('"');
                    idx += inner.len_utf8();
                    cur_col += inner.len_utf16() as u32;
                    if esc {
                        continue;
                    }
                    if inner == '"' || inner == '\n' {
                        break;
                    }
                }
            }
            '`' => {
                let (tick_line, tick_col) = (cur_line, cur_col);
                idx += 1;
                cur_col += 1;
                // SV quoted/escaped forms are not references.
                if matches!(raw[idx..].chars().next(), Some('"') | Some('\\')) {
                    continue;
                }
                let start = idx;
                if is_identifier_start(raw.as_bytes().get(idx).copied()) {
                    idx += 1;
                    cur_col += 1;
                    while raw
                        .as_bytes()
                        .get(idx)
                        .copied()
                        .is_some_and(is_identifier_continue)
                    {
                        idx += 1;
                        cur_col += 1;
                    }
                    let name = &raw[start..idx];
                    if !DIRECTIVE_KEYWORDS.contains(&name) {
                        out.push(MacroUsage {
                            line0: tick_line - 1,
                            col_start0: tick_col - 1,
                            col_end0: tick_col + name.chars().count() as u32,
                            name: name.to_owned(),
                            definition: table.get(name).cloned(),
                        });
                    }
                }
            }
            _ => {
                cur_col += ch.len_utf16() as u32;
                idx += ch.len_utf8();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a table over one file and return (usages, decls) for it.
    fn scan_one(config: &[&str], text: &str) -> (Vec<MacroUsage>, Vec<MacroDecl>, MacroTable) {
        let args: Vec<String> = config.iter().map(|define| (*define).to_owned()).collect();
        let table = build_table(&args, &[("t.sv", text)], None);
        let entries = table.files.get("t.sv").expect("scanned file");
        (entries.usages.clone(), entries.decls.clone(), table)
    }

    fn usage_for<'a>(usages: &'a [MacroUsage], name: &str) -> &'a MacroDefinition {
        usages
            .iter()
            .find(|u| u.definition.as_ref().is_some_and(|d| d.name == name))
            .and_then(|u| u.definition.as_ref())
            .unwrap_or_else(|| panic!("usage of `{name} not resolved"))
    }

    #[test]
    fn config_defines_resolve_everywhere_in_the_file() {
        let text = "module m;\n  localparam int A = `DEPTH;\n\
                     `undef DEPTH\n  localparam int B = `DEPTH;\nendmodule\n";
        let (usages, _, _) = scan_one(&["DEPTH=16"], text);
        assert_eq!(usage_for(&usages, "DEPTH").body, "16");
        // After `undef even the config-seeded macro is gone (LRM semantics).
        let later = usages
            .iter()
            .find(|u| u.line0 == 3)
            .expect("post-undef usage");
        assert!(later.definition.is_none(), "undef removes config seeds");
    }

    #[test]
    fn in_source_define_overrides_config_from_its_point_onward() {
        let text = "module m;\n  localparam int A = `WIDTH;\n\
                     `define WIDTH 8\n  localparam int B = `WIDTH;\nendmodule\n";
        let (usages, _, _) = scan_one(&["WIDTH=4"], text);
        assert_eq!(usage_for(&usages, "WIDTH").body, "4");
        assert!(usage_for(&usages, "WIDTH").from_config);
        let after = usages
            .iter()
            .find(|u| u.line0 == 3)
            .and_then(|u| u.definition.as_ref())
            .expect("resolved post-redefine usage");
        assert_eq!(after.body, "8");
        assert!(!after.from_config);
        assert_eq!(after.line1, 3);
        assert_eq!(after.file.as_deref(), Some("t.sv"));
    }

    #[test]
    fn use_before_define_resolves_to_config_only() {
        let text = "localparam int A = `W;\n`define W 2\nlocalparam int B = `W;\n";
        let (usages, _, _) = scan_one(&["W=1"], text);
        assert_eq!(usage_for(&usages, "W").body, "1");
        assert_eq!(
            usages
                .iter()
                .find(|u| u.line0 == 2)
                .and_then(|u| u.definition.as_ref())
                .map(|d| d.body.clone()),
            Some("2".to_owned())
        );
    }

    #[test]
    fn redefinition_last_wins() {
        let text = "`define V 1\nx = `V;\n`define V 2\ny = `V;\n";
        let (usages, _, _) = scan_one(&[], text);
        assert_eq!(usage_for(&usages, "V").body, "1");
        assert_eq!(
            usages
                .iter()
                .find(|u| u.line0 == 3)
                .and_then(|u| u.definition.as_ref())
                .map(|d| d.body.clone()),
            Some("2".to_owned())
        );
    }

    #[test]
    fn undef_removes_source_define() {
        let text = "`define W 8\n`undef W\nx = `W;\n";
        let (usages, _, _) = scan_one(&[], text);
        let only = usages.first().expect("one usage");
        assert!(only.definition.is_none(), "`undef must remove the macro");
    }

    #[test]
    fn comments_and_strings_hide_directives_and_usages() {
        let text = concat!(
            "// `define FAKE 1\n",
            "/* `define FAKE2 2\n   `FAKE2 */\n",
            "localparam string S = \"inside `FAKE3 literal\";\n",
            "x = `FAKE + `REAL;\n",
            "`define REAL 9\n",
        );
        let args = vec!["REAL=5".to_owned()];
        let table = build_table(&args, &[("t.sv", text)], None);
        let entries = table.files.get("t.sv").expect("scanned");
        // Only the two usages on the x = line are real.
        assert_eq!(entries.usages.len(), 2, "{:?}", entries.usages);
        assert!(entries.usages.iter().all(|u| u.line0 == 4));
        assert!(
            entries.usages[0].definition.is_none(),
            "FAKE stays undefined"
        );
        assert_eq!(entries.usages[1].definition.as_ref().unwrap().body, "5");
        // No definition leaked from comments; only the trailing real define.
        assert_eq!(table.files["t.sv"].decls.len(), 1);
        assert_eq!(table.files["t.sv"].decls[0].definition.name, "REAL");
    }

    #[test]
    fn false_ifdef_branches_are_skipped() {
        let text = concat!(
            "`ifdef MISSING\n",
            "`define WRONG 1\n",
            "`elsif ALSO_MISSING\n",
            "`define WRONG2 2\n",
            "`else\n",
            "`define RIGHT 3\n",
            "`endif\n",
            "x = `RIGHT;\n",
        );
        let (usages, decls, _) = scan_one(&[], text);
        assert_eq!(decls.len(), 1, "only the taken branch defines");
        assert_eq!(decls[0].definition.name, "RIGHT");
        assert_eq!(usage_for(&usages, "RIGHT").body, "3");
        assert!(
            !usages
                .iter()
                .any(|u| u.name() == "WRONG" || u.name() == "WRONG2"),
            "dead-region usages are not recorded"
        );
    }

    #[test]
    fn parenthesized_condition_selects_branches_like_the_dimmer() {
        // One optional surrounding paren pair is part of the shared grammar
        // with the inactive-region scanner.
        let defined = concat!(
            "`define PAREN 1\n",
            "`ifdef (PAREN)\n",
            "`define INSIDE 2\n",
            "`endif\n",
            "x = `INSIDE;\n",
        );
        let (usages, decls, _) = scan_one(&[], defined);
        assert_eq!(decls.len(), 2, "PAREN and INSIDE both define");
        assert_eq!(decls[1].definition.name, "INSIDE");
        assert_eq!(usage_for(&usages, "INSIDE").body, "2");

        let undefined = concat!(
            "`ifdef (PAREN)\n",
            "`define HIDDEN 2\n",
            "`else\n",
            "`define SHOWN 3\n",
            "`endif\n",
            "x = `SHOWN;\n",
        );
        let (_usages, decls, _) = scan_one(&[], undefined);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].definition.name, "SHOWN");

        let elsif_paren = concat!(
            "`ifdef A\n",
            "`define FROM_A 1\n",
            "`elsif (B)\n",
            "`define FROM_B 2\n",
            "`endif\n",
            "x = `FROM_B;\n",
        );
        let (usages, decls, _) = scan_one(&["B"], elsif_paren);
        assert_eq!(decls.len(), 1);
        assert_eq!(usage_for(&usages, "FROM_B").body, "2");
    }

    #[test]
    fn unparseable_condition_fails_safe_like_the_dimmer() {
        // An unparseable condition opens an always-active branch that hides
        // nothing and stays untaken — mirroring the inactive-range scanner's
        // fail-safe policy so hover never contradicts dimming.
        let bare = concat!(
            "`ifdef\n",
            "`define BARE 1\n",
            "`else\n",
            "`define FROM_ELSE 2\n",
            "`endif\n",
            "x = `BARE + `FROM_ELSE;\n",
        );
        let (usages, decls, _) = scan_one(&[], bare);
        assert_eq!(decls.len(), 2, "both branches stay live after a bare ifdef");
        assert_eq!(usage_for(&usages, "BARE").body, "1");
        assert_eq!(usage_for(&usages, "FROM_ELSE").body, "2");

        let double_paren = concat!(
            "`ifdef ((FOO)\n",
            "`define STILL_LIVE 1\n",
            "`endif\n",
            "x = `STILL_LIVE;\n",
        );
        let (usages, decls, _) = scan_one(&[], double_paren);
        assert_eq!(decls.len(), 1);
        assert_eq!(usage_for(&usages, "STILL_LIVE").body, "1");
    }

    #[test]
    fn nested_ifdef_tracks_parent_state() {
        let text = concat!(
            "`define OUTER 1\n",
            "`ifdef OUTER\n",
            "`ifndef INNER\n",
            "`define NESTED 7\n",
            "`endif\n",
            "`endif\n",
            "x = `NESTED;\n",
        );
        let (usages, _, _) = scan_one(&[], text);
        assert_eq!(usage_for(&usages, "NESTED").body, "7");
    }

    #[test]
    fn multiline_body_continuation_is_joined() {
        let text = "`define LONG \\\n  alpha + \\\n  beta\nx = `LONG;\n";
        let (usages, decls, _) = scan_one(&[], text);
        assert_eq!(decls[0].definition.body, "alpha + beta");
        assert_eq!(usage_for(&usages, "LONG").body, "alpha + beta");
    }

    #[test]
    fn function_like_macro_captures_args() {
        let text = "`define MAX(a, b) ((a) > (b)) ? (a) : (b)\nx = `MAX(p, q);\n";
        let (usages, decls, _) = scan_one(&[], text);
        let def = &decls[0].definition;
        assert_eq!(
            def.args.as_deref(),
            Some(&["a".to_owned(), "b".to_owned()][..])
        );
        assert_eq!(def.body, "((a) > (b)) ? (a) : (b)");
        assert_eq!(usage_for(&usages, "MAX").args, def.args);
    }

    #[test]
    fn function_like_default_arguments_do_not_leak_into_body() {
        // Regression: `b=5` used to end argument parsing at the `=`, so the
        // displayed body started at "= 5) a+b".
        let text = "`define M(a, b=5) a+b\nx = `M(p);\n";
        let (usages, decls, _) = scan_one(&[], text);
        let def = &decls[0].definition;
        assert_eq!(
            def.args.as_deref(),
            Some(&["a".to_owned(), "b".to_owned()][..])
        );
        assert_eq!(def.body, "a+b");
        assert_eq!(usage_for(&usages, "M").body, "a+b");
    }

    #[test]
    fn default_argument_with_nested_comma_stays_intact() {
        let text = "`define N(a, b={1, 2}, c=3) c\nx = `N(p);\n";
        let (_, decls, _) = scan_one(&[], text);
        let def = &decls[0].definition;
        assert_eq!(
            def.args.as_deref(),
            Some(&["a".to_owned(), "b".to_owned(), "c".to_owned()][..])
        );
        assert_eq!(def.body, "c");
    }

    #[test]
    fn bare_define_has_empty_body_and_decl_hover_site() {
        let text = "`define EN\nx = `EN;\n";
        let (usages, decls, table) = scan_one(&[], text);
        assert_eq!(decls[0].definition.body, "");
        assert_eq!(decls[0].line0, 0);
        assert_eq!(usage_for(&usages, "EN").body, "");
        // The NAME identifier of the directive is itself hoverable.
        assert!(table.decl_at("t.sv", 0, 8).is_some());
        assert!(table.decl_at("t.sv", 0, 10).is_none());
    }

    #[test]
    fn usage_inside_define_body_resolves_at_definition_point() {
        let text = "`define BASE 6\n`define WRAPPED (`BASE + 1)\ny = `WRAPPED;\n";
        let (usages, _, _) = scan_one(&[], text);
        // The nested `BASE sits on line 1 inside the body.
        let nested = usages
            .iter()
            .find(|u| u.line0 == 1 && u.name() == "BASE")
            .expect("nested usage recorded");
        assert_eq!(nested.definition.as_ref().unwrap().body, "6");
    }

    #[test]
    fn directives_are_never_usages() {
        let text = "`timescale 1ns/1ps\n`include \"x.vh\"\n`resetall\n`define OK 1\nx = `OK;\n";
        let (usages, _, _) = scan_one(&[], text);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].name(), "OK");
    }

    #[test]
    fn undefineall_clears_everything() {
        let text = "`undefineall\nx = `GONE;\n";
        let args = vec!["GONE=1".to_owned()];
        let table = build_table(&args, &[("t.sv", text)], None);
        let usages = &table.files["t.sv"].usages;
        assert!(usages.first().unwrap().definition.is_none());
    }

    #[test]
    fn per_file_scoping_keeps_tables_independent() {
        let text_b = "y = `SHARED;\n";
        let args = vec!["SHARED=cfg".to_owned()];
        let table = build_table(
            &args,
            &[
                ("a.sv", "`define SHARED local\nx = `SHARED;\n"),
                ("b.sv", text_b),
            ],
            None,
        );
        let a_usage = &table.files["a.sv"].usages[0];
        let b_usage = &table.files["b.sv"].usages[0];
        assert_eq!(a_usage.definition.as_ref().unwrap().body, "local");
        assert_eq!(b_usage.definition.as_ref().unwrap().body, "cfg");
    }

    #[test]
    fn usage_spans_cover_the_backtick_and_name() {
        let text = "module m; endmodule\nx = `WIDTHY + 1;\n`define WIDTHY 3\n";
        let args = vec![];
        let table = build_table(&args, &[("t.sv", text)], None);
        // `` `WIDTHY `` starts at col 4 (0-based), length 7.
        let line = 1u32;
        assert!(table.usage_at("t.sv", line, 4).is_some());
        assert!(table.usage_at("t.sv", line, 10).is_some()); // last char
        assert!(table.usage_at("t.sv", line, 11).is_none()); // past the end
        assert!(table.usage_at("t.sv", line, 3).is_none()); // before tick
    }

    #[test]
    fn parse_config_defines_rejects_non_identifiers() {
        let args = vec![
            "GOOD=1".to_owned(),
            "BARE".to_owned(),
            "_BAD_NAME=x".to_owned(),
            "9BAD=2".to_owned(),
        ];
        // Leading underscores are valid SystemVerilog identifiers; an entry
        // beginning with a digit is dropped.
        let defs = parse_config_defines(&args);
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, "GOOD");
        assert_eq!(defs[0].body, "1");
        assert_eq!(defs[1].name, "BARE");
        assert_eq!(defs[1].body, "");
        assert_eq!(defs[2].name, "_BAD_NAME");
    }
}
