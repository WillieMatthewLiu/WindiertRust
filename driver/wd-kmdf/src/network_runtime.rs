use std::fmt::{Display, Formatter};

use wd_proto::{
    EncodeIntoError, Layer, ProtocolVersion, RUNTIME_EVENT_HEADER_LEN, RUNTIME_EVENT_MAGIC,
    RuntimeSendDecodeError, decode_runtime_send_request, encode_network_event_payload,
    encode_network_event_payload_into, encode_runtime_event,
};

use crate::{FixedPacket, FixedPacketError, ReinjectionError, ReinjectionTable};

pub const ACCEPTED_PACKET_BYTES: usize = 2048;
type AcceptedPacket = FixedPacket<ACCEPTED_PACKET_BYTES>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedReinjection {
    pub layer: Layer,
    pub packet_id: u64,
    pub packet: AcceptedPacket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkRuntime;

impl NetworkRuntime {
    pub fn issue_event_into(
        table: &mut ReinjectionTable,
        layer: Layer,
        packet_id: u64,
        packet: &[u8],
        output: &mut [u8],
    ) -> Result<usize, NetworkRuntimeError> {
        validate_network_layer(layer)?;
        let payload_len = 16 + packet.len();
        let required = RUNTIME_EVENT_HEADER_LEN + payload_len;
        if output.len() < required {
            return Err(NetworkRuntimeError::EncodeInto(
                EncodeIntoError::BufferTooSmall {
                    required,
                    provided: output.len(),
                },
            ));
        }

        let token = table.issue_for_network_packet(packet_id);
        output[0..4].copy_from_slice(&RUNTIME_EVENT_MAGIC);
        output[4..6].copy_from_slice(&ProtocolVersion::CURRENT.major.to_le_bytes());
        output[6..8].copy_from_slice(&ProtocolVersion::CURRENT.minor.to_le_bytes());
        output[8] = layer.to_wire();
        output[9..12].copy_from_slice(&[0u8; 3]);
        output[12..16].copy_from_slice(&(payload_len as u32).to_le_bytes());
        let payload_written =
            encode_network_event_payload_into(token.raw(), packet, &mut output[16..required])
                .map_err(NetworkRuntimeError::EncodeInto)?;
        debug_assert_eq!(payload_written, payload_len);
        Ok(required)
    }

    pub fn issue_event(
        table: &mut ReinjectionTable,
        layer: Layer,
        packet_id: u64,
        packet: &[u8],
    ) -> Result<Vec<u8>, NetworkRuntimeError> {
        validate_network_layer(layer)?;
        let token = table.issue_for_network_packet(packet_id);
        let payload = encode_network_event_payload(token.raw(), packet);
        Ok(encode_runtime_event(layer, &payload))
    }

    pub fn accept_send(
        table: &mut ReinjectionTable,
        raw: &[u8],
    ) -> Result<AcceptedReinjection, NetworkRuntimeError> {
        let request = decode_runtime_send_request(raw).map_err(NetworkRuntimeError::DecodeSend)?;
        validate_network_layer(request.header.layer)?;
        let packet =
            AcceptedPacket::copy_from_slice(request.payload).map_err(NetworkRuntimeError::PacketBuffer)?;
        let packet_id = table
            .consume_raw(request.header.reinjection_token)
            .map_err(NetworkRuntimeError::from)?;

        Ok(AcceptedReinjection {
            layer: request.header.layer,
            packet_id,
            packet,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkRuntimeError {
    UnsupportedLayer(Layer),
    EncodeInto(EncodeIntoError),
    PacketBuffer(FixedPacketError),
    DecodeSend(RuntimeSendDecodeError),
    UnknownToken,
}

impl Display for NetworkRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLayer(layer) => {
                write!(f, "network runtime does not support layer {:?}", layer)
            }
            Self::EncodeInto(err) => write!(f, "{err}"),
            Self::PacketBuffer(err) => write!(f, "{err}"),
            Self::DecodeSend(err) => write!(f, "{err}"),
            Self::UnknownToken => write!(f, "unknown reinjection token"),
        }
    }
}

impl std::error::Error for NetworkRuntimeError {}

impl From<ReinjectionError> for NetworkRuntimeError {
    fn from(value: ReinjectionError) -> Self {
        match value {
            ReinjectionError::UnknownToken => Self::UnknownToken,
        }
    }
}

fn validate_network_layer(layer: Layer) -> Result<(), NetworkRuntimeError> {
    match layer {
        Layer::Network | Layer::NetworkForward => Ok(()),
        Layer::Flow | Layer::Socket | Layer::Reflect => {
            Err(NetworkRuntimeError::UnsupportedLayer(layer))
        }
    }
}
