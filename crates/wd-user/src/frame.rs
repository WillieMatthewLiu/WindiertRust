use wd_proto::{
    FlowEventPayload, Layer, RuntimeEventDecodeError, SocketEventPayload,
    decode_flow_event_payload, decode_network_event_payload, decode_runtime_event,
    decode_socket_event_payload,
};

use crate::{ChecksumUpdate, UserError};

#[derive(Debug, Clone)]
pub enum RecvEvent {
    Network(NetworkPacket),
    Socket(SocketEventPayload),
    Flow(FlowEventPayload),
}

impl RecvEvent {
    pub fn decode(raw: &[u8]) -> Result<Self, UserError> {
        match decode_runtime_event(raw) {
            Ok(frame) => match frame.header.layer {
                Layer::Network | Layer::NetworkForward => {
                    decode_runtime_network_packet(frame.header.layer, frame.payload)
                }
                Layer::Socket => Ok(Self::Socket(decode_socket_event_payload(frame.payload)?)),
                Layer::Flow => Ok(Self::Flow(decode_flow_event_payload(frame.payload)?)),
                Layer::Reflect => Err(UserError::InvalidFrame(
                    "reflect runtime events are not decoded yet",
                )),
            },
            Err(RuntimeEventDecodeError::InvalidMagic | RuntimeEventDecodeError::HeaderTooShort) => {
                decode_network_packet(raw)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn packet(&self) -> Option<&NetworkPacket> {
        match self {
            Self::Network(packet) => Some(packet),
            Self::Socket(_) | Self::Flow(_) => None,
        }
    }

    pub fn packet_mut(&mut self) -> Option<&mut NetworkPacket> {
        match self {
            Self::Network(packet) => Some(packet),
            Self::Socket(_) | Self::Flow(_) => None,
        }
    }

    pub fn socket(&self) -> Option<&SocketEventPayload> {
        match self {
            Self::Socket(event) => Some(event),
            Self::Network(_) | Self::Flow(_) => None,
        }
    }

    pub fn flow(&self) -> Option<&FlowEventPayload> {
        match self {
            Self::Flow(event) => Some(event),
            Self::Network(_) | Self::Socket(_) => None,
        }
    }

    pub fn repair_checksums(&mut self) -> Result<(), UserError> {
        match self {
            Self::Network(packet) => packet.repair_checksums(),
            Self::Socket(_) | Self::Flow(_) => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkPacket {
    layer: Layer,
    bytes: Vec<u8>,
    header_len: usize,
    checksum_dirty: bool,
    reinjection_token: Option<u64>,
}

impl NetworkPacket {
    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn reinjection_token(&self) -> Option<u64> {
        self.reinjection_token
    }

    pub fn set_ipv4_ttl(&mut self, ttl: u8) -> ChecksumUpdate {
        if self.bytes[8] == ttl {
            return ChecksumUpdate::Clean;
        }
        self.bytes[8] = ttl;
        self.checksum_dirty = true;
        ChecksumUpdate::Dirty
    }

    fn repair_checksums(&mut self) -> Result<(), UserError> {
        if self.header_len < 20 || self.bytes.len() < self.header_len {
            return Err(UserError::InvalidFrame("invalid ipv4 header length"));
        }
        if self.checksum_dirty {
            self.bytes[10] = 0;
            self.bytes[11] = 0;
            let checksum = ipv4_header_checksum(&self.bytes[..self.header_len]);
            self.bytes[10] = (checksum >> 8) as u8;
            self.bytes[11] = (checksum & 0xff) as u8;
            self.checksum_dirty = false;
        }
        Ok(())
    }
}

fn ipv4_header_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < header.len() {
        let word = u16::from_be_bytes([header[i], header[i + 1]]) as u32;
        sum = sum.wrapping_add(word);
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn decode_network_packet(raw: &[u8]) -> Result<RecvEvent, UserError> {
    decode_network_packet_with_metadata(raw, Layer::Network, None)
}

fn decode_runtime_network_packet(layer: Layer, raw: &[u8]) -> Result<RecvEvent, UserError> {
    match decode_network_event_payload(raw) {
        Ok(payload) => {
            decode_network_packet_with_metadata(payload.packet, layer, Some(payload.reinjection_token))
        }
        Err(RuntimeEventDecodeError::InvalidNetworkPayloadMagic)
        | Err(RuntimeEventDecodeError::NetworkPayloadHeaderTooShort) => decode_network_packet(raw),
        Err(err) => Err(err.into()),
    }
}

fn decode_network_packet_with_metadata(
    raw: &[u8],
    layer: Layer,
    reinjection_token: Option<u64>,
) -> Result<RecvEvent, UserError> {
    if raw.len() < 20 {
        return Err(UserError::InvalidFrame("frame too short for ipv4 header"));
    }
    let version = raw[0] >> 4;
    let ihl_words = raw[0] & 0x0f;
    if version != 4 || ihl_words < 5 {
        return Err(UserError::InvalidFrame("unsupported frame format"));
    }
    let header_len = usize::from(ihl_words) * 4;
    if raw.len() < header_len {
        return Err(UserError::InvalidFrame("frame shorter than ipv4 header length"));
    }
    Ok(RecvEvent::Network(NetworkPacket {
        layer,
        bytes: raw.to_vec(),
        header_len,
        checksum_dirty: false,
        reinjection_token,
    }))
}
