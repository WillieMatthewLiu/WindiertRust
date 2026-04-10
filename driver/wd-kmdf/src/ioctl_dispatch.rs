use std::fmt::{Display, Formatter};

use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
use wd_proto::{
    CapabilityFlags, EncodeIntoError, FlowEventKind, OpenRequest, OpenResponse, SocketEventKind,
    decode_open_request, encode_open_response_into,
};

use crate::{AcceptedReinjection, RuntimeDevice, RuntimeDeviceError};

#[derive(Debug, Clone)]
pub struct RuntimeIoctlDispatcher {
    device: RuntimeDevice,
    last_reinjection: Option<AcceptedReinjection>,
}

impl RuntimeIoctlDispatcher {
    pub fn new(queue_capacity: usize) -> Self {
        Self {
            device: RuntimeDevice::new(queue_capacity),
            last_reinjection: None,
        }
    }

    pub fn queue_network_event(
        &mut self,
        layer: wd_proto::Layer,
        packet_id: u64,
        packet: &[u8],
    ) -> Result<(), RuntimeIoctlError> {
        self.device
            .queue_network_event(layer, packet_id, packet)
            .map_err(RuntimeIoctlError::Device)
    }

    pub fn queue_socket_event(
        &mut self,
        kind: SocketEventKind,
        process_id: u64,
    ) -> Result<(), RuntimeIoctlError> {
        self.device
            .queue_socket_event(kind, process_id)
            .map_err(RuntimeIoctlError::Device)
    }

    pub fn queue_flow_event(
        &mut self,
        kind: FlowEventKind,
        flow_id: u64,
        process_id: u64,
    ) -> Result<(), RuntimeIoctlError> {
        self.device
            .queue_flow_event(kind, flow_id, process_id)
            .map_err(RuntimeIoctlError::Device)
    }

    pub fn last_reinjection(&self) -> Option<&AcceptedReinjection> {
        self.last_reinjection.as_ref()
    }

    pub fn last_open_request(&self) -> Option<&OpenRequest> {
        self.device.last_open_request()
    }

    pub fn dispatch_into(
        &mut self,
        ioctl: u32,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, RuntimeIoctlError> {
        match ioctl {
            IOCTL_OPEN => self.dispatch_open_into(input, output),
            IOCTL_RECV => self.device.recv_into(output).map_err(RuntimeIoctlError::Device),
            IOCTL_SEND => {
                let accepted = self.device.send(input).map_err(RuntimeIoctlError::Device)?;
                self.last_reinjection = Some(accepted);
                Ok(0)
            }
            _ => Err(RuntimeIoctlError::UnsupportedIoctl(ioctl)),
        }
    }

    fn dispatch_open_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, RuntimeIoctlError> {
        let request = decode_open_request(input).map_err(RuntimeIoctlError::DecodeOpen)?;
        self.device
            .open_with_request(request)
            .map_err(RuntimeIoctlError::Device)?;

        let capabilities = (CapabilityFlags::CHECKSUM_RECALC | CapabilityFlags::NETWORK_REINJECT).bits();
        encode_open_response_into(OpenResponse::success(capabilities), output)
            .map_err(RuntimeIoctlError::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeIoctlError {
    UnsupportedIoctl(u32),
    OutputTooSmall { required: usize, provided: usize },
    DecodeOpen(wd_proto::OpenRequestDecodeError),
    Device(RuntimeDeviceError),
}

impl Display for RuntimeIoctlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedIoctl(ioctl) => write!(f, "unsupported ioctl: 0x{ioctl:08x}"),
            Self::OutputTooSmall { required, provided } => {
                write!(
                    f,
                    "output buffer too small: required {required} bytes but provided {provided}"
                )
            }
            Self::DecodeOpen(err) => write!(f, "{err}"),
            Self::Device(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RuntimeIoctlError {}

impl From<EncodeIntoError> for RuntimeIoctlError {
    fn from(value: EncodeIntoError) -> Self {
        match value {
            EncodeIntoError::BufferTooSmall { required, provided } => {
                Self::OutputTooSmall { required, provided }
            }
        }
    }
}
