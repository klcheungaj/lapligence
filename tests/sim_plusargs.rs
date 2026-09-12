//! End-to-end IEEE 1800-2009 §21.6 command-line plusarg tests.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn plusargs_match_and_convert_with_optimizer_parity() {
    sim_cli::run_case_with_runtime_args(
        "plusargs",
        "basic",
        concat!(
            "hit=1 repeat=17 dec=-42\n",
            "hex=0000000000000000000000001234abcd bits=10xz01 oct=755 state=10000001\n",
            "real=1.25 2.50e+00 3.750 short=4.50\n",
            "text=<hello_world> empty=<> packed=0000000000616263 collision=41 percent=12\n",
            "direct=33 condition=9 seen=1\n",
        ),
        "",
        &[
            "+mode=run",
            "+repeat=17",
            "+repeat=23",
            "+dec=-42",
            "+hex=1234abcd",
            "+bits=10xz01",
            "+oct=755",
            "+state=1xz00001",
            "+fixed=1.25",
            "+exponent=2.5",
            "+general=3.75",
            "+short=4.5",
            "+text=hello_world",
            "+empty=",
            "+packed=abc",
            "+collision_extra=99",
            "+collision=41",
            "+literal%=12",
            "+direct=33",
            "+condition=9",
            "--not-an-llg-option",
        ],
    );
}

#[test]
fn plusargs_report_no_match_and_malformed_value_distinctly() {
    sim_cli::run_case_with_runtime_args(
        "plusargs",
        "retention",
        concat!(
            "no_match=10100101 result=0 malformed=xx result=1 zero=0 result=1\n",
            "real=0.0 result=1 string=<keep> result=0\n",
        ),
        "",
        &[
            "+bad=not-a-number",
            "+real=not-a-real",
            "+zero=",
            "+other=value",
        ],
    );
}

#[test]
fn malformed_plusarg_formats_are_rejected() {
    sim_cli::reject_case_with_runtime_args("plusargs", "malformed", "requires one conversion", &[]);
}
