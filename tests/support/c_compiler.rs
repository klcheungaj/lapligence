//! Host C compiler identification for native compile gates.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// True when `compiler` is GNU GCC rather than a Clang driver installed under
/// the same name (macOS `gcc` is Apple Clang). GCC-only diagnostics such as
/// `-Werror=jump-misses-init` must be requested only from real GCC: Clang
/// reports them as unknown warning options. Probed once per name per process.
pub(crate) fn is_gnu_gcc(compiler: &str) -> bool {
    static PROBED: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let probed = PROBED.get_or_init(Default::default);
    if let Some(&known) = probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(compiler)
    {
        return known;
    }
    let gnu = probe_gnu_gcc(compiler);
    probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(compiler.to_owned(), gnu);
    gnu
}

fn probe_gnu_gcc(compiler: &str) -> bool {
    let Ok(output) = Command::new(compiler)
        .args(["-dM", "-E", "-x", "c", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let macros = String::from_utf8_lossy(&output.stdout);
    let defined = |name: &str| {
        macros
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(name))
    };
    defined("__GNUC__") && !defined("__clang__")
}
