use wd_proto::OpenResponse;

use crate::DynamicHandle;

pub fn network_frame_bytes() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 64, 6, 0x9c, 0x93, 192, 168, 1, 10, 192,
        168, 1, 1,
    ]
}

pub fn opened_handle(response: OpenResponse) -> DynamicHandle {
    DynamicHandle::from_open_response(response).expect("open response should be accepted")
}
