//! The message catalog: every `llg` message id with its default severity,
//! meaning and how to disable it.
//!
//! Host messages are the constants below; lint rules are [`LINT_RULES`];
//! by-design rejections are [`LEGACY_FAMILIES`]; generated-model runtime
//! messages come from `src/sim/rt/llg_msg_ids.def`, which the runtime compiles
//! too. Frontend (Slang) diagnostics are not copied: their id is
//! `SLANG-<name>` with the name from Slang's own diagnostic list
//! (`vendor/slang/scripts/diagnostics.txt`). [`markdown`] renders
//! `docs/messages.md`; a drift test keeps the file equal to it.

use super::Severity;
use std::sync::OnceLock;

const RUNTIME_IDS_DEF: &str = include_str!("../../sim/rt/llg_msg_ids.def");

/// The tool or step that reports a message; the first part of its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Cli,
    Config,
    Host,
    Slang,
    Elab,
    Lint,
    Codegen,
    Opt,
    Build,
    Run,
    Sim,
}

impl Tool {
    pub const ALL: [Tool; 11] = [
        Tool::Cli,
        Tool::Config,
        Tool::Host,
        Tool::Slang,
        Tool::Elab,
        Tool::Lint,
        Tool::Codegen,
        Tool::Opt,
        Tool::Build,
        Tool::Run,
        Tool::Sim,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Cli => "CLI",
            Tool::Config => "CONFIG",
            Tool::Host => "HOST",
            Tool::Slang => "SLANG",
            Tool::Elab => "ELAB",
            Tool::Lint => "LINT",
            Tool::Codegen => "CODEGEN",
            Tool::Opt => "OPT",
            Tool::Build => "BUILD",
            Tool::Run => "RUN",
            Tool::Sim => "SIM",
        }
    }

    pub fn from_name(name: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn description(self) -> &'static str {
        match self {
            Tool::Cli => "command-line arguments of `llg`",
            Tool::Config => "the `--config` file and environment settings",
            Tool::Host => "the `llg` process itself (files it writes, memory guard)",
            Tool::Slang => "frontend diagnostics from Slang (parse, elaboration, type checks)",
            Tool::Elab => "llg checks of the elaborated design before lint and code generation",
            Tool::Lint => "lint rules",
            Tool::Codegen => "lowering and C code generation",
            Tool::Opt => "the optimizer (reserved: it reports nothing today)",
            Tool::Build => "CMake and the C compiler building the model",
            Tool::Run => "how the model process ended, seen by `llg`",
            Tool::Sim => "the generated model at run time",
        }
    }
}

/// One catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDef {
    pub tool: Tool,
    pub index: u16,
    pub severity: Severity,
    pub meaning: &'static str,
    pub disable: &'static str,
}

impl MessageDef {
    /// The rendered id, e.g. `CLI-003`.
    pub fn id(&self) -> String {
        super::id(self.tool.name(), self.index)
    }
}

const fn def(
    tool: Tool,
    index: u16,
    severity: Severity,
    meaning: &'static str,
    disable: &'static str,
) -> MessageDef {
    MessageDef {
        tool,
        index,
        severity,
        meaning,
        disable,
    }
}

const NONE: &str = "-";
const SUMMARY: &str = "end-of-phase counts, printed when the phase reported a warning or worse";

pub const CLI_UNSUPPORTED_PLATFORM: MessageDef = def(
    Tool::Cli,
    1,
    Severity::Error,
    "the simulator driver does not run on this platform (exit 2)",
    NONE,
);
pub const CLI_UNKNOWN_OPTION: MessageDef = def(
    Tool::Cli,
    2,
    Severity::Error,
    "an unknown `-` option (exit 2)",
    NONE,
);
pub const CLI_OPTION_VALUE: MessageDef = def(
    Tool::Cli,
    3,
    Severity::Error,
    "an option's value is missing or invalid (exit 2)",
    NONE,
);
pub const CLI_NO_SOURCES: MessageDef = def(
    Tool::Cli,
    4,
    Severity::Error,
    "no source files were named (exit 2)",
    NONE,
);
pub const CLI_IGNORED_OPTION: MessageDef = def(
    Tool::Cli,
    5,
    Severity::Warning,
    "build options are ignored with `--gen-only`",
    "leave the build options out",
);

pub const CONFIG_MISSING: MessageDef = def(
    Tool::Config,
    1,
    Severity::Error,
    "the `--config` file does not exist or cannot be read (exit 1)",
    NONE,
);
pub const CONFIG_INVALID: MessageDef = def(
    Tool::Config,
    2,
    Severity::Error,
    "a `--config` key or value is invalid; one message per error (exit 1)",
    NONE,
);
pub const CONFIG_WARNING: MessageDef = def(
    Tool::Config,
    3,
    Severity::Warning,
    "a `--config` key is accepted but has no effect for `llg`",
    "remove the key",
);
pub const CONFIG_ENVIRONMENT: MessageDef = def(
    Tool::Config,
    4,
    Severity::Error,
    "an environment setting is invalid (exit 1)",
    NONE,
);

pub const HOST_WRITE_FAILED: MessageDef = def(
    Tool::Host,
    1,
    Severity::Error,
    "`llg` cannot write an output file (lint JSON, import library)",
    NONE,
);
pub const HOST_WRITE_WARNING: MessageDef = def(
    Tool::Host,
    2,
    Severity::Warning,
    "`llg` cannot write an optional file (an `$sdf_annotate` log)",
    NONE,
);
pub const HOST_MEMORY_WARNING: MessageDef = def(
    Tool::Host,
    3,
    Severity::Warning,
    "process memory guard warning",
    "unset `LLG_MEMORY_LIMIT_MB`",
);
pub const HOST_MEMORY_INFO: MessageDef = def(
    Tool::Host,
    4,
    Severity::Info,
    "process memory guard status",
    "unset `LLG_MEMORY_LIMIT_MB`",
);

pub const SLANG_SUMMARY: MessageDef = def(Tool::Slang, 0, Severity::Info, SUMMARY, NONE);

pub const ELAB_SUMMARY: MessageDef = def(Tool::Elab, 0, Severity::Info, SUMMARY, NONE);
pub const ELAB_FRONTEND_START: MessageDef = def(
    Tool::Elab,
    1,
    Severity::Error,
    "the frontend could not start (sources unreadable, limits)",
    NONE,
);
pub const ELAB_DB_BUILD: MessageDef = def(
    Tool::Elab,
    2,
    Severity::Error,
    "the owned design database could not be built from the frontend snapshot",
    NONE,
);
pub const ELAB_PROTECTED: MessageDef = def(
    Tool::Elab,
    3,
    Severity::Error,
    "protected source envelopes are rejected by design",
    NONE,
);
pub const ELAB_EDITION: MessageDef = def(
    Tool::Elab,
    4,
    Severity::Error,
    "a construct outside the selected `--edition` profile",
    "select another `--edition`",
);

pub const LINT_SUMMARY: MessageDef = def(
    Tool::Lint,
    0,
    Severity::Info,
    "lint counts; printed when lint reported a warning or worse, and always with `--lint-only`",
    NONE,
);

pub const CODEGEN_SUMMARY: MessageDef = def(Tool::Codegen, 0, Severity::Info, SUMMARY, NONE);
pub const CODEGEN_UNSUPPORTED: MessageDef = def(
    Tool::Codegen,
    1,
    Severity::Error,
    "a construct or form llg does not simulate yet",
    NONE,
);
pub const CODEGEN_SEMANTIC: MessageDef = def(
    Tool::Codegen,
    2,
    Severity::Error,
    "an illegal design found during lowering (conflicting drivers, illegal targets, process rules)",
    NONE,
);
pub const CODEGEN_TYPE: MessageDef = def(
    Tool::Codegen,
    3,
    Severity::Error,
    "an operand or target of the wrong type or width",
    NONE,
);
pub const CODEGEN_ARGUMENT: MessageDef = def(
    Tool::Codegen,
    4,
    Severity::Error,
    "a system task or method called with the wrong number or kind of arguments",
    NONE,
);
pub const CODEGEN_LIMIT: MessageDef = def(
    Tool::Codegen,
    5,
    Severity::Error,
    "a design exceeds a runtime or generator limit",
    NONE,
);
pub const CODEGEN_INTERNAL: MessageDef = def(
    Tool::Codegen,
    6,
    Severity::Error,
    "an internal invariant of the generator failed (a bug in llg)",
    NONE,
);
pub const CODEGEN_OTHER: MessageDef = def(
    Tool::Codegen,
    9,
    Severity::Error,
    "any other lowering failure",
    NONE,
);
pub const CODEGEN_WARNING: MessageDef = def(
    Tool::Codegen,
    10,
    Severity::Warning,
    "a lowering warning (ignored annotation, approximated construct)",
    NONE,
);

pub const BUILD_IO: MessageDef = def(
    Tool::Build,
    1,
    Severity::Error,
    "a filesystem operation of the model build failed",
    NONE,
);
pub const BUILD_COMPILER_FLAG: MessageDef = def(
    Tool::Build,
    2,
    Severity::Error,
    "a `--cflags` token cannot pass through the CMake cache",
    NONE,
);
pub const BUILD_MODEL_ABI: MessageDef = def(
    Tool::Build,
    3,
    Severity::Error,
    "the generated model has an incompatible value ABI",
    NONE,
);
pub const BUILD_VALUE_CONFIG: MessageDef = def(
    Tool::Build,
    4,
    Severity::Error,
    "an invalid value backend selection or missing GMP input",
    NONE,
);
pub const BUILD_DPI_LIBRARY: MessageDef = def(
    Tool::Build,
    5,
    Severity::Error,
    "a `--dpi-lib` library is missing or unusable",
    NONE,
);
pub const BUILD_CMAKE_LAUNCH: MessageDef = def(
    Tool::Build,
    6,
    Severity::Error,
    "CMake could not be started",
    NONE,
);
pub const BUILD_CONFIGURE: MessageDef = def(
    Tool::Build,
    7,
    Severity::Error,
    "CMake configuration failed; the CMake output follows unchanged",
    NONE,
);
pub const BUILD_PROGRAM: MessageDef = def(
    Tool::Build,
    8,
    Severity::Error,
    "CMake found no build program for the generator",
    NONE,
);
pub const BUILD_COMPILE: MessageDef = def(
    Tool::Build,
    9,
    Severity::Error,
    "the C compiler failed; its output follows unchanged",
    NONE,
);
pub const BUILD_DPI_SYMBOL: MessageDef = def(
    Tool::Build,
    10,
    Severity::Error,
    "the model link left DPI-C symbols undefined",
    NONE,
);
pub const BUILD_IMPORT_LIBRARY: MessageDef = def(
    Tool::Build,
    11,
    Severity::Error,
    "an import library was requested from a non-MSVC toolchain",
    NONE,
);
pub const BUILD_NO_EXECUTABLE: MessageDef = def(
    Tool::Build,
    12,
    Severity::Error,
    "the build produced no model executable",
    NONE,
);
pub const BUILD_NO_RUNTIME: MessageDef = def(
    Tool::Build,
    13,
    Severity::Error,
    "the build produced no runtime archive",
    NONE,
);
pub const BUILD_CACHE_LOCK: MessageDef = def(
    Tool::Build,
    14,
    Severity::Error,
    "another process did not finish populating the runtime cache",
    NONE,
);

pub const RUN_START: MessageDef = def(
    Tool::Run,
    1,
    Severity::Error,
    "the model executable could not be started",
    NONE,
);
pub const RUN_SIGNAL: MessageDef = def(
    Tool::Run,
    2,
    Severity::Error,
    "the model was terminated by a signal (exit 1)",
    NONE,
);
pub const RUN_STATUS: MessageDef = def(
    Tool::Run,
    3,
    Severity::Error,
    "the model exited with a status outside 0-255 (exit 1)",
    NONE,
);
pub const RUN_NO_STATUS: MessageDef = def(
    Tool::Run,
    4,
    Severity::Error,
    "the model ended without an exit status (exit 1)",
    NONE,
);

/// Every host-side constant, in id order.
pub const HOST_MESSAGES: &[&MessageDef] = &[
    &CLI_UNSUPPORTED_PLATFORM,
    &CLI_UNKNOWN_OPTION,
    &CLI_OPTION_VALUE,
    &CLI_NO_SOURCES,
    &CLI_IGNORED_OPTION,
    &CONFIG_MISSING,
    &CONFIG_INVALID,
    &CONFIG_WARNING,
    &CONFIG_ENVIRONMENT,
    &HOST_WRITE_FAILED,
    &HOST_WRITE_WARNING,
    &HOST_MEMORY_WARNING,
    &HOST_MEMORY_INFO,
    &SLANG_SUMMARY,
    &ELAB_SUMMARY,
    &ELAB_FRONTEND_START,
    &ELAB_DB_BUILD,
    &ELAB_PROTECTED,
    &ELAB_EDITION,
    &LINT_SUMMARY,
    &CODEGEN_SUMMARY,
    &CODEGEN_UNSUPPORTED,
    &CODEGEN_SEMANTIC,
    &CODEGEN_TYPE,
    &CODEGEN_ARGUMENT,
    &CODEGEN_LIMIT,
    &CODEGEN_INTERNAL,
    &CODEGEN_OTHER,
    &CODEGEN_WARNING,
    &BUILD_IO,
    &BUILD_COMPILER_FLAG,
    &BUILD_MODEL_ABI,
    &BUILD_VALUE_CONFIG,
    &BUILD_DPI_LIBRARY,
    &BUILD_CMAKE_LAUNCH,
    &BUILD_CONFIGURE,
    &BUILD_PROGRAM,
    &BUILD_COMPILE,
    &BUILD_DPI_SYMBOL,
    &BUILD_IMPORT_LIBRARY,
    &BUILD_NO_EXECUTABLE,
    &BUILD_NO_RUNTIME,
    &BUILD_CACHE_LOCK,
    &RUN_START,
    &RUN_SIGNAL,
    &RUN_STATUS,
    &RUN_NO_STATUS,
];

/// A lint rule's stable index and default severity.
#[derive(Debug, Clone, Copy)]
pub struct LintEntry {
    pub rule: &'static str,
    pub index: u16,
    pub severity: Severity,
    /// Extra default-severity detail (for rules with more than one).
    pub note: &'static str,
}

impl LintEntry {
    pub fn def(&self) -> MessageDef {
        def(
            Tool::Lint,
            self.index,
            self.severity,
            "",
            "`[lint.rules.<rule>] enabled = false` or `severity = ...` in the `--config` file",
        )
    }
}

const fn lint(rule: &'static str, index: u16, severity: Severity) -> LintEntry {
    LintEntry {
        rule,
        index,
        severity,
        note: "",
    }
}

/// Lint rules in registry order. Indices are stable: a new rule takes the
/// next free index, a removed rule's index is never reused.
pub const LINT_RULES: &[LintEntry] = &[
    lint("unused-signal", 1, Severity::Warning),
    LintEntry {
        rule: "width-mismatch",
        index: 2,
        severity: Severity::Warning,
        note: "warning for truncation, info for extension",
    },
    lint("incomplete-case", 3, Severity::Warning),
    lint("combinational-loop", 4, Severity::Error),
    lint("multi-driver", 5, Severity::Warning),
    lint("casez-misuse", 6, Severity::Warning),
    lint("if-latch", 7, Severity::Warning),
    lint("naming-style", 8, Severity::Info),
    lint("blocking-in-always_ff", 9, Severity::Warning),
    lint("nba-in-always_comb", 10, Severity::Warning),
    lint("unused-parameter", 11, Severity::Info),
    lint("implicit-net", 12, Severity::Warning),
    lint("case-default-missing", 13, Severity::Warning),
    lint("comparison-width-mismatch", 14, Severity::Warning),
    lint("unconnected-port", 15, Severity::Warning),
    lint("mixed-assignments", 16, Severity::Error),
    lint("undriven-signal", 17, Severity::Warning),
    lint("incomplete-sensitivity-list", 18, Severity::Warning),
    lint("out-of-range-select", 19, Severity::Warning),
    lint("xz-logical-equality", 20, Severity::Warning),
    lint("duplicate-case-item", 21, Severity::Warning),
    lint("empty-implicit-sensitivity", 22, Severity::Warning),
    lint("assignment-in-condition", 23, Severity::Warning),
    lint("casex-statement", 24, Severity::Warning),
    lint("wide-index", 25, Severity::Warning),
];

/// The id of lint rule `rule`, or `LINT-999` for a rule missing from
/// [`LINT_RULES`] (the catalog test forbids that).
pub fn lint_id(rule: &str) -> String {
    let index = LINT_RULES
        .iter()
        .find(|entry| entry.rule == rule)
        .map_or(999, |entry| entry.index);
    super::id(Tool::Lint.name(), index)
}

/// By-design rejection families: `(index, family label)`. The labels are
/// `sim::legacy_unsupported::LegacyFamily::label`; the catalog test keeps
/// them equal.
pub const LEGACY_FAMILIES: &[(u16, &str)] = &[
    (10, "legacy procedural assign/deassign form"),
    (11, "MOS and resistive switch primitives"),
    (12, "trireg charge storage"),
    (13, "charge and delay-mode directives"),
    (14, "extended VCD port dumping"),
    (15, "stochastic queue form"),
    (16, "legacy PLA tasks"),
    (17, "legacy driver and scope inspection tasks"),
    (18, "PLI 1.0 TF/ACC interface"),
];

/// The id of the by-design rejection family labelled `label`.
pub fn legacy_family_id(label: &str) -> String {
    let index = LEGACY_FAMILIES
        .iter()
        .find(|(_, l)| *l == label)
        .map_or(19, |(i, _)| *i);
    super::id(Tool::Elab.name(), index)
}

/// A generated-model runtime message from `llg_msg_ids.def`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEntry {
    /// The C enumerator suffix (`LLG_MSG_<name>`).
    pub name: String,
    pub def: MessageDef,
}

/// The runtime catalog parsed from `llg_msg_ids.def`.
pub fn runtime_messages() -> &'static [RuntimeEntry] {
    static ENTRIES: OnceLock<Vec<RuntimeEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        parse_runtime_ids(RUNTIME_IDS_DEF).expect("src/sim/rt/llg_msg_ids.def is malformed")
    })
}

/// Parse `LLG_MSG_ID(NAME, TOOL, INDEX, SEVERITY, "meaning", "disable")`
/// entries.
pub fn parse_runtime_ids(text: &str) -> Result<Vec<RuntimeEntry>, String> {
    let mut out = Vec::new();
    for (name, args) in super::macro_calls(text, "LLG_MSG_ID") {
        let [tool, index, severity, meaning, disable] = args.as_slice() else {
            return Err(format!("{name}: expected 6 arguments"));
        };
        let tool = Tool::from_name(tool).ok_or_else(|| format!("{name}: unknown tool {tool}"))?;
        let index: u16 = index
            .parse()
            .map_err(|_| format!("{name}: bad index {index}"))?;
        let severity = match severity.as_str() {
            "LLG_SEVERITY_INFO" => Severity::Info,
            "LLG_SEVERITY_WARNING" => Severity::Warning,
            "LLG_SEVERITY_ERROR" => Severity::Error,
            "LLG_SEVERITY_FATAL" => Severity::Fatal,
            other => return Err(format!("{name}: bad severity {other}")),
        };
        // The texts are leaked once per process: the catalog is static data.
        let meaning: &'static str = Box::leak(meaning.clone().into_boxed_str());
        let disable: &'static str = Box::leak(disable.clone().into_boxed_str());
        out.push(RuntimeEntry {
            name,
            def: def(tool, index, severity, meaning, disable),
        });
    }
    Ok(out)
}

/// The lowering family of a code-generation failure, from its text. The
/// roughly two thousand lowering sites return plain strings; the families
/// group them until each site gets its own id.
pub fn codegen_family(text: &str) -> &'static MessageDef {
    let lower = text.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if lower.starts_with("internal") || lower.contains(": internal") {
        &CODEGEN_INTERNAL
    } else if lower.starts_with("semantic error") {
        &CODEGEN_SEMANTIC
    } else if has(&["not supported", "unsupported", "not yet", "emitter"]) {
        &CODEGEN_UNSUPPORTED
    } else if has(&["exceeds", "too many", "maximum", "limit"]) {
        &CODEGEN_LIMIT
    } else if has(&[
        "argument",
        "requires exactly",
        "requires at least",
        "requires one",
    ]) {
        &CODEGEN_ARGUMENT
    } else if has(&[
        "illegal",
        "not legal",
        "not allowed",
        "cannot",
        "duplicate",
        "multiple",
    ]) {
        &CODEGEN_SEMANTIC
    } else if has(&[
        "type",
        "width",
        "incompatible",
        "mismatch",
        "integral",
        "packed",
    ]) {
        &CODEGEN_TYPE
    } else if has(&[
        "has no",
        "missing",
        "unresolved",
        "does not precede",
        "no check",
    ]) {
        &CODEGEN_INTERNAL
    } else {
        &CODEGEN_OTHER
    }
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// `docs/messages.md`, generated from the catalog.
pub fn markdown() -> String {
    let layout = super::layout();
    let mut out = String::new();
    out.push_str(
        "# llg messages\n\n\
         <!-- Generated from src/core/report/catalog.rs and src/sim/rt/llg_msg_ids.def by \
         `cargo test --test general messages_catalog::regenerate -- --ignored`; \
         do not edit. -->\n\n\
         Every message `llg` and a generated model print on stderr has one layout and a \
         stable id. Design output (`$display` and friends) and the user text of \
         `$info`/`$warning`/`$error`/`$fatal` are never changed; streams and exit codes \
         are described in the [CLI readme](../src/bin/readme.md).\n\n\
         ## Layout\n\n",
    );
    out.push_str(&format!(
        "The layout is `src/sim/rt/llg_msg_layout.def`, read by the host and the model \
         runtime:\n\n```text\n{}\n```\n\n",
        layout.template
    ));
    let examples = [
        super::Message::new(&LINT_RULES[0].def(), "signal `x` in `tb` is never used")
            .location("f.sv:7:9")
            .hint("unused-signal"),
        super::Message::new(&CLI_OPTION_VALUE, "--top requires a module name"),
        super::Message {
            severity: Severity::Info,
            location: Some("f.sv:3:35".into()),
            id: super::id("SIM", 1),
            context: Some(super::context("tb", "5 ns")),
            text: "$finish called".into(),
            hint: None,
        },
    ];
    out.push_str("Examples:\n\n```text\n");
    for example in &examples {
        out.push_str(&example.render());
        out.push('\n');
    }
    out.push_str("```\n\n");
    out.push_str(&format!(
        "Severity words: `{}`, `{}`, `{}`, `{}`. The id is the tool name, `{}` and a \
         three-digit index; index 000 of a tool is its summary. The location is the \
         physical `file:line:col`, followed by `` (`line file:line)`` when a `` `line`` \
         directive maps it, and is left out (with its separator) when a message has none. \
         Design context is the hierarchical scope and the simulation time, joined by `{}`; \
         the time is in the largest unit that shows it exactly. The hint, when present, \
         names the lint rule or frontend warning option that selects the message.\n\n",
        layout.severity_fatal,
        layout.severity_error,
        layout.severity_warning,
        layout.severity_info,
        layout.id_join,
        layout.context_join
    ));
    out.push_str(&format!(
        "Each phase that reported a warning or worse ends with a summary message, \
         `{}` for lint, with the text `{}`.\n\n",
        super::summary("LINT", &super::Counts::default()).id,
        layout.summary
    ));
    out.push_str("## Tools\n\n| Tool | Reports |\n|---|---|\n");
    for tool in Tool::ALL {
        out.push_str(&format!("| `{}` | {} |\n", tool.name(), tool.description()));
    }
    out.push('\n');

    let mut rows: Vec<MessageDef> = HOST_MESSAGES.iter().map(|d| (*d).clone()).collect();
    rows.extend(runtime_messages().iter().map(|e| e.def.clone()));
    for tool in Tool::ALL {
        out.push_str(&format!("## {}\n\n", tool.name()));
        match tool {
            Tool::Slang => out.push_str(
                "Frontend diagnostics keep Slang's text; the id is `SLANG-<name>` with \
                 the diagnostic name from Slang's list (`vendor/slang/scripts/diagnostics.txt`), \
                 and the hint is the warning option (`-W<option>`) when Slang has one. \
                 Slang notes print as `INFO`. A frontend warning is disabled in the \
                 source with `` `pragma diagnostic ignore=\"-W<option>\"`` (IEEE 1800-2009 \
                 22.11 leaves pragma meaning to the tool).\n\n",
            ),
            Tool::Lint => {
                out.push_str(
                    "The hint is the rule name. Disable or re-grade a rule with \
                     `[lint.rules.<rule>] enabled = false` or `severity = \"error\"|\"warning\"|\"info\"` \
                     in the `--config` file, or every rule with `[lint] enabled = false`; \
                     `-Werror` reports warnings as errors ([configuration](config.md)).\n\n\
                     | Id | Rule | Default severity |\n|---|---|---|\n",
                );
                for entry in LINT_RULES {
                    let severity = if entry.note.is_empty() {
                        entry.severity.name().to_owned()
                    } else {
                        entry.note.to_owned()
                    };
                    out.push_str(&format!(
                        "| `{}` | `{}` | {} |\n",
                        entry.def().id(),
                        entry.rule,
                        severity
                    ));
                }
                out.push('\n');
            }
            Tool::Elab => {}
            Tool::Codegen => out.push_str(
                "Lowering failures are grouped into families by their text until each \
                 site gets its own id; a failure carries a location where the lowering \
                 had the source node at hand.\n\n",
            ),
            _ => {}
        }
        let tool_rows: Vec<&MessageDef> = rows.iter().filter(|d| d.tool == tool).collect();
        if !tool_rows.is_empty() || tool == Tool::Elab {
            out.push_str("| Id | Severity | Meaning | Disable |\n|---|---|---|---|\n");
            let mut sorted = tool_rows;
            if tool == Tool::Elab {
                sorted.retain(|d| d.index < 10);
            }
            sorted.sort_by_key(|d| d.index);
            for d in &sorted {
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    d.id(),
                    d.severity.name(),
                    escape_cell(d.meaning),
                    escape_cell(d.disable)
                ));
            }
            if tool == Tool::Elab {
                for (index, label) in LEGACY_FAMILIES {
                    out.push_str(&format!(
                        "| `{}` | error | rejected by design: {} | - |\n",
                        super::id(Tool::Elab.name(), *index),
                        label
                    ));
                }
            }
            out.push('\n');
        } else if tool == Tool::Opt {
            out.push_str("No messages.\n\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<String> = HOST_MESSAGES.iter().map(|d| d.id()).collect();
        ids.extend(runtime_messages().iter().map(|e| e.def.id()));
        ids.extend(LINT_RULES.iter().map(|e| e.def().id()));
        ids.extend(
            LEGACY_FAMILIES
                .iter()
                .map(|(i, _)| super::super::id("ELAB", *i)),
        );
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate message id");
    }

    #[test]
    fn families_classify_representative_texts() {
        assert_eq!(codegen_family("semantic error: x").index, 2);
        assert_eq!(codegen_family("`foo` is not supported").index, 1);
        assert_eq!(codegen_family("internal: bad").index, 6);
        assert_eq!(
            codegen_family("$q_full requires exactly two arguments").index,
            4
        );
        assert_eq!(codegen_family("something odd").index, 9);
    }
}
