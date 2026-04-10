use wd_filter::{compile, encode_ir, FilterIr, LayerMask, OpCode};
use wd_kmdf::{DriverEvent, FilterEngine};
use wd_proto::Layer;

fn ipv4_tcp_packet() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1, 2,
        2, 2, 2,
    ]
}

fn ipv4_udp_packet() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x11, 0xaa, 0xbb, 1, 1, 1, 1, 2,
        2, 2, 2,
    ]
}

fn ipv4_tcp_packet_with_ports(src_port: u16, dst_port: u16) -> Vec<u8> {
    vec![
        0x45,
        0x00,
        0x00,
        0x28,
        0x12,
        0x34,
        0x00,
        0x00,
        0x40,
        0x06,
        0xaa,
        0xbb,
        1,
        1,
        1,
        1,
        2,
        2,
        2,
        2,
        (src_port >> 8) as u8,
        src_port as u8,
        (dst_port >> 8) as u8,
        dst_port as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0x50,
        0x00,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
}

fn ipv6_tcp_packet_with_ports(src_port: u16, dst_port: u16) -> Vec<u8> {
    vec![
        0x60,
        0x00,
        0x00,
        0x00,
        0x00,
        0x14,
        0x06,
        0x40,
        0x20,
        0x01,
        0x0d,
        0xb8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        1,
        0x20,
        0x01,
        0x0d,
        0xb8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        2,
        (src_port >> 8) as u8,
        src_port as u8,
        (dst_port >> 8) as u8,
        dst_port as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0x50,
        0x00,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
}

#[test]
fn filter_engine_compile_helper_matches_socket_connect_for_process() {
    let engine = FilterEngine::compile(
        Layer::Socket,
        "event == CONNECT and processId == 7",
    )
    .expect("filter compile should succeed");

    assert!(engine.matches(&DriverEvent::socket_connect(7)));
}

#[test]
fn filter_engine_from_ir_bytes_matches_socket_connect_for_process() {
    let ir = compile("event == CONNECT and processId == 7").expect("compile should succeed");
    let bytes = encode_ir(&ir);
    let engine = FilterEngine::from_ir_bytes(Layer::Socket, &bytes).expect("ir load should succeed");

    assert!(engine.matches(&DriverEvent::socket_connect(7)));
}

#[test]
fn filter_engine_rejects_incompatible_socket_event() {
    let ir = compile("event == OPEN").expect("compile should succeed");
    let bytes = encode_ir(&ir);
    let err = FilterEngine::from_ir_bytes(Layer::Socket, &bytes).expect_err("ir load should fail");

    assert!(err.to_string().contains("incompatible"));
}

#[test]
fn filter_engine_matches_network_tcp_inbound_subset() {
    let engine = FilterEngine::compile(Layer::Network, "tcp and inbound")
        .expect("network filter compile should succeed");

    assert!(engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
    assert!(!engine.matches_network_packet(Layer::Network, &ipv4_udp_packet()));
    assert!(!engine.matches_network_packet(Layer::NetworkForward, &ipv4_tcp_packet()));
}

#[test]
fn filter_engine_accepts_tcp_filter_for_network_forward_layer() {
    let engine = FilterEngine::compile(Layer::NetworkForward, "tcp")
        .expect("network forward tcp filter should compile");

    assert!(engine.matches_network_packet(Layer::NetworkForward, &ipv4_tcp_packet()));
    assert!(!engine.matches_network_packet(Layer::NetworkForward, &ipv4_udp_packet()));
    assert!(!engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
}

#[test]
fn filter_engine_accepts_udp_filter_for_network_forward_layer() {
    let engine = FilterEngine::compile(Layer::NetworkForward, "udp")
        .expect("network forward udp filter should compile");

    assert!(engine.matches_network_packet(Layer::NetworkForward, &ipv4_udp_packet()));
    assert!(!engine.matches_network_packet(Layer::NetworkForward, &ipv4_tcp_packet()));
    assert!(!engine.matches_network_packet(Layer::Network, &ipv4_udp_packet()));
}

#[test]
fn filter_engine_accepts_outbound_filter_for_network_forward_layer() {
    let engine = FilterEngine::compile(Layer::NetworkForward, "outbound")
        .expect("network forward outbound filter should compile");

    assert!(engine.matches_network_packet(Layer::NetworkForward, &ipv4_tcp_packet()));
    assert!(!engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
}

#[test]
fn filter_engine_matches_network_local_and_remote_port_fields() {
    let packet = ipv4_tcp_packet_with_ports(12345, 443);
    let inbound_engine =
        FilterEngine::compile(Layer::Network, "tcp and localPort == 443 and remotePort == 12345")
            .expect("inbound port filter should compile");
    let outbound_engine = FilterEngine::compile(
        Layer::NetworkForward,
        "tcp and localPort == 12345 and remotePort == 443",
    )
    .expect("outbound port filter should compile");

    assert!(inbound_engine.matches_network_packet(Layer::Network, &packet));
    assert!(!inbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(outbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(!outbound_engine.matches_network_packet(Layer::Network, &packet));
}

#[test]
fn filter_engine_matches_ipv4_and_ipv6_version_symbols() {
    let ipv4_engine = FilterEngine::compile(Layer::Network, "ipv4")
        .expect("ipv4 filter should compile");
    let ipv6_engine = FilterEngine::compile(Layer::NetworkForward, "ipv6 and tcp")
        .expect("ipv6 filter should compile");
    let ipv6_packet = ipv6_tcp_packet_with_ports(12345, 443);

    assert!(ipv4_engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
    assert!(!ipv4_engine.matches_network_packet(Layer::Network, &ipv6_packet));
    assert!(ipv6_engine.matches_network_packet(Layer::NetworkForward, &ipv6_packet));
    assert!(!ipv6_engine.matches_network_packet(Layer::NetworkForward, &ipv4_tcp_packet()));
}

#[test]
fn filter_engine_matches_network_local_and_remote_ipv4_address_fields() {
    let packet = ipv4_tcp_packet();
    let inbound_engine = FilterEngine::compile(
        Layer::Network,
        "localAddr == 2.2.2.2 and remoteAddr == 1.1.1.1",
    )
    .expect("inbound address filter should compile");
    let outbound_engine = FilterEngine::compile(
        Layer::NetworkForward,
        "localAddr == 1.1.1.1 and remoteAddr == 2.2.2.2",
    )
    .expect("outbound address filter should compile");

    assert!(inbound_engine.matches_network_packet(Layer::Network, &packet));
    assert!(!inbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(outbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(!outbound_engine.matches_network_packet(Layer::Network, &packet));
}

#[test]
fn filter_engine_matches_network_local_and_remote_ipv4_cidr_fields() {
    let packet = ipv4_tcp_packet();
    let inbound_engine = FilterEngine::compile(
        Layer::Network,
        "localAddr == 2.2.2.0/24 and remoteAddr == 1.1.1.0/24",
    )
    .expect("inbound cidr filter should compile");
    let outbound_engine = FilterEngine::compile(
        Layer::NetworkForward,
        "localAddr == 1.1.1.0/24 and remoteAddr == 2.2.2.0/24",
    )
    .expect("outbound cidr filter should compile");

    assert!(inbound_engine.matches_network_packet(Layer::Network, &packet));
    assert!(!inbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(outbound_engine.matches_network_packet(Layer::NetworkForward, &packet));
    assert!(!outbound_engine.matches_network_packet(Layer::Network, &packet));
}

#[test]
fn filter_engine_matches_network_packet_byte_and_dword_access() {
    let byte_engine = FilterEngine::compile(Layer::Network, "packet[9] == 6")
        .expect("packet byte filter compile should succeed");
    let word_engine = FilterEngine::compile(Layer::Network, "packet16[10] == 0xaabb")
        .expect("packet word filter compile should succeed");
    let dword_engine = FilterEngine::compile(Layer::Network, "packet32[8] == 0x4006aabb")
        .expect("packet dword filter compile should succeed");
    let mut word_mismatch_packet = ipv4_tcp_packet();
    word_mismatch_packet[10] = 0xcc;

    assert!(byte_engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
    assert!(!byte_engine.matches_network_packet(Layer::Network, &ipv4_udp_packet()));
    assert!(word_engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
    assert!(!word_engine.matches_network_packet(Layer::Network, &word_mismatch_packet));
    assert!(dword_engine.matches_network_packet(Layer::Network, &ipv4_tcp_packet()));
    assert!(!dword_engine.matches_network_packet(Layer::Network, &ipv4_udp_packet()));
}

#[test]
fn filter_engine_rejects_incompatible_network_forward_inbound_filter() {
    let err = FilterEngine::compile(Layer::NetworkForward, "tcp and inbound")
        .expect_err("network forward scope should reject inbound-only network filter");

    assert!(err.to_string().contains("incompatible"));
}

#[test]
fn filter_engine_rejects_unsupported_field_test_in_supported_layer() {
    let ir = FilterIr {
        required_layers: LayerMask::SOCKET,
        needs_payload: false,
        referenced_fields: vec!["tcp"],
        program: vec![OpCode::FieldTest {
            field: "tcp",
            value: 1,
        }],
    };
    let bytes = encode_ir(&ir);

    let err = FilterEngine::from_ir_bytes(Layer::Socket, &bytes)
        .expect_err("unsupported field should be rejected");
    assert!(err.to_string().contains("unsupported field"));
}

#[test]
fn filter_engine_rejects_unsupported_opcode_in_supported_layer() {
    let ir = FilterIr {
        required_layers: LayerMask::SOCKET,
        needs_payload: false,
        referenced_fields: vec!["event"],
        program: vec![OpCode::PacketLoad8 {
            offset: 0,
            value: 1,
        }],
    };
    let bytes = encode_ir(&ir);

    let err = FilterEngine::from_ir_bytes(Layer::Socket, &bytes)
        .expect_err("unsupported opcode should be rejected");
    assert!(err.to_string().contains("unsupported opcode"));
}

#[test]
fn filter_engine_compile_helper_matches_flow_event_for_process() {
    let engine = FilterEngine::compile(Layer::Flow, "layer == FLOW and processId == 42")
        .expect("flow filter compile should succeed");

    assert!(engine.matches(&DriverEvent::flow_established(0xfeed, 42)));
}

#[test]
fn filter_engine_rejects_incompatible_flow_layer_scope() {
    let err = FilterEngine::compile(Layer::Flow, "layer == SOCKET")
        .expect_err("flow scope should reject socket layer filters");

    assert!(err.to_string().contains("incompatible"));
}
