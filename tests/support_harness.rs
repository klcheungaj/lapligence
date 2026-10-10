//! Regression coverage for shared integration-test lifecycle helpers.

use crate::sim_harness;

#[test]
fn test_build_directory_configuration() {
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    let directory = sim_harness::TempDir::new("build root with spaces").expect("test root");
    let custom_root = directory.path().join("new root");
    let sentinel = directory.path().join("keep.txt");
    std::fs::write(&sentinel, "preserve parent contents").expect("sentinel");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let default_root = std::env::temp_dir();
    for (configured, expected, error) in [
        (Some(custom_root.as_path()), custom_root.as_path(), ""),
        (Some(Path::new(".")), workspace, ""),
        (None, default_root.as_path(), ""),
        (Some(Path::new("")), workspace, "must not be empty"),
        (
            Some(sentinel.as_path()),
            workspace,
            "create test build root",
        ),
    ] {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args(["--exact", &child_test_name(), "--nocapture"])
            .current_dir(directory.path())
            .env("LLG_TEST_BUILD_PROBE_ROOT", expected)
            .env("LLG_TEST_BUILD_PROBE_ERROR", error)
            .env_remove("LLG_TEST_BUILD_DIR");
        if let Some(path) = configured {
            command.env("LLG_TEST_BUILD_DIR", path);
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(30))
            .expect("directory configuration probe");
        assert!(output.status.success(), "{configured:?}: {output:?}");
    }
    assert_eq!(
        std::fs::read_to_string(sentinel).unwrap(),
        "preserve parent contents"
    );
    assert!(custom_root.is_dir(), "configured root must survive cleanup");
    assert_eq!(std::fs::read_dir(custom_root).unwrap().count(), 0);
}

/// Harness name of `test_build_directory_child`: the module path without the
/// crate, because this suite is a module of a grouped test binary.
fn child_test_name() -> String {
    match module_path!().split_once("::") {
        Some((_crate, module)) => format!("{module}::test_build_directory_child"),
        None => "test_build_directory_child".to_owned(),
    }
}

#[test]
fn test_build_directory_child() {
    let Some(expected) = std::env::var_os("LLG_TEST_BUILD_PROBE_ROOT") else {
        return;
    };
    let expected = std::path::PathBuf::from(expected);
    // TempDir resolves its path (macOS temp_dir() is a /var symlink into
    // /private/var; Windows may spell it with 8.3 short names), so compare
    // against the resolved root.
    let expected = llg::ffi::platform::canonicalize(&expected).unwrap_or(expected);
    let error = std::env::var("LLG_TEST_BUILD_PROBE_ERROR").expect("probe error setting");
    if !error.is_empty() {
        let actual = sim_harness::TempDir::new("invalid-root")
            .err()
            .expect("invalid root must fail");
        assert!(actual.contains(&error), "{actual}");
        return;
    }

    sim_harness::with_temp_cwd("build-root-nested", |_| {
        let directories: Vec<_> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| sim_harness::TempDir::new("parallel-build")))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap().unwrap())
                .collect()
        });
        let paths: std::collections::HashSet<_> = directories
            .iter()
            .map(|directory| directory.path().to_path_buf())
            .collect();
        assert_eq!(
            paths.len(),
            8,
            "parallel builds must have distinct directories"
        );
        for path in &paths {
            assert_eq!(path.parent(), Some(expected.as_path()));
            std::fs::write(path.join("model.c"), "probe").expect("writable build directory");
        }
        drop(directories);
        assert!(paths.iter().all(|path| !path.exists()));
        Ok(())
    })
    .expect("nested build directories");
}

#[cfg(unix)]
#[test]
fn command_timeout_stops_descendants_holding_output_pipes() {
    use std::process::Command;
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let result = sim_harness::run_command(
        Command::new("sh").args(["-c", "sleep 30 & wait"]),
        Duration::from_millis(100),
    );
    assert!(result.unwrap_err().contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[cfg(unix)]
#[test]
fn command_timeout_reports_the_output_printed_before_the_deadline() {
    use std::process::Command;
    use std::time::Duration;

    let error = sim_harness::run_command(
        Command::new("sh").args(["-c", "echo started-step; echo failing-step >&2; sleep 30"]),
        Duration::from_millis(500),
    )
    .unwrap_err();
    assert!(error.starts_with("timed out"), "{error}");
    let stdout = error
        .find("started-step")
        .expect("stdout tail in the error");
    let stderr = error
        .find("failing-step")
        .expect("stderr tail in the error");
    assert!(stdout < stderr, "{error}");
}

#[test]
fn cwd_lock_recovers_after_an_action_panics() {
    let original =
        sim_harness::with_cwd_lock(|| std::env::current_dir().expect("current directory"));
    let panic = std::panic::catch_unwind(|| {
        let _: Result<(), String> = sim_harness::with_frontend_temp_cwd("cwd-panic", |_| {
            panic!("intentional harness unwind")
        });
    });
    assert!(panic.is_err());
    sim_harness::with_cwd_lock(|| {
        assert_eq!(
            std::env::current_dir().expect("current directory after unwind"),
            original
        );
    });

    sim_harness::with_frontend_temp_cwd("cwd-recovery", |dir| {
        assert_eq!(
            std::env::current_dir().expect("temporary current directory"),
            dir
        );
        Ok(())
    })
    .expect("poisoned CWD lock remains recoverable");
    sim_harness::with_cwd_lock(|| {
        assert_eq!(
            std::env::current_dir().expect("current directory after recovery"),
            original
        );
    });
}

#[test]
fn nested_cwd_guards_restore_each_scope() {
    let original =
        sim_harness::with_cwd_lock(|| std::env::current_dir().expect("current directory"));
    sim_harness::with_temp_cwd("cwd-nested", |outer| {
        assert_eq!(
            std::env::current_dir().expect("outer current directory"),
            outer
        );
        let inner = sim_harness::TempDir::new("cwd-inner")?;
        sim_harness::with_cwd(inner.path(), || {
            assert_eq!(
                std::env::current_dir().expect("inner current directory"),
                inner.path()
            );
            Ok(())
        })?;
        assert_eq!(
            std::env::current_dir().expect("restored outer current directory"),
            outer
        );
        Ok(())
    })
    .expect("nested CWD guards should complete");
    sim_harness::with_cwd_lock(|| {
        assert_eq!(
            std::env::current_dir().expect("current directory after nested guards"),
            original
        );
    });
}

/// Windows console and text-mode file output keep the native CRLF newline;
/// comparisons normalize exactly the CRLF pairs and nothing else.
#[test]
fn crlf_normalization_rewrites_only_crlf_pairs() {
    assert_eq!(
        llg::ffi::platform::crlf_to_lf(b"a\r\nb\r\n\r\nc\rd\n\r".to_vec()),
        b"a\nb\n\nc\rd\n\r"
    );
    assert_eq!(
        llg::ffi::platform::crlf_to_lf(b"plain\n".to_vec()),
        b"plain\n"
    );
    let host = sim_harness::host_text_to_lf(b"x\r\n".to_vec());
    let expected: &[u8] = if cfg!(windows) { b"x\n" } else { b"x\r\n" };
    assert_eq!(host, expected);
}
