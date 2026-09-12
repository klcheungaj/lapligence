use super::{reject_case, run_case};

#[test]
fn display_formatting_is_typed_and_exact() {
    run_case(
        "display_formatting",
        "d=[   175] h=[0000af] b=[10101111] o=[00000257] r=[-2.50] s=[      sv] m=tb %\ntailstrobe sv -2.5 175\n",
    );
}

#[test]
fn monitor_re_evaluates_real_and_string_arguments() {
    run_case(
        "display_monitor_typed",
        "monitor=a 1.0 tb\nmonitor=a 2.0 tb\nmonitor=b 2.0 tb\n",
    );
}

#[test]
fn display_rejects_real_for_integral_conversion() {
    reject_case(
        "display_formatting_invalid",
        "requires a packed or string argument",
    );
}

#[test]
fn display_extended_conversions_and_strobe_order() {
    run_case(
        "display_extended",
        "hex=x bin=1x0z char=* strength=St1 StX St0 HiZ\nupper=2a\npattern=8'd42 4'b1x0z\nraw2=*\0\0\0 raw4=\r\0\0\0\x05\0\0\0\ntime=0 library=work.tb\nfirst=43\nsecond=43\n",
    );
}

#[test]
fn display_arguments_are_evaluated_in_source_order() {
    run_case("display_evaluation_order", "args=1,2 calls=2\n");
}

#[test]
fn deferred_display_rejects_automatic_capture() {
    reject_case("display_deferred_automatic_invalid", "cannot be traced");
}
