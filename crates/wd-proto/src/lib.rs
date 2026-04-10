use bitflags::bitflags;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self { major: 0, minor: 1 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Network,
    NetworkForward,
    Flow,
    Socket,
    Reflect,
}

impl Layer {
    pub const fn all() -> [Layer; 5] {
        [
            Layer::Network,
            Layer::NetworkForward,
            Layer::Flow,
            Layer::Socket,
            Layer::Reflect,
        ]
    }

    pub const fn to_wire(self) -> u8 {
        match self {
            Layer::Network => 1,
            Layer::NetworkForward => 2,
            Layer::Flow => 3,
            Layer::Socket => 4,
            Layer::Reflect => 5,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Layer::Network),
            2 => Some(Layer::NetworkForward),
            3 => Some(Layer::Flow),
            4 => Some(Layer::Socket),
            5 => Some(Layer::Reflect),
            _ => None,
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilityFlags: u32 {
        const CHECKSUM_RECALC = 0x0001;
        const NETWORK_REINJECT = 0x0002;
        const FLOW_EVENTS = 0x0004;
        const SOCKET_EVENTS = 0x0008;
        const REFLECT_EVENTS = 0x0010;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub version: ProtocolVersion,
    pub layer: Layer,
    pub priority: i16,
    pub flags: u64,
    pub filter_len: u32,
    pub filter_ir: Vec<u8>,
}

impl OpenRequest {
    pub fn new(layer: Layer, filter_ir: Vec<u8>, priority: i16, flags: u64) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            layer,
            priority,
            flags,
            filter_len: filter_ir.len() as u32,
            filter_ir,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenResponse {
    pub version: ProtocolVersion,
    pub capabilities: u32,
    pub status: u32,
}

impl OpenResponse {
    pub const fn success(capabilities: u32) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            capabilities,
            status: 0,
        }
    }
}

const OPEN_REQUEST_HEADER_LEN: usize = 20;
const OPEN_RESPONSE_LEN: usize = 12;

pub const RUNTIME_EVENT_MAGIC: [u8; 4] = *b"WDRT";
pub const RUNTIME_EVENT_HEADER_LEN: usize = 16;
pub const RUNTIME_SEND_MAGIC: [u8; 4] = *b"WDSN";
pub const RUNTIME_SEND_HEADER_LEN: usize = 24;
const SOCKET_EVENT_PAYLOAD_LEN: usize = 16;
const FLOW_EVENT_PAYLOAD_LEN: usize = 24;
const NETWORK_EVENT_PAYLOAD_MAGIC: [u8; 4] = *b"WDNW";
const NETWORK_EVENT_PAYLOAD_HEADER_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventHeader {
    pub version: ProtocolVersion,
    pub layer: Layer,
    pub payload_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventFrame<'a> {
    pub header: RuntimeEventHeader,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkEventPayload<'a> {
    pub reinjection_token: u64,
    pub packet: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSendRequestHeader {
    pub version: ProtocolVersion,
    pub layer: Layer,
    pub reinjection_token: u64,
    pub payload_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSendRequest<'a> {
    pub header: RuntimeSendRequestHeader,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketEventKind {
    Connect,
}

impl SocketEventKind {
    pub const fn code(self) -> u32 {
        match self {
            Self::Connect => 2,
        }
    }

    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            2 => Some(Self::Connect),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowEventKind {
    Established,
}

impl FlowEventKind {
    pub const fn code(self) -> u32 {
        match self {
            Self::Established => 4,
        }
    }

    pub const fn from_code(value: u32) -> Option<Self> {
        match value {
            4 => Some(Self::Established),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketEventPayload {
    kind: SocketEventKind,
    process_id: u64,
}

impl SocketEventPayload {
    pub const fn new(kind: SocketEventKind, process_id: u64) -> Self {
        Self { kind, process_id }
    }

    pub const fn kind(self) -> SocketEventKind {
        self.kind
    }

    pub const fn process_id(self) -> u64 {
        self.process_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowEventPayload {
    kind: FlowEventKind,
    flow_id: u64,
    process_id: u64,
}

impl FlowEventPayload {
    pub const fn new(kind: FlowEventKind, flow_id: u64, process_id: u64) -> Self {
        Self {
            kind,
            flow_id,
            process_id,
        }
    }

    pub const fn kind(self) -> FlowEventKind {
        self.kind
    }

    pub const fn flow_id(self) -> u64 {
        self.flow_id
    }

    pub const fn process_id(self) -> u64 {
        self.process_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEventDecodeError {
    HeaderTooShort,
    InvalidMagic,
    UnsupportedLayer(u8),
    ProtocolVersionMismatch { major: u16, minor: u16 },
    TruncatedPayload { expected: usize, actual: usize },
    NetworkPayloadHeaderTooShort,
    InvalidNetworkPayloadMagic,
    TruncatedNetworkPacket { expected: usize, actual: usize },
    InvalidSocketPayloadLength(usize),
    InvalidFlowPayloadLength(usize),
    UnsupportedSocketEventKind(u32),
    UnsupportedFlowEventKind(u32),
}

impl Display for RuntimeEventDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HeaderTooShort => write!(f, "runtime event header too short"),
            Self::InvalidMagic => write!(f, "runtime event magic mismatch"),
            Self::UnsupportedLayer(layer) => write!(f, "unsupported runtime event layer: {layer}"),
            Self::ProtocolVersionMismatch { major, minor } => {
                write!(f, "runtime event protocol mismatch: {major}.{minor}")
            }
            Self::TruncatedPayload { expected, actual } => {
                write!(
                    f,
                    "runtime event payload truncated: expected {expected} bytes got {actual}"
                )
            }
            Self::NetworkPayloadHeaderTooShort => {
                write!(f, "runtime network payload header too short")
            }
            Self::InvalidNetworkPayloadMagic => {
                write!(f, "runtime network payload magic mismatch")
            }
            Self::TruncatedNetworkPacket { expected, actual } => {
                write!(
                    f,
                    "runtime network packet truncated: expected {expected} bytes got {actual}"
                )
            }
            Self::InvalidSocketPayloadLength(len) => {
                write!(f, "invalid socket event payload length: {len}")
            }
            Self::InvalidFlowPayloadLength(len) => {
                write!(f, "invalid flow event payload length: {len}")
            }
            Self::UnsupportedSocketEventKind(kind) => {
                write!(f, "unsupported socket event kind: {kind}")
            }
            Self::UnsupportedFlowEventKind(kind) => {
                write!(f, "unsupported flow event kind: {kind}")
            }
        }
    }
}

impl Error for RuntimeEventDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSendDecodeError {
    HeaderTooShort,
    InvalidMagic,
    UnsupportedLayer(u8),
    ProtocolVersionMismatch { major: u16, minor: u16 },
    TruncatedPayload { expected: usize, actual: usize },
}

impl Display for RuntimeSendDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HeaderTooShort => write!(f, "runtime send request header too short"),
            Self::InvalidMagic => write!(f, "runtime send request magic mismatch"),
            Self::UnsupportedLayer(layer) => write!(f, "unsupported runtime send layer: {layer}"),
            Self::ProtocolVersionMismatch { major, minor } => {
                write!(f, "runtime send protocol mismatch: {major}.{minor}")
            }
            Self::TruncatedPayload { expected, actual } => {
                write!(
                    f,
                    "runtime send payload truncated: expected {expected} bytes got {actual}"
                )
            }
        }
    }
}

impl Error for RuntimeSendDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenRequestDecodeError {
    HeaderTooShort,
    UnsupportedLayer(u8),
    ProtocolVersionMismatch { major: u16, minor: u16 },
    TruncatedFilter { expected: usize, actual: usize },
}

impl Display for OpenRequestDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HeaderTooShort => write!(f, "open request header too short"),
            Self::UnsupportedLayer(layer) => write!(f, "unsupported open request layer: {layer}"),
            Self::ProtocolVersionMismatch { major, minor } => {
                write!(f, "open request protocol mismatch: {major}.{minor}")
            }
            Self::TruncatedFilter { expected, actual } => {
                write!(f, "open request filter truncated: expected {expected} bytes got {actual}")
            }
        }
    }
}

impl Error for OpenRequestDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenResponseDecodeError {
    BufferTooShort,
    ProtocolVersionMismatch { major: u16, minor: u16 },
}

impl Display for OpenResponseDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooShort => write!(f, "open response buffer too short"),
            Self::ProtocolVersionMismatch { major, minor } => {
                write!(f, "open response protocol mismatch: {major}.{minor}")
            }
        }
    }
}

impl Error for OpenResponseDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeIntoError {
    BufferTooSmall { required: usize, provided: usize },
}

impl Display for EncodeIntoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "encode output buffer too small: required {required} bytes but provided {provided}"
                )
            }
        }
    }
}

impl Error for EncodeIntoError {}

pub fn encode_open_request(request: &OpenRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(OPEN_REQUEST_HEADER_LEN + request.filter_ir.len());
    bytes.extend_from_slice(&request.version.major.to_le_bytes());
    bytes.extend_from_slice(&request.version.minor.to_le_bytes());
    bytes.push(request.layer.to_wire());
    bytes.push(0);
    bytes.extend_from_slice(&request.priority.to_le_bytes());
    bytes.extend_from_slice(&request.flags.to_le_bytes());
    bytes.extend_from_slice(&request.filter_len.to_le_bytes());
    bytes.extend_from_slice(&request.filter_ir);
    bytes
}

pub fn decode_open_request(raw: &[u8]) -> Result<OpenRequest, OpenRequestDecodeError> {
    if raw.len() < OPEN_REQUEST_HEADER_LEN {
        return Err(OpenRequestDecodeError::HeaderTooShort);
    }

    let major = u16::from_le_bytes([raw[0], raw[1]]);
    let minor = u16::from_le_bytes([raw[2], raw[3]]);
    let version = ProtocolVersion { major, minor };
    if version != ProtocolVersion::CURRENT {
        return Err(OpenRequestDecodeError::ProtocolVersionMismatch { major, minor });
    }

    let layer = Layer::from_wire(raw[4]).ok_or(OpenRequestDecodeError::UnsupportedLayer(raw[4]))?;
    let priority = i16::from_le_bytes([raw[6], raw[7]]);
    let flags = u64::from_le_bytes([
        raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15],
    ]);
    let filter_len = u32::from_le_bytes([raw[16], raw[17], raw[18], raw[19]]);
    let expected_len = OPEN_REQUEST_HEADER_LEN + filter_len as usize;
    if raw.len() < expected_len {
        return Err(OpenRequestDecodeError::TruncatedFilter {
            expected: expected_len,
            actual: raw.len(),
        });
    }

    Ok(OpenRequest {
        version,
        layer,
        priority,
        flags,
        filter_len,
        filter_ir: raw[OPEN_REQUEST_HEADER_LEN..expected_len].to_vec(),
    })
}

pub fn encode_open_response(response: OpenResponse) -> Vec<u8> {
    let mut bytes = vec![0u8; OPEN_RESPONSE_LEN];
    let written = encode_open_response_into(response, &mut bytes)
        .expect("exact open response buffer should encode");
    debug_assert_eq!(written, bytes.len());
    bytes
}

pub fn encode_open_response_into(
    response: OpenResponse,
    output: &mut [u8],
) -> Result<usize, EncodeIntoError> {
    ensure_capacity(output, OPEN_RESPONSE_LEN)?;
    output[0..2].copy_from_slice(&response.version.major.to_le_bytes());
    output[2..4].copy_from_slice(&response.version.minor.to_le_bytes());
    output[4..8].copy_from_slice(&response.capabilities.to_le_bytes());
    output[8..12].copy_from_slice(&response.status.to_le_bytes());
    Ok(OPEN_RESPONSE_LEN)
}

pub fn decode_open_response(raw: &[u8]) -> Result<OpenResponse, OpenResponseDecodeError> {
    if raw.len() < OPEN_RESPONSE_LEN {
        return Err(OpenResponseDecodeError::BufferTooShort);
    }

    let major = u16::from_le_bytes([raw[0], raw[1]]);
    let minor = u16::from_le_bytes([raw[2], raw[3]]);
    let version = ProtocolVersion { major, minor };
    if version != ProtocolVersion::CURRENT {
        return Err(OpenResponseDecodeError::ProtocolVersionMismatch { major, minor });
    }

    Ok(OpenResponse {
        version,
        capabilities: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        status: u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]),
    })
}

pub fn encode_runtime_event(layer: Layer, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; RUNTIME_EVENT_HEADER_LEN + payload.len()];
    let written = encode_runtime_event_into(layer, payload, &mut bytes)
        .expect("exact runtime event buffer should encode");
    debug_assert_eq!(written, bytes.len());
    bytes
}

pub fn encode_runtime_event_into(
    layer: Layer,
    payload: &[u8],
    output: &mut [u8],
) -> Result<usize, EncodeIntoError> {
    let payload_len = u32::try_from(payload.len()).expect("runtime payload should fit in u32");
    let required = RUNTIME_EVENT_HEADER_LEN + payload.len();
    ensure_capacity(output, required)?;
    output[0..4].copy_from_slice(&RUNTIME_EVENT_MAGIC);
    output[4..6].copy_from_slice(&ProtocolVersion::CURRENT.major.to_le_bytes());
    output[6..8].copy_from_slice(&ProtocolVersion::CURRENT.minor.to_le_bytes());
    output[8] = layer.to_wire();
    output[9..12].copy_from_slice(&[0u8; 3]);
    output[12..16].copy_from_slice(&payload_len.to_le_bytes());
    output[16..required].copy_from_slice(payload);
    Ok(required)
}

pub fn decode_runtime_event(raw: &[u8]) -> Result<RuntimeEventFrame<'_>, RuntimeEventDecodeError> {
    if raw.len() < RUNTIME_EVENT_HEADER_LEN {
        return Err(RuntimeEventDecodeError::HeaderTooShort);
    }
    if raw[..4] != RUNTIME_EVENT_MAGIC {
        return Err(RuntimeEventDecodeError::InvalidMagic);
    }

    let major = u16::from_le_bytes([raw[4], raw[5]]);
    let minor = u16::from_le_bytes([raw[6], raw[7]]);
    let version = ProtocolVersion { major, minor };
    if version != ProtocolVersion::CURRENT {
        return Err(RuntimeEventDecodeError::ProtocolVersionMismatch { major, minor });
    }

    let layer = Layer::from_wire(raw[8]).ok_or(RuntimeEventDecodeError::UnsupportedLayer(raw[8]))?;
    let payload_len = u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]) as usize;
    let expected_len = RUNTIME_EVENT_HEADER_LEN + payload_len;
    if raw.len() < expected_len {
        return Err(RuntimeEventDecodeError::TruncatedPayload {
            expected: expected_len,
            actual: raw.len(),
        });
    }

    Ok(RuntimeEventFrame {
        header: RuntimeEventHeader {
            version,
            layer,
            payload_len: payload_len as u32,
        },
        payload: &raw[RUNTIME_EVENT_HEADER_LEN..expected_len],
    })
}

pub fn encode_network_event_payload(reinjection_token: u64, packet: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; NETWORK_EVENT_PAYLOAD_HEADER_LEN + packet.len()];
    let written = encode_network_event_payload_into(reinjection_token, packet, &mut bytes)
        .expect("exact network payload buffer should encode");
    debug_assert_eq!(written, bytes.len());
    bytes
}

pub fn encode_network_event_payload_into(
    reinjection_token: u64,
    packet: &[u8],
    output: &mut [u8],
) -> Result<usize, EncodeIntoError> {
    let packet_len = u32::try_from(packet.len()).expect("network packet should fit in u32");
    let required = NETWORK_EVENT_PAYLOAD_HEADER_LEN + packet.len();
    ensure_capacity(output, required)?;
    output[0..4].copy_from_slice(&NETWORK_EVENT_PAYLOAD_MAGIC);
    output[4..12].copy_from_slice(&reinjection_token.to_le_bytes());
    output[12..16].copy_from_slice(&packet_len.to_le_bytes());
    output[16..required].copy_from_slice(packet);
    Ok(required)
}

pub fn decode_network_event_payload(
    raw: &[u8],
) -> Result<NetworkEventPayload<'_>, RuntimeEventDecodeError> {
    if raw.len() < NETWORK_EVENT_PAYLOAD_HEADER_LEN {
        return Err(RuntimeEventDecodeError::NetworkPayloadHeaderTooShort);
    }
    if raw[..4] != NETWORK_EVENT_PAYLOAD_MAGIC {
        return Err(RuntimeEventDecodeError::InvalidNetworkPayloadMagic);
    }

    let reinjection_token = u64::from_le_bytes([
        raw[4], raw[5], raw[6], raw[7], raw[8], raw[9], raw[10], raw[11],
    ]);
    let packet_len = u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]) as usize;
    let expected_len = NETWORK_EVENT_PAYLOAD_HEADER_LEN + packet_len;
    if raw.len() < expected_len {
        return Err(RuntimeEventDecodeError::TruncatedNetworkPacket {
            expected: expected_len,
            actual: raw.len(),
        });
    }

    Ok(NetworkEventPayload {
        reinjection_token,
        packet: &raw[NETWORK_EVENT_PAYLOAD_HEADER_LEN..expected_len],
    })
}

pub fn encode_runtime_send_request(
    layer: Layer,
    reinjection_token: u64,
    payload: &[u8],
) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).expect("runtime send payload should fit in u32");
    let mut bytes = Vec::with_capacity(RUNTIME_SEND_HEADER_LEN + payload.len());
    bytes.extend_from_slice(&RUNTIME_SEND_MAGIC);
    bytes.extend_from_slice(&ProtocolVersion::CURRENT.major.to_le_bytes());
    bytes.extend_from_slice(&ProtocolVersion::CURRENT.minor.to_le_bytes());
    bytes.push(layer.to_wire());
    bytes.extend_from_slice(&[0u8; 3]);
    bytes.extend_from_slice(&reinjection_token.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub fn decode_runtime_send_request(
    raw: &[u8],
) -> Result<RuntimeSendRequest<'_>, RuntimeSendDecodeError> {
    if raw.len() < RUNTIME_SEND_HEADER_LEN {
        return Err(RuntimeSendDecodeError::HeaderTooShort);
    }
    if raw[..4] != RUNTIME_SEND_MAGIC {
        return Err(RuntimeSendDecodeError::InvalidMagic);
    }

    let major = u16::from_le_bytes([raw[4], raw[5]]);
    let minor = u16::from_le_bytes([raw[6], raw[7]]);
    let version = ProtocolVersion { major, minor };
    if version != ProtocolVersion::CURRENT {
        return Err(RuntimeSendDecodeError::ProtocolVersionMismatch { major, minor });
    }

    let layer =
        Layer::from_wire(raw[8]).ok_or(RuntimeSendDecodeError::UnsupportedLayer(raw[8]))?;
    let reinjection_token = u64::from_le_bytes([
        raw[12], raw[13], raw[14], raw[15], raw[16], raw[17], raw[18], raw[19],
    ]);
    let payload_len = u32::from_le_bytes([raw[20], raw[21], raw[22], raw[23]]) as usize;
    let expected_len = RUNTIME_SEND_HEADER_LEN + payload_len;
    if raw.len() < expected_len {
        return Err(RuntimeSendDecodeError::TruncatedPayload {
            expected: expected_len,
            actual: raw.len(),
        });
    }

    Ok(RuntimeSendRequest {
        header: RuntimeSendRequestHeader {
            version,
            layer,
            reinjection_token,
            payload_len: payload_len as u32,
        },
        payload: &raw[RUNTIME_SEND_HEADER_LEN..expected_len],
    })
}

pub fn encode_socket_event_payload(kind: SocketEventKind, process_id: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; SOCKET_EVENT_PAYLOAD_LEN];
    let written = encode_socket_event_payload_into(kind, process_id, &mut bytes)
        .expect("exact socket payload buffer should encode");
    debug_assert_eq!(written, bytes.len());
    bytes
}

pub fn encode_socket_event_payload_into(
    kind: SocketEventKind,
    process_id: u64,
    output: &mut [u8],
) -> Result<usize, EncodeIntoError> {
    ensure_capacity(output, SOCKET_EVENT_PAYLOAD_LEN)?;
    output[0..4].copy_from_slice(&kind.code().to_le_bytes());
    output[4..8].copy_from_slice(&0u32.to_le_bytes());
    output[8..16].copy_from_slice(&process_id.to_le_bytes());
    Ok(SOCKET_EVENT_PAYLOAD_LEN)
}

pub fn decode_socket_event_payload(
    raw: &[u8],
) -> Result<SocketEventPayload, RuntimeEventDecodeError> {
    if raw.len() != SOCKET_EVENT_PAYLOAD_LEN {
        return Err(RuntimeEventDecodeError::InvalidSocketPayloadLength(raw.len()));
    }

    let kind_code = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let kind = SocketEventKind::from_code(kind_code)
        .ok_or(RuntimeEventDecodeError::UnsupportedSocketEventKind(kind_code))?;
    let process_id = u64::from_le_bytes([
        raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15],
    ]);
    Ok(SocketEventPayload::new(kind, process_id))
}

pub fn encode_flow_event_payload(kind: FlowEventKind, flow_id: u64, process_id: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; FLOW_EVENT_PAYLOAD_LEN];
    let written = encode_flow_event_payload_into(kind, flow_id, process_id, &mut bytes)
        .expect("exact flow payload buffer should encode");
    debug_assert_eq!(written, bytes.len());
    bytes
}

pub fn encode_flow_event_payload_into(
    kind: FlowEventKind,
    flow_id: u64,
    process_id: u64,
    output: &mut [u8],
) -> Result<usize, EncodeIntoError> {
    ensure_capacity(output, FLOW_EVENT_PAYLOAD_LEN)?;
    output[0..4].copy_from_slice(&kind.code().to_le_bytes());
    output[4..8].copy_from_slice(&0u32.to_le_bytes());
    output[8..16].copy_from_slice(&flow_id.to_le_bytes());
    output[16..24].copy_from_slice(&process_id.to_le_bytes());
    Ok(FLOW_EVENT_PAYLOAD_LEN)
}

fn ensure_capacity(output: &[u8], required: usize) -> Result<(), EncodeIntoError> {
    if output.len() < required {
        return Err(EncodeIntoError::BufferTooSmall {
            required,
            provided: output.len(),
        });
    }

    Ok(())
}

pub fn decode_flow_event_payload(raw: &[u8]) -> Result<FlowEventPayload, RuntimeEventDecodeError> {
    if raw.len() != FLOW_EVENT_PAYLOAD_LEN {
        return Err(RuntimeEventDecodeError::InvalidFlowPayloadLength(raw.len()));
    }

    let kind_code = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let kind = FlowEventKind::from_code(kind_code)
        .ok_or(RuntimeEventDecodeError::UnsupportedFlowEventKind(kind_code))?;
    let flow_id = u64::from_le_bytes([
        raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15],
    ]);
    let process_id = u64::from_le_bytes([
        raw[16], raw[17], raw[18], raw[19], raw[20], raw[21], raw[22], raw[23],
    ]);
    Ok(FlowEventPayload::new(kind, flow_id, process_id))
}
