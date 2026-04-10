use crate::error::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

impl OutputMode {
    pub fn from_json_flag(json: bool) -> Self {
        if json { Self::Json } else { Self::Text }
    }
}

pub fn render_error_text(err: &CliError) -> String {
    format!(
        "{} ERROR code={} category={} message={} suggestion={}",
        err.command.to_ascii_uppercase(),
        err.code,
        err.category,
        err.message,
        err.suggestion
    )
}

pub fn render_error_json(err: &CliError) -> String {
    format!(
        "{{\"command\":\"{}\",\"status\":\"error\",\"code\":{},\"category\":\"{}\",\"message\":\"{}\",\"suggestion\":\"{}\"}}",
        escape_json_string(err.command),
        err.code,
        escape_json_string(err.category),
        escape_json_string(&err.message),
        escape_json_string(err.suggestion),
    )
}

fn escape_json_string(value: &str) -> String {
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
