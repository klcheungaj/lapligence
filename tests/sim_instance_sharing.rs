use crate::sim_cli;

#[test]
fn instance_paths_events_and_external_disable_stay_independent() {
    sim_cli::run_case(
        "instance_sharing",
        "identities",
        "start tb.u0\nstart tb.u1\nstart tb.u2\nstart tb.u3\ngenerate tb.__llg_ident_e_675B305D\ngenerate tb.__llg_ident_e_675B315D\ngenerate tb.__llg_ident_e_675B325D\ngenerate tb.__llg_ident_e_675B335D\ntask tb.u0 seen=1\nexit tb.u1\ntask tb.u2 seen=1\ntask tb.u3 seen=1\nseen 1 0 1 1 bits=1010\n",
        "", &[],
    );
}
