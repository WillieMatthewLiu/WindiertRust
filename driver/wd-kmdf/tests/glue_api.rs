use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
use wd_kmdf::{GlueIoResult, GlueIoStatus, RuntimeGlueApi};
use wd_proto::{
    Layer, OpenRequest, decode_network_event_payload, decode_open_response, decode_runtime_event,
    encode_open_request, encode_runtime_send_request,
};

fn ipv4_packet() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x06, 0xaa, 0xbb, 1, 1, 1, 1, 2,
        2, 2, 2,
    ]
}

#[test]
fn glue_api_open_returns_success_and_bytes_written() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 32];

    let result = glue.device_control(
        IOCTL_OPEN,
        &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
        &mut output,
    );
    assert_eq!(result.status, GlueIoStatus::Success);
    assert_eq!(result.bytes_written, 12);

    let response =
        decode_open_response(&output[..result.bytes_written as usize]).expect("response should decode");
    assert_eq!(response.status, 0);
}

#[test]
fn glue_api_recv_returns_success_and_runtime_frame() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 256];
    let _ = glue.device_control(
        IOCTL_OPEN,
        &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
        &mut output,
    );
    glue.queue_network_event(Layer::Network, 15, &ipv4_packet())
        .expect("queue should succeed");

    let result = glue.device_control(IOCTL_RECV, &[], &mut output);
    assert_eq!(result.status, GlueIoStatus::Success);
    assert!(result.bytes_written > 0);

    let frame =
        decode_runtime_event(&output[..result.bytes_written as usize]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");
    assert_eq!(payload.packet, ipv4_packet().as_slice());
}

#[test]
fn glue_api_send_returns_success_with_zero_bytes() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 256];
    let _ = glue.device_control(
        IOCTL_OPEN,
        &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
        &mut output,
    );
    glue.queue_network_event(Layer::Network, 15, &ipv4_packet())
        .expect("queue should succeed");
    let recv = glue.device_control(IOCTL_RECV, &[], &mut output);
    let frame =
        decode_runtime_event(&output[..recv.bytes_written as usize]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");

    let result = glue.device_control(
        IOCTL_SEND,
        &encode_runtime_send_request(Layer::Network, payload.reinjection_token, &ipv4_packet()),
        &mut output,
    );
    assert_eq!(result.status, GlueIoStatus::Success);
    assert_eq!(result.bytes_written, 0);
}

#[test]
fn glue_api_maps_small_output_buffer_to_stable_status() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 4];

    let result = glue.device_control(
        IOCTL_OPEN,
        &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
        &mut output,
    );
    assert_eq!(result.status, GlueIoStatus::OutputTooSmall);
    assert_eq!(result.bytes_written, 0);
}

#[test]
fn glue_api_maps_empty_queue_to_stable_status() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 32];
    let _ = glue.device_control(
        IOCTL_OPEN,
        &encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0)),
        &mut output,
    );

    let result = glue.device_control(IOCTL_RECV, &[], &mut output);
    assert_eq!(result.status, GlueIoStatus::QueueEmpty);
    assert_eq!(result.bytes_written, 0);
}

#[test]
fn glue_api_result_is_c_like_shape() {
    let result = GlueIoResult {
        status: GlueIoStatus::Success,
        bytes_written: 12,
    };
    assert_eq!(std::mem::size_of_val(&result), 8);
}

#[test]
fn glue_api_raw_open_returns_success_and_bytes_written() {
    let mut glue = RuntimeGlueApi::new(8);
    let input = encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0));
    let mut output = [0u8; 32];

    let result = unsafe {
        glue.device_control_raw(
            IOCTL_OPEN,
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    };
    assert_eq!(result.status, GlueIoStatus::Success);
    assert_eq!(result.bytes_written, 12);
}

#[test]
fn glue_api_raw_rejects_null_input_pointer_when_len_is_nonzero() {
    let mut glue = RuntimeGlueApi::new(8);
    let mut output = [0u8; 32];

    let result = unsafe {
        glue.device_control_raw(IOCTL_OPEN, std::ptr::null(), 1, output.as_mut_ptr(), output.len())
    };
    assert_eq!(result.status, GlueIoStatus::InvalidPointer);
    assert_eq!(result.bytes_written, 0);
}

#[test]
fn glue_api_raw_rejects_null_output_pointer_when_len_is_nonzero() {
    let mut glue = RuntimeGlueApi::new(8);
    let input = encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0));

    let result = unsafe {
        glue.device_control_raw(IOCTL_OPEN, input.as_ptr(), input.len(), std::ptr::null_mut(), 32)
    };
    assert_eq!(result.status, GlueIoStatus::InvalidPointer);
    assert_eq!(result.bytes_written, 0);
}
