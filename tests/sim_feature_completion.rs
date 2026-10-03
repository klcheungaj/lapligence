//! Feature completion acceptance: explicit task modules and independent fixture oracles.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[path = "sim_feature_completion/fnd_003.rs"]
mod fnd_003;

#[path = "sim_feature_completion/rtl_001.rs"]
mod rtl_001;

#[path = "sim_feature_completion/rtl_002.rs"]
mod rtl_002;

#[path = "sim_feature_completion/rtl_002b.rs"]
mod rtl_002b;

#[path = "sim_feature_completion/rtl_003.rs"]
mod rtl_003;

#[path = "sim_feature_completion/rtl_004.rs"]
mod rtl_004;

#[path = "sim_feature_completion/rtl_005.rs"]
mod rtl_005;

#[path = "sim_feature_completion/rtl_006.rs"]
mod rtl_006;

#[test]
fn component_fixture_integrity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = sim_harness::run_command(
        std::process::Command::new("python3")
            .arg(root.join("scripts/check_sim_fixture_integrity.py"))
            .arg("--root")
            .arg(root),
        std::time::Duration::from_secs(60),
    )
    .expect("fixture checker requires Python 3");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
