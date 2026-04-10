use thiserror::Error;
use wd_proto::Layer;

use crate::DeviceAvailability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbe {
    pub device_path: String,
    pub capabilities: Option<u32>,
    pub protocol_major: Option<u16>,
    pub protocol_minor: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOpenConfig {
    layer: Layer,
    filter_ir: Vec<u8>,
    priority: i16,
    flags: u64,
}

impl RuntimeOpenConfig {
    pub fn new(layer: Layer, filter_ir: Vec<u8>, priority: i16, flags: u64) -> Self {
        Self {
            layer,
            filter_ir,
            priority,
            flags,
        }
    }

    pub fn network(filter_ir: Vec<u8>) -> Self {
        Self::new(Layer::Network, filter_ir, 0, 0)
    }

    pub fn socket() -> Self {
        Self::new(Layer::Socket, Vec::new(), 0, 0)
    }

    pub fn flow() -> Self {
        Self::new(Layer::Flow, Vec::new(), 0, 0)
    }

    pub fn reflect() -> Self {
        Self::new(Layer::Reflect, Vec::new(), 0, 0)
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn filter_ir(&self) -> &[u8] {
        &self.filter_ir
    }

    pub fn priority(&self) -> i16 {
        self.priority
    }

    pub fn flags(&self) -> u64 {
        self.flags
    }
}

pub trait RuntimeSession {
    fn recv_one(&mut self, max_bytes: usize) -> Result<Vec<u8>, RuntimeError>;
    fn send_one(&mut self, bytes: &[u8]) -> Result<(), RuntimeError>;
    fn close(self) -> Result<(), RuntimeError>;
}

pub trait RuntimeTransport {
    type Session: RuntimeSession;

    fn probe(&self) -> Result<DeviceAvailability, RuntimeError>;
    fn open(&self, config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError>;
    fn open_session(&self, config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError>;
    fn close(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct RuntimeError {
    code: u8,
    category: &'static str,
    message: String,
    suggestion: &'static str,
}

impl RuntimeError {
    pub fn device_unavailable(path: &str) -> Self {
        Self {
            code: 3,
            category: "device_unavailable",
            message: format!("WdRust device not found at {path}"),
            suggestion: "verify driver is installed and device link is present",
        }
    }

    pub fn open_failed(message: impl Into<String>) -> Self {
        Self {
            code: 4,
            category: "open_failed",
            message: message.into(),
            suggestion: "verify permissions and exclusive access settings",
        }
    }

    pub fn protocol_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: 5,
            category: "protocol_mismatch",
            message: message.into(),
            suggestion: "verify driver and user-mode binaries are from the same build",
        }
    }

    pub fn io_failure(message: impl Into<String>) -> Self {
        Self {
            code: 6,
            category: "io_failure",
            message: message.into(),
            suggestion: "retry the command and inspect verbose diagnostics",
        }
    }

    pub fn code(&self) -> u8 {
        self.code
    }

    pub fn category(&self) -> &'static str {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn suggestion(&self) -> &'static str {
        self.suggestion
    }
}
