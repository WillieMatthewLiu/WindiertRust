use wd_kmdf::{ACCEPTED_PACKET_BYTES, NetworkRuntime, NetworkRuntimeError, ReinjectionTable};
use wd_proto::{Layer, decode_runtime_event, decode_runtime_send_request};

fn ipv4_packet() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1, 2,
        2, 2, 2,
    ]
}

#[test]
fn issue_network_event_encodes_runtime_frame_with_reinjection_token() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();

    let raw = NetworkRuntime::issue_event(&mut table, Layer::Network, 9001, &packet)
        .expect("network runtime event should encode");
    let frame = decode_runtime_event(&raw).expect("runtime event should decode");

    assert_eq!(frame.header.layer, Layer::Network);
    let accepted = NetworkRuntime::accept_send(&mut table, &wd_proto::encode_runtime_send_request(
        Layer::Network,
        1,
        &packet,
    ))
    .expect("send request should consume issued token");
    assert_eq!(accepted.packet_id, 9001);
    assert_eq!(accepted.layer, Layer::Network);
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}

#[test]
fn issue_event_into_writes_runtime_frame_into_caller_buffer() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let mut output = [0u8; 64];

    let written = NetworkRuntime::issue_event_into(&mut table, Layer::Network, 9001, &packet, &mut output)
        .expect("network runtime event should encode into fixed buffer");
    let frame = decode_runtime_event(&output[..written]).expect("runtime event should decode");

    assert_eq!(frame.header.layer, Layer::Network);
    let accepted = NetworkRuntime::accept_send(&mut table, &wd_proto::encode_runtime_send_request(
        Layer::Network,
        1,
        &packet,
    ))
    .expect("send request should consume issued token");
    assert_eq!(accepted.packet_id, 9001);
}

#[test]
fn issue_event_into_rejects_small_output_buffer() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let mut output = [0u8; 35];

    let err = NetworkRuntime::issue_event_into(&mut table, Layer::Network, 9001, &packet, &mut output)
        .expect_err("short buffer should be rejected");

    assert!(matches!(
        err,
        NetworkRuntimeError::EncodeInto(wd_proto::EncodeIntoError::BufferTooSmall {
            required: 52,
            provided: 35,
        })
    ));
}

#[test]
fn accept_send_rejects_unknown_token() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let request = wd_proto::encode_runtime_send_request(Layer::Network, 77, &packet);

    let err = NetworkRuntime::accept_send(&mut table, &request)
        .expect_err("unknown token should be rejected");
    assert!(matches!(err, NetworkRuntimeError::UnknownToken));
}

#[test]
fn issue_and_accept_preserve_network_forward_layer() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let raw = NetworkRuntime::issue_event(&mut table, Layer::NetworkForward, 44, &packet)
        .expect("network forward event should encode");
    let frame = decode_runtime_event(&raw).expect("runtime event should decode");
    assert_eq!(frame.header.layer, Layer::NetworkForward);

    let accepted = NetworkRuntime::accept_send(
        &mut table,
        &wd_proto::encode_runtime_send_request(Layer::NetworkForward, 1, &packet),
    )
    .expect("network forward send should be accepted");
    assert_eq!(accepted.layer, Layer::NetworkForward);
    assert_eq!(accepted.packet_id, 44);
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}

#[test]
fn accept_send_rejects_non_network_layers() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let request = wd_proto::encode_runtime_send_request(Layer::Socket, 1, &packet);

    let err = NetworkRuntime::accept_send(&mut table, &request)
        .expect_err("non-network layer should be rejected");
    assert!(matches!(err, NetworkRuntimeError::UnsupportedLayer(Layer::Socket)));
}

#[test]
fn issue_event_rejects_non_network_layers() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();

    let err = NetworkRuntime::issue_event(&mut table, Layer::Flow, 1, &packet)
        .expect_err("non-network issue should be rejected");
    assert!(matches!(err, NetworkRuntimeError::UnsupportedLayer(Layer::Flow)));
}

#[test]
fn accept_send_round_trips_runtime_send_request_shape() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let _ = NetworkRuntime::issue_event(&mut table, Layer::Network, 7, &packet)
        .expect("token issuance should succeed");
    let request = wd_proto::encode_runtime_send_request(Layer::Network, 1, &packet);
    let decoded = decode_runtime_send_request(&request).expect("request should decode");
    assert_eq!(decoded.header.layer, Layer::Network);

    let accepted = NetworkRuntime::accept_send(&mut table, &request)
        .expect("runtime contract should accept matching request");
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}

#[test]
fn accept_send_rejects_packets_larger_than_fixed_storage_without_consuming_token() {
    let mut table = ReinjectionTable::default();
    let packet = ipv4_packet();
    let _ = NetworkRuntime::issue_event(&mut table, Layer::Network, 7, &packet)
        .expect("token issuance should succeed");
    let oversized = vec![0u8; ACCEPTED_PACKET_BYTES + 1];
    let oversized_request = wd_proto::encode_runtime_send_request(Layer::Network, 1, &oversized);

    let err = NetworkRuntime::accept_send(&mut table, &oversized_request)
        .expect_err("oversized packet should be rejected");
    assert!(matches!(
        err,
        NetworkRuntimeError::PacketBuffer(wd_kmdf_core::FixedPacketError::PacketTooLarge {
            required,
            capacity
        }) if required == ACCEPTED_PACKET_BYTES + 1 && capacity == ACCEPTED_PACKET_BYTES
    ));

    let accepted = NetworkRuntime::accept_send(
        &mut table,
        &wd_proto::encode_runtime_send_request(Layer::Network, 1, &packet),
    )
    .expect("failed oversized send should not consume token");
    assert_eq!(accepted.packet_id, 7);
    assert_eq!(accepted.packet.as_slice(), packet.as_slice());
}
