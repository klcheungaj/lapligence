//! Configuration.

use super::*;

/// `settings_to_lint_config` maps the documented settings shape: a disabled
/// rule, a severity override, and untouched defaults for unmentioned rules.
#[test]
fn settings_to_lint_config_maps_rule_overrides() {
    let settings = settings_obj(vec![(
        "lint",
        settings_obj(vec![(
            "rules",
            settings_obj(vec![
                (
                    "unused-signal",
                    settings_obj(vec![("enabled", LSPAny::Bool(false))]),
                ),
                (
                    "width-mismatch",
                    settings_obj(vec![("severity", LSPAny::String("error".to_owned()))]),
                ),
            ]),
        )]),
    )]);

    let cfg = settings_to_lint_config(&settings);
    assert!(
        !cfg.is_enabled("unused-signal"),
        "unused-signal should be disabled"
    );
    assert_eq!(cfg.severity("width-mismatch"), Some(LintSeverity::Error));
    assert!(
        cfg.is_enabled("incomplete-case"),
        "unmentioned rule should stay enabled"
    );
    assert_eq!(cfg.severity("incomplete-case"), None);
}

/// A global `"enabled": false` disables every known rule; a per-rule entry
/// can re-enable one.
#[test]
fn settings_to_lint_config_global_enabled_false_disables_all() {
    let settings = settings_obj(vec![(
        "lint",
        settings_obj(vec![("enabled", LSPAny::Bool(false))]),
    )]);
    let cfg = settings_to_lint_config(&settings);
    assert!(!cfg.is_enabled("unused-signal"));
    assert!(!cfg.is_enabled("naming-style"));

    let with_override = settings_obj(vec![(
        "lint",
        settings_obj(vec![
            ("enabled", LSPAny::Bool(false)),
            (
                "rules",
                settings_obj(vec![(
                    "casez-misuse",
                    settings_obj(vec![("enabled", LSPAny::Bool(true))]),
                )]),
            ),
        ]),
    )]);
    let cfg = settings_to_lint_config(&with_override);
    assert!(!cfg.is_enabled("unused-signal"));
    assert!(cfg.is_enabled("casez-misuse"));
}

/// A bare `{"rules": ...}` payload (no `lint` wrapper) is accepted.
#[test]
fn settings_to_lint_config_accepts_bare_rules_object() {
    let settings = settings_obj(vec![(
        "rules",
        settings_obj(vec![(
            "casez-misuse",
            settings_obj(vec![("enabled", LSPAny::Bool(false))]),
        )]),
    )]);
    let cfg = settings_to_lint_config(&settings);
    assert!(!cfg.is_enabled("casez-misuse"));
    assert!(cfg.is_enabled("unused-signal"));
}
