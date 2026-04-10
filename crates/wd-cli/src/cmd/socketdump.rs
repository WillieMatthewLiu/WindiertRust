use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Args;
use wd_kmdf::{DriverEvent, FilterEngine};
use wd_proto::{Layer, SocketEventKind};
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
pub struct SocketdumpCmd {
    #[arg(long)]
    filter: String,
    #[arg(long)]
    process_id: Option<u64>,
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

impl SocketdumpCmd {
    pub fn run(self) -> ExitCode {
        let mode = OutputMode::from_json_flag(self.json);
        finish_with_cli_error(self.execute_with_transport(default_transport()), mode)
    }

    fn execute_with_transport<T: RuntimeTransport>(self, transport: T) -> Result<String, CliError> {
        let SocketdumpCmd {
            filter,
            process_id,
            count,
            follow,
            json,
            verbose,
            timeout_ms,
        } = self;

        validate_count(count, follow)?;
        let engine = FilterEngine::compile(Layer::Socket, &filter).map_err(|err| {
            CliError::argument_error(
                "socketdump",
                format!("invalid socket filter: {err}"),
                "use runtime-supported socket predicates such as event == CONNECT",
            )
        })?;
        let budget = TimeoutBudget::new(timeout_ms).map_err(|err| map_runtime_error("socketdump", err))?;
        let open_config = RuntimeOpenConfig::socket();

        let availability = budget
            .run("probe", || transport.probe())
            .map_err(|err| map_runtime_error("socketdump", err))?;
        if availability == DeviceAvailability::Missing {
            return Err(map_runtime_error(
                "socketdump",
                RuntimeError::device_unavailable(default_device_path()),
            ));
        }

        let probe = budget
            .run("open", || transport.open(&open_config))
            .map_err(|err| map_runtime_error("socketdump", err))?;
        let mut session = budget
            .run("open_session", || transport.open_session(&open_config))
            .map_err(|err| map_runtime_error("socketdump", err))?;

        let mut matches = Vec::new();
        while matches.len() < count as usize {
            let raw = budget
                .run("recv", || session.recv_one(MAX_RECV_BYTES))
                .map_err(|err| map_runtime_error("socketdump", err))?;
            let summary = decode_socket_summary(&probe.device_path, &raw, process_id, &engine)?;
            if let Some(summary) = summary {
                matches.push(summary);
            }
        }

        budget
            .run("close_session", || session.close())
            .map_err(|err| map_runtime_error("socketdump", err))?;

        Ok(match json {
            true => render_json(&matches, verbose, &probe.device_path, count, follow),
            false => render_text(&matches, verbose, &probe.device_path, count, follow),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SocketdumpSummary {
    event: SocketEventKind,
    process_id: u64,
    matched: bool,
    timestamp: String,
}

impl SocketdumpSummary {
    fn render_text(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut fields = vec![
            ("event", render_socket_event_kind(self.event).to_string()),
            ("process_id", self.process_id.to_string()),
            ("matched", self.matched.to_string()),
            ("timestamp", self.timestamp.clone()),
        ];
        if verbose {
            fields.push(("device_path", device_path.to_string()));
            fields.push(("count", count.to_string()));
            fields.push(("follow", follow.to_string()));
        }
        render_summary("SOCKETDUMP OK", &fields)
    }

    fn render_json(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut line = format!(
            "{{\"event\":\"{}\",\"process_id\":{},\"matched\":{},\"timestamp\":\"{}\"",
            render_socket_event_kind(self.event),
            self.process_id,
            if self.matched { "true" } else { "false" },
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
                "socketdump {step} exceeded timeout budget (elapsed={}ms timeout={}ms)",
                elapsed.as_millis(),
                self.timeout.as_millis()
            )));
        }
        Ok(())
    }
}

fn validate_count(count: u64, follow: bool) -> Result<(), CliError> {
    if count == 0 {
        return Err(CliError::argument_error(
            "socketdump",
            "count must be greater than 0",
            "set --count to 1 or more",
        ));
    }
    if !follow && count > 1 {
        return Err(CliError::argument_error(
            "socketdump",
            "count greater than 1 requires --follow",
            "add --follow or use --count 1",
        ));
    }
    Ok(())
}

fn decode_socket_summary(
    device_path: &str,
    raw: &[u8],
    process_id_filter: Option<u64>,
    engine: &FilterEngine,
) -> Result<Option<SocketdumpSummary>, CliError> {
    let event = RecvEvent::decode(raw).map_err(|err| {
        map_runtime_error(
            "socketdump",
            RuntimeError::io_failure(format!(
                "failed to decode runtime socket frame from {device_path}: {err}"
            )),
        )
    })?;
    let socket = event.socket().ok_or_else(|| {
        map_runtime_error(
            "socketdump",
            RuntimeError::io_failure(format!(
                "decoded runtime event from {device_path} did not contain a socket event"
            )),
        )
    })?;
    if let Some(expected) = process_id_filter {
        if socket.process_id() != expected {
            return Ok(None);
        }
    }

    let driver_event = DriverEvent::socket_connect(socket.process_id());
    if !engine.matches(&driver_event) {
        return Ok(None);
    }

    Ok(Some(SocketdumpSummary {
        event: socket.kind(),
        process_id: socket.process_id(),
        matched: true,
        timestamp: capture_timestamp().map_err(|err| map_runtime_error("socketdump", err))?,
    }))
}

fn render_text(
    matches: &[SocketdumpSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
) -> String {
    matches
        .iter()
        .map(|summary| summary.render_text(verbose, device_path, count, follow))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_json(
    matches: &[SocketdumpSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
) -> String {
    if let [summary] = matches {
        let mut line = String::from("{\"command\":\"socketdump\",\"status\":\"ok\",");
        line.push_str(&summary.render_json(verbose, device_path, count, follow)[1..]);
        return line;
    }

    let mut line = String::from("{\"command\":\"socketdump\",\"status\":\"ok\",\"events\":[");
    for (idx, summary) in matches.iter().enumerate() {
        if idx > 0 {
            line.push(',');
        }
        line.push_str(&summary.render_json(verbose, device_path, count, follow));
    }
    line.push_str("]}");
    line
}

fn render_socket_event_kind(kind: SocketEventKind) -> &'static str {
    match kind {
        SocketEventKind::Connect => "CONNECT",
    }
}

fn capture_timestamp() -> Result<String, RuntimeError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| RuntimeError::io_failure(format!("system clock before unix epoch: {err}")))?;
    Ok(now.as_millis().to_string())
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
    use super::SocketdumpCmd;
    use std::sync::{Arc, Mutex};
    use wd_kmdf::RuntimeDevice;
    use wd_proto::{Layer, SocketEventKind, encode_runtime_event, encode_socket_event_payload};
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

    #[derive(Debug)]
    struct LoopbackSession {
        device: RuntimeDevice,
    }

    impl RuntimeSession for LoopbackSession {
        fn recv_one(&mut self, _max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut output = [0u8; 2048];
            let written = self
                .device
                .recv_into(&mut output)
                .map_err(|err| RuntimeError::io_failure(format!("loopback recv failed: {err}")))?;
            Ok(output[..written].to_vec())
        }

        fn send_one(&mut self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn close(self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct LoopbackTransport {
        kind: SocketEventKind,
        process_id: u64,
    }

    impl RuntimeTransport for LoopbackTransport {
        type Session = LoopbackSession;

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
            let mut device = RuntimeDevice::new(8);
            device
                .open()
                .map_err(|err| RuntimeError::io_failure(format!("loopback open failed: {err}")))?;
            device
                .queue_socket_event(self.kind, self.process_id)
                .map_err(|err| RuntimeError::io_failure(format!("loopback queue failed: {err}")))?;
            Ok(LoopbackSession { device })
        }
    }

    #[test]
    fn socketdump_missing_device_maps_to_cli_error() {
        let cmd = SocketdumpCmd {
            filter: "event == CONNECT".to_string(),
            process_id: None,
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
    fn socketdump_decodes_runtime_event_and_matches_filter() {
        let cmd = SocketdumpCmd {
            filter: "event == CONNECT and processId == 7".to_string(),
            process_id: Some(7),
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let frame = encode_runtime_event(
            Layer::Socket,
            &encode_socket_event_payload(SocketEventKind::Connect, 7),
        );

        let line = cmd
            .execute_with_transport(BufferedTransport { frames: vec![frame] })
            .expect("socket runtime event should render");
        assert!(line.contains("SOCKETDUMP OK"), "unexpected output: {line}");
        assert!(line.contains("event=CONNECT"), "unexpected output: {line}");
        assert!(line.contains("process_id=7"), "unexpected output: {line}");
        assert!(line.contains("matched=true"), "unexpected output: {line}");
    }

    #[test]
    fn socketdump_loopback_transport_uses_driver_core_socket_event_path() {
        let cmd = SocketdumpCmd {
            filter: "event == CONNECT and processId == 7".to_string(),
            process_id: Some(7),
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let line = cmd
            .execute_with_transport(LoopbackTransport {
                kind: SocketEventKind::Connect,
                process_id: 7,
            })
            .expect("loopback socket runtime event should render");
        assert!(line.contains("SOCKETDUMP OK"), "unexpected output: {line}");
        assert!(line.contains("event=CONNECT"), "unexpected output: {line}");
        assert!(line.contains("process_id=7"), "unexpected output: {line}");
        assert!(line.contains("matched=true"), "unexpected output: {line}");
    }

    #[test]
    fn socketdump_passes_socket_open_config_to_runtime_transport() {
        let cmd = SocketdumpCmd {
            filter: "event == CONNECT".to_string(),
            process_id: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let opens = Arc::new(Mutex::new(Vec::new()));
        let frame = encode_runtime_event(
            Layer::Socket,
            &encode_socket_event_payload(SocketEventKind::Connect, 77),
        );

        cmd.execute_with_transport(RecordingTransport {
            frames: vec![frame],
            opens: Arc::clone(&opens),
        })
        .expect("socketdump should use runtime socket open config");

        let opens = opens.lock().expect("open config mutex should not be poisoned");
        assert_eq!(
            *opens,
            vec![RuntimeOpenConfig::socket(), RuntimeOpenConfig::socket()]
        );
    }

    #[test]
    fn socketdump_invalid_filter_maps_to_argument_error() {
        let cmd = SocketdumpCmd {
            filter: "tcp and inbound".to_string(),
            process_id: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let err = cmd
            .execute_with_transport(BufferedTransport { frames: Vec::new() })
            .expect_err("invalid runtime subset filter should fail");
        assert_eq!(err.code, 2);
        assert_eq!(err.category, "argument_error");
    }
}
