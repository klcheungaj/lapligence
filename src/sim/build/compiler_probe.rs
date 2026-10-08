//! Compiler identity and target probes, memoized.
//!
//! The runtime cache key and the toolchain seed key both contain the text the
//! C compiler reports about itself: the `--version` output (MSVC `cl` has no
//! version option and prints its banner while failing, so `/Bv` is tried as
//! well) and the target (`-dumpmachine`, or a `Target:` line, or the MSVC
//! banner's architecture). Each probe is a compiler spawn, and `cl.exe`
//! spawns cost tenths of a second, so the results are reused:
//!
//! - Within one probe, every argument runs at most once: the target parser
//!   reads the `--version`/`/Bv` outputs the identity probe already captured,
//!   and an MSVC banner (both probes failed, the banner names an MSVC
//!   architecture) skips `-dumpmachine`, which `cl` can only reject. GCC and
//!   Clang take two spawns, `cl` two instead of five.
//! - Per process, results are memoized by the compiler spelling, the resolved
//!   executable's canonical path, size and modification time, and every
//!   environment variable the reported text can depend on ([`MEMO_ENVIRONMENT`]).
//!   A replaced or upgraded compiler, another `PATH` hit or a changed developer
//!   environment therefore probes again.
//! - Across processes, the same key selects a `compiler-probe/<hash>` file in
//!   the runtime cache root's [`MEMO_DIR`], but only for MSVC results. Every
//!   other direct child of the root stays a runtime archive entry. `cl.exe` reports its
//!   version and target from its own binary, and every developer-shell input
//!   that selects another toolset (`PATH`, `VCToolsVersion`, ...) is in the key.
//!   GCC/Clang spellings are often wrappers whose real compiler can change while
//!   the wrapper file does not (ccache or distcc masquerade links, Xcode's
//!   `xcrun` shims, scoop/chocolatey shims), and their spawns are cheap, so
//!   they are memoized per process only.
//!
//! The produced identity and target text is exactly what probing without the
//! memo yields, so existing runtime caches and seeds stay valid. A compiler
//! whose path cannot be resolved (or whose file metadata is unreadable) is
//! probed without any memo.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use super::toolchain_seed::KEY_ENVIRONMENT;

/// Bumped whenever the memo key or file layout changes.
const FORMAT: &str = "llg-compiler-probe-v1";

/// Subdirectory of the runtime cache root that holds the memo files, so the
/// root's direct children remain runtime archive entries.
pub(super) const MEMO_DIR: &str = "compiler-probe";

/// Variables that can change the probed text beyond those CMake detection
/// reads: message locales, the options `cl` prepends/appends from `CL` and
/// `_CL_` (`/nologo` hides the banner), and driver overrides.
const EXTRA_ENVIRONMENT: [&str; 9] = [
    "LANG",
    "LC_ALL",
    "LC_MESSAGES",
    "LANGUAGE",
    "VSLANG",
    "CL",
    "_CL_",
    "CCC_OVERRIDE_OPTIONS",
    "GCC_EXEC_PREFIX",
];

/// Every environment variable in the memo key.
pub(super) const MEMO_ENVIRONMENT: [&str; KEY_ENVIRONMENT.len() + EXTRA_ENVIRONMENT.len()] = {
    let mut names = [""; KEY_ENVIRONMENT.len() + EXTRA_ENVIRONMENT.len()];
    let mut index = 0;
    while index < KEY_ENVIRONMENT.len() {
        names[index] = KEY_ENVIRONMENT[index];
        index += 1;
    }
    while index < names.len() {
        names[index] = EXTRA_ENVIRONMENT[index - KEY_ENVIRONMENT.len()];
        index += 1;
    }
    names
};

/// Compiler facts shared by the runtime cache key and the toolchain seed key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompilerFacts {
    pub identity: String,
    pub target: String,
}

/// One captured probe: exit success plus lossy stdout/stderr text.
#[derive(Clone, Debug)]
pub(super) struct ProbeOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Probed facts plus whether they came from an MSVC banner (the only kind the
/// cross-process memo stores).
#[derive(Clone, Debug)]
struct Probed {
    facts: CompilerFacts,
    msvc_banner: bool,
}

/// Process-wide memo: exact key text to facts.
static PROCESS_MEMO: Mutex<Vec<(String, CompilerFacts)>> = Mutex::new(Vec::new());

/// The facts for `cc`, probing only when neither memo knows this toolchain.
/// `cache_root` holds the cross-process memo files.
pub(super) fn facts(cc: &str, cache_root: &Path) -> CompilerFacts {
    let env = |name: &str| std::env::var_os(name);
    let key = resolve_program(cc).and_then(|path| memo_key(cc, &path, env));
    memo_facts(
        key.as_deref(),
        &PROCESS_MEMO,
        cache_root,
        |argument| spawn_probe(cc, argument),
        |name| std::env::var(name).ok(),
        cc,
    )
}

/// Memo lookup and fill around [`probe_facts`]; the runner and environment
/// are injected so tests can count spawns.
fn memo_facts(
    key: Option<&str>,
    process_memo: &Mutex<Vec<(String, CompilerFacts)>>,
    cache_root: &Path,
    run: impl FnMut(&str) -> Option<ProbeOutput>,
    env: impl Fn(&str) -> Option<String>,
    cc: &str,
) -> CompilerFacts {
    let Some(key) = key else {
        return probe_facts(cc, run, env).facts;
    };
    if let Some(facts) = process_memo.lock().ok().and_then(|memo| {
        memo.iter()
            .find(|(stored, _)| stored == key)
            .map(|(_, facts)| facts.clone())
    }) {
        return facts;
    }
    let memo_dir = cache_root.join(MEMO_DIR);
    let path = memo_dir.join(file_name(key));
    let facts = match read_memo(&path, key) {
        Some(facts) => facts,
        None => {
            let probed = probe_facts(cc, run, env);
            if probed.msvc_banner {
                write_memo(&memo_dir, &path, key, &probed.facts);
            }
            probed.facts
        }
    };
    if let Ok(mut memo) = process_memo.lock() {
        if !memo.iter().any(|(stored, _)| stored == key) {
            memo.push((key.to_owned(), facts.clone()));
        }
    }
    facts
}

fn spawn_probe(cc: &str, argument: &str) -> Option<ProbeOutput> {
    // `output()` closes stdin, so a compiler that wants a source file (cl)
    // cannot block.
    let output = Command::new(cc).arg(argument).output().ok()?;
    Some(ProbeOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Run the identity and target probes, each argument at most once.
fn probe_facts(
    cc: &str,
    mut run: impl FnMut(&str) -> Option<ProbeOutput>,
    env: impl Fn(&str) -> Option<String>,
) -> Probed {
    let mut outputs: Vec<(&'static str, Option<ProbeOutput>)> = Vec::new();
    let mut probe = |argument: &'static str| -> Option<ProbeOutput> {
        if let Some((_, output)) = outputs.iter().find(|(stored, _)| *stored == argument) {
            return output.clone();
        }
        let output = run(argument);
        outputs.push((argument, output.clone()));
        output
    };

    // Identity: version text. GCC/Clang answer `--version`. MSVC `cl` has no
    // version option: it prints its banner (version and target) on stderr and
    // then fails for lack of a source file (D8003); the banner is captured
    // either way.
    let mut identity = cc.to_owned();
    let mut identity_outputs = Vec::new();
    for argument in ["--version", "/Bv"] {
        if let Some(output) = probe(argument) {
            identity.push('\n');
            identity.push_str(&output.stdout);
            identity.push_str(&output.stderr);
            let success = output.success;
            identity_outputs.push(output);
            if success {
                break;
            }
        }
    }
    if identity.contains("Microsoft") {
        // The banner names the compiler version but not the toolset or SDK
        // whose headers and libraries the developer environment selected.
        for variable in ["VCToolsVersion", "WindowsSDKVersion", "VSCMD_ARG_TGT_ARCH"] {
            if let Some(value) = env(variable) {
                identity.push_str(&format!("\n{variable}={value}"));
            }
        }
    }

    // `cl` rejects `-dumpmachine` like any option without a source file, so a
    // pure banner answer (both probes ran and failed) needs no third spawn.
    let banner_target = (identity_outputs.len() == 2
        && identity_outputs.iter().all(|output| !output.success))
    .then(|| target_from_output(&probe_text(&identity_outputs[0])))
    .flatten()
    .filter(|target| target.starts_with("msvc-"));
    let msvc_banner = banner_target.is_some();
    let target = banner_target.unwrap_or_else(|| {
        if let Some(output) = probe("-dumpmachine") {
            let target = output.stdout.trim();
            if output.success && !target.is_empty() {
                return target.to_owned();
            }
        }
        for argument in ["--version", "/Bv"] {
            if let Some(target) = probe(argument)
                .as_ref()
                .and_then(|output| target_from_output(&probe_text(output)))
            {
                return target;
            }
        }
        format!(
            "unreported-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    });
    Probed {
        facts: CompilerFacts { identity, target },
        msvc_banner,
    }
}

fn probe_text(output: &ProbeOutput) -> String {
    format!("{}\n{}", output.stdout, output.stderr)
}

/// The target named by a `Target:` line (Clang `--version`) or an MSVC banner
/// (`... for x64`).
pub(super) fn target_from_output(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if let Some(target) = line.strip_prefix("Target:") {
            let target = target.trim();
            if !target.is_empty() {
                return Some(target.to_owned());
            }
        }
        if line.contains("Microsoft") {
            if let Some((_, architecture)) = line.rsplit_once(" for ") {
                let architecture = architecture.trim();
                if !architecture.is_empty() && !architecture.contains(' ') {
                    return Some(format!("msvc-{architecture}"));
                }
            }
        }
    }
    None
}

/// Exact memo key: spelling, canonical executable path, its size and
/// modification time, and the environment in [`MEMO_ENVIRONMENT`]. `None`
/// when the file metadata cannot identify the compiler.
fn memo_key(cc: &str, path: &Path, env: impl Fn(&str) -> Option<OsString>) -> Option<String> {
    let path = std::fs::canonicalize(path).ok()?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    let mut key = format!(
        "{FORMAT}\ncc={cc}\npath={}\nsize={}\nmtime={}.{:09}\n",
        path.display(),
        metadata.len(),
        modified.as_secs(),
        modified.subsec_nanos()
    );
    for name in MEMO_ENVIRONMENT {
        if let Some(value) = env(name) {
            key.push_str(&format!("env {name}={}\n", value.to_string_lossy()));
        }
    }
    Some(key)
}

fn file_name(key: &str) -> String {
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

/// Memo file body: the format line, then each field as `<name> <byte length>`
/// on its own line followed by the bytes and a newline. The stored key must
/// equal the requested key exactly, so a hash collision is never admitted.
fn encode(key: &str, facts: &CompilerFacts) -> String {
    let mut body = format!("{FORMAT}\n");
    for (name, value) in [
        ("key", key),
        ("identity", facts.identity.as_str()),
        ("target", facts.target.as_str()),
    ] {
        body.push_str(&format!("{name} {}\n{value}\n", value.len()));
    }
    body
}

fn decode(body: &str, key: &str) -> Option<CompilerFacts> {
    let mut rest = body.strip_prefix(FORMAT)?.strip_prefix('\n')?;
    let mut field = |name: &str| -> Option<String> {
        let (header, after) = rest.split_once('\n')?;
        let length: usize = header.strip_prefix(name)?.strip_prefix(' ')?.parse().ok()?;
        let value = after.get(..length)?;
        rest = after.get(length..)?.strip_prefix('\n')?;
        Some(value.to_owned())
    };
    if field("key")? != key {
        return None;
    }
    let identity = field("identity")?;
    let target = field("target")?;
    rest.is_empty()
        .then_some(CompilerFacts { identity, target })
}

fn read_memo(path: &Path, key: &str) -> Option<CompilerFacts> {
    decode(&std::fs::read_to_string(path).ok()?, key)
}

/// Publish through a rename so concurrent readers see all or nothing; any
/// failure only means the next process probes again.
fn write_memo(memo_dir: &Path, path: &Path, key: &str, facts: &CompilerFacts) {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    if std::fs::create_dir_all(memo_dir).is_ok()
        && std::fs::write(&temporary, encode(key, facts)).is_ok()
        && std::fs::rename(&temporary, path).is_err()
    {
        let _ = std::fs::remove_file(&temporary);
    }
}

/// The file `Command::new(cc)` would run, or `None` when that cannot be
/// determined. A spelling with a directory part is used as given (relative to
/// the working directory); a bare name is searched on `PATH`.
fn resolve_program(cc: &str) -> Option<PathBuf> {
    let spelled = Path::new(cc);
    if cc.is_empty() {
        return None;
    }
    if spelled.components().count() > 1 || spelled.is_absolute() {
        return executable_file(&with_exe_suffix(spelled));
    }
    let name = with_exe_suffix(spelled);
    #[cfg(windows)]
    {
        // Rust's Windows process spawning searches the application directory
        // and the system directories before `PATH`. Rather than mirror that
        // order exactly, refuse to name a file when any of them holds the
        // program, so the memo never keys on a file other than the one run.
        let system_root = PathBuf::from(std::env::var_os("SystemRoot")?);
        let application = std::env::current_exe().ok()?.parent()?.to_path_buf();
        for directory in [
            application,
            system_root.join("System32"),
            system_root.join("System"),
            system_root,
        ] {
            if directory.join(&name).exists() {
                return None;
            }
        }
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .find_map(|directory| executable_file(&directory.join(&name)))
}

fn with_exe_suffix(path: &Path) -> PathBuf {
    if cfg!(windows) && path.extension().is_none() {
        path.with_extension(std::env::consts::EXE_EXTENSION)
    } else {
        path.to_path_buf()
    }
}

fn executable_file(path: &Path) -> Option<PathBuf> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Exit success, stdout and stderr; `None` is a failed spawn.
    type Answer = Option<(bool, &'static str, &'static str)>;

    /// A scripted compiler: its answer to each probe argument.
    struct FakeCompiler {
        answers: Vec<(&'static str, Answer)>,
    }

    impl FakeCompiler {
        fn run(&self, argument: &str) -> Option<ProbeOutput> {
            self.answers
                .iter()
                .find(|(stored, _)| *stored == argument)
                .and_then(|(_, answer)| *answer)
                .map(|(success, stdout, stderr)| ProbeOutput {
                    success,
                    stdout: stdout.to_owned(),
                    stderr: stderr.to_owned(),
                })
        }
    }

    const MSVC_BANNER: &str =
        "Microsoft (R) C/C++ Optimizing Compiler Version 19.51.36231 for x64\r\nCopyright (C) Microsoft Corporation.  All rights reserved.\r\n\r\n";

    fn gcc() -> FakeCompiler {
        FakeCompiler {
            answers: vec![
                ("--version", Some((true, "cc (GCC) 13.2.0\n", ""))),
                ("-dumpmachine", Some((true, "x86_64-linux-gnu\n", ""))),
                ("/Bv", Some((false, "", "cc: error: /Bv: No such file\n"))),
            ],
        }
    }

    fn clang_without_dumpmachine() -> FakeCompiler {
        FakeCompiler {
            answers: vec![
                (
                    "--version",
                    Some((
                        true,
                        "clang version 19.1.0\nTarget: aarch64-apple-darwin\n",
                        "",
                    )),
                ),
                ("-dumpmachine", Some((false, "", "unknown argument\n"))),
                ("/Bv", Some((false, "", "no such file\n"))),
            ],
        }
    }

    fn msvc() -> FakeCompiler {
        FakeCompiler {
            answers: vec![
                (
                    "--version",
                    Some((
                        false,
                        "",
                        "Microsoft (R) C/C++ Optimizing Compiler Version 19.51.36231 for x64\r\nCopyright (C) Microsoft Corporation.  All rights reserved.\r\n\r\ncl : Command line warning D9002 : ignoring unknown option '--version'\r\ncl : Command line error D8003 : missing source filename\r\n",
                    )),
                ),
                (
                    "/Bv",
                    Some((
                        false,
                        "",
                        "Microsoft (R) C/C++ Optimizing Compiler Version 19.51.36231 for x64\r\nCopyright (C) Microsoft Corporation.  All rights reserved.\r\n\r\ncl : Command line error D8003 : missing source filename\r\n",
                    )),
                ),
                (
                    "-dumpmachine",
                    Some((false, "", "cl : Command line error D8003\r\n")),
                ),
            ],
        }
    }

    /// Answers nothing useful: spawns fail or print nothing.
    fn silent() -> FakeCompiler {
        FakeCompiler {
            answers: vec![
                ("--version", Some((false, "", ""))),
                ("/Bv", None),
                ("-dumpmachine", Some((false, "", ""))),
            ],
        }
    }

    /// The probes as written before memoization (one spawn per call), kept
    /// as the reference the memoized probe must reproduce exactly.
    fn reference_facts(
        cc: &str,
        run: impl Fn(&str) -> Option<ProbeOutput>,
        env: impl Fn(&str) -> Option<String>,
    ) -> (CompilerFacts, usize) {
        let spawns = Cell::new(0);
        let run = |argument: &str| {
            spawns.set(spawns.get() + 1);
            run(argument)
        };
        let mut identity = cc.to_owned();
        for argument in ["--version", "/Bv"] {
            if let Some(output) = run(argument) {
                identity.push('\n');
                identity.push_str(&output.stdout);
                identity.push_str(&output.stderr);
                if output.success {
                    break;
                }
            }
        }
        if identity.contains("Microsoft") {
            for variable in ["VCToolsVersion", "WindowsSDKVersion", "VSCMD_ARG_TGT_ARCH"] {
                if let Some(value) = env(variable) {
                    identity.push_str(&format!("\n{variable}={value}"));
                }
            }
        }
        let target = (|| {
            if let Some(output) = run("-dumpmachine") {
                if output.success {
                    let target = output.stdout.trim().to_owned();
                    if !target.is_empty() {
                        return target;
                    }
                }
            }
            for argument in ["--version", "/Bv"] {
                if let Some(output) = run(argument) {
                    let text = format!("{}\n{}", output.stdout, output.stderr);
                    if let Some(target) = target_from_output(&text) {
                        return target;
                    }
                }
            }
            format!(
                "unreported-{}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })();
        (CompilerFacts { identity, target }, spawns.get())
    }

    fn msvc_env(name: &str) -> Option<String> {
        match name {
            "VCToolsVersion" => Some("14.51.36231".to_owned()),
            "VSCMD_ARG_TGT_ARCH" => Some("x64".to_owned()),
            _ => None,
        }
    }

    #[test]
    fn probes_reproduce_the_unmemoized_text_with_fewer_spawns() {
        for (name, compiler, expected_spawns, reference_spawns) in [
            ("gcc", gcc(), 2, 2),
            ("clang", clang_without_dumpmachine(), 2, 3),
            ("msvc", msvc(), 2, 4),
            ("silent", silent(), 3, 5),
        ] {
            let (expected, old_spawns) =
                reference_facts(name, |argument| compiler.run(argument), msvc_env);
            assert_eq!(old_spawns, reference_spawns, "{name}: reference spawns");
            let spawns = Cell::new(0);
            let probed = probe_facts(
                name,
                |argument| {
                    spawns.set(spawns.get() + 1);
                    compiler.run(argument)
                },
                msvc_env,
            );
            assert_eq!(probed.facts, expected, "{name}: identical key text");
            assert_eq!(spawns.get(), expected_spawns, "{name}: spawns");
            assert_eq!(probed.msvc_banner, name == "msvc", "{name}");
        }
        let probed = probe_facts("cl", |argument| msvc().run(argument), msvc_env);
        assert_eq!(probed.facts.target, "msvc-x64");
        assert!(probed
            .facts
            .identity
            .contains("\nVCToolsVersion=14.51.36231"));
        assert!(probed
            .facts
            .identity
            .contains(MSVC_BANNER.lines().next().unwrap()));
    }

    #[test]
    fn target_parses_clang_and_msvc_reports() {
        assert_eq!(
            target_from_output("clang version 19\nTarget: aarch64-apple-darwin\n"),
            Some("aarch64-apple-darwin".to_owned())
        );
        assert_eq!(
            target_from_output("Microsoft (R) C/C++ Optimizing Compiler Version 19.44 for ARM64\n"),
            Some("msvc-ARM64".to_owned())
        );
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "llg-compiler-probe-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    #[test]
    fn memo_key_follows_path_contents_and_environment() {
        let dir = temp_dir("key");
        let compiler = dir.join("cl");
        std::fs::write(&compiler, "first").unwrap();
        let key = memo_key("cl", &compiler, no_env).unwrap();
        assert_eq!(key, memo_key("cl", &compiler, no_env).unwrap());
        assert!(key.contains(&format!(
            "path={}\n",
            std::fs::canonicalize(&compiler).unwrap().display()
        )));
        assert_ne!(key, memo_key("other-cl", &compiler, no_env).unwrap());

        let copy = dir.join("cl-copy");
        std::fs::copy(&compiler, &copy).unwrap();
        assert_ne!(key, memo_key("cl", &copy, no_env).unwrap(), "another path");

        std::fs::write(&compiler, "second, longer").unwrap();
        assert_ne!(key, memo_key("cl", &compiler, no_env).unwrap(), "replaced");

        let key = memo_key("cl", &compiler, no_env).unwrap();
        for name in ["PATH", "VCToolsVersion", "INCLUDE", "CL", "_CL_", "LANG"] {
            let with = memo_key("cl", &compiler, |variable: &str| {
                (variable == name).then(|| OsString::from("x"))
            })
            .unwrap();
            assert_ne!(key, with, "{name} is in the key");
        }
        let unrelated = memo_key("cl", &compiler, |variable: &str| {
            (variable == "LLG_UNRELATED").then(|| OsString::from("x"))
        })
        .unwrap();
        assert_eq!(key, unrelated);
        assert!(memo_key("cl", &dir.join("missing"), no_env).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn memo_files_round_trip_and_refuse_other_keys() {
        let facts = CompilerFacts {
            identity: "cl\nbanner line\nfeature 12\n".to_owned(),
            target: "msvc-x64".to_owned(),
        };
        let body = encode("key\nwith lines", &facts);
        assert_eq!(decode(&body, "key\nwith lines"), Some(facts.clone()));
        assert_eq!(decode(&body, "key\nwith line"), None);
        assert_eq!(decode(&body, "key"), None);
        assert_eq!(decode(&body[..body.len() - 2], "key\nwith lines"), None);
        assert_eq!(decode(&format!("{body}x"), "key\nwith lines"), None);
        assert_eq!(
            decode(
                &body.replacen(FORMAT, "llg-compiler-probe-v0", 1),
                "key\nwith lines"
            ),
            None
        );
    }

    #[test]
    fn msvc_results_are_memoized_per_process_and_on_disk() {
        let dir = temp_dir("memo");
        let compiler = dir.join("cl");
        std::fs::write(&compiler, "fake cl").unwrap();
        let cache = dir.join("cache");
        let spawns = Cell::new(0);
        let lookup = |memo: &Mutex<Vec<(String, CompilerFacts)>>, fake: &FakeCompiler| {
            let key = memo_key("cl", &compiler, no_env);
            memo_facts(
                key.as_deref(),
                memo,
                &cache,
                |argument| {
                    spawns.set(spawns.get() + 1);
                    fake.run(argument)
                },
                msvc_env,
                "cl",
            )
        };
        let first_process = Mutex::new(Vec::new());
        let first = lookup(&first_process, &msvc());
        assert_eq!(spawns.get(), 2);
        assert_eq!(first.target, "msvc-x64");
        assert_eq!(lookup(&first_process, &msvc()), first);
        assert_eq!(spawns.get(), 2, "the process memo answers");

        let second_process = Mutex::new(Vec::new());
        assert_eq!(lookup(&second_process, &msvc()), first);
        assert_eq!(spawns.get(), 2, "the disk memo answers a new process");
        let root_children = std::fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            root_children,
            [MEMO_DIR],
            "memo files stay in their own directory"
        );
        assert_eq!(std::fs::read_dir(cache.join(MEMO_DIR)).unwrap().count(), 1);

        // An upgraded compiler file is probed again.
        std::fs::write(&compiler, "fake cl, upgraded").unwrap();
        let third_process = Mutex::new(Vec::new());
        assert_eq!(lookup(&third_process, &msvc()), first);
        assert_eq!(spawns.get(), 4, "a changed compiler is re-probed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gcc_results_stay_in_the_process_memo() {
        let dir = temp_dir("gcc");
        let compiler = dir.join("cc");
        std::fs::write(&compiler, "fake cc").unwrap();
        let cache = dir.join("cache");
        let spawns = Cell::new(0);
        let lookup = |memo: &Mutex<Vec<(String, CompilerFacts)>>| {
            let key = memo_key("cc", &compiler, no_env);
            memo_facts(
                key.as_deref(),
                memo,
                &cache,
                |argument| {
                    spawns.set(spawns.get() + 1);
                    gcc().run(argument)
                },
                |_| None,
                "cc",
            )
        };
        let process = Mutex::new(Vec::new());
        let facts = lookup(&process);
        assert_eq!(facts.target, "x86_64-linux-gnu");
        assert_eq!(lookup(&process), facts);
        assert_eq!(spawns.get(), 2);
        assert!(!cache.exists(), "wrapper-prone compilers are not stored");
        assert_eq!(lookup(&Mutex::new(Vec::new())), facts);
        assert_eq!(spawns.get(), 4, "a new process probes again");

        // Without a key (unresolvable compiler) nothing is memoized.
        let unkeyed = Mutex::new(Vec::new());
        for _ in 0..2 {
            memo_facts(
                None,
                &unkeyed,
                &cache,
                |argument| {
                    spawns.set(spawns.get() + 1);
                    gcc().run(argument)
                },
                |_| None,
                "cc",
            );
        }
        assert_eq!(spawns.get(), 8);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn programs_resolve_like_process_spawning() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("resolve");
        let program = dir.join("llg-probe-cc");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            resolve_program(&program.to_string_lossy()),
            Some(program.clone())
        );
        let plain = dir.join("not-executable");
        std::fs::write(&plain, "").unwrap();
        assert_eq!(resolve_program(&plain.to_string_lossy()), None);
        assert_eq!(resolve_program(""), None);
        assert!(resolve_program("sh").is_some_and(|path| path.is_absolute()));
        assert_eq!(resolve_program("llg-no-such-compiler-on-path"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
