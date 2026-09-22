//! Regression coverage for shared integration-test lifecycle helpers.

#[path = "support/sim.rs"]
mod sim_harness;

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
