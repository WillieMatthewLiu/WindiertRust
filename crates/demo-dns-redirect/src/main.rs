//! DNS 重定向 Demo
//!
//! 拦截出站 DNS 请求（UDP 端口 53），将 DNS Server 地址修改为 8.8.8.8 后重注入。
//!
//! 用法: demo-dns-redirect [--count N]
//!
//! 需要管理员权限和已安装的 WdRust 驱动。

use std::process::ExitCode;

use wd_proto::encode_runtime_send_request;
use wd_user::{
    DeviceAvailability, HandleConfig, RecvEvent, RuntimeOpenConfig, RuntimeSession,
    RuntimeTransport, WindowsTransport, default_device_path,
};

const MAX_RECV_BYTES: usize = 65_535;
const DNS_PORT: u8 = 53;
const GOOGLE_DNS: [u8; 4] = [8, 8, 8, 8];

fn main() -> ExitCode {
    let count: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.strip_prefix("--count=").and_then(|v| v.parse().ok()))
        .unwrap_or(0); // 0 = 无限循环

    match run(count) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR code={} category={} message={}", err.code(), err.category(), err.message());
            ExitCode::from(err.code())
        }
    }
}

fn run(max_count: u64) -> Result<(), wd_user::RuntimeError> {
    let transport = WindowsTransport::default();

    // 探测设备
    let availability = transport.probe()?;
    if availability == DeviceAvailability::Missing {
        return Err(wd_user::RuntimeError::device_unavailable(default_device_path()));
    }

    // 编译过滤器: 拦截出站 UDP 53 端口
    let cfg = HandleConfig::network("udp and outbound")
        .map_err(|err| wd_user::RuntimeError::io_failure(format!("filter compile failed: {err}")))?;
    let open_config = RuntimeOpenConfig::network(cfg.filter_ir().to_vec());

    // 打开会话
    let _probe = transport.open(&open_config)?;
    let mut session = transport.open_session(&open_config)?;

    eprintln!("DNS 重定向已启动，拦截出站 UDP 53 并将目标改为 8.8.8.8 ...");
    eprintln!("按 Ctrl+C 停止");

    let mut processed: u64 = 0;
    loop {
        if max_count > 0 && processed >= max_count {
            break;
        }

        let raw = session.recv_one(MAX_RECV_BYTES)?;
        let mut event = match RecvEvent::decode(&raw) {
            Ok(event) => event,
            Err(_) => continue, // 跳过无法解码的帧
        };

        let packet = match event.packet_mut() {
            Some(p) => p,
            None => continue,
        };

        let bytes = packet.bytes();
        if !is_dns_request(bytes) {
            // 非 DNS 请求，原样重注入
            if let Some(token) = packet.reinjection_token() {
                let request = encode_runtime_send_request(packet.layer(), token, bytes);
                session.send_one(&request)?;
            }
            continue;
        }

        // 解析原始目标 IP 并打印
        let original_dst = extract_dst_ipv4(bytes);
        if original_dst == Some(GOOGLE_DNS) {
            // 已经是 8.8.8.8，无需修改
            if let Some(token) = packet.reinjection_token() {
                let request = encode_runtime_send_request(packet.layer(), token, bytes);
                session.send_one(&request)?;
            }
            continue;
        }

        // 修改目标 IP 为 8.8.8.8
        let modified = rewrite_dst_ipv4(bytes, GOOGLE_DNS);
        if let Some(token) = packet.reinjection_token() {
            let request = encode_runtime_send_request(packet.layer(), token, &modified);
            session.send_one(&request)?;
            processed += 1;

            let dst_str = original_dst
                .map(|ip| format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]))
                .unwrap_or_else(|| "unknown".to_string());
            eprintln!("[{processed}] DNS 请求重定向: {dst_str} -> 8.8.8.8");
        }
    }

    session.close()?;
    eprintln!("共处理 {processed} 个 DNS 请求");
    Ok(())
}

/// 判断是否为 DNS 请求（UDP 目标端口 53）
fn is_dns_request(bytes: &[u8]) -> bool {
    let ihl = match parse_ipv4_ihl(bytes) {
        Some(ihl) => ihl,
        None => return false,
    };
    let ip_header_len = ihl as usize * 4;
    if bytes.len() < ip_header_len + 8 {
        return false;
    }
    let protocol = bytes[9];
    if protocol != 17 {
        return false; // 非 UDP
    }
    let dst_port = u16::from_be_bytes([bytes[ip_header_len + 2], bytes[ip_header_len + 3]]);
    dst_port == DNS_PORT as u16
}

/// 从 IPv4 包中提取目标 IP 地址
fn extract_dst_ipv4(bytes: &[u8]) -> Option<[u8; 4]> {
    if bytes.len() < 20 {
        return None;
    }
    Some([bytes[16], bytes[17], bytes[18], bytes[19]])
}

/// 重写 IPv4 目标地址并重新计算校验和
fn rewrite_dst_ipv4(bytes: &[u8], new_dst: [u8; 4]) -> Vec<u8> {
    let mut modified = bytes.to_vec();
    if modified.len() < 20 {
        return modified;
    }

    // 写入新目标 IP
    modified[16] = new_dst[0];
    modified[17] = new_dst[1];
    modified[18] = new_dst[2];
    modified[19] = new_dst[3];

    // 重算 IPv4 头部校验和
    let ihl = (modified[0] & 0x0f) as usize * 4;
    modified[10] = 0;
    modified[11] = 0;
    let checksum = ipv4_header_checksum(&modified[..ihl]);
    modified[10] = (checksum >> 8) as u8;
    modified[11] = (checksum & 0xff) as u8;

    // 重算 UDP 校验和（UDP 校验和可选，置零表示不校验）
    if modified.len() > ihl + 6 {
        modified[ihl + 6] = 0;
        modified[ihl + 7] = 0;
    }

    modified
}

fn parse_ipv4_ihl(bytes: &[u8]) -> Option<u8> {
    if bytes.is_empty() {
        return None;
    }
    let version = bytes[0] >> 4;
    let ihl = bytes[0] & 0x0f;
    if version != 4 || ihl < 5 {
        return None;
    }
    Some(ihl)
}

fn ipv4_header_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < header.len() {
        let word = u16::from_be_bytes([header[i], header[i + 1]]) as u32;
        sum = sum.wrapping_add(word);
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
