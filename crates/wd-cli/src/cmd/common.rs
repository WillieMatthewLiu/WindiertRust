use std::process::ExitCode;

use crate::error::CliError;
use crate::output::{OutputMode, render_error_json, render_error_text};
use crate::runtime::exit_code;

pub fn render_summary(prefix: &str, fields: &[(&str, String)]) -> String {
    let mut line = String::from(prefix);
    for (key, value) in fields {
        line.push(' ');
        line.push_str(key);
        line.push('=');
        if is_summary_value_safe(value) {
            line.push_str(value);
        } else {
            line.push('"');
            line.push_str(&escape_summary_value(value));
            line.push('"');
        }
    }
    line
}

pub fn finish(result: Result<String, String>) -> ExitCode {
    match result {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

pub fn finish_with_cli_error(result: Result<String, CliError>, mode: OutputMode) -> ExitCode {
    match result {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            let rendered = match mode {
                OutputMode::Text => render_error_text(&err),
                OutputMode::Json => render_error_json(&err),
            };
            eprintln!("{rendered}");
            exit_code(err.code)
        }
    }
}

fn is_summary_value_safe(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| !ch.is_whitespace() && ch != '=' && !ch.is_control() && ch != '"' && ch != '\\')
}

fn escape_summary_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ if (ch as u32) < 0x20 => {
                escaped.push_str("\\u");
                escaped.push('0');
                escaped.push('0');
                escaped.push(nibble_to_hex(((ch as u32 >> 4) & 0x0f) as u8));
                escaped.push(nibble_to_hex(((ch as u32) & 0x0f) as u8));
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'a' + (nibble - 10)),
    }
}
