use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
use wd_kmdf::{RuntimeIoctlDispatcher, RuntimeIoctlError};
use wd_proto::{
    CapabilityFlags, FlowEventKind, Layer, OpenRequest, SocketEventKind,
    decode_flow_event_payload, decode_network_event_payload, decode_open_response,
    decode_runtime_event, decode_socket_event_payload, encode_open_request,
    encode_runtime_send_request,
};
use wd_filter::{compile, encode_ir};

fn ipv4_packet() -> Vec<u8> {
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

#[test]
fn ioctl_open_returns_capabilities_response() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut raw = [0u8; 32];
    let written = dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut raw,
        )
        .expect("open should succeed");

    let response = decode_open_response(&raw[..written]).expect("open response should decode");
    assert_eq!(
        response.capabilities,
        (CapabilityFlags::CHECKSUM_RECALC | CapabilityFlags::NETWORK_REINJECT).bits()
    );
}

#[test]
fn ioctl_open_dispatch_into_writes_response_and_byte_count() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 32];

    let written = dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut output,
        )
        .expect("open dispatch_into should succeed");
    let response = decode_open_response(&output[..written]).expect("open response should decode");

    assert_eq!(written, 12);
    assert_eq!(
        response.capabilities,
        (CapabilityFlags::CHECKSUM_RECALC | CapabilityFlags::NETWORK_REINJECT).bits()
    );
}

#[test]
fn ioctl_open_retains_decoded_request_header() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 32];
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("layer == NETWORK_FORWARD").expect("filter should compile")),
        -9,
        0x0102_0304_0506_0708,
    );

    dispatcher
        .dispatch_into(IOCTL_OPEN, &encode_open_request(&request), &mut output)
        .expect("open dispatch_into should succeed");

    let retained = dispatcher
        .last_open_request()
        .expect("dispatcher should retain decoded open request");
    assert_eq!(retained.version, request.version);
    assert_eq!(retained.layer, Layer::NetworkForward);
    assert_eq!(retained.priority, -9);
    assert_eq!(retained.flags, 0x0102_0304_0506_0708);
    assert_eq!(retained.filter_ir, request.filter_ir);
    assert_eq!(retained.filter_len, request.filter_len);
}

#[test]
fn ioctl_open_socket_filter_ir_applies_to_runtime_queueing() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 256];
    let request = OpenRequest::new(
        Layer::Socket,
        encode_ir(&compile("event == CONNECT and processId == 7").expect("filter should compile")),
        0,
        0,
    );

    dispatcher
        .dispatch_into(IOCTL_OPEN, &encode_open_request(&request), &mut output)
        .expect("open should succeed");

    dispatcher
        .queue_socket_event(SocketEventKind::Connect, 8)
        .expect("unmatched socket event should be ignored");
    let err = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut output)
        .expect_err("unmatched socket event should not be queued");
    assert!(matches!(err, RuntimeIoctlError::Device(_)));

    dispatcher
        .queue_socket_event(SocketEventKind::Connect, 7)
        .expect("matching socket event should queue");
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut output)
        .expect("matching socket event should be returned");
    let frame = decode_runtime_event(&output[..written]).expect("runtime event should decode");
    let payload = decode_socket_event_payload(frame.payload).expect("socket payload should decode");
    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(payload.process_id(), 7);
}

#[test]
fn ioctl_open_network_filter_ir_applies_to_runtime_queueing() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 256];
    let request = OpenRequest::new(
        Layer::Network,
        encode_ir(&compile("tcp and inbound").expect("filter should compile")),
        0,
        0,
    );

    dispatcher
        .dispatch_into(IOCTL_OPEN, &encode_open_request(&request), &mut output)
        .expect("open should succeed");

    dispatcher
        .queue_network_event(Layer::Network, 15, &ipv4_udp_packet())
        .expect("unmatched network packet should be ignored");
    let err = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut output)
        .expect_err("unmatched network packet should not be queued");
    assert!(matches!(err, RuntimeIoctlError::Device(_)));

    dispatcher
        .queue_network_event(Layer::Network, 16, &ipv4_packet())
        .expect("matching network packet should queue");
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut output)
        .expect("matching network packet should be returned");
    let frame = decode_runtime_event(&output[..written]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");
    assert_eq!(frame.header.layer, Layer::Network);
    assert_eq!(payload.packet, ipv4_packet().as_slice());
}

#[test]
fn ioctl_recv_returns_runtime_network_frame_after_queueing_packet() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_network_event(Layer::Network, 55, &ipv4_packet())
        .expect("event should queue");

    let mut raw = [0u8; 256];
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut raw)
        .expect("recv should return queued frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    assert_eq!(frame.header.layer, Layer::Network);
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");
    assert_eq!(payload.packet, ipv4_packet().as_slice());
}

#[test]
fn ioctl_send_consumes_token_from_recv_frame() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::NetworkForward, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_network_event(Layer::NetworkForward, 99, &ipv4_packet())
        .expect("event should queue");

    let mut raw = [0u8; 256];
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut raw)
        .expect("recv should return queued frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");

    let mut send_out = [0u8; 1];
    let send_written = dispatcher
        .dispatch_into(
            IOCTL_SEND,
            &encode_runtime_send_request(Layer::NetworkForward, payload.reinjection_token, &ipv4_packet()),
            &mut send_out,
        )
        .expect("send should consume token");
    assert_eq!(send_written, 0, "send should not return an output payload");

    let accepted = dispatcher.last_reinjection().expect("accepted reinjection should be recorded");
    assert_eq!(accepted.layer, Layer::NetworkForward);
    assert_eq!(accepted.packet_id, 99);
}

#[test]
fn ioctl_recv_dispatch_into_copies_runtime_frame() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_network_event(Layer::Network, 55, &ipv4_packet())
        .expect("event should queue");
    let mut output = [0u8; 256];

    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut output)
        .expect("recv dispatch_into should succeed");
    let frame = decode_runtime_event(&output[..written]).expect("runtime event should decode");

    assert_eq!(frame.header.layer, Layer::Network);
}

#[test]
fn ioctl_dispatch_into_rejects_small_output_buffer() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 4];

    let err = dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut output,
        )
        .expect_err("small output buffer should be rejected");
    assert!(matches!(
        err,
        RuntimeIoctlError::OutputTooSmall {
            required: 12,
            provided: 4
        }
    ));
}

#[test]
fn ioctl_send_dispatch_into_reports_zero_bytes_written() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_network_event(Layer::Network, 55, &ipv4_packet())
        .expect("event should queue");

    let mut raw = [0u8; 256];
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut raw)
        .expect("recv should return queued frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");
    let mut output = [0u8; 1];

    let written = dispatcher
        .dispatch_into(
            IOCTL_SEND,
            &encode_runtime_send_request(Layer::Network, payload.reinjection_token, &ipv4_packet()),
            &mut output,
        )
        .expect("send dispatch_into should succeed");
    assert_eq!(written, 0);
}

#[test]
fn ioctl_rejects_unknown_code() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut output = [0u8; 8];
    let err = dispatcher
        .dispatch_into(0xdead_beef, &[], &mut output)
        .expect_err("unknown ioctl should be rejected");
    assert!(matches!(err, RuntimeIoctlError::UnsupportedIoctl(0xdead_beef)));
}

#[test]
fn ioctl_recv_returns_socket_runtime_frame_after_queueing_event() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Socket, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_socket_event(SocketEventKind::Connect, 7)
        .expect("socket event should queue");

    let mut raw = [0u8; 256];
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut raw)
        .expect("recv should return queued frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_socket_event_payload(frame.payload).expect("socket payload should decode");

    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(payload.kind(), SocketEventKind::Connect);
    assert_eq!(payload.process_id(), 7);
}

#[test]
fn ioctl_recv_returns_flow_runtime_frame_after_queueing_event() {
    let mut dispatcher = RuntimeIoctlDispatcher::new(8);
    let mut open_output = [0u8; 32];
    dispatcher
        .dispatch_into(
            IOCTL_OPEN,
            &encode_open_request(&OpenRequest::new(Layer::Flow, Vec::new(), 0, 0)),
            &mut open_output,
        )
        .expect("open should succeed");
    dispatcher
        .queue_flow_event(FlowEventKind::Established, 9001, 42)
        .expect("flow event should queue");

    let mut raw = [0u8; 256];
    let written = dispatcher
        .dispatch_into(IOCTL_RECV, &[], &mut raw)
        .expect("recv should return queued frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_flow_event_payload(frame.payload).expect("flow payload should decode");

    assert_eq!(frame.header.layer, Layer::Flow);
    assert_eq!(payload.kind(), FlowEventKind::Established);
    assert_eq!(payload.flow_id(), 9001);
    assert_eq!(payload.process_id(), 42);
}
