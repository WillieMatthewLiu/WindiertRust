use std::process::Command;

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wd-cli"))
        .args(args)
        .output()
        .expect("wd-cli should run")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn is_supported_runtime_category(line: &str) -> bool {
    line.contains("category=device_unavailable")
        || line.contains("category=open_failed")
        || line.contains("category=protocol_mismatch")
        || line.contains("category=io_failure")
        || line.contains("\"category\":\"device_unavailable\"")
        || line.contains("\"category\":\"open_failed\"")
        || line.contains("\"category\":\"protocol_mismatch\"")
        || line.contains("\"category\":\"io_failure\"")
}

fn assert_reflectctl_shared_runtime_error_text(output: &std::process::Output, context: &str) {
    let line = stderr(output);
    assert!(
        line.contains("REFLECTCTL ERROR"),
        "{context} should use shared text error output, stderr={line}"
    );
    assert!(
        line.contains("code="),
        "{context} should include runtime code, stderr={line}"
    );
    assert!(
        is_supported_runtime_category(&line),
        "{context} should include shared runtime category, stderr={line}"
    );
}

fn assert_reflectctl_shared_runtime_error_json(output: &std::process::Output, context: &str) {
    let line = stderr(output);
    assert!(
        line.contains("\"command\":\"reflectctl\""),
        "{context} should include command in shared json error output, stderr={line}"
    );
    assert!(
        line.contains("\"status\":\"error\""),
        "{context} should include status=error in shared json error output, stderr={line}"
    );
    assert!(
        line.contains("\"code\":"),
        "{context} should include runtime code in shared json error output, stderr={line}"
    );
    assert!(
        is_supported_runtime_category(&line),
        "{context} should include shared runtime category in json error output, stderr={line}"
    );
}

fn assert_reflectctl_text_or_runtime_error(action: &str, expected_state: &str) {
    let output = run_cli(&["reflectctl", "--action", action]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("REFLECTCTL OK"), "unexpected stdout: {line}");
        assert!(line.contains("device=ready"), "unexpected stdout: {line}");
        assert!(
            line.contains(&format!("state={expected_state}")),
            "unexpected stdout: {line}"
        );
        return;
    }

    assert!(
        !output.status.success(),
        "reflectctl action '{action}' should not succeed in this branch, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert_reflectctl_shared_runtime_error_text(&output, &format!("reflectctl --action {action}"));
}

#[test]
fn netfilter_requires_filter_argument() {
    let output = run_cli(&["netfilter"]);

    assert!(
        !output.status.success(),
        "expected clap failure when --filter is missing, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("--filter"),
        "stderr should mention --filter, got: {}",
        stderr(&output)
    );
}

#[test]
fn netfilter_validate_reports_runtime_contract_or_shared_error() {
    let output = run_cli(&[
        "netfilter",
        "--filter",
        "tcp and inbound",
        "--mode",
        "validate",
        "--json",
    ]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"netfilter\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"mode\":\"validate\""), "unexpected stdout: {line}");
        return;
    }

    let line = stderr(&output);
    assert!(line.contains("\"command\":\"netfilter\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"code\":"), "unexpected stderr: {line}");
    assert!(is_supported_runtime_category(&line), "unexpected stderr: {line}");
}

#[test]
fn netfilter_reinject_reports_runtime_contract_or_shared_error() {
    let output = run_cli(&[
        "netfilter",
        "--filter",
        "tcp and inbound",
        "--mode",
        "reinject",
        "--json",
    ]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"netfilter\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"mode\":\"reinject\""), "unexpected stdout: {line}");
        assert!(line.contains("\"reinjection_token\":"), "unexpected stdout: {line}");
        return;
    }

    let line = stderr(&output);
    assert!(line.contains("\"command\":\"netfilter\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"code\":"), "unexpected stderr: {line}");
    assert!(is_supported_runtime_category(&line), "unexpected stderr: {line}");
}

#[test]
fn netfilter_rejects_zero_count_with_argument_error() {
    let output = run_cli(&[
        "netfilter",
        "--filter",
        "tcp and inbound",
        "--mode",
        "observe",
        "--count",
        "0",
    ]);

    assert!(
        !output.status.success(),
        "count=0 should fail, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "count validation should map to exit code 2, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let line = stderr(&output);
    assert!(line.contains("NETFILTER ERROR"), "unexpected stderr: {line}");
    assert!(line.contains("category=argument_error"), "unexpected stderr: {line}");
}

#[test]
fn netdump_json_reports_runtime_contract_or_shared_error() {
    let output = run_cli(&["netdump", "--json"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"netdump\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"layer\":\"NETWORK\""), "unexpected stdout: {line}");
        assert!(line.contains("\"ttl\":"), "unexpected stdout: {line}");
        assert!(line.contains("\"checksum\":\""), "unexpected stdout: {line}");
        assert!(line.contains("\"packet_len\":"), "unexpected stdout: {line}");
        assert!(line.contains("\"timestamp\":\""), "unexpected stdout: {line}");
        return;
    }

    let line = stderr(&output);
    assert!(line.contains("\"command\":\"netdump\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"code\":"), "unexpected stderr: {line}");
    assert!(is_supported_runtime_category(&line), "unexpected stderr: {line}");
}

#[test]
fn netdump_count_greater_than_one_requires_follow() {
    let output = run_cli(&["netdump", "--count", "2"]);

    assert!(
        !output.status.success(),
        "count>1 without follow should fail, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "count validation should map to exit code 2, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let line = stderr(&output);
    assert!(line.contains("NETDUMP ERROR"), "unexpected stderr: {line}");
    assert!(line.contains("category=argument_error"), "unexpected stderr: {line}");
}

#[test]
fn socketdump_json_reports_runtime_contract_or_shared_error() {
    let output = run_cli(&["socketdump", "--filter", "event == CONNECT", "--json"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"socketdump\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"event\":\"CONNECT\""), "unexpected stdout: {line}");
        assert!(line.contains("\"matched\":true"), "unexpected stdout: {line}");
        return;
    }

    let line = stderr(&output);
    assert!(line.contains("\"command\":\"socketdump\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"code\":"), "unexpected stderr: {line}");
    assert!(is_supported_runtime_category(&line), "unexpected stderr: {line}");
}

#[test]
fn reflectctl_probe_reports_runtime_contract_text_or_device_unavailable() {
    assert_reflectctl_text_or_runtime_error("probe", "Probed");
}

#[test]
fn reflectctl_probe_reports_runtime_contract_json_or_device_unavailable() {
    let output = run_cli(&["reflectctl", "--action", "probe", "--json"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"reflectctl\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"device\":\"ready\""), "unexpected stdout: {line}");
        return;
    }

    assert_reflectctl_shared_runtime_error_json(&output, "reflectctl --action probe --json");
}

#[test]
fn reflectctl_open_reports_runtime_contract_text_or_device_unavailable() {
    assert_reflectctl_text_or_runtime_error("open", "Open");
}

#[test]
fn reflectctl_close_reports_runtime_contract_text_or_device_unavailable() {
    assert_reflectctl_text_or_runtime_error("close", "CloseAttempted");
}

#[test]
fn reflectctl_capabilities_reports_runtime_contract_text_or_device_unavailable() {
    assert_reflectctl_text_or_runtime_error("capabilities", "Open");
}

#[test]
fn reflectctl_state_reports_runtime_contract_text_or_device_unavailable() {
    assert_reflectctl_text_or_runtime_error("state", "Open");
}

#[test]
fn reflectctl_timeout_zero_is_a_user_visible_runtime_error() {
    let output = run_cli(&[
        "reflectctl",
        "--action",
        "open",
        "--timeout-ms",
        "0",
        "--verbose",
        "--json",
    ]);

    assert!(
        !output.status.success(),
        "timeout=0 should fail, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        output.status.code(),
        Some(6),
        "timeout=0 should map to io_failure code 6, stdout={}, stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let line = stderr(&output);
    assert!(line.contains("\"command\":\"reflectctl\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"category\":\"io_failure\""), "unexpected stderr: {line}");
    assert!(
        line.contains("timeout-ms must be greater than 0"),
        "unexpected stderr: {line}"
    );
    assert!(line.contains("action=open"), "unexpected stderr: {line}");
    assert!(line.contains("timeout_ms=0"), "unexpected stderr: {line}");
}

#[test]
fn reflectctl_default_action_uses_open_path_or_shared_runtime_error() {
    let output = run_cli(&["reflectctl"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("REFLECTCTL OK"), "unexpected stdout: {line}");
        assert!(line.contains("state=Open"), "unexpected stdout: {line}");
        return;
    }

    assert_reflectctl_shared_runtime_error_text(&output, "reflectctl");
}

#[test]
fn reflectctl_default_verbose_json_reports_action_open() {
    let output = run_cli(&["reflectctl", "--verbose", "--json"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(
            line.contains("\"action\":\"open\""),
            "default action should be open, stdout={line}"
        );
        return;
    }

    let line = stderr(&output);
    assert!(
        line.contains("action=open"),
        "default action should be open in error context, stderr={line}"
    );
}

#[test]
fn flowtrack_json_reports_runtime_contract_or_shared_error() {
    let output = run_cli(&["flowtrack", "--json"]);

    if output.status.success() {
        let line = stdout(&output);
        assert!(line.contains("\"command\":\"flowtrack\""), "unexpected stdout: {line}");
        assert!(line.contains("\"status\":\"ok\""), "unexpected stdout: {line}");
        assert!(line.contains("\"event\":\"ESTABLISHED\""), "unexpected stdout: {line}");
        assert!(line.contains("\"flow_id\":"), "unexpected stdout: {line}");
        return;
    }

    let line = stderr(&output);
    assert!(line.contains("\"command\":\"flowtrack\""), "unexpected stderr: {line}");
    assert!(line.contains("\"status\":\"error\""), "unexpected stderr: {line}");
    assert!(line.contains("\"code\":"), "unexpected stderr: {line}");
    assert!(is_supported_runtime_category(&line), "unexpected stderr: {line}");
}
