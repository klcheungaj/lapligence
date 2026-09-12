use super::sim_cli;

fn normalized(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn system_task_function_and_empty_forms_run_once_in_both_optimizer_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "system tests require CMake"
    );

    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "partial_features",
            "system_host_command",
            optimized,
            &[],
            &[("LLG_ALLOW_SYSTEM", "1")],
            &[],
        );
        let label = format!("system_host_command, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: {}",
            normalized(&output.stderr)
        );
        let stdout = normalized(&output.stdout);
        let mut lines = stdout.lines();
        assert_eq!(lines.next(), Some("llg_system_task"), "{label}");
        assert_eq!(lines.next(), Some("llg_system_function"), "{label}");
        let fields = lines
            .next()
            .unwrap_or_else(|| panic!("{label}: missing status line"))
            .split_whitespace()
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 4, "{label}: {fields:?}");
        assert_eq!(fields[0], "status=0", "{label}: {fields:?}");
        assert_eq!(fields[1], "calls=1", "{label}: {fields:?}");
        let empty_status = fields[2]
            .strip_prefix("empty=")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_else(|| panic!("{label}: {fields:?}"));
        let _empty_string_status = fields[3]
            .strip_prefix("empty_string=")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_else(|| panic!("{label}: {fields:?}"));
        // The explicit echo command above proves a command processor exists;
        // the omitted form therefore returns its nonzero system(NULL) query
        // status. The explicit empty command's status is host-platform
        // behavior, but parsing it above proves the function result is wired.
        assert_ne!(empty_status, 0, "{label}: {fields:?}");
        assert!(
            lines.next().is_none(),
            "{label}: unexpected trailing output"
        );
        assert!(
            output.stderr.is_empty(),
            "{label}: {}",
            normalized(&output.stderr)
        );
    }
}

#[test]
fn system_is_denied_without_explicit_child_permission() {
    assert!(
        llg::sim::build::cmake_available(),
        "system tests require CMake"
    );

    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "partial_features",
            "system_host_command",
            optimized,
            &[],
            &[],
            &["LLG_ALLOW_SYSTEM"],
        );
        let label = format!("system_host_command denied, optimized={optimized}");
        assert_eq!(
            output.status.code(),
            Some(1),
            "{label}: {}",
            normalized(&output.stderr)
        );
        let stdout = normalized(&output.stdout);
        assert!(!stdout.contains("llg_system_task"), "{label}: {stdout:?}");
        assert!(
            !stdout.contains("llg_system_function"),
            "{label}: {stdout:?}"
        );
        assert!(
            normalized(&output.stderr).contains("$system is disabled"),
            "{label}: {}",
            normalized(&output.stderr)
        );
    }
}

#[test]
fn system_rejects_multiple_command_arguments() {
    sim_cli::reject_case(
        "partial_features",
        "system_invalid_arity",
        "too many arguments for '$system'",
    );
}

#[test]
fn system_rejects_embedded_nul_before_host_dispatch() {
    sim_cli::reject_case("partial_features", "system_invalid_nul", "embedded NUL");
}
