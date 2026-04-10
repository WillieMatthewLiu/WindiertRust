use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Args;
use wd_proto::FlowEventKind;
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
pub struct FlowtrackCmd {
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

impl FlowtrackCmd {
    pub fn run(self) -> ExitCode {
        let mode = OutputMode::from_json_flag(self.json);
        finish_with_cli_error(self.execute_with_transport(default_transport()), mode)
    }

    fn execute_with_transport<T: RuntimeTransport>(self, transport: T) -> Result<String, CliError> {
        let FlowtrackCmd {
            process_id,
            count,
            follow,
            json,
            verbose,
            timeout_ms,
        } = self;

        validate_count(count, follow)?;
        let budget = TimeoutBudget::new(timeout_ms).map_err(|err| map_runtime_error("flowtrack", err))?;
        let open_config = RuntimeOpenConfig::flow();

        let availability = budget
            .run("probe", || transport.probe())
            .map_err(|err| map_runtime_error("flowtrack", err))?;
        if availability == DeviceAvailability::Missing {
            return Err(map_runtime_error(
                "flowtrack",
                RuntimeError::device_unavailable(default_device_path()),
            ));
        }

        let probe = budget
            .run("open", || transport.open(&open_config))
            .map_err(|err| map_runtime_error("flowtrack", err))?;
        let mut session = budget
            .run("open_session", || transport.open_session(&open_config))
            .map_err(|err| map_runtime_error("flowtrack", err))?;

        let mut matches = Vec::new();
        while matches.len() < count as usize {
            let raw = budget
                .run("recv", || session.recv_one(MAX_RECV_BYTES))
                .map_err(|err| map_runtime_error("flowtrack", err))?;
            let summary = decode_flow_summary(&probe.device_path, &raw, process_id)?;
            if let Some(summary) = summary {
                matches.push(summary);
            }
        }

        budget
            .run("close_session", || session.close())
            .map_err(|err| map_runtime_error("flowtrack", err))?;

        Ok(match json {
            true => render_json(&matches, verbose, &probe.device_path, count, follow),
            false => render_text(&matches, verbose, &probe.device_path, count, follow),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowtrackSummary {
    event: FlowEventKind,
    flow_id: u64,
    process_id: u64,
    timestamp: String,
}

impl FlowtrackSummary {
    fn render_text(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut fields = vec![
            ("event", render_flow_event_kind(self.event).to_string()),
            ("flow_id", self.flow_id.to_string()),
            ("process_id", self.process_id.to_string()),
            ("timestamp", self.timestamp.clone()),
        ];
        if verbose {
            fields.push(("device_path", device_path.to_string()));
            fields.push(("count", count.to_string()));
            fields.push(("follow", follow.to_string()));
        }
        render_summary("FLOWTRACK OK", &fields)
    }

    fn render_json(&self, verbose: bool, device_path: &str, count: u64, follow: bool) -> String {
        let mut line = format!(
            "{{\"event\":\"{}\",\"flow_id\":{},\"process_id\":{},\"timestamp\":\"{}\"",
            render_flow_event_kind(self.event),
            self.flow_id,
            self.process_id,
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
                "flowtrack {step} exceeded timeout budget (elapsed={}ms timeout={}ms)",
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
            "flowtrack",
            "count must be greater than 0",
            "set --count to 1 or more",
        ));
    }
    if !follow && count > 1 {
        return Err(CliError::argument_error(
            "flowtrack",
            "count greater than 1 requires --follow",
            "add --follow or use --count 1",
        ));
    }
    Ok(())
}

fn decode_flow_summary(
    device_path: &str,
    raw: &[u8],
    process_id_filter: Option<u64>,
) -> Result<Option<FlowtrackSummary>, CliError> {
    let event = RecvEvent::decode(raw).map_err(|err| {
        map_runtime_error(
            "flowtrack",
            RuntimeError::io_failure(format!(
                "failed to decode runtime flow frame from {device_path}: {err}"
            )),
        )
    })?;
    let flow = event.flow().ok_or_else(|| {
        map_runtime_error(
            "flowtrack",
            RuntimeError::io_failure(format!(
                "decoded runtime event from {device_path} did not contain a flow event"
            )),
        )
    })?;
    if let Some(expected) = process_id_filter {
        if flow.process_id() != expected {
            return Ok(None);
        }
    }

    Ok(Some(FlowtrackSummary {
        event: flow.kind(),
        flow_id: flow.flow_id(),
        process_id: flow.process_id(),
        timestamp: capture_timestamp().map_err(|err| map_runtime_error("flowtrack", err))?,
    }))
}

fn render_text(
    matches: &[FlowtrackSummary],
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
    matches: &[FlowtrackSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
) -> String {
    if let [summary] = matches {
        let mut line = String::from("{\"command\":\"flowtrack\",\"status\":\"ok\",");
        line.push_str(&summary.render_json(verbose, device_path, count, follow)[1..]);
        return line;
    }

    let mut line = String::from("{\"command\":\"flowtrack\",\"status\":\"ok\",\"events\":[");
    for (idx, summary) in matches.iter().enumerate() {
        if idx > 0 {
            line.push(',');
        }
        line.push_str(&summary.render_json(verbose, device_path, count, follow));
    }
    line.push_str("]}");
    line
}

fn render_flow_event_kind(kind: FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::Established => "ESTABLISHED",
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
    use super::FlowtrackCmd;
    use std::sync::{Arc, Mutex};
    use wd_kmdf::RuntimeDevice;
    use wd_proto::{FlowEventKind, Layer, encode_flow_event_payload, encode_runtime_event};
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
        kind: FlowEventKind,
        flow_id: u64,
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
                .queue_flow_event(self.kind, self.flow_id, self.process_id)
                .map_err(|err| RuntimeError::io_failure(format!("loopback queue failed: {err}")))?;
            Ok(LoopbackSession { device })
        }
    }

    #[test]
    fn flowtrack_missing_device_maps_to_cli_error() {
        let cmd = FlowtrackCmd {
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
    fn flowtrack_decodes_runtime_event_summary() {
        let cmd = FlowtrackCmd {
            process_id: Some(42),
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let frame = encode_runtime_event(
            Layer::Flow,
            &encode_flow_event_payload(FlowEventKind::Established, 9001, 42),
        );

        let line = cmd
            .execute_with_transport(BufferedTransport { frames: vec![frame] })
            .expect("flow runtime event should render");
        assert!(line.contains("FLOWTRACK OK"), "unexpected output: {line}");
        assert!(line.contains("event=ESTABLISHED"), "unexpected output: {line}");
        assert!(line.contains("flow_id=9001"), "unexpected output: {line}");
        assert!(line.contains("process_id=42"), "unexpected output: {line}");
    }

    #[test]
    fn flowtrack_loopback_transport_uses_driver_core_flow_event_path() {
        let cmd = FlowtrackCmd {
            process_id: Some(42),
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let line = cmd
            .execute_with_transport(LoopbackTransport {
                kind: FlowEventKind::Established,
                flow_id: 9001,
                process_id: 42,
            })
            .expect("loopback flow runtime event should render");
        assert!(line.contains("FLOWTRACK OK"), "unexpected output: {line}");
        assert!(line.contains("event=ESTABLISHED"), "unexpected output: {line}");
        assert!(line.contains("flow_id=9001"), "unexpected output: {line}");
        assert!(line.contains("process_id=42"), "unexpected output: {line}");
    }

    #[test]
    fn flowtrack_passes_flow_open_config_to_runtime_transport() {
        let cmd = FlowtrackCmd {
            process_id: None,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let opens = Arc::new(Mutex::new(Vec::new()));
        let frame = encode_runtime_event(
            Layer::Flow,
            &encode_flow_event_payload(FlowEventKind::Established, 9001, 42),
        );

        cmd.execute_with_transport(RecordingTransport {
            frames: vec![frame],
            opens: Arc::clone(&opens),
        })
        .expect("flowtrack should use runtime flow open config");

        let opens = opens.lock().expect("open config mutex should not be poisoned");
        assert_eq!(
            *opens,
            vec![RuntimeOpenConfig::flow(), RuntimeOpenConfig::flow()]
        );
    }

    #[test]
    fn flowtrack_count_greater_than_one_requires_follow() {
        let cmd = FlowtrackCmd {
            process_id: None,
            count: 2,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let err = cmd
            .execute_with_transport(BufferedTransport { frames: Vec::new() })
            .expect_err("count>1 without follow should fail");
        assert_eq!(err.code, 2);
        assert_eq!(err.category, "argument_error");
    }
}
