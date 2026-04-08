use wd_proto::{Layer, OpenResponse, ProtocolVersion};
use wd_user::{ChecksumUpdate, DynamicHandle, HandleConfig, RecvEvent};

#[test]
fn handle_config_compiles_filter_before_open() {
    let cfg = HandleConfig::network("tcp and inbound").unwrap();
    assert_eq!(cfg.layer(), Layer::Network);
    assert!(!cfg.filter_ir().is_empty());
}

#[test]
fn decode_network_event_and_apply_checksum_fix() {
    let raw = wd_user::test_support::network_frame_bytes();
    let mut event = RecvEvent::decode(&raw).unwrap();
    let change = event.packet_mut().unwrap().set_ipv4_ttl(31);
    assert_eq!(change, ChecksumUpdate::Dirty);
    event.repair_checksums().unwrap();
}

#[test]
fn negotiated_capabilities_are_exposed_to_callers() {
    let handle = wd_user::test_support::opened_handle(OpenResponse::success(0x1f));
    assert_eq!(handle.capabilities_bits(), 0x1f);
}

#[test]
fn handle_config_uses_stable_machine_encoding() {
    let a = HandleConfig::network("tcp and inbound").unwrap();
    let b = HandleConfig::network("tcp and inbound").unwrap();

    assert_eq!(a.filter_ir(), b.filter_ir());
    assert!(a.filter_ir().starts_with(b"WDIR\x01"));
}

#[test]
fn handle_config_filter_ir_decodes_via_shared_codec() {
    let cfg = HandleConfig::network("tcp and inbound").unwrap();
    let decoded = wd_filter::decode_ir(cfg.filter_ir()).unwrap();

    assert_eq!(decoded, wd_filter::compile("tcp and inbound").unwrap());
}

#[test]
fn handle_config_network_rejects_reflect_only_filter() {
    assert!(HandleConfig::network("event == OPEN").is_err());
}

#[test]
fn repair_checksums_recomputes_ipv4_checksum_bytes() {
    let raw = wd_user::test_support::network_frame_bytes();
    let mut event = RecvEvent::decode(&raw).unwrap();
    let before = event.packet().unwrap().bytes();
    let before_checksum = u16::from_be_bytes([before[10], before[11]]);

    let change = event.packet_mut().unwrap().set_ipv4_ttl(31);
    assert_eq!(change, ChecksumUpdate::Dirty);
    event.repair_checksums().unwrap();

    let after = event.packet().unwrap().bytes();
    assert_eq!(after[8], 31);
    let after_checksum = u16::from_be_bytes([after[10], after[11]]);
    let expected = ipv4_checksum_with_zeroed_header_checksum(&after[..20]);
    assert_eq!(after_checksum, expected);
    assert_ne!(after_checksum, before_checksum);
}

#[test]
fn decode_rejects_too_short_frame() {
    assert!(RecvEvent::decode(&[0u8; 8]).is_err());
}

#[test]
fn open_response_with_nonzero_status_is_rejected() {
    let response = OpenResponse {
        version: ProtocolVersion::CURRENT,
        capabilities: 0x1f,
        status: 1,
    };
    assert!(DynamicHandle::from_open_response(response).is_err());
}

fn ipv4_checksum_with_zeroed_header_checksum(header: &[u8]) -> u16 {
    let mut bytes = header.to_vec();
    bytes[10] = 0;
    bytes[11] = 0;
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        sum = sum.wrapping_add(u16::from_be_bytes([bytes[i], bytes[i + 1]]) as u32);
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
