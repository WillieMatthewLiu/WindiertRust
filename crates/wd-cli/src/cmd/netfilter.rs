use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use wd_proto::{Layer, encode_runtime_send_request};
use wd_user::{
    DeviceAvailability, HandleConfig, RecvEvent, RuntimeError, RuntimeOpenConfig, RuntimeSession,
    RuntimeTransport, default_device_path,
};

use crate::cmd::common::{finish_with_cli_error, render_summary};
use crate::error::CliError;
use crate::output::OutputMode;
use crate::runtime::{default_transport, map_runtime_error};

const MAX_RECV_BYTES: usize = 65_535;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum NetfilterMode {
    Validate,
    Observe,
    Reinject,
}

#[derive(Debug, Args)]
pub struct NetfilterCmd {
    #[arg(long)]
    filter: String,
    #[arg(long, value_enum, default_value_t = NetfilterMode::Validate)]
    mode: NetfilterMode,
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

impl NetfilterCmd {
    pub fn run(self) -> ExitCode {
        let mode = OutputMode::from_json_flag(self.json);
        finish_with_cli_error(self.execute_with_transport(default_transport()), mode)
    }

    fn execute_with_transport<T: RuntimeTransport>(self, transport: T) -> Result<String, CliError> {
        let NetfilterCmd {
            filter,
            mode,
            count,
            follow,
            json,
            verbose,
            timeout_ms,
        } = self;

        validate_count(count, follow, mode)?;
        let cfg = HandleConfig::network(&filter).map_err(|err| {
            CliError::argument_error(
                "netfilter",
                format!("invalid network filter: {err}"),
                "use a valid network-layer filter expression",
            )
        })?;
        let budget = TimeoutBudget::new(timeout_ms).map_err(|err| map_runtime_error("netfilter", err))?;
        let open_config = RuntimeOpenConfig::network(cfg.filter_ir().to_vec());

        let availability = budget
            .run("probe", || transport.probe())
            .map_err(|err| map_runtime_error("netfilter", err))?;
        if availability == DeviceAvailability::Missing {
            return Err(map_runtime_error(
                "netfilter",
                RuntimeError::device_unavailable(default_device_path()),
            ));
        }

        let probe = budget
            .run("open", || transport.open(&open_config))
            .map_err(|err| map_runtime_error("netfilter", err))?;

        let line = match mode {
            NetfilterMode::Validate => render_validate(
                &filter,
                cfg.filter_ir().len(),
                verbose,
                &probe.device_path,
                timeout_ms,
                json,
            ),
            NetfilterMode::Observe => {
                let mut session = budget
                    .run("open_session", || transport.open_session(&open_config))
                    .map_err(|err| map_runtime_error("netfilter", err))?;

                let mut events = Vec::new();
                for _ in 0..count {
                    let raw = budget
                        .run("recv", || session.recv_one(MAX_RECV_BYTES))
                        .map_err(|err| map_runtime_error("netfilter", err))?;
                    events.push(decode_network_summary(&probe.device_path, &raw)?);
                }

                budget
                    .run("close_session", || session.close())
                    .map_err(|err| map_runtime_error("netfilter", err))?;

                render_observe(
                    &filter,
                    &events,
                    verbose,
                    &probe.device_path,
                    count,
                    follow,
                    json,
                )
            }
            NetfilterMode::Reinject => {
                let mut session = budget
                    .run("open_session", || transport.open_session(&open_config))
                    .map_err(|err| map_runtime_error("netfilter", err))?;
                let raw = budget
                    .run("recv", || session.recv_one(MAX_RECV_BYTES))
                    .map_err(|err| map_runtime_error("netfilter", err))?;
                let packet = decode_network_reinject_packet(&probe.device_path, &raw)?;
                let request = encode_runtime_send_request(
                    packet.layer,
                    packet.reinjection_token,
                    &packet.packet_bytes,
                );
                budget
                    .run("send", || session.send_one(&request))
                    .map_err(|err| map_runtime_error("netfilter", err))?;
                budget
                    .run("close_session", || session.close())
                    .map_err(|err| map_runtime_error("netfilter", err))?;

                render_reinject(
                    &filter,
                    &packet,
                    verbose,
                    &probe.device_path,
                    timeout_ms,
                    json,
                )
            }
        };

        Ok(line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkSummary {
    ttl: u8,
    checksum: u16,
    packet_len: usize,
    timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReinjectPacket {
    layer: Layer,
    reinjection_token: u64,
    ttl: u8,
    checksum: u16,
    packet_len: usize,
    timestamp: String,
    packet_bytes: Vec<u8>,
}

impl NetworkSummary {
    fn render_text(
        &self,
        filter: &str,
        verbose: bool,
        device_path: &str,
        count: u64,
        follow: bool,
    ) -> String {
        let mut fields = vec![
            ("mode", "observe".to_string()),
            ("filter", filter.to_string()),
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
        render_summary("NETFILTER OK", &fields)
    }

    fn render_json(
        &self,
        filter: &str,
        verbose: bool,
        device_path: &str,
        count: u64,
        follow: bool,
    ) -> String {
        let mut line = format!(
            "{{\"mode\":\"observe\",\"filter\":\"{}\",\"layer\":\"NETWORK\",\"ttl\":{},\"checksum\":\"0x{:04x}\",\"packet_len\":{},\"timestamp\":\"{}\"",
            escape_json_string(filter),
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
                "netfilter {step} exceeded timeout budget (elapsed={}ms timeout={}ms)",
                elapsed.as_millis(),
                self.timeout.as_millis()
            )));
        }
        Ok(())
    }
}

fn validate_count(count: u64, follow: bool, mode: NetfilterMode) -> Result<(), CliError> {
    if count == 0 {
        return Err(CliError::argument_error(
            "netfilter",
            "count must be greater than 0",
            "set --count to 1 or more",
        ));
    }
    if matches!(mode, NetfilterMode::Validate | NetfilterMode::Reinject) && (count != 1 || follow) {
        return Err(CliError::argument_error(
            "netfilter",
            "validate and reinject modes do not accept streaming count/follow semantics",
            "omit --count/--follow outside observe mode",
        ));
    }
    if mode == NetfilterMode::Observe && !follow && count > 1 {
        return Err(CliError::argument_error(
            "netfilter",
            "count greater than 1 requires --follow in observe mode",
            "add --follow or use --count 1",
        ));
    }
    Ok(())
}

fn render_validate(
    filter: &str,
    ir_bytes: usize,
    verbose: bool,
    device_path: &str,
    timeout_ms: u64,
    json: bool,
) -> String {
    if json {
        let mut line = format!(
            "{{\"command\":\"netfilter\",\"status\":\"ok\",\"mode\":\"validate\",\"filter\":\"{}\",\"layer\":\"NETWORK\",\"ir_bytes\":{}",
            escape_json_string(filter),
            ir_bytes,
        );
        if verbose {
            line.push_str(",\"device_path\":\"");
            line.push_str(&escape_json_string(device_path));
            line.push_str("\",\"timeout_ms\":");
            line.push_str(&timeout_ms.to_string());
        }
        line.push('}');
        return line;
    }

    let mut fields = vec![
        ("mode", "validate".to_string()),
        ("layer", "NETWORK".to_string()),
        ("filter", filter.to_string()),
        ("ir_bytes", ir_bytes.to_string()),
    ];
    if verbose {
        fields.push(("device_path", device_path.to_string()));
        fields.push(("timeout_ms", timeout_ms.to_string()));
    }
    render_summary("NETFILTER OK", &fields)
}

fn render_observe(
    filter: &str,
    events: &[NetworkSummary],
    verbose: bool,
    device_path: &str,
    count: u64,
    follow: bool,
    json: bool,
) -> String {
    if json {
        if let [event] = events {
            let mut line = String::from("{\"command\":\"netfilter\",\"status\":\"ok\",");
            line.push_str(&event.render_json(filter, verbose, device_path, count, follow)[1..]);
            return line;
        }

        let mut line = String::from("{\"command\":\"netfilter\",\"status\":\"ok\",\"events\":[");
        for (idx, event) in events.iter().enumerate() {
            if idx > 0 {
                line.push(',');
            }
            line.push_str(&event.render_json(filter, verbose, device_path, count, follow));
        }
        line.push_str("]}");
        return line;
    }

    events
        .iter()
        .map(|event| event.render_text(filter, verbose, device_path, count, follow))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_reinject(
    filter: &str,
    packet: &ReinjectPacket,
    verbose: bool,
    device_path: &str,
    timeout_ms: u64,
    json: bool,
) -> String {
    if json {
        let mut line = format!(
            "{{\"command\":\"netfilter\",\"status\":\"ok\",\"mode\":\"reinject\",\"filter\":\"{}\",\"layer\":\"{}\",\"reinjection_token\":{},\"ttl\":{},\"checksum\":\"0x{:04x}\",\"packet_len\":{},\"timestamp\":\"{}\"",
            escape_json_string(filter),
            runtime_layer_name(packet.layer),
            packet.reinjection_token,
            packet.ttl,
            packet.checksum,
            packet.packet_len,
            escape_json_string(&packet.timestamp),
        );
        if verbose {
            line.push_str(",\"device_path\":\"");
            line.push_str(&escape_json_string(device_path));
            line.push_str("\",\"timeout_ms\":");
            line.push_str(&timeout_ms.to_string());
        }
        line.push('}');
        return line;
    }

    let mut fields = vec![
        ("mode", "reinject".to_string()),
        ("filter", filter.to_string()),
        ("layer", runtime_layer_name(packet.layer).to_string()),
        ("reinjection_token", packet.reinjection_token.to_string()),
        ("ttl", packet.ttl.to_string()),
        ("checksum", format!("0x{:04x}", packet.checksum)),
        ("packet_len", packet.packet_len.to_string()),
        ("timestamp", packet.timestamp.clone()),
    ];
    if verbose {
        fields.push(("device_path", device_path.to_string()));
        fields.push(("timeout_ms", timeout_ms.to_string()));
    }
    render_summary("NETFILTER OK", &fields)
}

fn decode_network_summary(device_path: &str, raw: &[u8]) -> Result<NetworkSummary, CliError> {
    let event = RecvEvent::decode(raw).map_err(|err| {
        map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "failed to decode runtime network frame from {device_path}: {err}"
            )),
        )
    })?;
    let packet = event.packet().ok_or_else(|| {
        map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "decoded runtime event from {device_path} did not contain a network packet"
            )),
        )
    })?;
    let bytes = packet.bytes();
    if bytes.len() < 12 {
        return Err(map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "decoded runtime packet from {device_path} was shorter than the ipv4 checksum offset"
            )),
        ));
    }

    Ok(NetworkSummary {
        ttl: bytes[8],
        checksum: u16::from_be_bytes([bytes[10], bytes[11]]),
        packet_len: bytes.len(),
        timestamp: capture_timestamp().map_err(|err| map_runtime_error("netfilter", err))?,
    })
}

fn decode_network_reinject_packet(device_path: &str, raw: &[u8]) -> Result<ReinjectPacket, CliError> {
    let event = RecvEvent::decode(raw).map_err(|err| {
        map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "failed to decode runtime network frame from {device_path}: {err}"
            )),
        )
    })?;
    let packet = event.packet().ok_or_else(|| {
        map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "decoded runtime event from {device_path} did not contain a network packet"
            )),
        )
    })?;
    let bytes = packet.bytes();
    if bytes.len() < 12 {
        return Err(map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "decoded runtime packet from {device_path} was shorter than the ipv4 checksum offset"
            )),
        ));
    }
    let reinjection_token = packet.reinjection_token().ok_or_else(|| {
        map_runtime_error(
            "netfilter",
            RuntimeError::io_failure(format!(
                "decoded runtime packet from {device_path} did not include a reinjection token"
            )),
        )
    })?;

    Ok(ReinjectPacket {
        layer: packet.layer(),
        reinjection_token,
        ttl: bytes[8],
        checksum: u16::from_be_bytes([bytes[10], bytes[11]]),
        packet_len: bytes.len(),
        timestamp: capture_timestamp().map_err(|err| map_runtime_error("netfilter", err))?,
        packet_bytes: bytes.to_vec(),
    })
}

fn runtime_layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Network => "NETWORK",
        Layer::NetworkForward => "NETWORK_FORWARD",
        Layer::Flow => "FLOW",
        Layer::Socket => "SOCKET",
        Layer::Reflect => "REFLECT",
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
    use super::{NetfilterCmd, NetfilterMode};
    use std::sync::{Arc, Mutex};
    use wd_kmdf::{AcceptedReinjection, RuntimeDevice};
    use wd_proto::{
        Layer, decode_runtime_send_request, encode_network_event_payload, encode_runtime_event,
    };
    use wd_user::{
        DeviceAvailability, HandleConfig, RuntimeError, RuntimeOpenConfig, RuntimeProbe,
        RuntimeSession, RuntimeTransport, default_device_path,
    };

    #[derive(Debug)]
    struct MissingDeviceTransport;

    #[derive(Debug)]
    struct BufferedSession {
        frames: Vec<Vec<u8>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl RuntimeSession for BufferedSession {
        fn recv_one(&mut self, _max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
            if self.frames.is_empty() {
                return Err(RuntimeError::io_failure("buffer exhausted"));
            }
            Ok(self.frames.remove(0))
        }

        fn send_one(&mut self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            self.sent
                .lock()
                .expect("send buffer mutex should not be poisoned")
                .push(_bytes.to_vec());
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
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
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
                sent: Arc::clone(&self.sent),
            })
        }
    }

    #[derive(Debug)]
    struct RecordingTransport {
        frames: Vec<Vec<u8>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
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
                sent: Arc::clone(&self.sent),
            })
        }
    }

    #[derive(Debug)]
    struct LoopbackSession {
        device: RuntimeDevice,
        accepted: Arc<Mutex<Option<AcceptedReinjection>>>,
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

        fn send_one(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
            let accepted = self
                .device
                .send(bytes)
                .map_err(|err| RuntimeError::io_failure(format!("loopback send failed: {err}")))?;
            *self
                .accepted
                .lock()
                .expect("accepted mutex should not be poisoned") = Some(accepted);
            Ok(())
        }

        fn close(self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct LoopbackTransport {
        layer: Layer,
        packet_id: u64,
        packet: Vec<u8>,
        accepted: Arc<Mutex<Option<AcceptedReinjection>>>,
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
                .queue_network_event(self.layer, self.packet_id, &self.packet)
                .map_err(|err| RuntimeError::io_failure(format!("loopback queue failed: {err}")))?;

            Ok(LoopbackSession {
                device,
                accepted: Arc::clone(&self.accepted),
            })
        }
    }

    #[test]
    fn netfilter_validate_missing_device_maps_to_cli_error() {
        let cmd = NetfilterCmd {
            filter: "tcp and inbound".to_string(),
            mode: NetfilterMode::Validate,
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
    fn netfilter_observe_decodes_runtime_network_summary() {
        let cmd = NetfilterCmd {
            filter: "tcp and inbound".to_string(),
            mode: NetfilterMode::Observe,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let line = cmd
            .execute_with_transport(BufferedTransport {
                frames: vec![crate::fixtures::ipv4_frame()],
                sent: Arc::new(Mutex::new(Vec::new())),
            })
            .expect("observe mode should render runtime network event");
        assert!(line.contains("NETFILTER OK"), "unexpected output: {line}");
        assert!(line.contains("mode=observe"), "unexpected output: {line}");
        assert!(line.contains("layer=NETWORK"), "unexpected output: {line}");
    }

    #[test]
    fn netfilter_observe_passes_network_open_config_with_compiled_filter_ir() {
        let filter = "tcp and inbound";
        let expected_filter_ir = HandleConfig::network(filter)
            .expect("filter should compile")
            .filter_ir()
            .to_vec();
        let cmd = NetfilterCmd {
            filter: filter.to_string(),
            mode: NetfilterMode::Observe,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let opens = Arc::new(Mutex::new(Vec::new()));

        assert!(
            !expected_filter_ir.is_empty(),
            "compiled filter ir should not be empty"
        );

        cmd.execute_with_transport(RecordingTransport {
            frames: vec![crate::fixtures::ipv4_frame()],
            sent: Arc::new(Mutex::new(Vec::new())),
            opens: Arc::clone(&opens),
        })
        .expect("observe mode should pass compiled network open config");

        let opens = opens.lock().expect("open config mutex should not be poisoned");
        assert_eq!(
            *opens,
            vec![
                RuntimeOpenConfig::network(expected_filter_ir.clone()),
                RuntimeOpenConfig::network(expected_filter_ir)
            ]
        );
    }

    #[test]
    fn netfilter_reinject_sends_runtime_request() {
        let cmd = NetfilterCmd {
            filter: "tcp and inbound".to_string(),
            mode: NetfilterMode::Reinject,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let sent = Arc::new(Mutex::new(Vec::new()));
        let frame = encode_runtime_event(
            Layer::Network,
            &encode_network_event_payload(101, &crate::fixtures::ipv4_frame()),
        );

        let line = cmd
            .execute_with_transport(BufferedTransport {
                frames: vec![frame],
                sent: Arc::clone(&sent),
            })
            .expect("reinject mode should send runtime request");
        assert!(line.contains("NETFILTER OK"), "unexpected output: {line}");
        assert!(line.contains("mode=reinject"), "unexpected output: {line}");

        let sent = sent.lock().expect("send buffer mutex should not be poisoned");
        assert_eq!(sent.len(), 1, "reinject should issue exactly one send");
        let request =
            decode_runtime_send_request(&sent[0]).expect("sent payload should match runtime contract");
        assert_eq!(request.header.layer, Layer::Network);
        assert_eq!(request.header.reinjection_token, 101);
        assert_eq!(request.payload, crate::fixtures::ipv4_frame().as_slice());
    }

    #[test]
    fn netfilter_reinject_preserves_network_forward_layer() {
        let cmd = NetfilterCmd {
            filter: "tcp and inbound".to_string(),
            mode: NetfilterMode::Reinject,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let sent = Arc::new(Mutex::new(Vec::new()));
        let frame = encode_runtime_event(
            Layer::NetworkForward,
            &encode_network_event_payload(202, &crate::fixtures::ipv4_frame()),
        );

        cmd.execute_with_transport(BufferedTransport {
            frames: vec![frame],
            sent: Arc::clone(&sent),
        })
        .expect("reinject mode should send runtime request");

        let sent = sent.lock().expect("send buffer mutex should not be poisoned");
        let request =
            decode_runtime_send_request(&sent[0]).expect("sent payload should match runtime contract");
        assert_eq!(request.header.layer, Layer::NetworkForward);
        assert_eq!(request.header.reinjection_token, 202);
    }

    #[test]
    fn netfilter_reinject_round_trips_through_driver_core_contract() {
        let cmd = NetfilterCmd {
            filter: "tcp and inbound".to_string(),
            mode: NetfilterMode::Reinject,
            count: 1,
            follow: false,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let accepted = Arc::new(Mutex::new(None));

        let line = cmd
            .execute_with_transport(LoopbackTransport {
                layer: Layer::Network,
                packet_id: 404,
                packet: crate::fixtures::ipv4_frame(),
                accepted: Arc::clone(&accepted),
            })
            .expect("reinject mode should round-trip through driver core contract");
        assert!(line.contains("NETFILTER OK"), "unexpected output: {line}");
        assert!(line.contains("mode=reinject"), "unexpected output: {line}");

        let accepted = accepted.lock().expect("accepted mutex should not be poisoned");
        let accepted = accepted.as_ref().expect("driver core should accept reinjection");
        assert_eq!(accepted.layer, Layer::Network);
        assert_eq!(accepted.packet_id, 404);
        assert_eq!(accepted.packet.as_slice(), crate::fixtures::ipv4_frame().as_slice());
    }
}
