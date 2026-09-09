//! Shared lifecycle support for native-backed simulator integration tests.
#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use llg::core::compile;
use llg::sim;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
static CWD_LOCK: Mutex<()> = Mutex::new(());
const MODEL_TIMEOUT: Duration = Duration::from_secs(60);

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
    pub(crate) fn new(prefix: &str) -> Result<Self, String> {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock before epoch: {error}"))?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("llg-{prefix}-{}-{nonce}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|error| format!("create temp dir: {error}"))?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    fn enter(path: &Path) -> Result<Self, String> {
        let original = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
        std::env::set_current_dir(path).map_err(|error| format!("chdir: {error}"))?;
        Ok(Self { original })
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
    let _guard = lock_process_cwd();
    with_temp_cwd(prefix, action)
}

pub(crate) fn with_cwd<T>(
    path: &Path,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _cwd = CwdGuard::enter(path)?;
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

pub(crate) fn run_command(command: &mut Command, timeout: Duration) -> Result<Output, String> {
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
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("timed out after {timeout:?}"));
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
        stdout,
        stderr,
    })
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
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) model_c: String,
}

pub(crate) fn run_generated_sim(sv: &str, top: &str, tag: &str) -> Result<SimRun, String> {
    let _guard = lock_process_cwd();
    with_temp_cwd(tag, |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some(top.to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database = llg::core::db::Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("database: {error}"))?;
        let generated = sim::codegen::generate_from_db_with_opts(
            &database,
            &llg::sim::opt::OptConfig::default(),
        )
        .map_err(|error| format!("codegen: {error}"))?;
        let executable =
            sim::build::build_model_cmake(dir, &[("model.c", generated.model_c.as_str())])
                .map_err(|error| format!("cmake: {error}"))?;
        let output = run_executable_output(&executable)?;
        Ok(SimRun {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            warnings: generated.warnings,
            model_c: generated.model_c,
        })
    })
}

pub(crate) fn run_sim(sv: &str, top: &str, tag: &str) -> Result<String, String> {
    run_generated_sim(sv, top, tag).map(|run| run.stdout)
}
