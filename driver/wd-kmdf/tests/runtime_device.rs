use wd_kmdf::{ACCEPTED_PACKET_BYTES, RuntimeDevice, RuntimeDeviceError};
use wd_proto::{
    FlowEventKind, Layer, OpenRequest, SocketEventKind, decode_flow_event_payload, decode_runtime_event,
    decode_socket_event_payload, encode_runtime_send_request,
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
fn runtime_device_recv_returns_issued_network_event() {
    let packet = ipv4_packet();
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::Network, 7001, &packet)
        .expect("network event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");

    assert_eq!(frame.header.layer, Layer::Network);
}

#[test]
fn runtime_device_network_filter_ir_drops_unmatched_packets() {
    let tcp_packet = ipv4_packet();
    let mut udp_packet = ipv4_packet();
    udp_packet[9] = 0x11;
    let request = OpenRequest::new(
        Layer::Network,
        encode_ir(&compile("tcp and inbound").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network filter");

    device
        .queue_network_event(Layer::Network, 7001, &udp_packet)
        .expect("unmatched packet should be ignored without failing");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("unmatched network packet should not enter the runtime queue");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::Network, 7002, &tcp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching network packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::Network);
    assert_eq!(payload.packet, tcp_packet.as_slice());
}

#[test]
fn runtime_device_network_packet32_filter_controls_queueing() {
    let tcp_packet = ipv4_packet();
    let mut nonmatch_packet = ipv4_packet();
    nonmatch_packet[0] = 0x46;
    let request = OpenRequest::new(
        Layer::Network,
        encode_ir(&compile("packet32[0] == 0x45000014").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with packet32 filter");

    device
        .queue_network_event(Layer::Network, 7003, &nonmatch_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("packet32 mismatch should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::Network, 7004, &tcp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(payload.packet, tcp_packet.as_slice());
}

#[test]
fn runtime_device_network_packet16_filter_controls_queueing() {
    let tcp_packet = ipv4_packet();
    let mut nonmatch_packet = ipv4_packet();
    nonmatch_packet[10] = 0xcc;
    let request = OpenRequest::new(
        Layer::Network,
        encode_ir(&compile("packet16[10] == 0xaabb").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with packet16 filter");

    device
        .queue_network_event(Layer::Network, 7005, &nonmatch_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("packet16 mismatch should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::Network, 7006, &tcp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(payload.packet, tcp_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_tcp_filter_controls_queueing() {
    let tcp_packet = ipv4_packet();
    let udp_packet = ipv4_udp_packet();
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("tcp").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward tcp filter");

    device
        .queue_network_event(Layer::NetworkForward, 7007, &udp_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("udp packet should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7008, &tcp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, tcp_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_udp_filter_controls_queueing() {
    let tcp_packet = ipv4_packet();
    let udp_packet = ipv4_udp_packet();
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("udp").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward udp filter");

    device
        .queue_network_event(Layer::NetworkForward, 7009, &tcp_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("tcp packet should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7010, &udp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, udp_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_outbound_filter_controls_queueing() {
    let tcp_packet = ipv4_packet();
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("outbound").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward outbound filter");

    device
        .queue_network_event(Layer::Network, 7011, &tcp_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("network-layer packet should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7012, &tcp_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, tcp_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_remote_port_filter_controls_queueing() {
    let match_packet = ipv4_tcp_packet_with_ports(12345, 443);
    let nonmatch_packet = ipv4_tcp_packet_with_ports(12345, 80);
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("tcp and remotePort == 443").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward remote port filter");

    device
        .queue_network_event(Layer::NetworkForward, 7013, &nonmatch_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("wrong remote port should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7014, &match_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, match_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_ipv6_filter_controls_queueing() {
    let ipv4_packet = ipv4_packet();
    let ipv6_packet = ipv6_tcp_packet_with_ports(12345, 443);
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("ipv6 and tcp").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward ipv6 filter");

    device
        .queue_network_event(Layer::NetworkForward, 7015, &ipv4_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("ipv4 packet should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7016, &ipv6_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, ipv6_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_remote_ipv4_address_filter_controls_queueing() {
    let match_packet = ipv4_packet();
    let nonmatch_packet = ipv4_tcp_packet_with_ports(12345, 443);
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(
            &compile("remoteAddr == 2.2.2.2 and localAddr == 1.1.1.1")
                .expect("filter should compile"),
        ),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward address filter");

    let mut mutated_nonmatch = nonmatch_packet.clone();
    mutated_nonmatch[16] = 3;

    device
        .queue_network_event(Layer::NetworkForward, 7017, &mutated_nonmatch)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("wrong remote address should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7018, &match_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, match_packet.as_slice());
}

#[test]
fn runtime_device_network_forward_remote_ipv4_cidr_filter_controls_queueing() {
    let match_packet = ipv4_packet();
    let mut nonmatch_packet = ipv4_packet();
    nonmatch_packet[16] = 3;
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(
            &compile("remoteAddr == 2.2.2.0/24 and localAddr == 1.1.1.0/24")
                .expect("filter should compile"),
        ),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with network forward cidr filter");

    device
        .queue_network_event(Layer::NetworkForward, 7019, &nonmatch_packet)
        .expect("non-matching packet should be ignored");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("wrong remote cidr should keep queue empty");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_network_event(Layer::NetworkForward, 7020, &match_packet)
        .expect("matching packet should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching packet should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = wd_proto::decode_network_event_payload(frame.payload).expect("payload should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);
    assert_eq!(payload.packet, match_packet.as_slice());
}

#[test]
fn runtime_device_open_with_request_retains_full_open_config() {
    let request = OpenRequest::new(
        Layer::NetworkForward,
        encode_ir(&compile("layer == NETWORK_FORWARD").expect("filter should compile")),
        -9,
        0x0102_0304_0506_0708,
    );
    let mut device = RuntimeDevice::new(8);

    device
        .open_with_request(request.clone())
        .expect("device should open with request");

    let retained = device
        .last_open_request()
        .expect("device should retain the request that opened it");
    assert_eq!(retained, &request);
}

#[test]
fn runtime_device_close_clears_retained_open_config() {
    let request = OpenRequest::new(Layer::Socket, Vec::new(), 0, 0);
    let mut device = RuntimeDevice::new(8);

    device
        .open_with_request(request)
        .expect("device should open with request");
    device.shutdown_recv().expect("recv shutdown should succeed");
    device.shutdown_send().expect("send shutdown should succeed");
    device.close().expect("close should succeed");

    assert!(
        device.last_open_request().is_none(),
        "closed runtime device should not retain stale open config"
    );
}

#[test]
fn runtime_device_socket_filter_ir_drops_unmatched_events() {
    let request = OpenRequest::new(
        Layer::Socket,
        encode_ir(&compile("event == CONNECT and processId == 7").expect("filter should compile")),
        0,
        0,
    );
    let mut device = RuntimeDevice::new(8);
    device
        .open_with_request(request)
        .expect("device should open with socket filter");

    device
        .queue_socket_event(SocketEventKind::Connect, 8)
        .expect("unmatched event should be ignored without failing");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("unmatched socket event should not enter the runtime queue");
    assert!(matches!(err, RuntimeDeviceError::QueueEmpty));

    device
        .queue_socket_event(SocketEventKind::Connect, 7)
        .expect("matching event should queue");
    let written = device
        .recv_into(&mut raw)
        .expect("matching socket event should be returned");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_socket_event_payload(frame.payload).expect("socket payload should decode");
    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(payload.process_id(), 7);
}

#[test]
fn runtime_device_send_consumes_token_from_recv_frame() {
    let packet = ipv4_packet();
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::Network, 9001, &packet)
        .expect("network event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let token_payload = wd_proto::decode_network_event_payload(frame.payload)
        .expect("network payload should decode");
    let accepted = device
        .send(&encode_runtime_send_request(
            Layer::Network,
            token_payload.reinjection_token,
            &packet,
        ))
    .expect("send should consume token");

    assert_eq!(accepted.layer, Layer::Network);
    assert_eq!(accepted.packet_id, 9001);
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}

#[test]
fn runtime_device_rejects_recv_after_recv_shutdown() {
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device.shutdown_recv().expect("recv shutdown should succeed");

    let mut raw = [0u8; 256];
    let err = device
        .recv_into(&mut raw)
        .expect_err("recv should be blocked after shutdown");
    assert!(matches!(err, RuntimeDeviceError::RecvDisabled));
}

#[test]
fn runtime_device_allows_send_after_recv_shutdown_until_send_shutdown() {
    let packet = ipv4_packet();
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::NetworkForward, 77, &packet)
        .expect("network forward event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let token_payload = wd_proto::decode_network_event_payload(frame.payload)
        .expect("network payload should decode");

    device.shutdown_recv().expect("recv shutdown should succeed");
    let accepted = device
        .send(&encode_runtime_send_request(
            Layer::NetworkForward,
            token_payload.reinjection_token,
            &packet,
        ))
        .expect("send should still be allowed after recv shutdown");
    assert_eq!(accepted.layer, Layer::NetworkForward);

    device.shutdown_send().expect("send shutdown should succeed");
    let err = device
        .send(&encode_runtime_send_request(
            Layer::NetworkForward,
            token_payload.reinjection_token,
            &packet,
        ))
        .expect_err("send should be blocked after send shutdown");
    assert!(matches!(err, RuntimeDeviceError::SendDisabled));
}

#[test]
fn runtime_device_close_requires_shutdown_order() {
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");

    let err = device.close().expect_err("close should require shutdown order");
    assert!(matches!(err, RuntimeDeviceError::InvalidState(_)));
}

#[test]
fn runtime_device_drops_oldest_frame_when_queue_capacity_is_hit() {
    let packet = ipv4_packet();
    let mut device = RuntimeDevice::new(2);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::Network, 1, &packet)
        .expect("first frame should queue");
    device
        .queue_network_event(Layer::Network, 2, &packet)
        .expect("second frame should queue");
    device
        .queue_network_event(Layer::Network, 3, &packet)
        .expect("third frame should evict oldest");

    let mut first = [0u8; 256];
    let first_written = device
        .recv_into(&mut first)
        .expect("first visible frame should exist");
    let first_frame = decode_runtime_event(&first[..first_written]).expect("frame should decode");
    let first_payload =
        wd_proto::decode_network_event_payload(first_frame.payload).expect("payload should decode");
    let mut second = [0u8; 256];
    let second_written = device
        .recv_into(&mut second)
        .expect("second visible frame should exist");
    let second_frame = decode_runtime_event(&second[..second_written]).expect("frame should decode");
    let second_payload =
        wd_proto::decode_network_event_payload(second_frame.payload).expect("payload should decode");

    let accepted_second = device
        .send(&encode_runtime_send_request(
            Layer::Network,
            first_payload.reinjection_token,
            &packet,
        ))
        .expect("second queued token should still be valid");
    let accepted_third = device
        .send(&encode_runtime_send_request(
            Layer::Network,
            second_payload.reinjection_token,
            &packet,
        ))
        .expect("third queued token should still be valid");

    assert_eq!(accepted_second.packet_id, 2);
    assert_eq!(accepted_third.packet_id, 3);
}

#[test]
fn runtime_device_recv_into_rejects_small_output_buffer() {
    let packet = ipv4_packet();
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::Network, 123, &packet)
        .expect("network event should queue");

    let mut output = [0u8; 4];
    let err = device
        .recv_into(&mut output)
        .expect_err("too-small buffer should be rejected");
    assert!(matches!(err, RuntimeDeviceError::QueueStorage(_)));
}

#[test]
fn runtime_device_send_rejects_oversized_packet_without_consuming_token() {
    let packet = ipv4_packet();
    let oversized = vec![0u8; ACCEPTED_PACKET_BYTES + 1];
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_network_event(Layer::Network, 9001, &packet)
        .expect("network event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let token_payload = wd_proto::decode_network_event_payload(frame.payload)
        .expect("network payload should decode");

    let err = device
        .send(&encode_runtime_send_request(
            Layer::Network,
            token_payload.reinjection_token,
            &oversized,
        ))
        .expect_err("oversized packet should be rejected");
    assert!(matches!(err, RuntimeDeviceError::NetworkRuntime(_)));

    let accepted = device
        .send(&encode_runtime_send_request(
            Layer::Network,
            token_payload.reinjection_token,
            &packet,
        ))
        .expect("oversized failure should not consume token");
    assert_eq!(accepted.packet_id, 9001);
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}

#[test]
fn runtime_device_recv_returns_queued_socket_event() {
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_socket_event(SocketEventKind::Connect, 7)
        .expect("socket event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_socket_event_payload(frame.payload).expect("socket payload should decode");

    assert_eq!(frame.header.layer, Layer::Socket);
    assert_eq!(payload.kind(), SocketEventKind::Connect);
    assert_eq!(payload.process_id(), 7);
}

#[test]
fn runtime_device_recv_returns_queued_flow_event() {
    let mut device = RuntimeDevice::new(8);
    device.open().expect("device should open");
    device
        .queue_flow_event(FlowEventKind::Established, 9001, 42)
        .expect("flow event should queue");

    let mut raw = [0u8; 256];
    let written = device
        .recv_into(&mut raw)
        .expect("recv should return queued runtime frame");
    let frame = decode_runtime_event(&raw[..written]).expect("runtime event should decode");
    let payload = decode_flow_event_payload(frame.payload).expect("flow payload should decode");

    assert_eq!(frame.header.layer, Layer::Flow);
    assert_eq!(payload.kind(), FlowEventKind::Established);
    assert_eq!(payload.flow_id(), 9001);
    assert_eq!(payload.process_id(), 42);
}
