use wd_filter::{compile, encode_ir};
use wd_proto::{Layer, OpenResponse, ProtocolVersion};

use crate::UserError;

#[derive(Debug, Clone)]
pub struct HandleConfig {
    layer: Layer,
    filter_ir: Vec<u8>,
}

impl HandleConfig {
    pub fn network(filter: &str) -> Result<Self, UserError> {
        let ir = compile(filter)?;
        validate_network_layer(ir.required_layers)?;
        Ok(Self {
            layer: Layer::Network,
            filter_ir: encode_ir(&ir),
        })
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn filter_ir(&self) -> &[u8] {
        &self.filter_ir
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DynamicHandle {
    capabilities: u32,
}

impl DynamicHandle {
    pub fn from_open_response(response: OpenResponse) -> Result<Self, UserError> {
        if response.status != 0 {
            return Err(UserError::OpenResponseStatus(response.status));
        }
        if response.version != ProtocolVersion::CURRENT {
            return Err(UserError::ProtocolVersionMismatch);
        }
        Ok(Self {
            capabilities: response.capabilities,
        })
    }

    pub fn capabilities_bits(&self) -> u32 {
        self.capabilities
    }
}

fn validate_network_layer(mask: wd_filter::LayerMask) -> Result<(), UserError> {
    let has_network = mask.contains(wd_filter::LayerMask::NETWORK);
    let has_other = mask.contains(wd_filter::LayerMask::NETWORK_FORWARD)
        || mask.contains(wd_filter::LayerMask::FLOW)
        || mask.contains(wd_filter::LayerMask::SOCKET)
        || mask.contains(wd_filter::LayerMask::REFLECT);
    if !has_network || has_other {
        return Err(UserError::IncompatibleLayer(
            "filter required layers are incompatible with Layer::Network",
        ));
    }
    Ok(())
}
