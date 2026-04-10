use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Args;
use wd_user::{
    DeviceAvailability, RecvEvent, RuntimeError, RuntimeOpenConfig, RuntimeSession, RuntimeTransport,
    default_device_path,
};

use crate::cmd::common::{finish_with_cli_error, render_summary};
use crate::error::CliError;
use crate::output::OutputMode;
use crate::runtime::{default_transport, map_runtime_error};

const MAX_RECV_BYTES: usize = 65_535;

#[derive(Debug, Args)]
pub struct NetdumpCmd {
    #[arg(long)]
    filter: Option<String>,
    #[arg(long, default_value_t = 1)]
    count: u64,
    #[arg(long)]
    follow: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
}

impl NetdumpCmd {
    pub fn run(self) -> ExitCode {
        let mode = OutputMode::from_json_flag(self.json);
        finish_with_cli_error(self.execute_with_transport(default_transport()), mode)
    }

    fn execute_with_transport<T: RuntimeTransport>(self, transport: T) -> Result<String, CliError> {
        let NetdumpCmd {
            filter,
            count,
            follow,
            json,
            verbose,
            timeout_ms,
        } = self;

        validate_filter(filter.as_deref())?;
        validate_count(count, follow)?;
        let budget = TimeoutBudget::new(timeout_ms)
            .map_err(|err| map_runtime_error("netdump", err))?;
        let open_config = RuntimeOpenConfig::network(Vec::new());

        let availability = budget
            .run("probe", || transport.probe())
            .map_err(|err| map_runtime_error("netdump", err))?;
        if availability == DeviceAvailability::Missing {
            return Err(map_runtime_error(
                "netdump",
                RuntimeError::device_unavailable(default_device_path()),
            ));
        }

        let probe = budget
            .run("open", || transport.open(&open_config))
            .map_err(|err| map_runtime_error("netdump", err))?;
        let mut session = budget
            .run("open_session", || transport.open_session(&open_config))
            .map_err(|err| map_runtime_error("netdump", err))?;

        let mut events = Vec::new();
        for _ in 0..count {
            let raw = budget
                .run("recv", || session.recv_one(MAX_RECV_BYTES))
                .map_err(|err| map_runtime_error("netdump", err))?;
            events.push(decode_event_summary(&probe.device_path, &raw)?);
        }

        budget
            .run("close_session", || session.close())
            .map_err(|err| map_runtime_error("netdump", err))?;

        Ok(match json {
            true => render_json(&events, verbose, &probe.device_path, count, follow),
            false => render_text(&events, verbose, &probe.device_path, count, follow),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetdumpEventSummary {
    ttl: u8,
    checksum: u16,
    packet_len: usize,
    timestamp: String,
}

impl NetdumpEventSummary {
    fn render_text(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut fields = vec![
            ("layer", "NETWORK".to_string()),
            ("ttl", self.ttl.to_string()),
            ("checksum", format!("0x{:04x}", self.checksum)),
            ("packet_len", self.packet_len.to_string()),
            ("timestamp", self.timestamp.clone()),
        ];
        if verbose {
            fields.push(("device_path", device_path.to_string()));
            fields.push(("count", count.to_string()));
            fields.push(("follow", follow.to_string()));
        }
        render_summary("NETDUMP OK", &fields)
    }

    fn render_json(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut line = format!(
            "{{\"layer\":\"NETWORK\",\"ttl\":{},\"checksum\":\"0x{:04x}\",\"packet_len\":{},\"timestamp\":\"{}\"",
            self.ttl,
            self.checksum,
            self.packet_len,
            escape_json_string(&self.timestamp),
        );
        if verbose {
            line.push_str(",\"device_path\":\"");
            line.push_str(&escape_json_string(device_path));
            line.push_str("\",\"count\":");
            line.push_str(&count.to_string());
            line.push_str(",\"follow\":");
            line.push_str(if follow { "true" } else { "false" });
        }
        line.push('}');
        line
    }
}

#[derive(Debug, Clone, Copy)]
struct TimeoutBudget {
    start: Instant,
    timeout: Duration,
}

impl TimeoutBudget {
    fn new(timeout_ms: u64) -> Result<Self, RuntimeError> {
        if timeout_ms == 0 {
            return Err(RuntimeError::io_failure(
                "timeout-ms must be greater than 0",
            ));
        }
        Ok(Self {
            start: Instant::now(),
            timeout: Duration::from_millis(timeout_ms),
        })
    }

    fn run<T>(&self, step: &'static str, work: impl FnOnce() -> Result<T, RuntimeError>) -> Result<T, RuntimeError> {
        self.check(step)?;
        let value = work()?;
        self.check(step)?;
        Ok(value)
    }

    fn check(&self, step: &'static str) -> Result<(), RuntimeError> {
        let elapsed = self.start.elapsed();
        if elapsed > self.timeout {
            return Err(RuntimeError::io_failure(format!(
                "netdump {step} exceeded timeout budget (elapsed={}ms timeout={}ms)",
                elapsed.as_millis(),
                self.timeout.as_millis()
            )));
        }
        Ok(())
    }
}

fn validate_filter(filter: Option<&str>) -> Result<(), CliError> {
    let Some(filter) = filter else {
        return Ok(());
    };
    if filter.trim().is_empty() {
        return Ok(());
    }
    Err(CliError::argument_error(
        "netdump",
        "runtime netdump filtering is not implemented yet",
        "omit --filter for runtime capture",
    ))
}

fn validate_count(count: u64, follow: bool) -> Result<(), CliError> {
    if count == 0 {
        return Err(CliError::argument_error(
            "netdump",
            "count must be greater than 0",
            "set --count to 1 or more",
        ));
    }
    if !follow && count > 1 {
        return Err(CliError::argument_error(
            "netdump",
            "count greater than 1 requires --follow",
            "add --follow or use --count 1",
        ));
    }
    Ok(())
}

fn decode_event_summary(device_path: &str, raw: &[u8]) -> Result<NetdumpEventSummary, CliError> {
    let event = RecvEvent::decode(raw).map_err(|err| {
        map_runtime_error(
            "netdump",
            RuntimeError::io_failure(format!(
                "failed to decode runtime frame from {device_path}: {err}"
            )),
        )
    })?;
    let packet = event.packet().ok_or_else(|| {
        map_runtime_error(
            "netdump",
            RuntimeError::io_failure(format!(
                "decoded runtime event from {device_path} did not contain a network packet"
            )),
        )
    })?;
    let bytes = packet.bytes();
    if bytes.len() < 12 {
        return Err(map_runtime_error(
            "netdump",
            RuntimeError::io_failure(format!(
                "decoded runtime packet from {device_path} was shorter than the ipv4 checksum offset"
            )),
        ));
    }

    Ok(NetdumpEventSummary {
        ttl: bytes[8],
        checksum: u16::from_be_bytes([bytes[10], bytes[11]]),
        packet_len: bytes.len(),
        timestamp: capture_timestamp().map_err(|err| map_runtime_error("netdump", err))?,
    })
}

fn capture_timestamp() -> Result<String, RuntimeError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| RuntimeError::io_failure(format!("system clock before unix epoch: {err}")))?;
    Ok(now.as_millis().to_string())
}

fn render_text(
    events: &[NetdumpEventSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
) -> String {
    events
        .iter()
        .map(|event| event.render_text(verbose, device_path, count, follow))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_json(
    events: &[NetdumpEventSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
) -> String {
    if let [event] = events {
        let mut line = String::from("{\"command\":\"netdump\",\"status\":\"ok\",");
        line.push_str(&event.render_json(verbose, device_path, count, follow)[1..]);
        return line;
    }

    let mut line = String::from("{\"command\":\"netdump\",\"status\":\"ok\",\"events\":[");
    for (idx, event) in events.iter().enumerate() {
        if idx > 0 {
            line.push(',');
        }
        line.push_str(&event.render_json(verbose, device_path, count, follow));
    }
    line.push_str("]}");
    line
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ if (ch as u32) < 0x20 => {
                escaped.push_str("\\u");
                escaped.push('0');
                escaped.push('0');
                escaped.push(nibble_to_hex(((ch as u32 >> 4) & 0x0f) as u8));
                escaped.push(nibble_to_hex(((ch as u32) & 0x0f) as u8));
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'a' + (nibble - 10)),
    }
}

#[cfg(test)]
mod tests {
    use super::NetdumpCmd;
    use std::sync::{Arc, Mutex};
    use wd_user::{
        DeviceAvailability, RuntimeError, RuntimeOpenConfig, RuntimeProbe, RuntimeSession, RuntimeTransport,
        default_device_path,
    };

    #[derive(Debug)]
    struct MissingDeviceTransport;

    #[derive(Debug)]
    struct BufferedSession {
        frames: Vec<Vec<u8>>,
    }

    impl RuntimeSession for BufferedSession {
        fn recv_one(&mut self, _max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
            if self.frames.is_empty() {
                return Err(RuntimeError::io_failure("buffer exhausted"));
            }
            Ok(self.frames.remove(0))
        }

        fn send_one(&mut self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn close(self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    impl RuntimeTransport for MissingDeviceTransport {
        type Session = BufferedSession;

        fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
            Ok(DeviceAvailability::Missing)
        }

        fn open(&self, _config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
            Err(RuntimeError::device_unavailable(default_device_path()))
        }

        fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
            Err(RuntimeError::device_unavailable(default_device_path()))
        }
    }

    #[derive(Debug)]
    struct BufferedTransport {
        frames: Vec<Vec<u8>>,
    }

    impl RuntimeTransport for BufferedTransport {
        type Session = BufferedSession;

        fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
            Ok(DeviceAvailability::Present)
        }

        fn open(&self, _config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
            Ok(RuntimeProbe {
                device_path: default_device_path().to_string(),
                capabilities: None,
                protocol_major: None,
                protocol_minor: None,
            })
        }

        fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
            Ok(BufferedSession {
                frames: self.frames.clone(),
            })
        }
    }

    #[derive(Debug)]
    struct RecordingTransport {
        frames: Vec<Vec<u8>>,
        opens: Arc<Mutex<Vec<RuntimeOpenConfig>>>,
    }

    impl RuntimeTransport for RecordingTransport {
        type Session = BufferedSession;

        fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
            Ok(DeviceAvailability::Present)
        }

        fn open(&self, config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
            self.opens
                .lock()
                .expect("open config mutex should not be poisoned")
                .push(config.clone());
            Ok(RuntimeProbe {
                device_path: default_device_path().to_string(),
                capabilities: None,
                protocol_major: None,
                protocol_minor: None,
            })
        }

        fn open_session(&self, config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
            self.opens
                .lock()
                .expect("open config mutex should not be poisoned")
                .push(config.clone());
            Ok(BufferedSession {
                frames: self.frames.clone(),
            })
        }
    }

    #[test]
    fn netdump_missing_device_maps_to_cli_error() {
        let cmd = NetdumpCmd {
            filter: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let err = cmd
            .execute_with_transport(MissingDeviceTransport)
            .expect_err("missing device should surface as cli error");
        assert_eq!(err.code, 3);
        assert_eq!(err.category, "device_unavailable");
    }

    #[test]
    fn netdump_decodes_runtime_frame_into_summary() {
        let cmd = NetdumpCmd {
            filter: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let line = cmd
            .execute_with_transport(BufferedTransport {
                frames: vec![crate::fixtures::ipv4_frame()],
            })
            .expect("runtime frame should decode");
        assert!(line.contains("NETDUMP OK"), "unexpected output: {line}");
        assert!(line.contains("layer=NETWORK"), "unexpected output: {line}");
        assert!(line.contains("ttl="), "unexpected output: {line}");
        assert!(line.contains("checksum="), "unexpected output: {line}");
        assert!(line.contains("packet_len="), "unexpected output: {line}");
        assert!(line.contains("timestamp="), "unexpected output: {line}");
    }

    #[test]
    fn netdump_passes_network_open_config_without_filter_ir() {
        let cmd = NetdumpCmd {
            filter: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let opens = Arc::new(Mutex::new(Vec::new()));

        cmd.execute_with_transport(RecordingTransport {
            frames: vec![crate::fixtures::ipv4_frame()],
            opens: Arc::clone(&opens),
        })
        .expect("netdump should use runtime network open config");

        let opens = opens.lock().expect("open config mutex should not be poisoned");
        assert_eq!(
            *opens,
            vec![
                RuntimeOpenConfig::network(Vec::new()),
                RuntimeOpenConfig::network(Vec::new())
            ]
        );
    }

    #[test]
    fn netdump_count_greater_than_one_requires_follow() {
        let cmd = NetdumpCmd {
            filter: None,
            count: 2,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let err = cmd
            .execute_with_transport(BufferedTransport { frames: Vec::new() })
            .expect_err("count>1 without follow should fail before runtime access");
        assert_eq!(err.code, 2);
        assert_eq!(err.category, "argument_error");
    }
}
