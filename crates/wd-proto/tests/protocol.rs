use wd_driver_shared::{DEVICE_NAME, IOCTL_OPEN};
use wd_proto::{
    CapabilityFlags, FlowEventKind, Layer, OpenRequest, OpenResponse, ProtocolVersion,
    EncodeIntoError,
    SocketEventKind, decode_network_event_payload, decode_open_request, decode_open_response,
    decode_runtime_event, decode_runtime_send_request, encode_flow_event_payload,
    encode_flow_event_payload_into,
    encode_network_event_payload, encode_network_event_payload_into, encode_open_request,
    encode_open_response, encode_open_response_into, encode_runtime_event,
    encode_runtime_event_into, encode_runtime_send_request, encode_socket_event_payload,
    encode_socket_event_payload_into,
};

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

#[test]
fn open_request_round_trips_machine_encoding() {
    let request = OpenRequest::new(Layer::Network, b"abc".to_vec(), -1000, 9);
    let raw = encode_open_request(&request);
    let decoded = decode_open_request(&raw).expect("open request should decode");

    assert_eq!(decoded, request);
}

#[test]
fn open_response_round_trips_machine_encoding() {
    let response = OpenResponse::success(0x1f);
    let raw = encode_open_response(response);
    let decoded = decode_open_response(&raw).expect("open response should decode");

    assert_eq!(decoded, response);
}

#[test]
fn open_response_into_writes_machine_encoding_without_allocating() {
    let response = OpenResponse::success(0x55);
    let mut output = [0u8; 12];

    let written = encode_open_response_into(response, &mut output)
        .expect("fixed buffer should fit open response");

    assert_eq!(written, output.len());
    let decoded = decode_open_response(&output[..written]).expect("open response should decode");
    assert_eq!(decoded, response);
}

#[test]
fn open_response_into_rejects_small_output_buffer() {
    let response = OpenResponse::success(0x55);
    let mut output = [0u8; 11];

    let err = encode_open_response_into(response, &mut output)
        .expect_err("short buffer should be rejected");

    assert_eq!(
        err,
        EncodeIntoError::BufferTooSmall {
            required: 12,
            provided: 11,
        }
    );
}

#[test]
fn runtime_event_header_round_trips_socket_payload() {
    let payload = encode_socket_event_payload(SocketEventKind::Connect, 42);
    let raw = encode_runtime_event(Layer::Socket, &payload);

    let frame = decode_runtime_event(&raw).expect("runtime event should decode");
    assert_eq!(frame.header.version, ProtocolVersion::CURRENT);
    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(frame.payload, payload.as_slice());
}

#[test]
fn runtime_event_header_round_trips_flow_payload() {
    let payload = encode_flow_event_payload(FlowEventKind::Established, 9001, 7);
    let raw = encode_runtime_event(Layer::Flow, &payload);

    let frame = decode_runtime_event(&raw).expect("runtime event should decode");
    assert_eq!(frame.header.version, ProtocolVersion::CURRENT);
    assert_eq!(frame.header.layer, Layer::Flow);
    assert_eq!(frame.payload, payload.as_slice());
}

#[test]
fn runtime_event_into_writes_header_and_payload_into_caller_buffer() {
    let payload = encode_socket_event_payload(SocketEventKind::Connect, 42);
    let mut output = [0u8; 32];

    let written = encode_runtime_event_into(Layer::Socket, &payload, &mut output)
        .expect("fixed buffer should fit runtime event");

    assert_eq!(written, 16 + payload.len());
    let frame = decode_runtime_event(&output[..written]).expect("runtime event should decode");
    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(frame.payload, payload.as_slice());
}

#[test]
fn runtime_event_into_rejects_small_output_buffer() {
    let payload = encode_socket_event_payload(SocketEventKind::Connect, 42);
    let mut output = [0u8; 16 + 15];

    let err = encode_runtime_event_into(Layer::Socket, &payload, &mut output)
        .expect_err("short buffer should be rejected");

    assert_eq!(
        err,
        EncodeIntoError::BufferTooSmall {
            required: 32,
            provided: 31,
        }
    );
}

#[test]
fn network_event_payload_round_trips_reinjection_token_and_packet() {
    let packet = [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1,
        2, 2, 2, 2,
    ];
    let payload = encode_network_event_payload(77, &packet);
    let decoded = decode_network_event_payload(&payload).expect("network payload should decode");

    assert_eq!(decoded.reinjection_token, 77);
    assert_eq!(decoded.packet, packet.as_slice());
}

#[test]
fn network_event_payload_into_writes_packet_into_caller_buffer() {
    let packet = [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1,
        2, 2, 2, 2,
    ];
    let mut output = [0u8; 36];

    let written = encode_network_event_payload_into(77, &packet, &mut output)
        .expect("fixed buffer should fit network payload");

    assert_eq!(written, output.len());
    let decoded =
        decode_network_event_payload(&output[..written]).expect("network payload should decode");
    assert_eq!(decoded.reinjection_token, 77);
    assert_eq!(decoded.packet, packet.as_slice());
}

#[test]
fn network_event_payload_into_rejects_small_output_buffer() {
    let packet = [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1,
        2, 2, 2, 2,
    ];
    let mut output = [0u8; 35];

    let err = encode_network_event_payload_into(77, &packet, &mut output)
        .expect_err("short buffer should be rejected");

    assert_eq!(
        err,
        EncodeIntoError::BufferTooSmall {
            required: 36,
            provided: 35,
        }
    );
}

#[test]
fn runtime_send_request_round_trips_network_packet_and_token() {
    let packet = [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1,
        2, 2, 2, 2,
    ];
    let raw = encode_runtime_send_request(Layer::Network, 91, &packet);
    let request = decode_runtime_send_request(&raw).expect("runtime send request should decode");

    assert_eq!(request.header.version, ProtocolVersion::CURRENT);
    assert_eq!(request.header.layer, Layer::Network);
    assert_eq!(request.header.reinjection_token, 91);
    assert_eq!(request.payload, packet.as_slice());
}

#[test]
fn socket_event_payload_into_writes_fixed_layout_into_caller_buffer() {
    let mut output = [0u8; 16];

    let written = encode_socket_event_payload_into(SocketEventKind::Connect, 42, &mut output)
        .expect("fixed buffer should fit socket payload");

    assert_eq!(written, output.len());
    let decoded =
        wd_proto::decode_socket_event_payload(&output[..written]).expect("payload should decode");
    assert_eq!(decoded.kind(), SocketEventKind::Connect);
    assert_eq!(decoded.process_id(), 42);
}

#[test]
fn socket_event_payload_into_rejects_small_output_buffer() {
    let mut output = [0u8; 15];

    let err = encode_socket_event_payload_into(SocketEventKind::Connect, 42, &mut output)
        .expect_err("short buffer should be rejected");

    assert_eq!(
        err,
        EncodeIntoError::BufferTooSmall {
            required: 16,
            provided: 15,
        }
    );
}

#[test]
fn flow_event_payload_into_writes_fixed_layout_into_caller_buffer() {
    let mut output = [0u8; 24];

    let written =
        encode_flow_event_payload_into(FlowEventKind::Established, 9001, 7, &mut output)
            .expect("fixed buffer should fit flow payload");

    assert_eq!(written, output.len());
    let decoded = wd_proto::decode_flow_event_payload(&output[..written])
        .expect("flow payload should decode");
    assert_eq!(decoded.kind(), FlowEventKind::Established);
    assert_eq!(decoded.flow_id(), 9001);
    assert_eq!(decoded.process_id(), 7);
}

#[test]
fn flow_event_payload_into_rejects_small_output_buffer() {
    let mut output = [0u8; 23];

    let err = encode_flow_event_payload_into(FlowEventKind::Established, 9001, 7, &mut output)
        .expect_err("short buffer should be rejected");

    assert_eq!(
        err,
        EncodeIntoError::BufferTooSmall {
            required: 24,
            provided: 23,
        }
    );
}
