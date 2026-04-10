#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub command: &'static str,
    pub code: u8,
    pub category: &'static str,
    pub message: String,
    pub suggestion: &'static str,
}

impl CliError {
    pub fn argument_error(
        command: &'static str,
        message: impl Into<String>,
        suggestion: &'static str,
    ) -> Self {
        Self {
            command,
            code: 2,
            category: "argument_error",
            message: message.into(),
            suggestion,
        }
    }

    pub fn from_runtime(
        command: &'static str,
        code: u8,
        category: &'static str,
        message: impl Into<String>,
        suggestion: &'static str,
    ) -> Self {
        Self {
            command,
            code,
            category,
            message: message.into(),
            suggestion,
        }
    }
}
