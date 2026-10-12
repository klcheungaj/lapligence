//! Toolchain-detection seeds for fresh CMake build trees.
//!
//! Every fresh configure normally re-runs CMake's toolchain detection: system
//! inspection, compiler identification (compiling and linking
//! `CMakeCCompilerId.c`), the ABI `try_compile`, and on MSVC the resource
//! compiler. With `cl.exe`/`link.exe` that costs seconds per model directory.
//!
//! CMake skips detection for a language whose `CMake<LANG>Compiler.cmake`
//! already exists in `<build>/CMakeFiles/<version>/`, provided the cache
//! carries `CMAKE_PLATFORM_INFO_INITIALIZED` (without it CMake discards the
//! directory as left over from another version). That is the path every
//! re-configure of an existing tree takes. A seed stores the platform files of
//! one probe configure plus the cache entries detection creates (for example
//! `CMAKE_AR` and `CMAKE_EXECUTABLE_FORMAT`, which later modules read), and
//! [`Seed::apply`] recreates that state in a fresh tree.
//!
//! A seed is published only after a self-check: a probe project is configured
//! from scratch and once more (CMake's steady re-configure state), then from
//! the seed into a second tree. The generated build files of both trees must
//! match after replacing the tree paths and dropping lines that name CMake's
//! own modules (re-run dependency lists). A mismatch, or platform files that
//! name the probe tree, records a `rejected` marker so the key is never probed
//! again; builds for it configure unseeded as before.
//!
//! The key covers everything that steers detection: the CMake program and its
//! `--version`, generator, compiler (spelling, `--version`/banner and
//! reported target), flags, launcher, host OS/architecture, the environment
//! variables CMake or the MSVC/Apple toolchains read (including `PATH`, which
//! selects tools such as `rc.exe`, `ar` and `ninja`), and the working
//! directory when the compiler is a relative path. A compiler or CMake upgrade
//! therefore selects a new entry. Seeds live in their own [`SEED_DIR`]
//! subdirectory of the runtime cache root, so every other child of the root
//! with a `ready` marker remains a runtime archive entry. They use the same
//! per-entry lock file and exact-key ready marker as the archives; files are
//! written before the marker is renamed into place and are never changed
//! afterwards. `LLG_CMAKE_TOOLCHAIN_SEED=0` disables seeding.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use super::RuntimeCacheLock;

/// Environment switch: `0`, `off`, `false` or `no` disables seeding.
pub const SEED_ENV: &str = "LLG_CMAKE_TOOLCHAIN_SEED";

/// Subdirectory of the runtime cache root that holds every seed entry
/// (`<root>/cmake-toolchain/<hash>/`) and its lock file.
pub const SEED_DIR: &str = "cmake-toolchain";

/// Bumped whenever the layout or the self-check changes, so older entries are
/// never read by a newer llg.
const FORMAT: &str = "llg-cmake-toolchain-seed-v4";

/// The text every generated project places before `project()`. Detection
/// runs inside `project()`, so the policies in effect there must match the
/// probe's; a unit test keeps the generated templates on this preamble.
pub(super) const PROJECT_PREAMBLE: &str = "cmake_minimum_required(VERSION 3.16)\n";

/// Probe project: a static library linked into an executable, the target
/// kinds of the runtime archive and the model.
const PROBE_PROJECT: &str = "project(llg_toolchain_probe C)\n\
add_library(probe_lib STATIC probe_lib.c)\n\
add_executable(probe_exe probe_main.c)\n\
target_link_libraries(probe_exe PRIVATE probe_lib)\n";

/// Environment read by CMake's detection or by the compilers it runs.
pub(super) const KEY_ENVIRONMENT: [&str; 22] = [
    "PATH",
    "CFLAGS",
    "CPPFLAGS",
    "LDFLAGS",
    "RC",
    "RCFLAGS",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "VCToolsVersion",
    "WindowsSDKVersion",
    "VSCMD_ARG_TGT_ARCH",
    "SDKROOT",
    "DEVELOPER_DIR",
    "MACOSX_DEPLOYMENT_TARGET",
    "CMAKE_OSX_ARCHITECTURES",
    "CMAKE_APPLE_SILICON_PROCESSOR",
    "CMAKE_TOOLCHAIN_FILE",
    "CMAKE_GENERATOR_PLATFORM",
    "CMAKE_GENERATOR_TOOLSET",
    "CMAKE_GENERATOR_INSTANCE",
    "CMAKE_SYSROOT",
];

/// Configure inputs that determine CMake's toolchain detection.
pub(super) struct Toolchain<'a> {
    pub cmake: &'a str,
    pub generator: &'a str,
    pub cc: &'a str,
    pub flags: &'a str,
    pub launcher: &'a str,
    pub compiler_identity: &'a str,
    pub compiler_target: &'a str,
}

/// Configuration of CMake's detection `try_compile` projects. Without it
/// they use Debug flags; with MSVC that means `/Zi /RTC1`, so every compiler
/// check writes a PDB through the shared `mspdbsrv.exe`, which is slower and
/// was seen failing with C1041 (cannot open the PDB) under parallel CI tests.
/// The models themselves build in Release as well.
const TRY_COMPILE_CONFIGURATION: &str = "Release";

/// `-G`, launcher, compiler, flag and detection arguments shared by the probe
/// and the generated projects' configures, so they cannot drift apart.
///
/// The install prefix is explicit (and empty) because llg's trees never
/// install, while CMake's Windows default depends on detection order:
/// `CMakeGenericSystem.cmake` picks `Program Files (x86)` when it runs before
/// `CMAKE_SIZEOF_VOID_P` is known (a clean configure) and `Program Files`
/// when a seeded tree already has it, so a seeded tree would otherwise differ
/// from a clean one in `cmake_install.cmake`. The prefix also feeds
/// `CMAKE_SYSTEM_PREFIX_PATH`; `CMAKE_FIND_NO_INSTALL_PREFIX` keeps the empty
/// value out of the find search paths.
pub(super) fn toolchain_args(
    generator: &str,
    launcher: &str,
    cc: &str,
    flags: &str,
) -> [String; 8] {
    [
        "-G".to_owned(),
        generator.to_owned(),
        format!("-DCMAKE_C_COMPILER_LAUNCHER={launcher}"),
        format!("-DCMAKE_C_COMPILER={cc}"),
        format!("-DCMAKE_C_FLAGS:STRING={flags}"),
        format!("-DCMAKE_TRY_COMPILE_CONFIGURATION={TRY_COMPILE_CONFIGURATION}"),
        "-DCMAKE_INSTALL_PREFIX:PATH=".to_owned(),
        "-DCMAKE_FIND_NO_INSTALL_PREFIX:BOOL=ON".to_owned(),
    ]
}

/// Whether the raw `$LLG_CMAKE_TOOLCHAIN_SEED` value leaves seeding enabled.
pub(super) fn enabled_by(value: Option<&str>) -> bool {
    !value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        )
    })
}

/// A published seed for one key.
#[derive(Debug)]
pub(super) struct Seed {
    entry: PathBuf,
    key: String,
    version_dir: String,
    cache_args: Vec<String>,
}

/// Look up the seed for `toolchain` under `root`, probing and publishing it
/// when no process has done so yet. Never waits: when another process holds
/// the entry lock this configure simply runs unseeded.
pub(super) fn prepare(root: &Path, toolchain: &Toolchain<'_>) -> Option<Seed> {
    if !enabled_by(std::env::var(SEED_ENV).ok().as_deref()) {
        return None;
    }
    let version = cmake_version(toolchain.cmake)?;
    let cwd = std::env::current_dir().ok();
    let key = key_text(toolchain, &version, cwd.as_deref(), |name| {
        std::env::var_os(name)
    });
    let seeds = root.join(SEED_DIR);
    let entry = seeds.join(entry_name(&key));
    if let Some(seed) = load(&entry, &key) {
        return Some(seed);
    }
    if is_rejected(&entry, &key) {
        return None;
    }
    std::fs::create_dir_all(&seeds).ok()?;
    let _lock = RuntimeCacheLock::try_acquire(&entry).ok()??;
    if let Some(seed) = load(&entry, &key) {
        return Some(seed);
    }
    if is_rejected(&entry, &key) {
        return None;
    }
    match probe(&entry, &key, toolchain) {
        Ok(seed) => Some(seed),
        Err(ProbeError::Rejected(reason)) => {
            write_marker(&entry, "rejected", &format!("{reason}\n{key}"));
            None
        }
        Err(ProbeError::Failed) => None,
    }
}

impl Seed {
    /// The exact key this seed was published under.
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    /// Copy the platform files into the fresh `build_dir` and return the
    /// extra configure arguments, or `None` (with `build_dir` removed) when
    /// the copy fails.
    pub(super) fn apply(&self, build_dir: &Path) -> Option<Vec<String>> {
        let source = self.entry.join("platform").join(&self.version_dir);
        if copy_platform_files(&source, build_dir, &self.version_dir).is_err() {
            super::remove_dir_all_quiet(build_dir);
            return None;
        }
        let mut args = vec!["-DCMAKE_PLATFORM_INFO_INITIALIZED:INTERNAL=1".to_owned()];
        args.extend(self.cache_args.iter().cloned());
        Some(args)
    }

    /// Stop using this seed: a seeded configure failed where a clean one
    /// succeeded. Builds for the key then configure unseeded.
    pub(super) fn reject(&self, reason: &str) {
        write_marker(&self.entry, "rejected", &format!("{reason}\n{}", self.key));
    }
}

/// Exact key text; the entry directory name is its hash and both markers
/// store the full text, so a hash collision cannot admit a foreign seed.
fn key_text(
    toolchain: &Toolchain<'_>,
    cmake_version: &str,
    cwd: Option<&Path>,
    env: impl Fn(&str) -> Option<OsString>,
) -> String {
    let mut key = format!(
        "{FORMAT}\n{PROJECT_PREAMBLE}{PROBE_PROJECT}cmake={}\n{cmake_version}\ngenerator={}\ncc={}\nflags={}\nlauncher={}\ntarget={}\nhost={}-{}\n",
        toolchain.cmake,
        toolchain.generator,
        toolchain.cc,
        toolchain.flags,
        toolchain.launcher,
        toolchain.compiler_target,
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    key.push_str(toolchain.compiler_identity);
    key.push('\n');
    // A relative compiler path with a directory part resolves against the
    // working directory, so the same spelling can name different compilers.
    let cc = Path::new(toolchain.cc);
    if cc.is_relative() && cc.components().count() > 1 {
        key.push_str(&format!(
            "cwd={}\n",
            cwd.map(|cwd| cwd.display().to_string()).unwrap_or_default()
        ));
    }
    for name in KEY_ENVIRONMENT {
        if let Some(value) = env(name) {
            key.push_str(&format!("env {name}={}\n", value.to_string_lossy()));
        }
    }
    key
}

pub(super) fn entry_name(key: &str) -> String {
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

/// `cmake --version`, probed once per CMake program and process.
fn cmake_version(cmake: &str) -> Option<String> {
    static VERSIONS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let mut versions = VERSIONS.lock().ok()?;
    if let Some((_, version)) = versions.iter().find(|(program, _)| program == cmake) {
        return Some(version.clone());
    }
    let output = Command::new(cmake).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    versions.push((cmake.to_owned(), version.clone()));
    Some(version)
}

/// Marker body: first line is the detail (version directory or reason), the
/// rest is the exact key.
fn read_marker(entry: &Path, name: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(entry.join(name)).ok()?;
    let (detail, stored) = text.split_once('\n')?;
    (stored == key).then(|| detail.to_owned())
}

fn is_rejected(entry: &Path, key: &str) -> bool {
    read_marker(entry, "rejected", key).is_some()
}

fn load(entry: &Path, key: &str) -> Option<Seed> {
    let version_dir = read_marker(entry, "ready", key)?;
    if version_dir.is_empty() || version_dir.contains(['/', '\\']) || version_dir == ".." {
        return None;
    }
    let cache = std::fs::read_to_string(entry.join("cache.txt")).ok()?;
    Some(Seed {
        entry: entry.to_path_buf(),
        key: key.to_owned(),
        version_dir,
        cache_args: cache
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| format!("-D{line}"))
            .collect(),
    })
}

/// Write `<entry>/<name>` through a rename so readers see all or nothing.
fn write_marker(entry: &Path, name: &str, body: &str) {
    let temporary = entry.join(format!("{name}.tmp-{}", std::process::id()));
    if std::fs::create_dir_all(entry).is_ok()
        && std::fs::write(&temporary, body).is_ok()
        && std::fs::rename(&temporary, entry.join(name)).is_err()
    {
        let _ = std::fs::remove_file(&temporary);
    }
}

fn copy_platform_files(source: &Path, build_dir: &Path, version_dir: &str) -> std::io::Result<()> {
    let target = build_dir.join("CMakeFiles").join(version_dir);
    std::fs::create_dir_all(&target)?;
    for path in platform_files(source)? {
        if let Some(name) = path.file_name() {
            std::fs::copy(&path, target.join(name))?;
        }
    }
    Ok(())
}

/// The `*.cmake` files directly in a platform-information directory
/// (`CMakeSystem.cmake`, `CMake<LANG>Compiler.cmake`). Detection scratch such
/// as `CompilerIdC/` and the ABI binary is not reused.
fn platform_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "cmake")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

#[derive(Debug)]
enum ProbeError {
    /// The toolchain cannot be seeded; remembered for the key.
    Rejected(String),
    /// The probe could not run (for example a broken compiler); not
    /// remembered, the real configure reports the problem.
    Failed,
}

fn probe(entry: &Path, key: &str, toolchain: &Toolchain<'_>) -> Result<Seed, ProbeError> {
    for name in ["ready", "rejected", "cache.txt"] {
        let _ = std::fs::remove_file(entry.join(name));
    }
    for name in ["platform", "probe"] {
        super::remove_dir_all_quiet(&entry.join(name));
    }
    let probe_dir = entry.join("probe");
    let result = run_probe(entry, &probe_dir, key, toolchain);
    super::remove_dir_all_quiet(&probe_dir);
    result
}

fn run_probe(
    entry: &Path,
    probe_dir: &Path,
    key: &str,
    toolchain: &Toolchain<'_>,
) -> Result<Seed, ProbeError> {
    let source = probe_dir.join("src");
    std::fs::create_dir_all(&source).map_err(|_| ProbeError::Failed)?;
    for (name, text) in [
        (
            "CMakeLists.txt",
            format!("{PROJECT_PREAMBLE}{PROBE_PROJECT}"),
        ),
        (
            "probe_lib.c",
            "int llg_probe(void) { return 0; }\n".to_owned(),
        ),
        (
            "probe_main.c",
            "int llg_probe(void);\nint main(void) { return llg_probe(); }\n".to_owned(),
        ),
    ] {
        std::fs::write(source.join(name), text).map_err(|_| ProbeError::Failed)?;
    }
    let configure = |build: &Path, extra: &[String]| -> bool {
        Command::new(toolchain.cmake)
            .arg("-S")
            .arg(&source)
            .arg("-B")
            .arg(build)
            .args(toolchain_args(
                toolchain.generator,
                toolchain.launcher,
                toolchain.cc,
                toolchain.flags,
            ))
            .args(extra)
            .output()
            .is_ok_and(|output| output.status.success())
    };

    // Reference: a clean configure, then CMake's own re-configure, whose
    // state every incremental model rebuild already uses.
    let full = probe_dir.join("full");
    if !configure(&full, &[]) {
        return Err(ProbeError::Failed);
    }
    let version_dir = platform_version_dir(&full)?;
    let platform = full.join("CMakeFiles").join(&version_dir);
    let files = platform_files(&platform).map_err(|_| ProbeError::Failed)?;
    let names: BTreeSet<String> = files
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    for required in ["CMakeSystem.cmake", "CMakeCCompiler.cmake"] {
        if !names.contains(required) {
            return Err(ProbeError::Rejected(format!("probe wrote no {required}")));
        }
    }
    let tree_paths = path_spellings(&[probe_dir]);
    for path in &files {
        let text = std::fs::read(path).map_err(|_| ProbeError::Failed)?;
        if tree_paths
            .iter()
            .any(|spelling| contains_bytes(&text, spelling.as_bytes()))
        {
            return Err(ProbeError::Rejected(format!(
                "{} names the probe tree",
                path.display()
            )));
        }
    }
    if !configure(&full, &[]) {
        return Err(ProbeError::Failed);
    }

    // Seeded without cache entries: names missing there are the entries
    // detection creates.
    let marker = vec!["-DCMAKE_PLATFORM_INFO_INITIALIZED:INTERNAL=1".to_owned()];
    let seeded = probe_dir.join("seeded");
    copy_platform_files(&platform, &seeded, &version_dir).map_err(|_| ProbeError::Failed)?;
    if !configure(&seeded, &marker) {
        return Err(ProbeError::Rejected(
            "seeded probe configure failed".to_owned(),
        ));
    }
    let full_cache = read_cache(&full).ok_or(ProbeError::Failed)?;
    let seeded_cache = read_cache(&seeded).ok_or(ProbeError::Failed)?;
    let detected: Vec<String> = full_cache
        .iter()
        .filter(|(name, _)| !seeded_cache.contains_key(*name))
        .map(|(name, line)| {
            if tree_paths
                .iter()
                .any(|spelling| line.contains(spelling.as_str()))
            {
                Err(ProbeError::Rejected(format!(
                    "cache entry {name} names the probe tree"
                )))
            } else {
                Ok(line.clone())
            }
        })
        .collect::<Result<_, _>>()?;

    let check = probe_dir.join("check");
    copy_platform_files(&platform, &check, &version_dir).map_err(|_| ProbeError::Failed)?;
    let mut check_args = marker;
    check_args.extend(detected.iter().map(|line| format!("-D{line}")));
    if !configure(&check, &check_args) {
        return Err(ProbeError::Rejected(
            "checked probe configure failed".to_owned(),
        ));
    }
    let cmake_root = full_cache
        .get("CMAKE_ROOT")
        .and_then(|line| line.split_once('='))
        .map(|(_, value)| value.to_owned())
        .unwrap_or_default();
    let reference = generated_tree(&full, &version_dir, &cmake_root)?;
    let candidate = generated_tree(&check, &version_dir, &cmake_root)?;
    let differences = differences(&reference, &candidate);
    if !differences.is_empty() {
        return Err(ProbeError::Rejected(format!(
            "seeded build files differ from a clean configure: {}",
            differences.join("; ")
        )));
    }

    copy_platform_files(&platform, &entry.join("platform"), &version_dir)
        .map_err(|_| ProbeError::Failed)?;
    // copy_platform_files nests under CMakeFiles/; publish the flat layout.
    let nested = entry.join("platform").join("CMakeFiles").join(&version_dir);
    std::fs::rename(&nested, entry.join("platform").join(&version_dir))
        .map_err(|_| ProbeError::Failed)?;
    super::remove_dir_all_quiet(&entry.join("platform").join("CMakeFiles"));
    let mut cache = detected.join("\n");
    cache.push('\n');
    std::fs::write(entry.join("cache.txt"), cache).map_err(|_| ProbeError::Failed)?;
    write_marker(entry, "ready", &format!("{version_dir}\n{key}"));
    load(entry, key).ok_or(ProbeError::Failed)
}

/// The `CMakeFiles/<version>` directory holding `CMakeSystem.cmake`.
fn platform_version_dir(build: &Path) -> Result<String, ProbeError> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(build.join("CMakeFiles")).map_err(|_| ProbeError::Failed)? {
        let path = entry.map_err(|_| ProbeError::Failed)?.path();
        if path.join("CMakeSystem.cmake").is_file() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                found.push(name.to_owned());
            }
        }
    }
    match found.as_slice() {
        [single] => Ok(single.clone()),
        _ => Err(ProbeError::Rejected(format!(
            "expected one platform directory, found {found:?}"
        ))),
    }
}

/// Cache entries `NAME:TYPE=VALUE` by name. Quoted names and anything that
/// cannot round-trip through one `-D` argument are skipped.
fn read_cache(build: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(build.join("CMakeCache.txt")).ok()?;
    Some(parse_cache(&text))
}

fn parse_cache(text: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let Some((declaration, _)) = line.split_once('=') else {
            continue;
        };
        let Some((name, kind)) = declaration.split_once(':') else {
            continue;
        };
        if name.is_empty() || name.starts_with('"') || kind.is_empty() {
            continue;
        }
        entries.insert(name.to_owned(), line.to_owned());
    }
    entries
}

/// Every spelling of `paths` CMake may write: native, forward-slash, the
/// canonical form, and each of those as escaped in a Ninja file.
fn path_spellings(paths: &[&Path]) -> Vec<String> {
    let mut forms = Vec::new();
    for path in paths {
        forms.push(path.to_string_lossy().into_owned());
        if let Ok(canonical) = crate::ffi::platform::canonicalize(path) {
            forms.push(canonical.to_string_lossy().into_owned());
        }
    }
    spellings_of(forms)
}

/// `texts` in native and forward-slash form, each also in its Ninja-escaped
/// spellings, without empty strings and longest first (so a shorter
/// spelling never splits a longer one that contains it).
fn spellings_of(texts: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut spellings = BTreeSet::new();
    for text in texts {
        for form in [text.replace('\\', "/"), text] {
            spellings.extend(ninja_spellings(&form));
            spellings.insert(form);
        }
    }
    let mut spellings: Vec<String> = spellings.into_iter().filter(|s| !s.is_empty()).collect();
    spellings.sort_by_key(|spelling| std::cmp::Reverse(spelling.len()));
    spellings
}

/// How a Ninja file may spell `text`. Ninja escapes `$` as `$$` everywhere,
/// and CMake's Ninja generator also escapes `:` as `$:` and a space as `$ `
/// in paths (`cmake_ninja_workdir = D$:\a\...` on Windows); command lines may
/// leave them unescaped. Every combination is listed because which escapes a
/// line uses depends on where the path appears.
fn ninja_spellings(text: &str) -> [String; 4] {
    let dollars = text.replace('$', "$$");
    let colons = dollars.replace(':', "$:");
    [
        dollars.replace(' ', "$ "),
        colons.replace(' ', "$ "),
        colons,
        dollars,
    ]
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Detection scratch directories under `CMakeFiles/`: CMake writes them only
/// while it detects a toolchain, so a seeded tree never has them, and nothing
/// generated reads them back. `ShowIncludes/` holds the sources MSVC-like
/// compilers compile to find the `/showIncludes` prefix, whose result is kept
/// in `CMake<LANG>Compiler.cmake` (`CMAKE_<LANG>_CL_SHOWINCLUDES_PREFIX`).
const DETECTION_SCRATCH: [&str; 3] = [
    "CMakeFiles/CMakeScratch/",
    "CMakeFiles/pkgRedirects/",
    "CMakeFiles/ShowIncludes/",
];

/// Generated files of `build` relative to it, with the tree's own path
/// replaced and lines naming CMake's modules dropped. Detection outputs,
/// logs and the cache itself are not build rules and are skipped.
fn generated_tree(
    build: &Path,
    version_dir: &str,
    cmake_root: &str,
) -> Result<BTreeMap<String, Vec<u8>>, ProbeError> {
    let mut paths = Vec::new();
    super::collect_paths(build, &mut paths);
    let spellings = path_spellings(&[build]);
    let module_spellings = module_spellings(cmake_root);
    let platform_prefix = format!("CMakeFiles/{version_dir}/");
    let mut tree = BTreeMap::new();
    for path in paths.into_iter().filter(|path| path.is_file()) {
        let relative = path
            .strip_prefix(build)
            .map_err(|_| ProbeError::Failed)?
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "CMakeCache.txt"
            || relative.starts_with(&platform_prefix)
            || DETECTION_SCRATCH
                .iter()
                .any(|scratch| relative.starts_with(scratch))
            || matches!(
                relative.as_str(),
                "CMakeFiles/CMakeConfigureLog.yaml"
                    | "CMakeFiles/CMakeOutput.log"
                    | "CMakeFiles/CMakeError.log"
            )
        {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|_| ProbeError::Failed)?;
        tree.insert(relative, normalize(bytes, &spellings, &module_spellings));
    }
    Ok(tree)
}

/// Spellings of CMake's module directory (`CMAKE_ROOT`, always with forward
/// slashes): CMake files use it as is, `build.ninja` with native and
/// Ninja-escaped separators.
fn module_spellings(cmake_root: &str) -> Vec<String> {
    if cmake_root.is_empty() {
        return Vec::new();
    }
    spellings_of([cmake_root.to_owned(), cmake_root.replace('/', "\\")])
}

fn normalize(bytes: Vec<u8>, tree_spellings: &[String], module_spellings: &[String]) -> Vec<u8> {
    let Ok(mut text) = String::from_utf8(bytes.clone()) else {
        return bytes;
    };
    for spelling in tree_spellings {
        text = text.replace(spelling.as_str(), "<BUILD>");
    }
    text.lines()
        .filter(|line| {
            !module_spellings
                .iter()
                .any(|spelling| line.contains(spelling.as_str()))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

/// At most this many differing files are named in a rejection, so one
/// rejected probe reports every cause a CI log needs without unbounded text.
const MAX_REPORTED_DIFFERENCES: usize = 8;

/// The differing files between two generated trees (missing, changed with
/// their first differing line, unexpected), at most
/// [`MAX_REPORTED_DIFFERENCES`] of them plus a count of the rest.
fn differences(
    reference: &BTreeMap<String, Vec<u8>>,
    candidate: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut found = Vec::new();
    for (path, bytes) in reference {
        match candidate.get(path) {
            None => found.push(format!("{path} missing")),
            Some(other) if other != bytes => found.push(format!(
                "{path}{}",
                differing_lines(
                    &String::from_utf8_lossy(bytes),
                    &String::from_utf8_lossy(other)
                )
            )),
            Some(_) => {}
        }
    }
    found.extend(
        candidate
            .keys()
            .filter(|path| !reference.contains_key(*path))
            .map(|path| format!("{path} unexpected")),
    );
    if found.len() > MAX_REPORTED_DIFFERENCES {
        let rest = found.len() - MAX_REPORTED_DIFFERENCES;
        found.truncate(MAX_REPORTED_DIFFERENCES);
        found.push(format!("{rest} more"));
    }
    found
}

/// The first differing line of two files, and how many other line positions
/// differ, so one CI log shows whether a file has one cause or many.
fn differing_lines(reference: &str, candidate: &str) -> String {
    let reference: Vec<&str> = reference.lines().collect();
    let candidate: Vec<&str> = candidate.lines().collect();
    let mut differing = (0..reference.len().max(candidate.len()))
        .filter(|&index| reference.get(index) != candidate.get(index));
    let Some(first) = differing.next() else {
        // Same lines: only line endings or a final newline differ.
        return ": same lines, different line endings".to_owned();
    };
    let line = |lines: &[&str]| {
        lines
            .get(first)
            .map_or("<no line>".to_owned(), |line| format!("`{line}`"))
    };
    let mut text = format!(
        ": line {}: {} vs {}",
        first + 1,
        line(&reference),
        line(&candidate)
    );
    let rest = differing.count();
    if rest > 0 {
        text.push_str(&format!(" (+{rest} more differing lines)"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toolchain<'a>(cc: &'a str, identity: &'a str) -> Toolchain<'a> {
        Toolchain {
            cmake: "cmake",
            generator: "Ninja",
            cc,
            flags: "",
            launcher: "",
            compiler_identity: identity,
            compiler_target: "x86_64-pc-linux-gnu",
        }
    }

    #[test]
    fn seed_switch_disables_only_on_explicit_off_values() {
        assert!(enabled_by(None));
        assert!(enabled_by(Some("")));
        assert!(enabled_by(Some("1")));
        for off in ["0", "off", "OFF", " false ", "no"] {
            assert!(!enabled_by(Some(off)), "{off}");
        }
    }

    #[test]
    fn key_covers_toolchain_inputs_and_detection_environment() {
        let none = |_: &str| None;
        let base = key_text(
            &toolchain("cc", "gcc 14"),
            "cmake version 3.31.6",
            None,
            none,
        );
        assert_eq!(
            base,
            key_text(
                &toolchain("cc", "gcc 14"),
                "cmake version 3.31.6",
                None,
                none
            )
        );
        let variants = [
            key_text(
                &toolchain("cc", "gcc 15"),
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &toolchain("clang", "gcc 14"),
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &toolchain("cc", "gcc 14"),
                "cmake version 4.1.0",
                None,
                none,
            ),
            key_text(
                &Toolchain {
                    generator: "Unix Makefiles",
                    ..toolchain("cc", "gcc 14")
                },
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &Toolchain {
                    flags: "-m32",
                    ..toolchain("cc", "gcc 14")
                },
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &Toolchain {
                    launcher: "ccache",
                    ..toolchain("cc", "gcc 14")
                },
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &Toolchain {
                    compiler_target: "aarch64-linux-gnu",
                    ..toolchain("cc", "gcc 14")
                },
                "cmake version 3.31.6",
                None,
                none,
            ),
            key_text(
                &toolchain("cc", "gcc 14"),
                "cmake version 3.31.6",
                None,
                |name| (name == "INCLUDE").then(|| OsString::from("C:/sdk/include")),
            ),
            key_text(
                &toolchain("cc", "gcc 14"),
                "cmake version 3.31.6",
                None,
                |name| (name == "PATH").then(|| OsString::from("/opt/other/bin")),
            ),
        ];
        let mut distinct = BTreeSet::from([entry_name(&base)]);
        for variant in &variants {
            assert_ne!(&base, variant);
            distinct.insert(entry_name(variant));
        }
        assert_eq!(distinct.len(), variants.len() + 1);
        // An unrelated variable never splits the key.
        assert_eq!(
            base,
            key_text(
                &toolchain("cc", "gcc 14"),
                "cmake version 3.31.6",
                None,
                |name| (name == "HOME").then(|| OsString::from("/home/user")),
            )
        );
    }

    #[test]
    fn relative_compiler_paths_key_on_the_working_directory() {
        let none = |_: &str| None;
        let one = Path::new("/work/one");
        let two = Path::new("/work/two");
        let relative = toolchain("tools/cc", "gcc 14");
        assert_ne!(
            key_text(&relative, "v", Some(one), none),
            key_text(&relative, "v", Some(two), none)
        );
        let searched = toolchain("cc", "gcc 14");
        assert_eq!(
            key_text(&searched, "v", Some(one), none),
            key_text(&searched, "v", Some(two), none)
        );
    }

    #[test]
    fn markers_admit_only_the_exact_key() {
        let dir = std::env::temp_dir().join(format!(
            "llg-seed-markers-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cache.txt"), "CMAKE_AR:FILEPATH=/usr/bin/ar\n").unwrap();
        assert!(load(&dir, "key\nmore").is_none(), "no marker yet");
        write_marker(&dir, "ready", "3.31.6\nkey\nmore");
        let seed = load(&dir, "key\nmore").expect("exact key loads");
        assert_eq!(seed.version_dir, "3.31.6");
        assert_eq!(seed.cache_args, ["-DCMAKE_AR:FILEPATH=/usr/bin/ar"]);
        assert!(load(&dir, "key").is_none(), "a prefix of the key is stale");
        assert!(load(&dir, "key\nmore\n").is_none());
        assert!(!is_rejected(&dir, "key\nmore"));
        seed.reject("seeded configure failed");
        assert!(is_rejected(&dir, "key\nmore"));
        assert!(!is_rejected(&dir, "other"));
        write_marker(&dir, "ready", "../x\nkey\nmore");
        assert!(load(&dir, "key\nmore").is_none(), "no escaping directory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_parser_keeps_typed_entries_only() {
        let parsed = parse_cache(
            "# comment\n//help\nCMAKE_AR:FILEPATH=/usr/bin/ar\n\"QUOTED:NAME\":STRING=x\nCMAKE_EXECUTABLE_FORMAT:INTERNAL=ELF\nnot an entry\nEMPTY:STRING=\n",
        );
        assert_eq!(
            parsed.keys().collect::<Vec<_>>(),
            ["CMAKE_AR", "CMAKE_EXECUTABLE_FORMAT", "EMPTY"]
        );
        assert_eq!(parsed["CMAKE_AR"], "CMAKE_AR:FILEPATH=/usr/bin/ar");
    }

    #[test]
    fn tree_comparison_ignores_tree_paths_and_module_lists_only() {
        let spellings = vec!["/b/full".to_owned()];
        let other = vec!["/b/check".to_owned()];
        let modules = vec!["/usr/share/cmake".to_owned()];
        let left = normalize(
            b"cmd /b/full/x.o\ndeps /usr/share/cmake/Modules/A.cmake\n".to_vec(),
            &spellings,
            &modules,
        );
        let right = normalize(
            b"cmd /b/check/x.o\ndeps /usr/share/cmake/Modules/B.cmake\n".to_vec(),
            &other,
            &modules,
        );
        assert_eq!(left, right);
        let flags = normalize(b"cmd -O2 /b/check/x.o\n".to_vec(), &other, &modules);
        let reference = BTreeMap::from([("build.ninja".to_owned(), left)]);
        let candidate = BTreeMap::from([("build.ninja".to_owned(), flags)]);
        let found = differences(&reference, &candidate);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("build.ninja"), "{found:?}");
        assert_eq!(
            differences(&reference, &BTreeMap::new()),
            ["build.ninja missing"]
        );
        assert!(differences(&reference, &reference).is_empty());
    }

    #[test]
    fn rejections_name_every_difference_up_to_a_bound() {
        let reference: BTreeMap<String, Vec<u8>> = (0..12)
            .map(|index| (format!("f{index:02}"), b"a".to_vec()))
            .collect();
        let mut candidate = reference.clone();
        candidate.remove("f00");
        candidate.insert("f01".to_owned(), b"b".to_vec());
        candidate.insert("z".to_owned(), Vec::new());
        assert_eq!(
            differences(&reference, &candidate),
            ["f00 missing", "f01: line 1: `a` vs `b`", "z unexpected"]
        );
        let all_missing = differences(&reference, &BTreeMap::new());
        assert_eq!(all_missing.len(), MAX_REPORTED_DIFFERENCES + 1);
        assert_eq!(all_missing.last().unwrap(), "4 more");
    }

    #[test]
    fn ninja_spellings_cover_every_escape_combination() {
        assert_eq!(
            ninja_spellings(r"C:\Program Files\x$y"),
            [
                r"C:\Program$ Files\x$$y".to_owned(),
                r"C$:\Program$ Files\x$$y".to_owned(),
                r"C$:\Program Files\x$$y".to_owned(),
                r"C:\Program Files\x$$y".to_owned(),
            ]
        );
        // POSIX paths without special characters gain no extra spelling.
        assert_eq!(
            path_spellings(&[Path::new("/work/probe/full")]),
            ["/work/probe/full"]
        );
    }

    /// Windows probe trees as CMake's Ninja generator wrote them in CI
    /// (Windows x64 and arm64 runs of 5576b057): `cmake_ninja_workdir` and
    /// build edges spell the tree with `$:`, command lines do not, and the
    /// re-run dependency lines name CMake's modules with `$:` and `$ `. The
    /// clean (`full`) and seeded (`check`) trees must compare equal, while a
    /// real difference is still reported.
    #[test]
    fn windows_ninja_escaped_paths_are_normalized_like_plain_ones() {
        let entry = r"D:\a\lapligence\lapligence\target\llg-runtime-cache\cmake-toolchain\36c593e66a04e03e\probe";
        let escaped = entry.replacen(':', "$:", 1);
        let modules = module_spellings("C:/Program Files/CMake/share/cmake-4.1");
        let modules_dir = r"C$:\Program$ Files\CMake\share\cmake-4.1\Modules";
        let ninja = |tree: &str, detection: &str| {
            format!(
                "cmake_ninja_workdir = {escaped}\\{tree}\\\n\
                 build CMakeFiles\\probe_lib.dir\\probe_lib.c.obj: C_COMPILER__probe_lib_unscanned_Debug {escaped}\\src\\probe_lib.c\n\
                 \x20 COMMAND = C:\\WINDOWS\\system32\\cmd.exe /C \"cd /D {entry}\\{tree} && \"C:\\Program Files\\CMake\\bin\\cmake.exe\" --regenerate-during-build -S{entry}\\src -B{entry}\\{tree}\"\n\
                 build build.ninja: RERUN_CMAKE | {modules_dir}\\CMakeCInformation.cmake {detection}{escaped}\\src\\CMakeLists.txt\n\
                 build {modules_dir}\\CMakeCInformation.cmake {detection}{escaped}\\src\\CMakeLists.txt: phony\n"
            )
        };
        let clean = ninja(
            "full",
            &format!("{modules_dir}\\CMakeDetermineCCompiler.cmake "),
        );
        let seeded = ninja("check", "");
        // The line exactly as the x64 rejection quoted it.
        assert!(clean.starts_with(
            r"cmake_ninja_workdir = D$:\a\lapligence\lapligence\target\llg-runtime-cache\cmake-toolchain\36c593e66a04e03e\probe\full\"
        ));
        let normalized = |text: &str, tree: &str| {
            let spellings = path_spellings(&[Path::new(&format!("{entry}\\{tree}"))]);
            normalize(text.as_bytes().to_vec(), &spellings, &modules)
        };
        let reference = normalized(&clean, "full");
        let candidate = normalized(&seeded, "check");
        assert_eq!(
            String::from_utf8_lossy(&reference),
            String::from_utf8_lossy(&candidate)
        );
        assert!(
            String::from_utf8_lossy(&reference).starts_with("cmake_ninja_workdir = <BUILD>\\\n")
        );

        let reference = BTreeMap::from([("build.ninja".to_owned(), reference)]);
        let changed = normalized(&seeded.replace("Debug", "Release"), "check");
        let found = differences(
            &reference,
            &BTreeMap::from([("build.ninja".to_owned(), changed)]),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("build.ninja: line 2: "), "{found:?}");

        // The install prefixes the two CI trees recorded still differ; the
        // explicit prefix in `toolchain_args` is what makes them agree.
        let install = |prefix: &str| {
            BTreeMap::from([(
                "cmake_install.cmake".to_owned(),
                format!("if(NOT DEFINED CMAKE_INSTALL_PREFIX)\n  set(CMAKE_INSTALL_PREFIX \"{prefix}/llg_toolchain_probe\")\nendif()\n").into_bytes(),
            )])
        };
        assert_eq!(
            differences(&install("C:/Program Files (x86)"), &install("C:/Program Files")),
            ["cmake_install.cmake: line 2: `  set(CMAKE_INSTALL_PREFIX \"C:/Program Files (x86)/llg_toolchain_probe\")` vs `  set(CMAKE_INSTALL_PREFIX \"C:/Program Files/llg_toolchain_probe\")`"]
        );
    }

    #[test]
    fn configures_never_take_the_platform_default_install_prefix() {
        let args = toolchain_args("Ninja", "", "cl", "");
        assert!(args.contains(&"-DCMAKE_INSTALL_PREFIX:PATH=".to_owned()));
        assert!(args.contains(&"-DCMAKE_FIND_NO_INSTALL_PREFIX:BOOL=ON".to_owned()));
    }

    #[test]
    fn differing_files_name_their_first_line_and_the_rest_count() {
        assert_eq!(
            differing_lines("a\nb\nc", "a\nx\ny\nz"),
            ": line 2: `b` vs `x` (+2 more differing lines)"
        );
        assert_eq!(differing_lines("a\n", "a\nb"), ": line 2: <no line> vs `b`");
        assert_eq!(
            differing_lines("a\r\n", "a\n"),
            ": same lines, different line endings"
        );
    }

    /// MSVC-like compilers leave `CMakeFiles/ShowIncludes/` behind only in a
    /// tree that ran detection; a seeded tree without it is still equivalent.
    #[test]
    fn detection_scratch_is_not_part_of_the_compared_tree() {
        let dir = std::env::temp_dir().join(format!(
            "llg-seed-scratch-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let write = |relative: &str, text: &str| {
            let path = dir.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("build.ninja", "rule cc\n");
        write("CMakeFiles/ShowIncludes/foo.h", "\n");
        write("CMakeFiles/ShowIncludes/main.c", "#include \"foo.h\"\n");
        write("CMakeFiles/CMakeScratch/TryCompile-1/a.c", "");
        write("CMakeFiles/3.31.6/CMakeCCompiler.cmake", "set(X 1)\n");
        write("CMakeCache.txt", "X:STRING=1\n");
        let tree = generated_tree(&dir, "3.31.6", "").unwrap();
        assert_eq!(tree.keys().collect::<Vec<_>>(), ["build.ninja"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
