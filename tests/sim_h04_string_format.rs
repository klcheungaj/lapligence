//! Regression coverage for string-producing system formatting tasks/functions.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn string_formatting_is_typed_and_retains_results() {
    sim_cli::run_case(
        "string_format",
        "entrypoints",
        "out=<d=17 h=af b=1010 o=017 c=* s=ok r=1.5>\nb=1010\no=017\nh=af\nsformat=<prefix:x:3>\ndynamic=<v=7>\ndynamic-f=<v=8>\nwide=00000000000000000000000000004142\nwide-real=00000000000000000000000000312e35\nnarrow=45\npacked=4243\nfunction=fn=4\nnested=[9]\nempty=<>\ncalls=2 side=1/2\nheld=<hold>\nsegments=left=1 right=0f\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
    );
}
