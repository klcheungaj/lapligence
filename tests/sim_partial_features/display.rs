use super::{reject_case, run_case};

#[test]
fn display_formatting_is_typed_and_exact() {
    run_case(
        "display_formatting",
        "d=[+00175] h=[0x00af] b=[10101111] o=[0257] r=[-2.50] s=[      sv] m=tb %\ntailstrobe sv -2.5 175\n",
    );
}

#[test]
fn monitor_re_evaluates_real_and_string_arguments() {
    run_case(
        "display_monitor_typed",
        "monitor=a 1.0 tb\nmonitor=a 2.0 tb\n",
    );
}

#[test]
fn display_rejects_real_for_integral_conversion() {
    reject_case(
        "display_formatting_invalid",
        "requires a packed argument",
    );
}

#[test]
fn deferred_display_rejects_automatic_capture() {
    reject_case(
        "display_deferred_automatic_invalid",
        "cannot escape a function or task activation",
    );
}
