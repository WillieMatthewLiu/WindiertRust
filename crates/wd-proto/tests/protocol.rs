use wd_proto::{CapabilityFlags, Layer, ProtocolVersion};

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
