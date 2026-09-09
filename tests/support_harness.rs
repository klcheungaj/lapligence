//! Regression coverage for shared integration-test lifecycle helpers.

#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn cwd_lock_recovers_after_an_action_panics() {
    let original = std::env::current_dir().expect("current directory");
    let panic = std::panic::catch_unwind(|| {
        let _: Result<(), String> = sim_harness::with_frontend_temp_cwd("cwd-panic", |_| {
            panic!("intentional harness unwind")
        });
    });
    assert!(panic.is_err());
    assert_eq!(
        std::env::current_dir().expect("current directory after unwind"),
        original
    );

    sim_harness::with_frontend_temp_cwd("cwd-recovery", |dir| {
        assert_eq!(
            std::env::current_dir().expect("temporary current directory"),
            dir
        );
        Ok(())
    })
    .expect("poisoned CWD lock remains recoverable");
    assert_eq!(
        std::env::current_dir().expect("current directory after recovery"),
        original
    );
}
