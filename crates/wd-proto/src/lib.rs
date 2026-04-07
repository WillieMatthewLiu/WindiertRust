use bitflags::bitflags;

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
