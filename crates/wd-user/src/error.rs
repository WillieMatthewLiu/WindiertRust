use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum UserError {
    FilterCompile(wd_filter::CompileError),
    InvalidFrame(&'static str),
    IncompatibleLayer(&'static str),
    OpenResponseStatus(u32),
    ProtocolVersionMismatch,
}

impl Display for UserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilterCompile(err) => write!(f, "{err}"),
            Self::InvalidFrame(msg) => write!(f, "{msg}"),
            Self::IncompatibleLayer(msg) => write!(f, "{msg}"),
            Self::OpenResponseStatus(status) => {
                write!(f, "open response returned non-zero status: {status}")
            }
            Self::ProtocolVersionMismatch => write!(f, "protocol version mismatch"),
        }
    }
}

impl Error for UserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FilterCompile(err) => Some(err),
            Self::InvalidFrame(_)
            | Self::IncompatibleLayer(_)
            | Self::OpenResponseStatus(_)
            | Self::ProtocolVersionMismatch => None,
        }
    }
}

impl From<wd_filter::CompileError> for UserError {
    fn from(value: wd_filter::CompileError) -> Self {
        Self::FilterCompile(value)
    }
}
