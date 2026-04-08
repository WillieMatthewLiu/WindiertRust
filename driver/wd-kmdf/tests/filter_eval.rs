use wd_filter::{compile, encode_ir, FilterIr, LayerMask, OpCode};
use wd_kmdf::{DriverEvent, FilterEngine};
use wd_proto::Layer;

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
fn filter_engine_rejects_unsupported_layer_scope() {
    let err = FilterEngine::compile(Layer::Network, "tcp and inbound")
        .expect_err("network scope should be rejected explicitly");

    assert!(err.to_string().contains("unsupported layer"));
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
