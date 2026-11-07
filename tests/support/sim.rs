//! Shared lifecycle support for native-backed simulator integration tests.
#![allow(dead_code)]

use std::cell::Cell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use llg::core::compile;
use llg::sim;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
static CWD_LOCK: Mutex<()> = Mutex::new(());
thread_local! {
    static CWD_DEPTH: Cell<usize> = const { Cell::new(0) };
}
const MODEL_TIMEOUT: Duration = Duration::from_secs(60);

/// Job count for `cmake --build --parallel` in tests that run plain `cmake`
/// commands instead of `sim::build`: `$CMAKE_BUILD_PARALLEL_LEVEL` when set,
/// else the CPUs this process may use (cgroup and affinity aware). Plain
/// `cmake --build` on a Makefiles tree is serial without it, and CI containers
/// set neither a default generator nor the variable.
pub(crate) fn cmake_build_jobs() -> String {
    sim::build::resolve_build_jobs(
        None,
        std::env::var(sim::build::BUILD_PARALLEL_LEVEL_ENV)
            .ok()
            .as_deref(),
        thread::available_parallelism().ok().map(usize::from),
    )
    .to_string()
}

fn lock_process_cwd() -> std::sync::MutexGuard<'static, ()> {
    // CwdGuard restores the process directory while unwinding. A poisoned
    // mutex therefore records a failed test, not a damaged CWD invariant.
    CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create an isolated build directory under `LLG_TEST_BUILD_DIR`, or the
    /// system temporary directory when unset. Relative overrides are anchored
    /// to the workspace so nested CWD guards cannot change their meaning.
    pub(crate) fn new(prefix: &str) -> Result<Self, String> {
        let root = match std::env::var_os("LLG_TEST_BUILD_DIR") {
            Some(path) if path.is_empty() => {
                return Err("LLG_TEST_BUILD_DIR must not be empty".to_owned());
            }
            Some(path) => Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
            None => std::env::temp_dir(),
        };
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create test build root {}: {error}", root.display()))?;
        let path = create_unique_dir(&root, prefix)?;
        // Tools report resolved paths (macOS /var/... is /private/var/...;
        // Windows expands 8.3 short names such as RUNNER~1).
        let path = llg::ffi::platform::canonicalize(&path)
            .map_err(|error| format!("resolve temp dir {}: {error}", path.display()))?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Create a fresh directory under `root` named after `prefix`, the process
/// and a clock nonce (`llg-<prefix>-<pid>-<nanos>-<n>`).
#[cfg(not(windows))]
fn create_unique_dir(root: &Path, prefix: &str) -> Result<PathBuf, String> {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let path = root.join(format!("llg-{prefix}-{}-{nonce}-{id}", std::process::id()));
    std::fs::create_dir(&path)
        .map_err(|error| format!("create temp dir {}: {error}", path.display()))?;
    Ok(path)
}

/// Windows variant: a short `llg-<pid>-<n>` name (hex), skipping names a
/// dead process left behind. MSVC's `cl` cannot write outputs whose path
/// exceeds `MAX_PATH` (260): a runtime cache inside a test directory under
/// `%TEMP%` (40 characters on CI) puts CMake's `try_compile` objects
/// (`<cache>\<69-character entry>\build\CMakeFiles\CMakeScratch\TryCompile-*
/// \CMakeFiles\cmTC_*.dir\<check>.c.obj`) about 200 characters below the
/// test directory, which left no room for `<prefix>-<pid>-<nanos>` names
/// (C1083 "Cannot open compiler generated file", C1041 for the PDB).
#[cfg(windows)]
fn create_unique_dir(root: &Path, prefix: &str) -> Result<PathBuf, String> {
    let _ = prefix;
    loop {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!("llg-{:x}-{id:x}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create temp dir {}: {error}", path.display())),
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct CwdLockGuard {
    _lock: Option<std::sync::MutexGuard<'static, ()>>,
}

impl CwdLockGuard {
    fn enter() -> Self {
        let lock = CWD_DEPTH.with(|depth| {
            let lock = (depth.get() == 0).then(lock_process_cwd);
            depth.set(depth.get() + 1);
            lock
        });
        Self { _lock: lock }
    }
}

impl Drop for CwdLockGuard {
    fn drop(&mut self) {
        CWD_DEPTH.with(|depth| {
            let current = depth.get();
            debug_assert!(current > 0, "CWD guard depth underflow");
            depth.set(current.saturating_sub(1));
        });
    }
}

struct CwdGuard {
    original: PathBuf,
    _lock: CwdLockGuard,
}

impl CwdGuard {
    fn enter(path: &Path) -> Result<Self, String> {
        let lock = CwdLockGuard::enter();
        let original = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
        std::env::set_current_dir(path).map_err(|error| format!("chdir: {error}"))?;
        Ok(Self {
            original,
            _lock: lock,
        })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original).expect("restore simulator test CWD");
    }
}

pub(crate) fn with_temp_cwd<T>(
    prefix: &str,
    action: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let dir = TempDir::new(prefix)?;
    with_cwd(dir.path(), || action(dir.path()))
}

pub(crate) fn with_frontend_temp_cwd<T>(
    prefix: &str,
    action: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    with_temp_cwd(prefix, action)
}

pub(crate) fn with_cwd<T>(
    path: &Path,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _cwd = CwdGuard::enter(path)?;
    action()
}

/// Run a read-only process-CWD observation while holding the shared guard.
pub(crate) fn with_cwd_lock<T>(action: impl FnOnce() -> T) -> T {
    let _lock = CwdLockGuard::enter();
    action()
}

/// Compile one admitted source and return Slang's complete named diagnostics.
/// Negative frontend tests use this instead of flattening diagnostics into a
/// codegen error string.
pub(crate) fn frontend_diagnostics(
    source_text: &str,
    top: &str,
) -> Result<Vec<llg::ffi::slang::Diagnostic>, String> {
    let source = compile::OwnedSource::compilation_unit("tb.sv", source_text);
    let out = compile::compile_sources(
        &[source],
        &compile::CompileOpts {
            top: Some(top.to_owned()),
            ..Default::default()
        },
    )
    .map_err(|error| format!("compile: {error}"))?;
    Ok(out.snapshot.diagnostics)
}

/// The tools whose end-of-phase summaries (`TOOL-000`) belong to the
/// compile report.
const COMPILE_TOOLS: [&str; 4] = ["SLANG", "ELAB", "LINT", "CODEGEN"];

fn is_summary(message: &llg::core::report::ParsedMessage) -> bool {
    message.id == llg::core::report::id(message.tool(), 0)
}

/// Whether an `llg` stderr line is a non-fatal compile report: a frontend
/// warning or note, a lint warning/info finding, or a compile-phase summary.
/// Every `llg` run lints, so simulation expectations ignore these the way
/// they ignore frontend warnings; lint errors still stop the run and fail the
/// test. Lines are parsed with the product layout (`core::report::parse`), so
/// a layout change does not touch this helper.
#[allow(dead_code)]
pub(crate) fn is_compile_report_line(line: &str) -> bool {
    let Some(message) = llg::core::report::parse(line) else {
        return false;
    };
    let tool = message.tool();
    (matches!(tool, "SLANG" | "LINT") && message.severity <= llg::core::report::Severity::Warning)
        || (COMPILE_TOOLS.contains(&tool) && is_summary(&message))
}

/// Whether an `llg` stderr line belongs to a non-blocking lint report: a
/// warning/info finding or the lint summary.
#[allow(dead_code)]
pub(crate) fn is_lint_report_line(line: &str) -> bool {
    let Some(message) = llg::core::report::parse(line) else {
        return false;
    };
    message.tool() == "LINT"
        && (message.severity <= llg::core::report::Severity::Warning || is_summary(&message))
}

/// `llg` stderr without its lint report lines (see [`is_lint_report_line`])
/// and without the compile-phase summaries (`SLANG-000`, `ELAB-000`,
/// `CODEGEN-000`, which have their own CLI tests), every message in the
/// fixed test form of [`canonical_message`], for exact comparisons of what the
/// model and the driver print otherwise.
#[allow(dead_code)]
pub(crate) fn strip_lint_reports(stderr: &[u8]) -> String {
    let kept: String = String::from_utf8_lossy(stderr)
        .split_inclusive('\n')
        .filter(|line| {
            let line = line.trim_end_matches(['\r', '\n']);
            !is_lint_report_line(line)
                && !llg::core::report::parse(line).is_some_and(|message| {
                    COMPILE_TOOLS.contains(&message.tool()) && is_summary(&message)
                })
        })
        .collect();
    canonical_messages(&kept)
}

/// A message line in the fixed form test expectations are written in:
/// `SEVERITY: <file>:<line>:<col>: [ID] (<context>) <text>`, the location
/// reduced to the file name and left out (with its `: `) when absent, the
/// context likewise. The line is parsed with the product layout, so a later
/// layout change leaves every expectation as it is. Other lines (design
/// output, tool output passed through) are returned unchanged.
#[allow(dead_code)]
pub(crate) fn canonical_message(line: &str) -> String {
    let Some(message) = llg::core::report::parse(line) else {
        return line.to_owned();
    };
    let mut out = format!("{}: ", message.severity.name().to_ascii_uppercase());
    if let Some(location) = &message.location {
        out.push_str(&short_location(location));
        out.push_str(": ");
    }
    out.push_str(&format!("[{}] ", message.id));
    if let Some(context) = &message.context {
        out.push_str(&format!("({context}) "));
    }
    out.push_str(&message.text);
    out
}

/// `file:line:col` with the file reduced to its name; a `` (`line ...)``
/// suffix is kept.
#[allow(dead_code)]
pub(crate) fn short_location(location: &str) -> String {
    let (physical, logical) = match location.find(" (`line ") {
        Some(at) => location.split_at(at),
        None => (location, ""),
    };
    let mut parts = physical.rsplitn(3, ':');
    let (Some(col), Some(line), Some(file)) = (parts.next(), parts.next(), parts.next()) else {
        return location.to_owned();
    };
    let name = Path::new(file)
        .file_name()
        .map_or(file.to_owned(), |name| name.to_string_lossy().into_owned());
    format!("{name}:{line}:{col}{logical}")
}

/// Whether `stderr` has a message with id `id` (`SIM-009`) whose location
/// ends in `at` (`:36:5`; any location when empty) and whose text contains
/// `text`. Lines are parsed with the product layout.
#[allow(dead_code)]
pub(crate) fn has_message(stderr: &str, id: &str, at: &str, text: &str) -> bool {
    stderr
        .lines()
        .filter_map(llg::core::report::parse)
        .any(|message| {
            message.id == id
                && (at.is_empty()
                    || message.location.as_deref().is_some_and(|location| {
                        location
                            .split(" (`line ")
                            .next()
                            .is_some_and(|physical| physical.ends_with(at))
                    }))
                && message.text.contains(text)
        })
}

/// A simulation time of `ticks` design-precision ticks of `precision_fs`
/// femtoseconds as messages print it: the largest unit that shows it
/// exactly, a space, the unit (an independent statement of the runtime rule).
#[allow(dead_code)]
pub(crate) fn sim_time(ticks: u64, precision_fs: u64) -> String {
    const UNITS: [&str; 6] = ["fs", "ps", "ns", "us", "ms", "s"];
    let mut value = u128::from(ticks) * u128::from(precision_fs);
    let mut unit = 0;
    if value == 0 {
        let mut precision = precision_fs.max(1);
        while unit < 5 && precision.is_multiple_of(1000) {
            precision /= 1000;
            unit += 1;
        }
        return format!("0 {}", UNITS[unit]);
    }
    while unit < 5 && value.is_multiple_of(1000) {
        value /= 1000;
        unit += 1;
    }
    format!("{value} {}", UNITS[unit])
}

/// Whether `stderr` mentions `diagnostic`: a substring of the raw stream, or
/// of one message read as `file.sv:l:c text` (location reduced to the file
/// name, then the text), so a rejection test can name a located diagnostic
/// without the layout or the message id.
#[allow(dead_code)]
pub(crate) fn mentions(stderr: &str, diagnostic: &str) -> bool {
    stderr.contains(diagnostic)
        || stderr
            .lines()
            .filter_map(llg::core::report::parse)
            .any(|message| located_text(&message).contains(diagnostic))
}

/// `file.sv:l:c text` of a message (location reduced to the file name), or
/// its text when it has no location.
#[allow(dead_code)]
pub(crate) fn located_text(message: &llg::core::report::ParsedMessage) -> String {
    match &message.location {
        Some(location) => format!("{} {}", short_location(location), message.text),
        None => message.text.clone(),
    }
}

/// The run-time messages of `stderr` (tool `SIM`) in the canonical test
/// form, without the `$finish` report (`SIM-001`) and the end-of-run summary
/// (`SIM-000`).
#[allow(dead_code)]
pub(crate) fn runtime_messages(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| {
            llg::core::report::parse(line).is_some_and(|message| {
                message.tool() == "SIM" && message.id != "SIM-001" && message.id != "SIM-000"
            })
        })
        .map(canonical_message)
        .collect()
}

/// Warnings a case lists by text instead of in its stderr oracle: lowering
/// warnings and the run-time warnings of statements (qualifier violations,
/// format and delay fallbacks, memory files, timing checks). Container and
/// waveform warnings stay in the stderr oracle.
#[allow(dead_code)]
pub(crate) fn is_listed_warning(message: &llg::core::report::ParsedMessage) -> bool {
    use llg::core::report::catalog;
    let id = message.id.as_str();
    id == catalog::CODEGEN_WARNING.id()
        || id == "SIM-013"
        || id == "SIM-015"
        || (id == "SIM-011" && !message.text.starts_with("waveform: "))
}

/// The texts of the [`is_listed_warning`] messages of `stderr`, in order.
#[allow(dead_code)]
pub(crate) fn listed_warnings(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(llg::core::report::parse)
        .filter(is_listed_warning)
        .map(|message| message.text)
        .collect()
}

/// The file name of `path`, as [`canonical_message`] shows a location.
#[allow(dead_code)]
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Every line of `stderr` through [`canonical_message`].
#[allow(dead_code)]
pub(crate) fn canonical_messages(stderr: &str) -> String {
    stderr
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\r', '\n']);
            let mut out = canonical_message(body);
            if line.ends_with('\n') {
                out.push('\n');
            }
            out
        })
        .collect()
}

pub(crate) fn run_command(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| format!("spawn: {error}"))?;
    let stdout = child.stdout.take().ok_or("capture stdout")?;
    let stderr = child.stderr.take().ok_or("capture stderr")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = stdout;
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = stderr;
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|error| format!("wait: {error}"))? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                // The CLI launches CMake and a simulator that inherit its
                // output pipes. Stop descendants before joining the readers.
                #[cfg(unix)]
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                #[cfg(windows)]
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &child.id().to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                // What the command printed before the deadline names the
                // step that was slow or hung (e.g. ctest's last started test).
                let stdout = stdout_reader.join().ok().and_then(Result::ok);
                let stderr = stderr_reader.join().ok().and_then(Result::ok);
                return Err(format!(
                    "timed out after {timeout:?}\n--- stdout before the timeout (tail) ---\n{}\n\
                     --- stderr before the timeout (tail) ---\n{}",
                    output_tail(stdout.as_deref()),
                    output_tail(stderr.as_deref())
                ));
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "stdout reader panicked".to_owned())?
        .map_err(|error| format!("read stdout: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "stderr reader panicked".to_owned())?
        .map_err(|error| format!("read stderr: {error}"))?;
    Ok(Output {
        status,
        stdout: host_text_to_lf(stdout),
        stderr: host_text_to_lf(stderr),
    })
}

/// The last `TIMEOUT_OUTPUT_TAIL_BYTES` of a partial capture, lossily decoded.
fn output_tail(bytes: Option<&[u8]>) -> String {
    const TIMEOUT_OUTPUT_TAIL_BYTES: usize = 16 * 1024;
    let Some(bytes) = bytes else {
        return "<unavailable>".to_owned();
    };
    let start = bytes.len().saturating_sub(TIMEOUT_OUTPUT_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

/// Simulators keep the OS-native newline: on Windows the console and files
/// opened in text mode end lines with CRLF. Expected outputs are written with
/// LF, so captured text goes through the platform layer's normalization;
/// other hosts stay byte-exact.
pub(crate) use llg::ffi::platform::native_text_to_lf as host_text_to_lf;

/// The spelling diagnostics and runtime reports use for an input file: its
/// resolved native path, because sources are admitted through handles.
/// Expected messages use this rather than a joined fixture path, whose `/`
/// separators stay literal on Windows (`C:\repo\tests/fixtures/...`).
pub(crate) fn source_display(path: &Path) -> String {
    llg::ffi::platform::canonicalize(path)
        .unwrap_or_else(|error| panic!("resolve source {}: {error}", path.display()))
        .display()
        .to_string()
}

/// Read a file the simulation wrote in text mode (`$fopen` without `b`,
/// `$writemem`), with the host's native newlines normalized to LF.
pub(crate) fn read_text_output(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    String::from_utf8(host_text_to_lf(bytes))
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))
}

pub(crate) fn run_executable(executable: &Path) -> Result<String, String> {
    let output = run_executable_output(executable)?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn run_executable_output(executable: &Path) -> Result<Output, String> {
    let output = run_command(&mut Command::new(executable), MODEL_TIMEOUT)?;
    if !output.status.success() {
        return Err(format!(
            "simulation exited with {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

pub(crate) struct SimRun {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) model_c: String,
}

pub(crate) fn run_generated_sim(sv: &str, top: &str, tag: &str) -> Result<SimRun, String> {
    run_generated_sim_inner(sv, top, tag, true)
}

/// Compile and run a source fixture after placing exact memory-file contents
/// in the simulator's temporary working directory. This keeps file-task tests
/// isolated while preserving the ordinary generated-model harness contract.
pub(crate) fn run_generated_sim_with_files(
    sv: &str,
    top: &str,
    tag: &str,
    files: &[(&str, &str)],
) -> Result<SimRun, String> {
    run_generated_sim_with_files_opts(sv, top, tag, files, &llg::sim::opt::OptConfig::default())
}

pub(crate) fn run_generated_sim_with_files_opts(
    sv: &str,
    top: &str,
    tag: &str,
    files: &[(&str, &str)],
    options: &llg::sim::opt::OptConfig,
) -> Result<SimRun, String> {
    with_temp_cwd(tag, |dir| {
        run_generated_sim_in_dir(sv, top, dir, true, files, options)
    })
}

pub(crate) fn run_generated_sim_allow_failure(
    sv: &str,
    top: &str,
    tag: &str,
) -> Result<SimRun, String> {
    run_generated_sim_inner(sv, top, tag, false)
}

fn run_generated_sim_inner(
    sv: &str,
    top: &str,
    tag: &str,
    require_success: bool,
) -> Result<SimRun, String> {
    with_temp_cwd(tag, |dir| {
        run_generated_sim_in_dir(
            sv,
            top,
            dir,
            require_success,
            &[],
            &llg::sim::opt::OptConfig::default(),
        )
    })
}

fn run_generated_sim_in_dir(
    sv: &str,
    top: &str,
    dir: &Path,
    require_success: bool,
    files: &[(&str, &str)],
    options: &llg::sim::opt::OptConfig,
) -> Result<SimRun, String> {
    // An in-memory unit named `tb.sv`: the generated C embeds source
    // locations, and the path of a file in this per-run directory would make
    // the model text, and so its compiler-cache key, differ on every run.
    let source = compile::OwnedSource::compilation_unit("tb.sv", sv);
    let compiled = compile::compile_sources_checked(
        &[source],
        &compile::CompileOpts {
            top: Some(top.to_owned()),
            ..Default::default()
        },
    )
    .map_err(|error| format!("compile: {error}"))?;
    let database = llg::core::db::Db::from_slang(&compiled.snapshot)
        .map_err(|error| format!("database: {error}"))?;
    let value_config = sim::value_backend::ValueConfig::from_env()?;
    let generated = sim::codegen::generate_from_db_with_codegen_options(
        &database,
        &sim::codegen::CodegenOptions {
            optimization: *options,
            value_config,
            ..Default::default()
        },
    )
    .map_err(|error| format!("codegen: {error}"))?;
    let executable = sim::build::build_model_cmake_with_opts(
        dir,
        &[("model.c", generated.model_c.as_str())],
        &sim::build::CmakeBuildOpts {
            value_config,
            ..Default::default()
        },
    )
    .map_err(|error| format!("cmake: {error}"))?;
    // The model builder intentionally removes stale entries from the output
    // directory, so fixture files must be written after CMake generation.
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents)
            .map_err(|error| format!("write fixture {name}: {error}"))?;
    }
    let output = run_command(&mut Command::new(&executable), MODEL_TIMEOUT)?;
    if require_success && !output.status.success() {
        return Err(format!(
            "simulation exited with {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(SimRun {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        warnings: generated.warnings,
        model_c: generated.model_c,
    })
}

pub(crate) fn run_sim(sv: &str, top: &str, tag: &str) -> Result<String, String> {
    run_generated_sim(sv, top, tag).map(|run| run.stdout)
}

/// Write the selected value facade's nested dependencies for standalone probes.
pub fn write_value_backend_sources(dir: &std::path::Path) {
    for (name, source) in
        llg::sim::rt::value_backend_sources(llg::sim::value_backend::ValueBackend::Legacy)
    {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).expect("create value source directory");
        std::fs::write(path, source).expect("write value source dependency");
    }
    for (name, source) in llg::sim::rt::message_headers() {
        std::fs::write(dir.join(name), source).expect("write runtime message header");
    }
}

/// Whether the native CMake probe projects of the suite (the dynamic-storage
/// probes, the GMP closure check) build their Debug configuration with debug
/// information. Off unless `LLG_TEST_DEBUG_INFO` is `1`, `on`, `true` or
/// `yes`: no probe reads its own symbols, and `-g`, `/Z7` and the `/debug`
/// link only cost compile, link and compiler-cache time. Optimization levels,
/// `/RTC1`, `assert()` and `LLG_CO_DEBUG` checks are unaffected. Generated
/// models build in Release without debug information either way.
pub(crate) fn test_debug_info() -> bool {
    debug_info_requested(std::env::var("LLG_TEST_DEBUG_INFO").ok().as_deref())
}

/// [`test_debug_info`] for the variable's value (`None` when unset).
pub(crate) fn debug_info_requested(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "on" | "true" | "yes"
        )
    })
}

/// `GMP_ROOT` for compact/GMP lanes: `LLG_TEST_GMP_ROOT` to qualify an
/// external GMP, otherwise empty, which selects the bundled GMP sources (an
/// empty `GMP_ROOT` also masks one inherited from the environment).
pub(crate) fn test_gmp_root() -> String {
    std::env::var("LLG_TEST_GMP_ROOT").unwrap_or_default()
}

/// The external installation for tests of the `GMP_ROOT` override itself,
/// which need `include/gmp.h` and a static library; without
/// `LLG_TEST_GMP_ROOT` they are reported and skipped.
pub(crate) fn test_gmp_installation(test: &str) -> Option<std::path::PathBuf> {
    let root = test_gmp_root();
    if root.is_empty() {
        eprintln!("SKIP {test}: set LLG_TEST_GMP_ROOT to test an external GMP");
        return None;
    }
    Some(root.into())
}
