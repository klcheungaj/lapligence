//! ADV-030: protected (encrypted) source is unsupported by `llg` (user
//! decision 2026-10-27). Every protected envelope, wherever it sits, stops
//! the run before C generation with one `unsupported:` line at its opening
//! directive (IEEE 1800-2009 34.2: "A decryption envelope begins in the
//! source text when a begin_protected pragma expression is encountered");
//! the encoded text is never skipped silently. `pragma protect` keywords
//! outside any envelope stay ordinary pragmas. Expected lines are written out
//! literally; the fixtures carry synthetic base64 payloads, no real keys.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_030";
const BOTH: &[&str] = &["v2001", "sv2009"];

const BEGIN_PROTECTED: &str = "protected envelope `` `pragma protect begin_protected``";

/// The `<line>:<col>: unsupported: <construct> (protected source envelopes)
/// is not supported by llg` tail of one expected `error:` line.
fn finding(line: u32, col: u32, construct: &str) -> String {
    format!(
        "{line}:{col}: unsupported: {construct} (protected source envelopes) is not supported by llg"
    )
}

#[test]
fn envelope_inside_a_module_body_is_rejected() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "envelope_in_body.sv",
        &[finding(3, 1, BEGIN_PROTECTED)],
        &[],
    );
}

/// The envelope holds a whole module; the instantiation's unknown-module
/// error is the frontend's own and follows the envelope's line.
#[test]
fn envelope_around_a_whole_module_is_rejected() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "whole_module.sv",
        &[finding(12, 1, BEGIN_PROTECTED)],
        &[],
    );
}

/// Plaintext header, protected body and `endmodule` (the usual vendor
/// layout).
#[test]
fn envelope_after_a_plaintext_header_is_rejected() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "plaintext_header.sv",
        &[finding(13, 1, BEGIN_PROTECTED)],
        &[],
    );
}

/// IEEE 1800-2009 34.2: decryption envelopes nest; each one is reported.
#[test]
fn nested_envelopes_are_each_rejected() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "nested.sv",
        &[
            finding(3, 1, BEGIN_PROTECTED),
            finding(11, 1, BEGIN_PROTECTED),
        ],
        &[],
    );
}

#[test]
fn truncated_envelope_is_rejected_as_unterminated() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "truncated.sv",
        &[finding(
            13,
            1,
            "unterminated protected envelope `` `pragma protect begin_protected``",
        )],
        &[],
    );
}

/// An encryption envelope (34.5.1, `begin`/`end`) asks an encrypting tool to
/// protect plaintext; `llg` is not one and does not compile it either.
#[test]
fn encryption_envelope_is_rejected() {
    sim_cli::reject_case_with_error_lines(
        SUITE,
        "encryption_envelope.sv",
        &[finding(
            2,
            1,
            "encryption envelope `` `pragma protect begin``",
        )],
        &[],
    );
}

/// The Verilog-XL directive forms in both editions.
#[test]
fn legacy_protected_and_protect_directives_are_rejected() {
    for edition in BOTH {
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "legacy_protected.v",
            &[finding(5, 1, "protected envelope `` `protected``")],
            &["--edition", edition],
        );
        sim_cli::reject_case_with_error_lines(
            SUITE,
            "legacy_encryption.v",
            &[finding(4, 1, "encryption envelope `` `protect``")],
            &["--edition", edition],
        );
    }
}

/// Nearest legal form: protect keywords that open no envelope (34.5) are
/// pragmas without simulation effect, and the design runs.
#[test]
fn protect_keywords_outside_an_envelope_simulate() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_030/no_envelope.out");
    sim_cli::run_case_with_args(
        SUITE,
        "no_envelope.sv",
        expected,
        "llg: $finish at time 1000 at tb:13:5\n",
        &[],
        &[],
    );
}
