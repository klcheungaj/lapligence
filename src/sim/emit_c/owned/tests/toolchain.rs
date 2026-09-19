//! Fresh generated-model workspaces and bounded native execution.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub(super) struct Directory(PathBuf);

impl Directory {
    pub(super) fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("llg-owned-{label}-{}-{serial}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned-model workspace: {error}"),
            }
        }
        panic!("could not allocate a fresh owned-model workspace");
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn execute(binary: &Path) -> Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start emitted model");
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .expect("collect emitted-model output")
            }
            Ok(None) if started.elapsed() < Duration::from_secs(60) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            state => {
                let _ = child.kill();
                let output = child.wait_with_output().expect("reap failed emitted model");
                panic!(
                    "emitted model timed out or failed to wait ({state:?}): {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}
