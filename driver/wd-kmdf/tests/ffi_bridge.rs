use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
use wd_kmdf::{
    GlueIoStatus, wd_runtime_glue_create, wd_runtime_glue_destroy, wd_runtime_glue_device_control,
    wd_runtime_glue_queue_network_event,
};
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
fn ffi_bridge_open_recv_send_round_trips() {
    let input = encode_open_request(&OpenRequest::new(Layer::Network, Vec::new(), 0, 0));
    let mut output = [0u8; 256];
    let packet = ipv4_packet();

    let handle = wd_runtime_glue_create(8);
    assert!(!handle.is_null(), "ffi create should return a non-null handle");

    let open = unsafe {
        wd_runtime_glue_device_control(
            handle,
            IOCTL_OPEN,
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    };
    assert_eq!(open.status, GlueIoStatus::Success);
    let response =
        decode_open_response(&output[..open.bytes_written as usize]).expect("open response should decode");
    assert_eq!(response.status, 0);

    let queued = unsafe {
        wd_runtime_glue_queue_network_event(
            handle,
            Layer::Network.to_wire(),
            5150,
            packet.as_ptr(),
            packet.len(),
        )
    };
    assert_eq!(queued.status, GlueIoStatus::Success);

    let recv = unsafe {
        wd_runtime_glue_device_control(handle, IOCTL_RECV, std::ptr::null(), 0, output.as_mut_ptr(), output.len())
    };
    assert_eq!(recv.status, GlueIoStatus::Success);
    let frame =
        decode_runtime_event(&output[..recv.bytes_written as usize]).expect("runtime event should decode");
    let payload = decode_network_event_payload(frame.payload).expect("network payload should decode");

    let send_input = encode_runtime_send_request(Layer::Network, payload.reinjection_token, &packet);
    let send = unsafe {
        wd_runtime_glue_device_control(
            handle,
            IOCTL_SEND,
            send_input.as_ptr(),
            send_input.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    };
    assert_eq!(send.status, GlueIoStatus::Success);
    assert_eq!(send.bytes_written, 0);

    unsafe { wd_runtime_glue_destroy(handle) };
}

#[test]
fn ffi_bridge_rejects_null_handle() {
    let mut output = [0u8; 16];
    let result = unsafe {
        wd_runtime_glue_device_control(
            std::ptr::null_mut(),
            IOCTL_RECV,
            std::ptr::null(),
            0,
            output.as_mut_ptr(),
            output.len(),
        )
    };

    assert_eq!(result.status, GlueIoStatus::InvalidHandle);
    assert_eq!(result.bytes_written, 0);
}

#[test]
fn ffi_bridge_rejects_invalid_layer_value() {
    let packet = ipv4_packet();
    let handle = wd_runtime_glue_create(8);
    assert!(!handle.is_null(), "ffi create should return a non-null handle");

    let result = unsafe {
        wd_runtime_glue_queue_network_event(handle, 255, 1, packet.as_ptr(), packet.len())
    };

    assert_eq!(result.status, GlueIoStatus::InvalidLayer);
    assert_eq!(result.bytes_written, 0);
    unsafe { wd_runtime_glue_destroy(handle) };
}

#[test]
fn ffi_bridge_destroy_accepts_null_handle() {
    unsafe { wd_runtime_glue_destroy(std::ptr::null_mut()) };
}
