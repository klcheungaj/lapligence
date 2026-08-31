//! compile — unified Surelog compilation pipeline shared by the LSP and the
//! simulator.  High-level facade over `surelog::SessionBuilder`.
//!
//! Surelog work enters through [`compile`] or [`parse_only`].  The full
//! pipeline returns [`CompileOut::session`], which owns every C++ object and
//! must be used (and dropped) on the calling thread.  The parse-only path
//! collects owned parse-tree tokens and drops its session before returning.
//! Frontend problems come back as [`Diag`]s, not as errors — the `Result` is
//! reserved for session-construction failures.

use crate::core::tokens::{self, FileTokens};
use crate::ffi::surelog;
use crate::ffi::vpi::VpiHandle;
use std::sync::Mutex;

pub use crate::ffi::surelog::{Diag, Severity};

/// Serializes every Surelog session in this process.
///
/// Surelog keeps process-global C++ singletons (its `FileSystem` captures the
/// CWD at the first session and reuses it), so concurrent `compile`/`parse_only`
/// calls from any lib consumer — tests, sim, demos, future code — would race
/// that state.  The LSP binary already serializes its analyses behind
/// `ANALYZE_LOCK` (plus the process-global shadow-staging lock), so this lib
/// lock is defense-in-depth for non-LSP callers; it is acquired here, before
/// any `SessionBuilder` work, and held for the whole Surelog session.
static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Options controlling one compile+elaborate run.
#[derive(Debug, Clone)]
pub struct CompileOpts {
    /// Source files, absolute or CWD-relative paths.
    pub files: Vec<String>,
    /// `-top <module>`; `None` lets Surelog auto-detect the top.
    pub top: Option<String>,
    /// Preprocessor defines, e.g. `-DNAME=1` (passed through verbatim).
    pub defines: Vec<String>,
    /// Top-level parameter overrides, e.g. `-PWIDTH=8` (passed through
    /// verbatim).  Surelog applies them to top-level module instances during
    /// elaboration; overriding a name that no top module declares produces a
    /// Surelog error diagnostic.
    pub param_overrides: Vec<String>,
    /// Include directories, e.g. `-Isrc` (passed through verbatim).
    pub include_dirs: Vec<String>,
    /// Run Surelog elaboration (default `true`).
    pub elaborate: bool,
    /// Enable UHDM full elaboration / ref binding (default `true`).
    pub elab_uhdm: bool,
    /// Suppress Surelog's own stdout messages (default `true`).
    pub mute_stdout: bool,
    /// Filter informational diagnostics: adds `-noinfo -nonote`
    /// (default `true`).  Error and warning diagnostics are never filtered,
    /// so `CompileOut::ok()` and `diagnostics` stay reliable.
    pub quiet: bool,
}

impl Default for CompileOpts {
    fn default() -> Self {
        CompileOpts {
            files: Vec::new(),
            top: None,
            defines: Vec::new(),
            param_overrides: Vec::new(),
            include_dirs: Vec::new(),
            elaborate: true,
            elab_uhdm: true,
            mute_stdout: true,
            quiet: true,
        }
    }
}

/// The outcome of a [`compile`] run: the owning session plus a snapshot of
/// its diagnostics.
pub struct CompileOut {
    /// Owning session; drop order matters — it frees compiler, parser, error
    /// container and symbol table in that order.
    pub session: surelog::SurelogSession,
    /// Diagnostics collected right after the compile finished.
    pub diagnostics: Vec<Diag>,
}

/// Owned result of a single-file Surelog parse-only run.
///
/// Unlike [`CompileOut`], this result does not expose the session: parse-tree
/// tokens are collected while the session is alive, then the session is
/// dropped before this value is returned.
pub struct ParseOnlyOut {
    /// Diagnostics collected after parsing the source file.
    pub diagnostics: Vec<Diag>,
    /// Parse-tree semantic-token inputs.  `-parseonly` bypasses preprocessing,
    /// so this contains only the requested source file.
    pub tokens: Vec<FileTokens>,
}

impl CompileOut {
    /// `true` when there are no fatal, syntax or error diagnostics.
    pub fn ok(&self) -> bool {
        !self.diagnostics.iter().any(|d| {
            matches!(
                d.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )
        })
    }

    /// The elaborated UHDM design handle, valid while the session is alive.
    pub fn uhdm_design(&self) -> Option<VpiHandle> {
        self.session.uhdm_design()
    }

    /// A borrowed handle to Surelog's `Design`, valid while the session is
    /// alive.
    pub fn design(&self) -> Option<surelog::Design<'_>> {
        self.session.design()
    }
}

/// Compile + elaborate (per `opts`) the given files.
///
/// Returns `Err` only when the Surelog session itself cannot be constructed
/// (e.g. a bad argument); compilation problems are reported through
/// `CompileOut::diagnostics`.  All Surelog work happens inside this call —
/// the returned session must be used (and dropped) on the calling thread.
pub fn compile(opts: &CompileOpts) -> Result<CompileOut, String> {
    let _guard = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut builder = surelog::SessionBuilder::new()
        .ok_or_else(|| "surelog: failed to initialise session".to_string())?;
    // write_pp_output is required for the full pipeline (Surelog's `-parse`
    // flag enables it implicitly); the design is empty without it.
    builder.set_parse().set_write_pp_output().set_compile();
    // `-nocache`: Surelog's compilation cache is UNSAFE for repeated
    // sessions inside one process — cached payloads carry file references
    // from the session that produced them, so from the third sequential
    // compile on, `vpiFile` strings (and anything derived from them, like
    // the source-text recovery of `#N` delays and `disable` targets) point
    // at earlier sessions' paths.  Correctness over cache speed.
    builder.add_arg("-nocache");
    if opts.elaborate {
        builder.set_elaborate();
    }
    if opts.elab_uhdm {
        builder.set_elab_uhdm();
    }
    if opts.mute_stdout {
        builder.set_mute_stdout();
    }
    if opts.quiet {
        // -noinfo/-nonote filter *info* and *note* messages only; errors and
        // warnings always stay in the container so diagnostics stay useful.
        builder.add_arg("-noinfo");
        builder.add_arg("-nonote");
    }

    for arg in opts
        .defines
        .iter()
        .chain(&opts.include_dirs)
        .chain(&opts.param_overrides)
        .chain(&opts.files)
    {
        builder.add_arg(arg);
    }
    if let Some(top) = &opts.top {
        builder.add_arg("-top");
        builder.add_arg(top);
    }

    let session = builder
        .build()
        .ok_or_else(|| "surelog compile failed to start".to_string())?;
    let diagnostics = session.diagnostics();
    Ok(CompileOut {
        session,
        diagnostics,
    })
}

/// Parse exactly one source file without preprocessing, compilation,
/// elaboration, builtins, or cache reuse.
///
/// Surelog's `-parseonly` mode feeds the original source directly to the
/// parser.  Consequently, `` `include `` files and macro expansions are not
/// consumed, preserving the source file's original positions.  `defines`
/// accepts the same validated `-D...` arguments as [`CompileOpts::defines`].
pub fn parse_only(file: &str, defines: &[String]) -> Result<ParseOnlyOut, String> {
    let _guard = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut builder = surelog::SessionBuilder::new()
        .ok_or_else(|| "surelog: failed to initialise parse-only session".to_string())?;
    builder.set_mute_stdout();
    for arg in ["-parseonly", "-nocache", "-nobuiltin", "-noinfo", "-nonote"] {
        if !builder.add_arg(arg) {
            return Err(format!("surelog parse-only argument contains NUL: {arg:?}"));
        }
    }
    for define in defines {
        if !builder.add_arg(define) {
            return Err(format!(
                "surelog parse-only define contains NUL: {define:?}"
            ));
        }
    }
    if !builder.add_arg(file) {
        return Err("surelog parse-only source path contains NUL".to_owned());
    }

    let session = builder
        .build()
        .ok_or_else(|| "surelog parse-only failed to start".to_string())?;
    let diagnostics = session.diagnostics();
    let mut tokens = session
        .design()
        .map(|design| tokens::collect_parse_tokens(&design).0)
        .unwrap_or_default();
    if let Ok(source) = std::fs::read_to_string(file) {
        tokens::supplement_source_local_module_tokens(file, &source, &mut tokens);
    }
    Ok(ParseOnlyOut {
        diagnostics,
        tokens,
    })
}
