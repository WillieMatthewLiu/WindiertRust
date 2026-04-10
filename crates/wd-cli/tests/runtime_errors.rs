use wd_cli::error::CliError;
use wd_cli::output::{render_error_json, render_error_text};
use wd_cli::runtime::{exit_code, map_runtime_error};
use wd_user::RuntimeError;

#[test]
fn map_runtime_error_preserves_runtime_error_contract() {
    let err = map_runtime_error("netdump", RuntimeError::device_unavailable(r"\\.\WdRust"));

    assert_eq!(err.command, "netdump");
    assert_eq!(err.code, 3);
    assert_eq!(err.category, "device_unavailable");
    assert_eq!(err.message, r"WdRust device not found at \\.\WdRust");
    assert_eq!(
        err.suggestion,
        "verify driver is installed and device link is present"
    );
}

#[test]
fn text_errors_include_code_category_and_message() {
    let err = map_runtime_error("netdump", RuntimeError::device_unavailable(r"\\.\WdRust"));
    let line = render_error_text(&err);

    assert!(line.contains("NETDUMP ERROR"));
    assert!(line.contains("code=3"));
    assert!(line.contains("category=device_unavailable"));
    assert!(line.contains(r"message=WdRust device not found at \\.\WdRust"));
}

#[test]
fn json_errors_include_stable_fields() {
    let err = map_runtime_error("netdump", RuntimeError::open_failed("open denied"));
    let line = render_error_json(&err);

    assert!(line.contains("\"command\":\"netdump\""));
    assert!(line.contains("\"status\":\"error\""));
    assert!(line.contains("\"code\":4"));
    assert!(line.contains("\"category\":\"open_failed\""));
    assert!(line.contains("\"message\":\"open denied\""));
}

#[test]
fn exit_code_uses_cli_error_code() {
    let err = map_runtime_error("reflectctl", RuntimeError::io_failure("recv timeout"));
    assert_eq!(exit_code(err.code), std::process::ExitCode::from(6));
}

#[test]
fn json_errors_escape_all_control_characters() {
    let message = "bad\u{0001}\u{001f}\n\t\r\"\\";
    let err = map_runtime_error("netdump", RuntimeError::open_failed(message));
    let line = render_error_json(&err);

    assert!(line.contains("\\u0001"));
    assert!(line.contains("\\u001f"));
    assert!(line.contains("\\n"));
    assert!(line.contains("\\t"));
    assert!(line.contains("\\r"));
    assert!(line.contains("\\\""));
    assert!(line.contains("\\\\"));
    assert!(line.bytes().all(|byte| byte >= 0x20));
}

#[test]
fn render_summary_quotes_and_escapes_unsafe_values() {
    let line = wd_cli::cmd::common::render_summary(
        "TEST OK",
        &[
            ("safe", "alpha-01".to_string()),
            ("spaced", "hello world".to_string()),
            ("equals", "a=b".to_string()),
            ("quote", "x\"y".to_string()),
            ("slash", "x\\y".to_string()),
            ("ctrl", "x\u{0001}y".to_string()),
            ("empty", String::new()),
        ],
    );

    assert!(line.contains("safe=alpha-01"));
    assert!(line.contains("spaced=\"hello world\""));
    assert!(line.contains("equals=\"a=b\""));
    assert!(line.contains("quote=\"x\\\"y\""));
    assert!(line.contains("slash=\"x\\\\y\""));
    assert!(line.contains("ctrl=\"x\\u0001y\""));
    assert!(line.contains("empty=\"\""));
}

#[test]
fn json_errors_escape_command_and_category_fields() {
    let err = CliError::from_runtime(
        "net\"dump\u{0002}\\x",
        4,
        "open\n\"failed\\cat",
        "open denied",
        "retry",
    );
    let line = render_error_json(&err);

    assert!(line.contains("\"command\":\"net\\\"dump\\u0002\\\\x\""));
    assert!(line.contains("\"category\":\"open\\n\\\"failed\\\\cat\""));
    assert!(line.bytes().all(|byte| byte >= 0x20));
}
