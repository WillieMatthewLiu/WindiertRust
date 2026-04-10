use std::process::ExitCode;

use wd_user::{RuntimeError, RuntimeTransport, WindowsTransport};

use crate::error::CliError;

pub fn default_transport() -> impl RuntimeTransport {
    WindowsTransport::default()
}

pub fn map_runtime_error(command: &'static str, err: RuntimeError) -> CliError {
    CliError::from_runtime(
        command,
        err.code(),
        err.category(),
        err.message(),
        err.suggestion(),
    )
}

pub fn exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}
