use wd_kmdf::DriverEvent;
use wd_proto::OpenResponse;

pub const DEFAULT_PACKET_ID: u64 = 0x1234;
pub const DEFAULT_SOCKET_PROCESS_ID: u64 = 7;
pub const DEFAULT_FLOW_ID: u64 = 0xfeed;
pub const DEFAULT_FLOW_PROCESS_ID: u64 = 42;
pub const DEFAULT_CAPABILITIES: u32 = 0x1f;

pub fn ipv4_frame() -> Vec<u8> {
    vec![
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 64, 6, 0x9c, 0x93, 192, 168, 1, 10, 192,
        168, 1, 1,
    ]
}

pub fn socket_connect_event(process_id: u64) -> DriverEvent {
    DriverEvent::socket_connect(process_id)
}

pub fn reflect_open_response() -> OpenResponse {
    OpenResponse::success(DEFAULT_CAPABILITIES)
}

pub fn flow_established_event(process_id: u64) -> DriverEvent {
    DriverEvent::flow_established(DEFAULT_FLOW_ID, process_id)
}
