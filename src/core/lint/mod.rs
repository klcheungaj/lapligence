//! core::lint — shared Verilog/SystemVerilog linter.
//!
//! Rule engine over the owned database + design model.  Consumed by the LSP
//! (lint diagnostics) and the simulator (`llg --lint` gate).  No native AST
//! access, no raw FFI, no LSP dependencies.
//!
//! The default rule set lives in [`rules::default_rules`] and currently runs
//! 24 rules in a stable order (`unused-signal`, `width-mismatch`,
//! `incomplete-case`, `combinational-loop`, `multi-driver`, `casez-misuse`,
//! `if-latch`, `naming-style`, `blocking-in-always_ff`, `nba-in-always_comb`,
//! `unused-parameter`, `implicit-net`, `case-default-missing`,
//! `comparison-width-mismatch`, `unconnected-port`, `mixed-assignments`,
//! `undriven-signal`, `incomplete-sensitivity-list`, `out-of-range-select`,
//! `xz-logical-equality`, `duplicate-case-item`,
//! `empty-implicit-sensitivity`, `assignment-in-condition`,
//! `casex-statement`);
//! treat that registry as the source of truth rather than this list.

pub mod rules;

use std::collections::HashMap;

use crate::core::db::Db;
use crate::core::model::DesignModel;

/// Severity of a lint finding, mirroring the compiler severity ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

/// Per-rule configuration: enabled + severity override.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleConfig {
    pub enabled: bool,
    pub severity: Option<LintSeverity>,
}

impl Default for RuleConfig {
    /// The default for an unconfigured rule: enabled, no severity override.
    fn default() -> Self {
        RuleConfig {
            enabled: true,
            severity: None,
        }
    }
}

/// Lint configuration.  Missing rules use defaults (enabled, rule's own severity).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LintConfig {
    rules: HashMap<String, RuleConfig>,
}

impl LintConfig {
    /// An empty configuration: every rule enabled with no severity overrides.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration for one rule.
    pub fn set(&mut self, rule: impl Into<String>, cfg: RuleConfig) {
        self.rules.insert(rule.into(), cfg);
    }

    /// The configuration for `rule`, or the default (enabled, no override).
    pub fn get(&self, rule: &str) -> RuleConfig {
        self.rules.get(rule).copied().unwrap_or_default()
    }

    /// Whether `rule` is enabled (default: yes).
    pub fn is_enabled(&self, rule: &str) -> bool {
        self.get(rule).enabled
    }

    /// Severity override for `rule`, or `None` when the rule's own severity
    /// should be used.
    pub fn severity(&self, rule: &str) -> Option<LintSeverity> {
        self.rules.get(rule).and_then(|c| c.severity)
    }

    /// Parse a `llg-lint.toml` text (see `src/core/lint/readme.md`).
    ///
    /// Errors are reported as a `Vec<String>` of human-readable messages; on
    /// any error the partial parse result is still returned (best-effort).
    pub fn parse_toml(&mut self, text: &str) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut section: Option<String> = None;
        // A malformed or unknown section header disables key parsing until the
        // next valid header; the header error itself is reported once.
        let mut section_valid = true;

        for (idx, raw) in text.lines().enumerate() {
            let lineno = idx + 1;
            let line = strip_comment(raw).trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) else {
                    errors.push(format!("line {lineno}: malformed section header `{line}`"));
                    section = None;
                    section_valid = false;
                    continue;
                };
                let id = inner
                    .trim()
                    .strip_prefix("rules.")
                    .map(str::trim)
                    .filter(|id| !id.is_empty());
                match id {
                    Some(id) if known_rule(id) => {
                        section = Some(id.to_string());
                        section_valid = true;
                    }
                    Some(id) => {
                        errors.push(format!("line {lineno}: unknown rule `{id}`"));
                        section = None;
                        section_valid = false;
                    }
                    None => {
                        errors.push(format!(
                            "line {lineno}: expected `[rules.<id>]` section, got `[{inner}]`"
                        ));
                        section = None;
                        section_valid = false;
                    }
                }
                continue;
            }
            if !section_valid {
                continue;
            }
            let Some(section) = &section else {
                errors.push(format!(
                    "line {lineno}: key/value outside a `[rules.<id>]` section"
                ));
                continue;
            };
            let Some((key, value)) = line.split_once('=') else {
                errors.push(format!(
                    "line {lineno}: malformed line `{line}` (expected `key = value`)"
                ));
                continue;
            };
            match (key.trim(), value.trim()) {
                ("enabled", "true") => {
                    self.rules.entry(section.clone()).or_default().enabled = true;
                }
                ("enabled", "false") => {
                    self.rules.entry(section.clone()).or_default().enabled = false;
                }
                ("enabled", v) => {
                    errors.push(format!(
                        "line {lineno}: invalid `enabled` value `{v}` (expected true or false)"
                    ));
                }
                ("severity", v) => match parse_severity(v) {
                    Some(sev) => {
                        self.rules.entry(section.clone()).or_default().severity = Some(sev);
                    }
                    None => {
                        errors.push(format!(
                            "line {lineno}: invalid `severity` value `{v}` (expected \"error\", \"warning\" or \"info\")"
                        ));
                    }
                },
                (k, _) => {
                    errors.push(format!(
                        "line {lineno}: unknown key `{k}` (expected `enabled` or `severity`)"
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Strip a trailing `#` comment (the `#` must be preceded by whitespace, per TOML).
fn strip_comment(line: &str) -> &str {
    match line.find(" #") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Parse a quoted `"error"` / `"warning"` / `"info"` severity value.
fn parse_severity(value: &str) -> Option<LintSeverity> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    match inner {
        "error" => Some(LintSeverity::Error),
        "warning" => Some(LintSeverity::Warning),
        "info" => Some(LintSeverity::Info),
        _ => None,
    }
}

/// Whether `id` is the id of a registered default rule.
fn known_rule(id: &str) -> bool {
    LintRegistry::default_rules()
        .all()
        .iter()
        .any(|r| r.id() == id)
}

/// One lint finding produced by a rule.
#[derive(Debug, Clone, PartialEq)]
pub struct LintDiag {
    /// Stable rule id, e.g. `"unused-signal"`.
    pub rule: String,
    pub severity: LintSeverity,
    pub file: Option<String>,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
    pub message: String,
}

/// Context handed to every rule: the owned db + model (+ anything rules need
/// later, e.g. a per-signal read/write analysis cache — add fields later).
pub struct LintCtx<'a> {
    pub db: &'a Db,
    pub model: &'a DesignModel,
}

/// A lint rule.  Implementations are stateless; all state lives in LintCtx.
pub trait LintRule {
    /// Stable rule identifier (lowercase, hyphenated).
    fn id(&self) -> &'static str;
    /// Human-readable one-line description (used in docs / config).
    fn description(&self) -> &'static str;
    /// Run the rule; return all findings.
    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag>;
}

/// The rule registry: all rules enabled by default, in a stable order.
pub struct LintRegistry {
    rules: Vec<Box<dyn LintRule>>,
}

impl LintRegistry {
    /// The registry with every default (always-on) rule, in run order.
    pub fn default_rules() -> Self {
        LintRegistry {
            rules: rules::default_rules(),
        }
    }

    /// All registered rules, in run order.
    pub fn all(&self) -> &[Box<dyn LintRule>] {
        &self.rules
    }

    /// Run every enabled rule over `ctx`, concatenating findings in rule order.
    /// Disabled rules are skipped; `config` severity overrides replace the
    /// rule's own severity.
    pub fn lint(&self, ctx: &LintCtx<'_>, config: &LintConfig) -> Vec<LintDiag> {
        let mut out = Vec::new();
        for rule in &self.rules {
            let id = rule.id();
            if !config.is_enabled(id) {
                continue;
            }
            let override_sev = config.severity(id);
            for mut diag in rule.check(ctx) {
                if let Some(sev) = override_sev {
                    diag.severity = sev;
                }
                out.push(diag);
            }
        }
        out
    }
}

/// Convenience: run all default rules over a db + model.
pub fn lint(db: &Db, model: &DesignModel) -> Vec<LintDiag> {
    lint_with_config(db, model, &LintConfig::default())
}

/// Convenience: run all default rules over a db + model with a config.
pub fn lint_with_config(db: &Db, model: &DesignModel, config: &LintConfig) -> Vec<LintDiag> {
    let registry = LintRegistry::default_rules();
    registry.lint(&LintCtx { db, model }, config)
}

/// Serialize lint findings as a single machine-readable JSON object
/// (hand-built; the crate has no JSON dependency).
///
/// Schema:
///
/// ```json
/// {
///   "diagnostics": [
///     {
///       "rule": "unused-signal",
///       "severity": "warning",
///       "file": "/abs/path.sv",
///       "line": 3,
///       "col": 10,
///       "message": "signal `b` in `unused` is never used"
///     }
///   ],
///   "summary": {"errors": 0, "warnings": 1, "infos": 0, "total": 1}
/// }
/// ```
///
/// `severity` is one of `"error"`, `"warning"`, `"info"`; `file` is a string
/// or `null`; `line`/`col` are 1-based integers.  Strings are escaped per
/// JSON: `"` → `\"`, `\` → `\\`, and control characters U+0000..U+001F use
/// the standard `\b`/`\f`/`\n`/`\r`/`\t` or `\u00xx` forms.
pub fn diags_to_json(diags: &[LintDiag]) -> String {
    let errors = diags
        .iter()
        .filter(|d| d.severity == LintSeverity::Error)
        .count();
    let warnings = diags
        .iter()
        .filter(|d| d.severity == LintSeverity::Warning)
        .count();
    let infos = diags
        .iter()
        .filter(|d| d.severity == LintSeverity::Info)
        .count();

    let mut out = String::new();
    out.push_str("{\n  \"diagnostics\": [");
    for (i, d) in diags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n");
        out.push_str(&format!("      \"rule\": \"{}\",\n", json_escape(&d.rule)));
        out.push_str(&format!(
            "      \"severity\": \"{}\",\n",
            severity_json(d.severity)
        ));
        match &d.file {
            Some(f) => out.push_str(&format!("      \"file\": \"{}\",\n", json_escape(f))),
            None => out.push_str("      \"file\": null,\n"),
        }
        out.push_str(&format!("      \"line\": {},\n", d.line));
        out.push_str(&format!("      \"col\": {},\n", d.col));
        out.push_str(&format!(
            "      \"message\": \"{}\"\n",
            json_escape(&d.message)
        ));
        out.push_str("    }");
    }
    if diags.is_empty() {
        out.push_str("],\n");
    } else {
        out.push_str("\n  ],\n");
    }
    out.push_str(&format!(
        "  \"summary\": {{\"errors\": {errors}, \"warnings\": {warnings}, \"infos\": {infos}, \"total\": {}}}\n}}",
        diags.len()
    ));
    out
}

/// JSON-escape a string: `"` `\` and control characters U+0000..U+001F.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// The JSON string for a lint severity.
fn severity_json(sev: LintSeverity) -> &'static str {
    match sev {
        LintSeverity::Error => "error",
        LintSeverity::Warning => "warning",
        LintSeverity::Info => "info",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compile;

    /// Run a compile with the CWD in a fresh temp dir, serialized against the
    /// other working-directory-mutating tests.  Restores the CWD and cleans up even
    /// on panic, so a failing test cannot strand other tests in a deleted CWD.
    fn in_temp_dir<R>(f: impl FnOnce() -> R) -> R {
        let _guard = crate::core::lint::rules::tests::CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        struct Restore(std::path::PathBuf, std::path::PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
                let _ = std::fs::remove_dir_all(&self.1);
            }
        }
        let dir = std::env::temp_dir().join(format!("llg_lint_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = Restore(orig_cwd.clone(), dir.clone());
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        f()
    }

    /// Compile a tiny module and run the default lint pipeline over its
    /// db + model, asserting the registry wiring works end to end.  The only
    /// signal is driven by a continuous assignment, so every rule stays quiet
    /// (cont-assign-driven signals are exempt from unused-signal).
    #[test]
    fn lint_over_compiled_design_is_empty() {
        in_temp_dir(|| {
            let dir = std::env::current_dir().expect("temp dir");
            let sv = dir.join("tiny.sv");
            std::fs::write(
                &sv,
                "module tiny;\n  logic a;\n  assign a = 1'b0;\nendmodule\n",
            )
            .expect("write design");
            let out = compile::compile_checked(&compile::CompileOpts {
                files: vec![sv.to_string_lossy().into_owned()],
                ..Default::default()
            })
            .expect("compile should start");
            assert!(out.ok(), "compile must succeed: {:?}", out.diagnostics);
            let db = Db::from_slang(&out.snapshot).expect("semantic capture");
            let model = DesignModel::from_db(&db);

            assert!(lint(&db, &model).is_empty());
        });
    }

    /// The registry builds with exactly the default rules, in stable order.
    #[test]
    fn registry_builds_with_default_rules() {
        let registry = LintRegistry::default_rules();
        let ids: Vec<&str> = registry.all().iter().map(|r| r.id()).collect();
        assert_eq!(
            ids,
            vec![
                "unused-signal",
                "width-mismatch",
                "incomplete-case",
                "combinational-loop",
                "multi-driver",
                "casez-misuse",
                "if-latch",
                "naming-style",
                "blocking-in-always_ff",
                "nba-in-always_comb",
                "unused-parameter",
                "implicit-net",
                "case-default-missing",
                "comparison-width-mismatch",
                "unconnected-port",
                "mixed-assignments",
                "undriven-signal",
                "incomplete-sensitivity-list",
                "out-of-range-select",
                "xz-logical-equality",
                "duplicate-case-item",
                "empty-implicit-sensitivity",
                "assignment-in-condition",
                "casex-statement",
            ]
        );
        for rule in registry.all() {
            assert!(!rule.description().is_empty());
        }
    }

    /// All LintDiag fields are public and readable.
    #[test]
    fn lint_diag_fields_are_accessible() {
        let diag = LintDiag {
            rule: "unused-signal".to_string(),
            severity: LintSeverity::Warning,
            file: Some("tiny.sv".to_string()),
            line: 2,
            col: 9,
            message: "scaffolding finding".to_string(),
        };
        assert_eq!(diag.rule, "unused-signal");
        assert_eq!(diag.severity, LintSeverity::Warning);
        assert_eq!(diag.file.as_deref(), Some("tiny.sv"));
        assert_eq!((diag.line, diag.col), (2, 9));
        assert_eq!(diag.message, "scaffolding finding");
    }

    /// `set`/`get` roundtrip and default values for unconfigured rules.
    #[test]
    fn config_set_and_get_roundtrip() {
        let mut cfg = LintConfig::new();
        assert_eq!(
            cfg.get("unconfigured-rule"),
            RuleConfig {
                enabled: true,
                severity: None,
            }
        );
        assert!(cfg.is_enabled("unconfigured-rule"));
        assert_eq!(cfg.severity("unconfigured-rule"), None);

        cfg.set(
            "width-mismatch",
            RuleConfig {
                enabled: false,
                severity: Some(LintSeverity::Info),
            },
        );
        assert_eq!(
            cfg.get("width-mismatch"),
            RuleConfig {
                enabled: false,
                severity: Some(LintSeverity::Info),
            }
        );
        assert!(!cfg.is_enabled("width-mismatch"));
        assert_eq!(cfg.severity("width-mismatch"), Some(LintSeverity::Info));
    }

    /// A valid `llg-lint.toml` parses and applies enabled + severity overrides.
    #[test]
    fn config_parse_valid_toml() {
        let mut cfg = LintConfig::new();
        cfg.parse_toml(
            r#"
            # llg-lint.toml
            [rules.unused-signal]
            enabled = false
            severity = "warning"   # inline comment

            [rules.width-mismatch]
            severity = "info"
            "#,
        )
        .expect("valid toml parses");
        assert!(!cfg.is_enabled("unused-signal"));
        assert_eq!(cfg.severity("unused-signal"), Some(LintSeverity::Warning));
        assert_eq!(
            cfg.get("width-mismatch"),
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Info),
            }
        );
        // Unmentioned rules keep their defaults.
        assert!(cfg.is_enabled("casez-misuse"));
        assert_eq!(cfg.severity("casez-misuse"), None);
    }

    /// Unknown rule ids are reported as errors.
    #[test]
    fn config_parse_rejects_unknown_rule() {
        let mut cfg = LintConfig::new();
        let errs = cfg
            .parse_toml("[rules.nope]\nenabled = false\n")
            .unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("unknown rule `nope`")),
            "{errs:?}"
        );
    }

    /// Bad severity values (unknown or unquoted) are reported as errors.
    #[test]
    fn config_parse_rejects_bad_severity() {
        let mut cfg = LintConfig::new();
        let errs = cfg
            .parse_toml("[rules.width-mismatch]\nseverity = \"fatal\"\n")
            .unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("invalid `severity` value")),
            "{errs:?}"
        );
        let errs = cfg
            .parse_toml("[rules.width-mismatch]\nseverity = error\n")
            .unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("invalid `severity` value")),
            "{errs:?}"
        );
    }

    /// Bad `enabled` values are reported as errors.
    #[test]
    fn config_parse_rejects_bad_enabled() {
        let mut cfg = LintConfig::new();
        let errs = cfg
            .parse_toml("[rules.unused-signal]\nenabled = maybe\n")
            .unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("invalid `enabled` value")),
            "{errs:?}"
        );
    }

    /// Malformed headers and keys outside sections are reported as errors.
    #[test]
    fn config_parse_rejects_malformed_lines() {
        let mut cfg = LintConfig::new();
        let errs = cfg.parse_toml("enabled = false\n").unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.contains("outside a `[rules.<id>]` section")),
            "{errs:?}"
        );

        let errs = cfg.parse_toml("[rules]\n").unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.contains("expected `[rules.<id>]` section")),
            "{errs:?}"
        );

        let errs = cfg
            .parse_toml("[rules.unused-signal\nenabled = false\n")
            .unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("malformed section header")),
            "{errs:?}"
        );
    }

    /// On parse errors the valid entries parsed so far are still applied.
    #[test]
    fn config_parse_keeps_partial_result_on_error() {
        let mut cfg = LintConfig::new();
        let text = "[rules.unused-signal]\nenabled = false\n[rules.nope]\nenabled = true\n";
        assert!(cfg.parse_toml(text).is_err());
        assert!(
            !cfg.is_enabled("unused-signal"),
            "valid entry before the error is kept"
        );
    }

    /// Disabling a rule removes its findings from `lint_with_config` output.
    #[test]
    fn config_disables_a_rule() {
        let (db, model) = crate::core::lint::rules::tests::build_design(
            "module unused;\n  logic a;\n  assign a = 1'b0;\n  logic b;\nendmodule\n",
            "unused",
        );
        let default = lint(&db, &model);
        assert_eq!(
            crate::core::lint::rules::tests::rule_diags(&default, "unused-signal").len(),
            1,
            "sanity: default lint finds the unused signal: {default:?}"
        );

        let mut cfg = LintConfig::new();
        cfg.set(
            "unused-signal",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        assert!(
            crate::core::lint::rules::tests::rule_diags(&diags, "unused-signal").is_empty(),
            "{diags:?}"
        );
    }

    /// A severity override replaces the rule's own severity in the output.
    #[test]
    fn config_severity_override() {
        let (db, model) = crate::core::lint::rules::tests::build_design(
            "module wm;\n  logic [3:0] x;\n  logic [7:0] y;\n  assign y = x;\nendmodule\n",
            "wm",
        );
        let default = lint(&db, &model);
        let width = crate::core::lint::rules::tests::rule_diags(&default, "width-mismatch");
        assert!(
            !width.is_empty(),
            "sanity: default lint finds a width mismatch"
        );
        assert!(
            width.iter().all(|d| d.severity == LintSeverity::Info),
            "extension is Info by default: {default:?}"
        );

        let mut cfg = LintConfig::new();
        cfg.set(
            "width-mismatch",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let width = crate::core::lint::rules::tests::rule_diags(&diags, "width-mismatch");
        assert!(!width.is_empty(), "{diags:?}");
        assert!(
            width.iter().all(|d| d.severity == LintSeverity::Error),
            "override is applied: {diags:?}"
        );
    }

    /// An empty finding list serializes to a JSON object with empty diagnostics.
    #[test]
    fn diags_to_json_empty() {
        assert_eq!(
            diags_to_json(&[]),
            "{\n  \"diagnostics\": [],\n  \"summary\": {\"errors\": 0, \"warnings\": 0, \"infos\": 0, \"total\": 0}\n}"
        );
    }

    /// All severities map to their JSON strings and `file: null` is emitted
    /// for findings without a file.
    #[test]
    fn diags_to_json_severity_and_null_file() {
        let diags = vec![
            LintDiag {
                rule: "r-error".to_string(),
                severity: LintSeverity::Error,
                file: None,
                line: 1,
                col: 2,
                message: "err".to_string(),
            },
            LintDiag {
                rule: "r-info".to_string(),
                severity: LintSeverity::Info,
                file: Some("a.sv".to_string()),
                line: 3,
                col: 4,
                message: "inf".to_string(),
            },
        ];
        let json = diags_to_json(&diags);
        assert!(json.contains("\"rule\": \"r-error\""), "{json}");
        assert!(json.contains("\"severity\": \"error\""), "{json}");
        assert!(json.contains("\"file\": null"), "{json}");
        assert!(json.contains("\"line\": 1"), "{json}");
        assert!(json.contains("\"col\": 2"), "{json}");
        assert!(json.contains("\"rule\": \"r-info\""), "{json}");
        assert!(json.contains("\"severity\": \"info\""), "{json}");
        assert!(json.contains("\"file\": \"a.sv\""), "{json}");
        assert!(
            json.contains(
                "\"summary\": {\"errors\": 1, \"warnings\": 0, \"infos\": 1, \"total\": 2}"
            ),
            "{json}"
        );
    }

    /// Strings are JSON-escaped: `"`, `\`, newline and other control chars.
    #[test]
    fn diags_to_json_escapes_strings() {
        let diag = LintDiag {
            rule: "quote\"rule".to_string(),
            severity: LintSeverity::Warning,
            file: Some("a\"b\\c.sv".to_string()),
            line: 0,
            col: 0,
            message: "say \"hi\"\nnext\tline\\end\u{1}".to_string(),
        };
        let json = diags_to_json(&[diag]);
        assert!(
            json.contains("\"rule\": \"quote\\\"rule\""),
            "rule escaped: {json}"
        );
        assert!(
            json.contains("\"file\": \"a\\\"b\\\\c.sv\""),
            "file escaped: {json}"
        );
        assert!(
            json.contains("\"message\": \"say \\\"hi\\\"\\nnext\\tline\\\\end\\u0001\""),
            "message escaped: {json}"
        );
    }

    /// Severity strings are exactly `error` / `warning` / `info`.
    #[test]
    fn diags_to_json_severity_strings() {
        let diags = vec![
            LintDiag {
                rule: "r".to_string(),
                severity: LintSeverity::Error,
                file: None,
                line: 0,
                col: 0,
                message: String::new(),
            },
            LintDiag {
                rule: "r".to_string(),
                severity: LintSeverity::Warning,
                file: None,
                line: 0,
                col: 0,
                message: String::new(),
            },
        ];
        let json = diags_to_json(&diags);
        assert!(json.contains("\"severity\": \"error\""), "{json}");
        assert!(json.contains("\"severity\": \"warning\""), "{json}");
        assert!(!json.contains("\"severity\": \"ERROR\""), "{json}");
    }
}
