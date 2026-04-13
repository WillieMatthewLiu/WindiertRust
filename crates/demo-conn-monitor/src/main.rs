//! 网络连接实时监控 Demo
//!
//! 监控当前系统的网络连接事件（Socket 连接 + Flow 建立），
//! 实时打印连接信息。
//!
//! 用法: demo-conn-monitor [--socket] [--flow]
//!
//! 默认同时监控 Socket 和 Flow 事件。
//! 需要管理员权限和已安装的 WdRust 驱动。

use std::process::ExitCode;

use wd_proto::FlowEventPayload;
use wd_user::{
    DeviceAvailability, RecvEvent, RuntimeOpenConfig, RuntimeSession, RuntimeTransport,
    WindowsTransport, default_device_path,
};

const MAX_RECV_BYTES: usize = 65_535;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let monitor_socket = !args.contains(&"--flow-only".to_string());
    let monitor_flow = !args.contains(&"--socket-only".to_string());

    match run(monitor_socket, monitor_flow) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR code={} category={} message={}", err.code(), err.category(), err.message());
            ExitCode::from(err.code())
        }
    }
}

fn run(monitor_socket: bool, monitor_flow: bool) -> Result<(), wd_user::RuntimeError> {
    let transport = WindowsTransport::default();

    // 探测设备
    let availability = transport.probe()?;
    if availability == DeviceAvailability::Missing {
        return Err(wd_user::RuntimeError::device_unavailable(default_device_path()));
    }

    let mut socket_session = None;
    let mut flow_session = None;

    if monitor_socket {
        let config = RuntimeOpenConfig::socket();
        let _probe = transport.open(&config)?;
        socket_session = Some(transport.open_session(&config)?);
        eprintln!("[SOCKET] 监控已启动 - 等待 Socket 连接事件...");
    }

    if monitor_flow {
        let config = RuntimeOpenConfig::flow();
        let _probe = transport.open(&config)?;
        flow_session = Some(transport.open_session(&config)?);
        eprintln!("[FLOW]   监控已启动 - 等待 Flow 建立事件...");
    }

    eprintln!("按 Ctrl+C 停止");
    eprintln!("{}", "-".repeat(70));

    let mut socket_count: u64 = 0;
    let mut flow_count: u64 = 0;

    // 简单轮询：交替检查 socket 和 flow 事件
    loop {
        if let Some(ref mut session) = socket_session {
            match session.recv_one(MAX_RECV_BYTES) {
                Ok(raw) => {
                    if let Ok(event) = RecvEvent::decode(&raw) {
                        if let Some(socket_event) = event.socket() {
                            socket_count += 1;
                            print_socket_event(socket_count, socket_event);
                        }
                    }
                }
                Err(err) if err.code() == 6 => {
                    // io_failure 可能是队列为空，继续轮询
                }
                Err(err) => return Err(err),
            }
        }

        if let Some(ref mut session) = flow_session {
            match session.recv_one(MAX_RECV_BYTES) {
                Ok(raw) => {
                    if let Ok(event) = RecvEvent::decode(&raw) {
                        if let Some(flow_event) = event.flow() {
                            flow_count += 1;
                            print_flow_event(flow_count, flow_event);
                        }
                    }
                }
                Err(err) if err.code() == 6 => {
                    // io_failure 可能是队列为空，继续轮询
                }
                Err(err) => return Err(err),
            }
        }

        // 如果两个 session 都没有，短暂休眠避免空转
        if socket_session.is_none() && flow_session.is_none() {
            break;
        }
    }

    // 清理
    if let Some(session) = socket_session {
        session.close()?;
    }
    if let Some(session) = flow_session {
        session.close()?;
    }

    eprintln!("{}", "-".repeat(70));
    eprintln!("监控结束: Socket 事件={}, Flow 事件={}", socket_count, flow_count);
    Ok(())
}

fn print_socket_event(seq: u64, event: &wd_proto::SocketEventPayload) {
    let kind = match event.kind() {
        wd_proto::SocketEventKind::Connect => "CONNECT",
    };
    let pid = event.process_id();
    let timestamp = now_timestamp_ms();
    eprintln!("[{seq:>4}] [SOCKET] {kind} pid={pid} @ {timestamp}ms");
}

fn print_flow_event(seq: u64, event: &FlowEventPayload) {
    let kind = match event.kind() {
        wd_proto::FlowEventKind::Established => "ESTABLISHED",
    };
    let flow_id = event.flow_id();
    let pid = event.process_id();
    let timestamp = now_timestamp_ms();
    eprintln!("[{seq:>4}] [FLOW]   {kind} flow_id={flow_id} pid={pid} @ {timestamp}ms");
}

fn now_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
