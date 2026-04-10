use std::fmt::{Display, Formatter};

use wd_filter::{FilterIr, LayerMask, OpCode};
use wd_proto::Layer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    SocketConnect,
    ReflectOpen,
    ReflectClose,
    FlowEstablished,
}

impl EventKind {
    const fn code(self) -> u64 {
        match self {
            Self::ReflectOpen => 1,
            Self::SocketConnect => 2,
            Self::ReflectClose => 3,
            Self::FlowEstablished => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverEvent {
    NetworkPacket { layer: Layer, protocol: u8 },
    SocketConnect { process_id: u64 },
    ReflectOpen { handle_id: u64 },
    ReflectClose { handle_id: u64 },
    FlowEstablished { flow_id: u64, process_id: u64 },
}

impl DriverEvent {
    pub const fn network_packet(layer: Layer, protocol: u8) -> Self {
        Self::NetworkPacket { layer, protocol }
    }

    pub const fn socket_connect(process_id: u64) -> Self {
        Self::SocketConnect { process_id }
    }

    pub const fn reflect_open(handle_id: u64) -> Self {
        Self::ReflectOpen { handle_id }
    }

    pub const fn reflect_close(handle_id: u64) -> Self {
        Self::ReflectClose { handle_id }
    }

    pub const fn flow_established(flow_id: u64, process_id: u64) -> Self {
        Self::FlowEstablished {
            flow_id,
            process_id,
        }
    }

    const fn layer(self) -> Layer {
        match self {
            Self::NetworkPacket { layer, .. } => layer,
            Self::SocketConnect { .. } => Layer::Socket,
            Self::ReflectOpen { .. } | Self::ReflectClose { .. } => Layer::Reflect,
            Self::FlowEstablished { .. } => Layer::Flow,
        }
    }

    const fn event_kind(self) -> EventKind {
        match self {
            Self::NetworkPacket { .. } => EventKind::ReflectOpen,
            Self::SocketConnect { .. } => EventKind::SocketConnect,
            Self::ReflectOpen { .. } => EventKind::ReflectOpen,
            Self::ReflectClose { .. } => EventKind::ReflectClose,
            Self::FlowEstablished { .. } => EventKind::FlowEstablished,
        }
    }

    const fn process_id(self) -> Option<u64> {
        match self {
            Self::NetworkPacket { .. } => None,
            Self::SocketConnect { process_id } => Some(process_id),
            Self::FlowEstablished { process_id, .. } => Some(process_id),
            Self::ReflectOpen { .. } | Self::ReflectClose { .. } => None,
        }
    }

    pub const fn flow_id(self) -> Option<u64> {
        match self {
            Self::FlowEstablished { flow_id, .. } => Some(flow_id),
            Self::NetworkPacket { .. }
            | Self::SocketConnect { .. }
            | Self::ReflectOpen { .. }
            | Self::ReflectClose { .. } => {
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterEngine {
    layer: Layer,
    ir: FilterIr,
}

impl FilterEngine {
    pub fn compile(layer: Layer, input: &str) -> Result<Self, FilterCompileError> {
        let ir = wd_filter::compile(input).map_err(FilterCompileError::from_display)?;
        let bytes = wd_filter::encode_ir(&ir);
        Self::from_ir_bytes(layer, &bytes)
    }

    pub fn from_ir_bytes(layer: Layer, input: &[u8]) -> Result<Self, FilterCompileError> {
        let ir = wd_filter::decode_ir(input).map_err(FilterCompileError::from_display)?;
        Self::from_ir(layer, ir)
    }

    fn from_ir(layer: Layer, ir: FilterIr) -> Result<Self, FilterCompileError> {
        validate_ir(layer, &ir)?;
        Ok(Self { layer, ir })
    }

    pub fn matches(&self, event: &DriverEvent) -> bool {
        if event.layer() != self.layer {
            return false;
        }
        evaluate_program(&self.ir.program, *event).unwrap_or(false)
    }

    pub fn matches_network_packet(&self, layer: Layer, packet: &[u8]) -> bool {
        if !matches!(self.layer, Layer::Network | Layer::NetworkForward) || layer != self.layer {
            return false;
        }
        evaluate_network_program(&self.ir.program, layer, packet).unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterCompileError {
    msg: String,
}

impl FilterCompileError {
    fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }

    fn from_display(err: impl Display) -> Self {
        Self::new(err.to_string())
    }
}

impl Display for FilterCompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for FilterCompileError {}

fn validate_ir(layer: Layer, ir: &FilterIr) -> Result<(), FilterCompileError> {
    if !matches!(
        layer,
        Layer::Network | Layer::NetworkForward | Layer::Socket | Layer::Reflect | Layer::Flow
    ) {
        return Err(FilterCompileError::new(format!(
            "unsupported layer for FilterEngine: {:?}",
            layer
        )));
    }

    let selected_layer = layer_mask(layer);
    if ir.required_layers != LayerMask::empty() && !ir.required_layers.contains(selected_layer) {
        return Err(FilterCompileError::new(format!(
            "incompatible filter for {:?}: requires different layer",
            layer
        )));
    }
    if ir.needs_payload && !matches!(layer, Layer::Network | Layer::NetworkForward) {
        return Err(FilterCompileError::new(
            "payload predicates are not supported by FilterEngine",
        ));
    }

    for field in &ir.referenced_fields {
        validate_supported_field(layer, field, None)?;
    }

    for op in &ir.program {
        match op {
            OpCode::FieldTest { field, value } => {
                validate_supported_field(layer, field, Some(*value))?;
            }
            OpCode::PacketLoad32 { .. } | OpCode::PacketLoad16 { .. } | OpCode::PacketLoad8 { .. }
                if !matches!(layer, Layer::Network | Layer::NetworkForward) =>
            {
                return Err(FilterCompileError::new(
                    "unsupported opcode for FilterEngine runtime subset: packet load",
                ));
            }
            OpCode::PacketLoad32 { .. } | OpCode::PacketLoad16 { .. } | OpCode::PacketLoad8 { .. } => {}
            OpCode::And | OpCode::Or | OpCode::Not => {}
        }
    }

    validate_program_shape(&ir.program)?;
    Ok(())
}

fn validate_supported_field(
    layer: Layer,
    field: &str,
    value: Option<u64>,
) -> Result<(), FilterCompileError> {
    match (layer, field, value) {
        (Layer::Network | Layer::NetworkForward, "packet", _) => Ok(()),
        (Layer::Network | Layer::NetworkForward, "tcp" | "udp" | "ipv4" | "ipv6", _) => Ok(()),
        (Layer::Network | Layer::NetworkForward, "localAddr" | "remoteAddr", _) => Ok(()),
        (Layer::Network | Layer::NetworkForward, "localPort" | "remotePort", _) => Ok(()),
        (Layer::Network | Layer::NetworkForward, "inbound" | "outbound", _) => Ok(()),
        (Layer::Network, "layer", Some(v)) if v != layer_value(Layer::Network) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Network: layer",
            ))
        }
        (Layer::NetworkForward, "layer", Some(v)) if v != layer_value(Layer::NetworkForward) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer NetworkForward: layer",
            ))
        }
        (Layer::Network | Layer::NetworkForward, "layer", _) => Ok(()),
        (Layer::Socket, "event", Some(v)) if v != EventKind::SocketConnect.code() => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Socket: event",
            ))
        }
        (Layer::Flow, "event", Some(v)) if v != EventKind::FlowEstablished.code() => Err(
            FilterCompileError::new(
                "unsupported field/value combination for layer Flow: event",
            ),
        ),
        (Layer::Reflect, "event", Some(v))
            if v != EventKind::ReflectOpen.code() && v != EventKind::ReflectClose.code() =>
        {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Reflect: event",
            ))
        }
        (Layer::Socket, "event", _) | (Layer::Reflect, "event", _) | (Layer::Flow, "event", _) => {
            Ok(())
        }
        (Layer::Socket, "processId", _) | (Layer::Flow, "processId", _) => Ok(()),
        (Layer::Reflect, "processId", _) => Err(FilterCompileError::new(
            "unsupported field for layer Reflect: processId",
        )),
        (Layer::Socket, "layer", Some(v)) if v != layer_value(Layer::Socket) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Socket: layer",
            ))
        }
        (Layer::Flow, "layer", Some(v)) if v != layer_value(Layer::Flow) => Err(
            FilterCompileError::new(
                "unsupported field/value combination for layer Flow: layer",
            ),
        ),
        (Layer::Reflect, "layer", Some(v)) if v != layer_value(Layer::Reflect) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Reflect: layer",
            ))
        }
        (Layer::Socket, "layer", _)
        | (Layer::Reflect, "layer", _)
        | (Layer::Flow, "layer", _) => Ok(()),
        (_, "tcp" | "udp" | "ipv4" | "ipv6", _)
        | (_, "localAddr" | "remoteAddr", _)
        | (_, "localPort" | "remotePort", _)
        | (_, "inbound" | "outbound", _) => Err(FilterCompileError::new(format!(
            "unsupported field for FilterEngine runtime subset: {field}",
        ))),
        (_, other, _) => Err(FilterCompileError::new(format!(
            "unsupported field for FilterEngine runtime subset: {other}",
        ))),
    }
}

fn validate_program_shape(program: &[OpCode]) -> Result<(), FilterCompileError> {
    if program.is_empty() {
        return Err(FilterCompileError::new("filter program is empty"));
    }

    let mut depth = 0usize;
    for op in program {
        match op {
            OpCode::FieldTest { .. }
            | OpCode::PacketLoad32 { .. }
            | OpCode::PacketLoad16 { .. }
            | OpCode::PacketLoad8 { .. } => {
                depth += 1;
            }
            OpCode::Not => {
                if depth < 1 {
                    return Err(FilterCompileError::new("invalid filter program"));
                }
            }
            OpCode::And | OpCode::Or => {
                if depth < 2 {
                    return Err(FilterCompileError::new("invalid filter program"));
                }
                depth -= 1;
            }
        }
    }

    if depth != 1 {
        return Err(FilterCompileError::new("invalid filter program"));
    }

    Ok(())
}

fn evaluate_program(program: &[OpCode], event: DriverEvent) -> Option<bool> {
    let mut stack = Vec::with_capacity(program.len());
    for op in program {
        match op {
            OpCode::FieldTest { field, value } => stack.push(eval_field_test(event, field, *value)),
            OpCode::PacketLoad32 { .. } | OpCode::PacketLoad16 { .. } | OpCode::PacketLoad8 { .. } => {
                return None;
            }
            OpCode::And => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs && rhs);
            }
            OpCode::Or => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs || rhs);
            }
            OpCode::Not => {
                let value = stack.pop()?;
                stack.push(!value);
            }
        }
    }

    let result = stack.pop()?;
    if stack.is_empty() {
        Some(result)
    } else {
        None
    }
}

fn evaluate_network_program(program: &[OpCode], layer: Layer, packet: &[u8]) -> Option<bool> {
    let mut stack = Vec::with_capacity(program.len());
    for op in program {
        match op {
            OpCode::FieldTest { field, value } => {
                stack.push(eval_network_field_test(layer, packet, field, *value))
            }
            OpCode::PacketLoad32 { offset, value } => {
                stack.push(load_packet_u32(packet, *offset) == Some(*value))
            }
            OpCode::PacketLoad16 { offset, value } => {
                stack.push(load_packet_u16(packet, *offset) == Some(*value))
            }
            OpCode::PacketLoad8 { offset, value } => {
                stack.push(load_packet_u8(packet, *offset) == Some(*value))
            }
            OpCode::And => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs && rhs);
            }
            OpCode::Or => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs || rhs);
            }
            OpCode::Not => {
                let value = stack.pop()?;
                stack.push(!value);
            }
        }
    }

    let result = stack.pop()?;
    if stack.is_empty() {
        Some(result)
    } else {
        None
    }
}

fn eval_field_test(event: DriverEvent, field: &str, value: u64) -> bool {
    match field {
        "event" => event.event_kind().code() == value,
        "processId" => event.process_id() == Some(value),
        "layer" => layer_value(event.layer()) == value,
        "tcp" | "udp" | "ipv4" | "ipv6" | "localAddr" | "remoteAddr" | "localPort" | "remotePort" | "inbound" | "outbound" => false,
        _ => false,
    }
}

fn eval_network_field_test(layer: Layer, packet: &[u8], field: &str, value: u64) -> bool {
    match field {
        "tcp" => u64::from(network_protocol(packet) == Some(6)) == value,
        "udp" => u64::from(network_protocol(packet) == Some(17)) == value,
        "ipv4" => u64::from(network_ip_version(packet) == Some(4)) == value,
        "ipv6" => u64::from(network_ip_version(packet) == Some(6)) == value,
        "localAddr" => network_local_remote_ipv4_addrs(layer, packet)
            .map(|(local, _)| ipv4_match(local, value))
            .unwrap_or(false),
        "remoteAddr" => network_local_remote_ipv4_addrs(layer, packet)
            .map(|(_, remote)| ipv4_match(remote, value))
            .unwrap_or(false),
        "localPort" => network_local_remote_ports(layer, packet)
            .map(|(local, _)| u64::from(local) == value)
            .unwrap_or(false),
        "remotePort" => network_local_remote_ports(layer, packet)
            .map(|(_, remote)| u64::from(remote) == value)
            .unwrap_or(false),
        "inbound" => u64::from(matches!(layer, Layer::Network)) == value,
        "outbound" => u64::from(matches!(layer, Layer::NetworkForward)) == value,
        "layer" => layer_value(layer) == value,
        _ => false,
    }
}

fn network_protocol(packet: &[u8]) -> Option<u8> {
    match network_ip_version(packet)? {
        4 if packet.len() > 9 => Some(packet[9]),
        6 if packet.len() > 6 => Some(packet[6]),
        _ => None,
    }
}

fn network_ip_version(packet: &[u8]) -> Option<u8> {
    packet.first().map(|byte| byte >> 4)
}

fn load_packet_u8(packet: &[u8], offset: u16) -> Option<u8> {
    packet.get(offset as usize).copied()
}

fn load_packet_u16(packet: &[u8], offset: u16) -> Option<u16> {
    let start = offset as usize;
    let bytes = packet.get(start..start + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn load_packet_u32(packet: &[u8], offset: u16) -> Option<u32> {
    let start = offset as usize;
    let bytes = packet.get(start..start + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn network_local_remote_ipv4_addrs(layer: Layer, packet: &[u8]) -> Option<(u32, u32)> {
    let (src, dst) = ipv4_addrs(packet)?;
    match layer {
        Layer::Network => Some((dst, src)),
        Layer::NetworkForward => Some((src, dst)),
        Layer::Socket | Layer::Flow | Layer::Reflect => None,
    }
}

fn network_local_remote_ports(layer: Layer, packet: &[u8]) -> Option<(u16, u16)> {
    let (src, dst) = transport_ports(packet)?;
    match layer {
        Layer::Network => Some((dst, src)),
        Layer::NetworkForward => Some((src, dst)),
        Layer::Socket | Layer::Flow | Layer::Reflect => None,
    }
}

fn transport_ports(packet: &[u8]) -> Option<(u16, u16)> {
    match network_ip_version(packet)? {
        4 => {
            let header_len = usize::from(packet.first()? & 0x0f) * 4;
            let protocol = *packet.get(9)?;
            if !matches!(protocol, 6 | 17) {
                return None;
            }
            let bytes = packet.get(header_len..header_len + 4)?;
            Some((
                u16::from_be_bytes([bytes[0], bytes[1]]),
                u16::from_be_bytes([bytes[2], bytes[3]]),
            ))
        }
        6 => {
            let protocol = *packet.get(6)?;
            if !matches!(protocol, 6 | 17) {
                return None;
            }
            let bytes = packet.get(40..44)?;
            Some((
                u16::from_be_bytes([bytes[0], bytes[1]]),
                u16::from_be_bytes([bytes[2], bytes[3]]),
            ))
        }
        _ => None,
    }
}

fn ipv4_addrs(packet: &[u8]) -> Option<(u32, u32)> {
    if network_ip_version(packet)? != 4 {
        return None;
    }
    let src = packet.get(12..16)?;
    let dst = packet.get(16..20)?;
    Some((
        u32::from_be_bytes([src[0], src[1], src[2], src[3]]),
        u32::from_be_bytes([dst[0], dst[1], dst[2], dst[3]]),
    ))
}

fn ipv4_match(actual: u32, encoded: u64) -> bool {
    let prefix = ((encoded >> 32) & 0xff) as u8;
    let expected = encoded as u32;
    let mask = ipv4_prefix_mask(prefix);
    (actual & mask) == expected
}

fn ipv4_prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    }
}

const fn layer_mask(layer: Layer) -> LayerMask {
    match layer {
        Layer::Network => LayerMask::NETWORK,
        Layer::NetworkForward => LayerMask::NETWORK_FORWARD,
        Layer::Flow => LayerMask::FLOW,
        Layer::Socket => LayerMask::SOCKET,
        Layer::Reflect => LayerMask::REFLECT,
    }
}

const fn layer_value(layer: Layer) -> u64 {
    match layer {
        Layer::Network => 1,
        Layer::NetworkForward => 2,
        Layer::Flow => 3,
        Layer::Socket => 4,
        Layer::Reflect => 5,
    }
}
