use crate::sim_cli;

/// `%m` names the enclosing named block, task or generate block as written
/// (SV 21.2.1.6).
#[test]
fn instance_paths_events_and_external_disable_stay_independent() {
    sim_cli::run_case(
        "instance_sharing",
        "identities",
        "start tb.u0.live\nstart tb.u1.live\nstart tb.u2.live\nstart tb.u3.live\ngenerate tb.g[0]\ngenerate tb.g[1]\ngenerate tb.g[2]\ngenerate tb.g[3]\ntask tb.u0.mark seen=1\nexit tb.u1\ntask tb.u2.mark seen=1\ntask tb.u3.mark seen=1\nseen 1 0 1 1 bits=1010\n",
        "", &[],
    );
}
