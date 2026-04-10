use std::fmt::{Display, Formatter};
use wd_kmdf_core::{ByteRing, ByteRingError};

use wd_proto::{
    EncodeIntoError, FlowEventKind, Layer, OpenRequest, SocketEventKind,
    encode_flow_event_payload_into, encode_runtime_event_into, encode_socket_event_payload_into,
};

use crate::{
    filter_eval::FilterCompileError, AcceptedReinjection, DriverEvent, FilterEngine, HandleState,
    NetworkRuntime, NetworkRuntimeError, ReinjectionTable,
};

const RUNTIME_FRAME_SLOTS: usize = 32;
const RUNTIME_FRAME_BYTES: usize = 2048;

#[derive(Debug, Clone)]
pub struct RuntimeDevice {
    state: HandleState,
    open_request: Option<OpenRequest>,
    filter_engine: Option<FilterEngine>,
    queue: ByteRing<RUNTIME_FRAME_SLOTS, RUNTIME_FRAME_BYTES>,
    queue_capacity: usize,
    reinjection: ReinjectionTable,
}

impl RuntimeDevice {
    pub fn new(queue_capacity: usize) -> Self {
        Self {
            state: HandleState::opening(),
            open_request: None,
            filter_engine: None,
            queue: ByteRing::new(),
            queue_capacity,
            reinjection: ReinjectionTable::default(),
        }
    }

    pub fn open(&mut self) -> Result<(), RuntimeDeviceError> {
        self.state
            .mark_running()
            .map_err(RuntimeDeviceError::InvalidState)?;
        self.open_request = None;
        self.filter_engine = None;
        Ok(())
    }

    pub fn open_with_request(&mut self, request: OpenRequest) -> Result<(), RuntimeDeviceError> {
        let filter_engine = compile_filter_for_request(&request)?;
        self.state
            .mark_running()
            .map_err(RuntimeDeviceError::InvalidState)?;
        self.open_request = Some(request);
        self.filter_engine = filter_engine;
        Ok(())
    }

    pub fn last_open_request(&self) -> Option<&OpenRequest> {
        self.open_request.as_ref()
    }

    pub fn queue_network_event(
        &mut self,
        layer: Layer,
        packet_id: u64,
        packet: &[u8],
    ) -> Result<(), RuntimeDeviceError> {
        if !matches!(self.state, HandleState::Running) {
            return Err(RuntimeDeviceError::RecvDisabled);
        }
        if !self.filter_allows_network(layer, packet) {
            return Ok(());
        }

        let mut raw = [0u8; RUNTIME_FRAME_BYTES];
        let written = NetworkRuntime::issue_event_into(
            &mut self.reinjection,
            layer,
            packet_id,
            packet,
            &mut raw,
        )
        .map_err(RuntimeDeviceError::NetworkRuntime)?;
        self.enqueue_frame(&raw[..written])
    }

    pub fn queue_socket_event(
        &mut self,
        kind: SocketEventKind,
        process_id: u64,
    ) -> Result<(), RuntimeDeviceError> {
        self.ensure_recv_running()?;
        let event = DriverEvent::socket_connect(process_id);
        if !self.filter_allows(&event) {
            return Ok(());
        }

        let mut payload = [0u8; 16];
        let payload_written = encode_socket_event_payload_into(kind, process_id, &mut payload)
            .map_err(RuntimeDeviceError::EncodeInto)?;
        let mut raw = [0u8; RUNTIME_FRAME_BYTES];
        let written = encode_runtime_event_into(Layer::Socket, &payload[..payload_written], &mut raw)
            .map_err(RuntimeDeviceError::EncodeInto)?;
        self.enqueue_frame(&raw[..written])
    }

    pub fn queue_flow_event(
        &mut self,
        kind: FlowEventKind,
        flow_id: u64,
        process_id: u64,
    ) -> Result<(), RuntimeDeviceError> {
        self.ensure_recv_running()?;
        let event = DriverEvent::flow_established(flow_id, process_id);
        if !self.filter_allows(&event) {
            return Ok(());
        }

        let mut payload = [0u8; 24];
        let payload_written =
            encode_flow_event_payload_into(kind, flow_id, process_id, &mut payload)
                .map_err(RuntimeDeviceError::EncodeInto)?;
        let mut raw = [0u8; RUNTIME_FRAME_BYTES];
        let written = encode_runtime_event_into(Layer::Flow, &payload[..payload_written], &mut raw)
            .map_err(RuntimeDeviceError::EncodeInto)?;
        self.enqueue_frame(&raw[..written])
    }

    pub fn recv_into(&mut self, output: &mut [u8]) -> Result<usize, RuntimeDeviceError> {
        if !matches!(self.state, HandleState::Running) {
            return Err(RuntimeDeviceError::RecvDisabled);
        }

        self.queue
            .pop_into(output)
            .map_err(RuntimeDeviceError::QueueStorage)?
            .ok_or(RuntimeDeviceError::QueueEmpty)
    }

    pub fn send(&mut self, raw: &[u8]) -> Result<AcceptedReinjection, RuntimeDeviceError> {
        if matches!(self.state, HandleState::SendShutdown | HandleState::Closing | HandleState::Closed)
        {
            return Err(RuntimeDeviceError::SendDisabled);
        }
        if matches!(self.state, HandleState::Opening) {
            return Err(RuntimeDeviceError::InvalidState("send requires Running or RecvShutdown state"));
        }

        NetworkRuntime::accept_send(&mut self.reinjection, raw)
            .map_err(RuntimeDeviceError::NetworkRuntime)
    }

    pub fn shutdown_recv(&mut self) -> Result<(), RuntimeDeviceError> {
        self.state
            .shutdown_recv()
            .map_err(RuntimeDeviceError::InvalidState)
    }

    pub fn shutdown_send(&mut self) -> Result<(), RuntimeDeviceError> {
        self.state
            .shutdown_send()
            .map_err(RuntimeDeviceError::InvalidState)
    }

    pub fn close(&mut self) -> Result<(), RuntimeDeviceError> {
        self.state.close().map_err(RuntimeDeviceError::InvalidState)?;
        self.open_request = None;
        self.filter_engine = None;
        self.queue.clear();
        Ok(())
    }

    fn ensure_recv_running(&self) -> Result<(), RuntimeDeviceError> {
        if !matches!(self.state, HandleState::Running) {
            return Err(RuntimeDeviceError::RecvDisabled);
        }

        Ok(())
    }

    fn filter_allows(&self, event: &DriverEvent) -> bool {
        self.filter_engine
            .as_ref()
            .is_none_or(|engine| engine.matches(event))
    }

    fn filter_allows_network(&self, layer: Layer, packet: &[u8]) -> bool {
        self.filter_engine.as_ref().is_none_or(|engine| {
            if matches!(layer, Layer::Network | Layer::NetworkForward) {
                engine.matches_network_packet(layer, packet)
            } else {
                true
            }
        })
    }

    fn enqueue_frame(&mut self, raw: &[u8]) -> Result<(), RuntimeDeviceError> {
        if self.queue_capacity != 0 {
            let effective_capacity = self.queue_capacity.min(RUNTIME_FRAME_SLOTS);
            if self.queue.len() == effective_capacity {
                let _ = self.queue.drop_oldest();
            }
            self.queue
                .push(raw)
                .map_err(RuntimeDeviceError::QueueStorage)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDeviceError {
    InvalidState(&'static str),
    RecvDisabled,
    SendDisabled,
    QueueEmpty,
    FilterCompile(FilterCompileError),
    EncodeInto(EncodeIntoError),
    QueueStorage(ByteRingError),
    NetworkRuntime(NetworkRuntimeError),
}

impl Display for RuntimeDeviceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidState(msg) => write!(f, "{msg}"),
            Self::RecvDisabled => write!(f, "runtime recv path is disabled"),
            Self::SendDisabled => write!(f, "runtime send path is disabled"),
            Self::QueueEmpty => write!(f, "runtime queue is empty"),
            Self::FilterCompile(err) => write!(f, "{err}"),
            Self::EncodeInto(EncodeIntoError::BufferTooSmall { required, provided }) => {
                write!(
                    f,
                    "runtime encode buffer too small: required {required} bytes but provided {provided}"
                )
            }
            Self::QueueStorage(ByteRingError::FrameTooLarge) => {
                write!(f, "runtime frame exceeds fixed queue storage")
            }
            Self::QueueStorage(ByteRingError::OutputTooSmall { required, provided }) => {
                write!(
                    f,
                    "runtime output buffer too small: required {required} bytes but provided {provided}"
                )
            }
            Self::NetworkRuntime(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RuntimeDeviceError {}

fn compile_filter_for_request(request: &OpenRequest) -> Result<Option<FilterEngine>, RuntimeDeviceError> {
    if request.filter_ir.is_empty() {
        return Ok(None);
    }

    match request.layer {
        Layer::Network | Layer::NetworkForward | Layer::Socket | Layer::Flow | Layer::Reflect => {
            FilterEngine::from_ir_bytes(request.layer, &request.filter_ir)
                .map(Some)
                .map_err(RuntimeDeviceError::FilterCompile)
        }
    }
}
