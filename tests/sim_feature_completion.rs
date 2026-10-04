//! Feature completion acceptance: explicit task modules and independent fixture oracles.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[path = "sim_feature_completion/fnd_003.rs"]
mod fnd_003;

#[path = "sim_feature_completion/sim_003.rs"]
mod sim_003;

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

#[path = "sim_feature_completion/rtl_007.rs"]
mod rtl_007;

#[path = "sim_feature_completion/rtl_007b.rs"]
mod rtl_007b;

#[path = "sim_feature_completion/rtl_008.rs"]
mod rtl_008;

#[path = "sim_feature_completion/rtl_009.rs"]
mod rtl_009;

#[path = "sim_feature_completion/rtl_010.rs"]
mod rtl_010;

#[path = "sim_feature_completion/rtl_011.rs"]
mod rtl_011;

#[path = "sim_feature_completion/rtl_012.rs"]
mod rtl_012;

#[path = "sim_feature_completion/rtl_013.rs"]
mod rtl_013;

#[path = "sim_feature_completion/rtl_014.rs"]
mod rtl_014;

#[path = "sim_feature_completion/rtl_015.rs"]
mod rtl_015;

#[path = "sim_feature_completion/rtl_016.rs"]
mod rtl_016;

#[path = "sim_feature_completion/rtl_017.rs"]
mod rtl_017;

#[path = "sim_feature_completion/rtl_018.rs"]
mod rtl_018;

#[path = "sim_feature_completion/rtl_019.rs"]
mod rtl_019;

#[path = "sim_feature_completion/rtl_020.rs"]
mod rtl_020;

#[path = "sim_feature_completion/rtl_099.rs"]
mod rtl_099;

#[path = "sim_feature_completion/rtl_102.rs"]
mod rtl_102;

#[path = "sim_feature_completion/sim_001.rs"]
mod sim_001;

#[path = "sim_feature_completion/sim_002.rs"]
mod sim_002;

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
