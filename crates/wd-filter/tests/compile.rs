use wd_filter::{compile, decode_ir, encode_ir, LayerMask, OpCode};

#[test]
fn compiles_boolean_logic_and_packet_access() {
    let ir = compile("tcp and inbound and packet32[0] == 0x12345678").expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.needs_payload);
    assert!(matches!(
        ir.program.first(),
        Some(OpCode::FieldTest { .. })
    ));
}

#[test]
fn compiles_packet16_access_and_roundtrips_shared_codec() {
    let ir = compile("tcp and packet16[10] == 0xaabb").expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.needs_payload);

    let encoded = encode_ir(&ir);
    let decoded = decode_ir(&encoded).expect("decode should pass");
    assert_eq!(decoded, ir);
}

#[test]
fn bare_tcp_symbol_does_not_pin_filter_to_network_only() {
    let ir = compile("tcp").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::empty());
    assert!(!ir.needs_payload);
    assert_eq!(ir.referenced_fields, vec!["tcp"]);
}

#[test]
fn bare_udp_symbol_does_not_pin_filter_to_network_only() {
    let ir = compile("udp").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::empty());
    assert!(!ir.needs_payload);
    assert_eq!(ir.referenced_fields, vec!["udp"]);
}

#[test]
fn bare_outbound_symbol_pins_filter_to_network_forward() {
    let ir = compile("outbound").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::NETWORK_FORWARD);
    assert!(!ir.needs_payload);
    assert_eq!(ir.referenced_fields, vec!["outbound"]);
}

#[test]
fn transport_port_fields_require_network_layers() {
    let ir = compile("localPort == 443 and remotePort == 12345").expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.required_layers.contains(LayerMask::NETWORK_FORWARD));
    assert!(!ir.needs_payload);
    assert!(ir.referenced_fields.contains(&"localPort"));
    assert!(ir.referenced_fields.contains(&"remotePort"));
}

#[test]
fn ip_version_symbols_require_network_layers() {
    let ir = compile("ipv4 or ipv6").expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.required_layers.contains(LayerMask::NETWORK_FORWARD));
    assert!(!ir.needs_payload);
    assert!(ir.referenced_fields.contains(&"ipv4"));
    assert!(ir.referenced_fields.contains(&"ipv6"));
}

#[test]
fn ipv4_address_fields_require_network_layers() {
    let ir = compile("localAddr == 2.2.2.2 and remoteAddr == 1.1.1.1")
        .expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.required_layers.contains(LayerMask::NETWORK_FORWARD));
    assert!(!ir.needs_payload);
    assert!(ir.referenced_fields.contains(&"localAddr"));
    assert!(ir.referenced_fields.contains(&"remoteAddr"));
}

#[test]
fn ipv4_cidr_address_fields_require_network_layers() {
    let ir = compile("localAddr == 2.2.2.0/24 and remoteAddr == 1.1.1.0/24")
        .expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.required_layers.contains(LayerMask::NETWORK_FORWARD));
    assert!(!ir.needs_payload);
    assert!(ir.referenced_fields.contains(&"localAddr"));
    assert!(ir.referenced_fields.contains(&"remoteAddr"));
}

#[test]
fn compiles_symbolic_fields_without_payload() {
    let ir = compile("event == OPEN and layer == NETWORK").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::REFLECT);
    assert!(!ir.needs_payload);
}

#[test]
fn rejects_packet_access_for_flow_layer() {
    let err = compile("layer == FLOW and packet[0] == 1").expect_err("compile should fail");

    assert!(err.to_string().contains("packet access is not valid for FLOW"));
}

#[test]
fn or_between_reflect_event_and_network_layer_requires_both_layers() {
    let ir = compile("event == OPEN or layer == NETWORK").expect("compile should pass");

    assert!(ir.required_layers.contains(LayerMask::REFLECT));
    assert!(ir.required_layers.contains(LayerMask::NETWORK));
}

#[test]
fn negated_reflect_event_does_not_suppress_network_layer_requirement() {
    let ir = compile("not (event == OPEN) and layer == NETWORK").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::NETWORK);
}

#[test]
fn compiles_socket_connect_process_filter_and_roundtrips_shared_codec() {
    let ir = compile("event == CONNECT and processId == 7").expect("compile should pass");

    assert_eq!(ir.required_layers, LayerMask::SOCKET);
    assert!(!ir.needs_payload);
    assert!(ir.referenced_fields.contains(&"event"));
    assert!(ir.referenced_fields.contains(&"processId"));

    let encoded = encode_ir(&ir);
    assert!(encoded.starts_with(b"WDIR\x01"));

    let decoded = decode_ir(&encoded).expect("decode should pass");
    assert_eq!(decoded, ir);
}

#[test]
fn decode_ir_rejects_excessive_referenced_field_count() {
    let mut bytes = wdir_header();
    push_u16(&mut bytes, 257);
    for _ in 0..257 {
        push_bytes(&mut bytes, b"event");
    }
    push_u32(&mut bytes, 1);
    bytes.push(6);

    let err = decode_ir(&bytes).expect_err("decode should reject oversized referenced field count");
    assert!(err.to_string().contains("too many referenced fields"));
}

#[test]
fn decode_ir_rejects_excessive_program_length() {
    let mut bytes = wdir_header();
    push_u16(&mut bytes, 0);
    push_u32(&mut bytes, 4097);
    bytes.resize(bytes.len() + 4097, 6);

    let err = decode_ir(&bytes).expect_err("decode should reject oversized program length");
    assert!(err.to_string().contains("program too long"));
}

#[test]
fn decode_ir_rejects_excessive_field_byte_length() {
    let mut bytes = wdir_header();
    push_u16(&mut bytes, 1);
    push_u16(&mut bytes, 33);
    bytes.resize(bytes.len() + 33, b'a');
    push_u32(&mut bytes, 1);
    bytes.push(6);

    let err = decode_ir(&bytes).expect_err("decode should reject oversized field length");
    assert!(err.to_string().contains("field byte length"));
}

fn wdir_header() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"WDIR");
    bytes.push(1);
    bytes.push(0);
    bytes.push(0);
    bytes
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    push_u16(out, bytes.len() as u16);
    out.extend_from_slice(bytes);
}
