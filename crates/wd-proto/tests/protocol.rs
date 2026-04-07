use wd_driver_shared::{DEVICE_NAME, IOCTL_OPEN};
use wd_proto::{CapabilityFlags, Layer, OpenRequest, OpenResponse, ProtocolVersion};

#[test]
fn protocol_version_and_layers_match_phase_one_contract() {
    assert_eq!(ProtocolVersion::CURRENT.major, 0);
    assert_eq!(ProtocolVersion::CURRENT.minor, 1);
    assert_eq!(
        Layer::all(),
        [
            Layer::Network,
            Layer::NetworkForward,
            Layer::Flow,
            Layer::Socket,
            Layer::Reflect,
        ]
    );
    assert!(CapabilityFlags::CHECKSUM_RECALC.bits() != 0);
}

#[test]
fn open_request_has_stable_header_and_filter_bytes() {
    let request = OpenRequest::new(Layer::Network, "tcp and inbound".into(), 0, 0);
    assert_eq!(request.version, ProtocolVersion::CURRENT);
    assert_eq!(request.filter_len as usize, request.filter_ir.len());
    assert!(IOCTL_OPEN != 0);
    assert!(DEVICE_NAME.starts_with(r"\\Device\\"));
}

#[test]
fn open_response_exposes_capabilities() {
    let response = OpenResponse::success(0x1f);
    assert_eq!(response.version, ProtocolVersion::CURRENT);
    assert_eq!(response.capabilities, 0x1f);
}
