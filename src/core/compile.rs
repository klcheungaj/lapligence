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

/// Surelog command-line parser modes configured through the C API setters.
///
/// These values are kept separate from [`SurelogInvocation::argv`] because
/// the corresponding switches do not appear in the argument vector handed to
/// Surelog.  `quiet` is included here as the logical mode even though its
/// `-noinfo`/`-nonote` representation is part of `argv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurelogSetterModes {
    pub parse: bool,
    pub write_pp_output: bool,
    pub compile: bool,
    pub elaborate: bool,
    pub elab_uhdm: bool,
    pub mute_stdout: bool,
    pub quiet: bool,
}

/// Exact Surelog invocation description used by the LSP diagnostic logger.
///
/// `argv` includes the program name (`llg`) at index zero, matching the
/// vector passed to the native command-line parser.  It contains only the
/// explicit strings accepted by [`surelog::SessionBuilder::add_arg`];
/// setter-configured modes are reported in [`setters`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurelogInvocation {
    pub argv: Vec<String>,
    pub setters: SurelogSetterModes,
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

fn compile_setter_modes(opts: &CompileOpts) -> SurelogSetterModes {
    SurelogSetterModes {
        parse: true,
        write_pp_output: true,
        compile: true,
        elaborate: opts.elaborate,
        elab_uhdm: opts.elab_uhdm,
        mute_stdout: opts.mute_stdout,
        quiet: opts.quiet,
    }
}

/// Visit the explicit compile arguments in the exact order used by
/// [`compile`].  Keeping construction here avoids the logged representation
/// drifting from the native invocation while allowing the normal compile
/// path to avoid an extra allocation.
fn visit_compile_args(opts: &CompileOpts, mut visit: impl FnMut(&str) -> bool) -> bool {
    for arg in ["-nocache"] {
        if !visit(arg) {
            return false;
        }
    }
    if opts.quiet {
        for arg in ["-noinfo", "-nonote"] {
            if !visit(arg) {
                return false;
            }
        }
    }
    for arg in opts
        .defines
        .iter()
        .chain(&opts.include_dirs)
        .chain(&opts.param_overrides)
        .chain(&opts.files)
    {
        if !visit(arg) {
            return false;
        }
    }
    if let Some(top) = &opts.top {
        for arg in ["-top", top.as_str()] {
            if !visit(arg) {
                return false;
            }
        }
    }
    true
}

fn compile_argument_nul_error(arg: &str) -> String {
    format!("surelog compile argument contains NUL: {arg:?}")
}

fn add_compile_args(
    builder: &mut surelog::SessionBuilder,
    opts: &CompileOpts,
) -> Result<(), String> {
    let mut rejected_arg = None;
    if visit_compile_args(opts, |arg| {
        if builder.add_arg(arg) {
            true
        } else {
            rejected_arg = Some(arg.to_owned());
            false
        }
    }) {
        Ok(())
    } else {
        Err(compile_argument_nul_error(
            rejected_arg.as_deref().unwrap_or_default(),
        ))
    }
}

/// Return the exact explicit argv and setter modes for a full compile run.
pub fn compile_invocation(opts: &CompileOpts) -> Result<SurelogInvocation, String> {
    let mut argv = vec!["llg".to_owned()];
    let mut rejected_arg = None;
    if !visit_compile_args(opts, |arg| {
        if arg.contains('\0') {
            rejected_arg = Some(arg.to_owned());
            false
        } else {
            argv.push(arg.to_owned());
            true
        }
    }) {
        return Err(compile_argument_nul_error(
            rejected_arg.as_deref().unwrap_or_default(),
        ));
    }
    Ok(SurelogInvocation {
        argv,
        setters: compile_setter_modes(opts),
    })
}

fn parse_only_setter_modes() -> SurelogSetterModes {
    SurelogSetterModes {
        parse: false,
        write_pp_output: false,
        compile: false,
        elaborate: false,
        elab_uhdm: false,
        mute_stdout: true,
        quiet: true,
    }
}

/// Visit the explicit parse-only arguments in the exact order used by
/// [`parse_only`].
fn visit_parse_only_args(
    file: &str,
    defines: &[String],
    mut visit: impl FnMut(&str) -> bool,
) -> bool {
    for arg in ["-parseonly", "-nocache", "-nobuiltin", "-noinfo", "-nonote"] {
        if !visit(arg) {
            return false;
        }
    }
    for define in defines {
        if !visit(define) {
            return false;
        }
    }
    visit(file)
}

/// Return the exact explicit argv and setter modes for an isolated parse.
pub fn parse_only_invocation(file: &str, defines: &[String]) -> Result<SurelogInvocation, String> {
    let mut argv = vec!["llg".to_owned()];
    let mut rejected_arg = None;
    if !visit_parse_only_args(file, defines, |arg| {
        if arg.contains('\0') {
            rejected_arg = Some(arg.to_owned());
            false
        } else {
            argv.push(arg.to_owned());
            true
        }
    }) {
        let rejected_arg = rejected_arg.as_deref().unwrap_or_default();
        if rejected_arg == file {
            return Err("surelog parse-only source path contains NUL".to_owned());
        }
        if defines.iter().any(|define| define == rejected_arg) {
            return Err(format!(
                "surelog parse-only define contains NUL: {rejected_arg:?}"
            ));
        }
        return Err(format!(
            "surelog parse-only argument contains NUL: {rejected_arg:?}"
        ));
    }
    Ok(SurelogInvocation {
        argv,
        setters: parse_only_setter_modes(),
    })
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
    /// Number of token nodes collected directly from Surelog before source
    /// supplementation.
    pub parsed_token_count: usize,
    /// Number of token nodes added by the source-local module-boundary
    /// supplement.
    pub supplemented_token_count: usize,
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
    let setters = compile_setter_modes(opts);
    let mut builder = surelog::SessionBuilder::new()
        .ok_or_else(|| "surelog: failed to initialise session".to_string())?;
    // write_pp_output is required for the full pipeline (Surelog's `-parse`
    // flag enables it implicitly); the design is empty without it.
    if setters.parse {
        builder.set_parse();
    }
    if setters.write_pp_output {
        builder.set_write_pp_output();
    }
    if setters.compile {
        builder.set_compile();
    }
    // `-nocache`: Surelog's compilation cache is UNSAFE for repeated
    // sessions inside one process — cached payloads carry file references
    // from the session that produced them, so from the third sequential
    // compile on, `vpiFile` strings (and anything derived from them, like
    // the source-text recovery of `#N` delays and `disable` targets) point
    // at earlier sessions' paths.  Correctness over cache speed.
    if setters.elaborate {
        builder.set_elaborate();
    }
    if setters.elab_uhdm {
        builder.set_elab_uhdm();
    }
    if setters.mute_stdout {
        builder.set_mute_stdout();
    }

    add_compile_args(&mut builder, opts)?;

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
    let setters = parse_only_setter_modes();
    if setters.mute_stdout {
        builder.set_mute_stdout();
    }
    let mut rejected_arg = None;
    if !visit_parse_only_args(file, defines, |arg| {
        if builder.add_arg(arg) {
            true
        } else {
            rejected_arg = Some(arg.to_owned());
            false
        }
    }) {
        let rejected_arg = rejected_arg.unwrap_or_default();
        if rejected_arg == file {
            return Err("surelog parse-only source path contains NUL".to_owned());
        }
        if defines.iter().any(|define| define == &rejected_arg) {
            return Err(format!(
                "surelog parse-only define contains NUL: {rejected_arg:?}"
            ));
        }
        return Err(format!(
            "surelog parse-only argument contains NUL: {rejected_arg:?}"
        ));
    }

    let session = builder
        .build()
        .ok_or_else(|| "surelog parse-only failed to start".to_string())?;
    let diagnostics = session.diagnostics();
    let mut tokens = session
        .design()
        .map(|design| tokens::collect_parse_tokens(&design).0)
        .unwrap_or_default();
    let parsed_token_count = tokens.iter().map(|file| file.nodes.len()).sum();
    if let Ok(source) = std::fs::read_to_string(file) {
        tokens::supplement_source_local_module_tokens(file, &source, &mut tokens);
    }
    let token_count: usize = tokens.iter().map(|file| file.nodes.len()).sum();
    let supplemented_token_count = token_count.saturating_sub(parsed_token_count);
    // The returned value owns no session-scoped data.  Drop explicitly so the
    // LSP's post-return log can distinguish native teardown from token
    // encoding, and so this guarantee remains obvious if the result grows.
    drop(session);
    Ok(ParseOnlyOut {
        diagnostics,
        tokens,
        parsed_token_count,
        supplemented_token_count,
    })
}

#[cfg(test)]
mod tests {
    use super::{compile_invocation, parse_only_invocation, CompileOpts, SurelogSetterModes};

    #[test]
    fn compile_invocation_matches_surelog_argument_order_and_setters() {
        let opts = CompileOpts {
            files: vec!["shadow/top.sv".to_owned(), "shadow/child.sv".to_owned()],
            top: Some("top".to_owned()),
            defines: vec!["-DDEBUG=1".to_owned()],
            param_overrides: vec!["-PWIDTH=8".to_owned()],
            include_dirs: vec!["-Ishadow/inc".to_owned(), "-Ireal/inc".to_owned()],
            ..Default::default()
        };

        let invocation = compile_invocation(&opts).expect("valid compile arguments");

        assert_eq!(
            invocation.argv,
            vec![
                "llg",
                "-nocache",
                "-noinfo",
                "-nonote",
                "-DDEBUG=1",
                "-Ishadow/inc",
                "-Ireal/inc",
                "-PWIDTH=8",
                "shadow/top.sv",
                "shadow/child.sv",
                "-top",
                "top",
            ]
        );
        assert_eq!(
            invocation.setters,
            SurelogSetterModes {
                parse: true,
                write_pp_output: true,
                compile: true,
                elaborate: true,
                elab_uhdm: true,
                mute_stdout: true,
                quiet: true,
            }
        );
    }

    #[test]
    fn compile_invocation_omits_disabled_quiet_and_top_arguments() {
        let opts = CompileOpts {
            files: vec!["top.sv".to_owned()],
            elaborate: false,
            elab_uhdm: false,
            mute_stdout: false,
            quiet: false,
            ..Default::default()
        };

        let invocation = compile_invocation(&opts).expect("valid compile arguments");

        assert_eq!(invocation.argv, vec!["llg", "-nocache", "top.sv"]);
        assert_eq!(
            invocation.setters,
            SurelogSetterModes {
                parse: true,
                write_pp_output: true,
                compile: true,
                elaborate: false,
                elab_uhdm: false,
                mute_stdout: false,
                quiet: false,
            }
        );
    }

    #[test]
    fn parse_only_invocation_includes_flags_defines_and_file_in_order() {
        let invocation = parse_only_invocation(
            "/tmp/shadow/open.sv",
            &["-DDEBUG".to_owned(), "-DWIDTH=8".to_owned()],
        )
        .expect("valid parse-only arguments");

        assert_eq!(
            invocation.argv,
            vec![
                "llg",
                "-parseonly",
                "-nocache",
                "-nobuiltin",
                "-noinfo",
                "-nonote",
                "-DDEBUG",
                "-DWIDTH=8",
                "/tmp/shadow/open.sv",
            ]
        );
        assert_eq!(
            invocation.setters,
            SurelogSetterModes {
                parse: false,
                write_pp_output: false,
                compile: false,
                elaborate: false,
                elab_uhdm: false,
                mute_stdout: true,
                quiet: true,
            }
        );
    }

    #[test]
    fn invocation_rejects_nul_without_claiming_the_argument() {
        let opts = CompileOpts {
            files: vec!["top\0.sv".to_owned()],
            ..Default::default()
        };

        let error = compile_invocation(&opts).expect_err("NUL must be rejected");
        assert!(error.contains("NUL"));
        assert!(error.contains("top\\0.sv"));

        let error = parse_only_invocation("open\0.sv", &[]).expect_err("NUL must be rejected");
        assert_eq!(error, "surelog parse-only source path contains NUL");
    }
}
