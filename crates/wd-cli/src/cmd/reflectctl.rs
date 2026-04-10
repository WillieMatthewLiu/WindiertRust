use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Args, ValueEnum};
use wd_user::{
    DeviceAvailability, RuntimeError, RuntimeOpenConfig, RuntimeProbe, RuntimeTransport,
    default_device_path,
};

use crate::cmd::common::{finish_with_cli_error, render_summary};
use crate::error::CliError;
use crate::output::OutputMode;
use crate::runtime::{default_transport, map_runtime_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ReflectAction {
    Probe,
    Open,
    Close,
    Capabilities,
    State,
}

#[derive(Debug, Args)]
pub struct ReflectctlCmd {
    #[arg(long, value_enum, default_value_t = ReflectAction::Open)]
    action: ReflectAction,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
}

impl ReflectctlCmd {
    pub fn run(self) -> ExitCode {
        let mode = OutputMode::from_json_flag(self.json);
        finish_with_cli_error(self.execute_with_transport(default_transport()), mode)
    }

    fn execute_with_transport<T: RuntimeTransport>(self, transport: T) -> Result<String, CliError> {
        let ReflectctlCmd {
            action,
            json,
            verbose,
            timeout_ms,
        } = self;
        let budget = TimeoutBudget::new(timeout_ms)
            .map_err(|err| map_runtime_error_with_context(action, timeout_ms, verbose, err))?;
        let open_config = RuntimeOpenConfig::reflect();

        let availability = budget
            .run("probe", || transport.probe())
            .map_err(|err| map_runtime_error_with_context(action, timeout_ms, verbose, err))?;

        if availability == DeviceAvailability::Missing {
            return Err(map_runtime_error_with_context(
                action,
                timeout_ms,
                verbose,
                RuntimeError::device_unavailable(default_device_path()),
            ));
        }

        let success = match action {
            ReflectAction::Probe => ReflectSuccess::probed(action, timeout_ms, verbose),
            ReflectAction::Open | ReflectAction::Capabilities | ReflectAction::State => {
                let probe = budget
                    .run("open", || transport.open(&open_config))
                    .map_err(|err| map_runtime_error_with_context(action, timeout_ms, verbose, err))?;
                ReflectSuccess::from_probe(action, timeout_ms, verbose, probe, "Open")
            }
            ReflectAction::Close => {
                let probe = budget
                    .run("open", || transport.open(&open_config))
                    .map_err(|err| map_runtime_error_with_context(action, timeout_ms, verbose, err))?;
                budget
                    .run("close", || transport.close())
                    .map_err(|err| map_runtime_error_with_context(action, timeout_ms, verbose, err))?;
                ReflectSuccess::from_probe(action, timeout_ms, verbose, probe, "CloseAttempted")
            }
        };

        Ok(match json {
            true => success.render_json(),
            false => success.render_text(),
        })
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
                "reflectctl {step} exceeded timeout budget (elapsed={}ms timeout={}ms)",
                elapsed.as_millis(),
                self.timeout.as_millis()
            )));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ReflectSuccess {
    device: &'static str,
    capabilities: Option<u32>,
    protocol: Option<String>,
    state: &'static str,
    action: &'static str,
    timeout_ms: u64,
    verbose: bool,
}

impl ReflectSuccess {
    fn probed(action: ReflectAction, timeout_ms: u64, verbose: bool) -> Self {
        Self {
            device: "ready",
            capabilities: None,
            protocol: None,
            state: "Probed",
            action: action.as_str(),
            timeout_ms,
            verbose,
        }
    }

    fn from_probe(
        action: ReflectAction,
        timeout_ms: u64,
        verbose: bool,
        probe: RuntimeProbe,
        state: &'static str,
    ) -> Self {
        Self {
            device: "ready",
            capabilities: probe.capabilities,
            protocol: format_protocol(&probe),
            state,
            action: action.as_str(),
            timeout_ms,
            verbose,
        }
    }

    fn render_text(&self) -> String {
        let mut fields = vec![
            ("device", self.device.to_string()),
            (
                "capabilities",
                self.capabilities
                    .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            ),
            (
                "protocol",
                self.protocol
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            ("state", self.state.to_string()),
        ];
        if self.verbose {
            fields.push(("action", self.action.to_string()));
            fields.push(("timeout_ms", self.timeout_ms.to_string()));
        }
        render_summary("REFLECTCTL OK", &fields)
    }

    fn render_json(&self) -> String {
        let capabilities = self
            .capabilities
            .map_or_else(|| "null".to_string(), |value| value.to_string());
        let protocol = self.protocol.as_deref().map_or_else(
            || "null".to_string(),
            |value| format!("\"{}\"", escape_json_string(value)),
        );
        let mut line = format!(
            "{{\"command\":\"reflectctl\",\"status\":\"ok\",\"device\":\"{}\",\"capabilities\":{},\"protocol\":{},\"state\":\"{}\"",
            escape_json_string(self.device),
            capabilities,
            protocol,
            escape_json_string(self.state),
        );
        if self.verbose {
            line.push_str(",\"action\":\"");
            line.push_str(&escape_json_string(self.action));
            line.push_str("\",\"timeout_ms\":");
            line.push_str(&self.timeout_ms.to_string());
        }
        line.push('}');
        line
    }
}

impl ReflectAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Open => "open",
            Self::Close => "close",
            Self::Capabilities => "capabilities",
            Self::State => "state",
        }
    }
}

fn map_runtime_error_with_context(
    action: ReflectAction,
    timeout_ms: u64,
    verbose: bool,
    err: RuntimeError,
) -> CliError {
    let mut cli_error = map_runtime_error("reflectctl", err);
    if verbose {
        cli_error.message = format!(
            "{} [action={}, timeout_ms={}]",
            cli_error.message,
            action.as_str(),
            timeout_ms
        );
    }
    cli_error
}

fn format_protocol(probe: &RuntimeProbe) -> Option<String> {
    match (probe.protocol_major, probe.protocol_minor) {
        (Some(major), Some(minor)) => Some(format!("{major}.{minor}")),
        _ => None,
    }
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
                escaped.push(nibble_to_hex((ch as u32 & 0x0f) as u8));
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
    use super::{ReflectAction, ReflectctlCmd, TimeoutBudget};
    use std::sync::{Arc, Mutex};
    use std::thread::sleep;
    use std::time::Duration;
    use wd_user::{
        DeviceAvailability, RuntimeError, RuntimeOpenConfig, RuntimeProbe, RuntimeSession,
        RuntimeTransport,
        default_device_path,
    };

    #[derive(Debug, Clone, Copy)]
    struct FakePresentTransport;

    #[derive(Debug)]
    struct FakeSession;

    impl RuntimeSession for FakeSession {
        fn recv_one(&mut self, _max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::io_failure("reflectctl test session does not receive packets"))
        }

        fn send_one(&mut self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Err(RuntimeError::io_failure("reflectctl test session does not send packets"))
        }

        fn close(self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    impl RuntimeTransport for FakePresentTransport {
        type Session = FakeSession;

        fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
            Ok(DeviceAvailability::Present)
        }

        fn open(&self, _config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
            Ok(RuntimeProbe {
                device_path: default_device_path().to_string(),
                capabilities: Some(7),
                protocol_major: Some(1),
                protocol_minor: Some(0),
            })
        }

        fn close(&self) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
            Ok(FakeSession)
        }
    }

    #[derive(Debug)]
    struct RecordingTransport {
        opens: Arc<Mutex<Vec<RuntimeOpenConfig>>>,
    }

    impl RuntimeTransport for RecordingTransport {
        type Session = FakeSession;

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
                capabilities: Some(7),
                protocol_major: Some(1),
                protocol_minor: Some(0),
            })
        }

        fn close(&self) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
            Ok(FakeSession)
        }
    }

    #[test]
    fn reflectctl_close_success_output_is_honest_about_close_state() {
        let cmd = ReflectctlCmd {
            action: ReflectAction::Close,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };

        let line = cmd
            .execute_with_transport(FakePresentTransport)
            .expect("close action should succeed");
        assert!(line.contains("state=CloseAttempted"), "unexpected output: {line}");
        assert!(!line.contains("state=Closed"), "unexpected output: {line}");
    }

    #[test]
    fn reflectctl_open_passes_reflect_open_config_and_reports_open_metadata() {
        let cmd = ReflectctlCmd {
            action: ReflectAction::Open,
            json: false,
            verbose: false,
            timeout_ms: 5_000,
        };
        let opens = Arc::new(Mutex::new(Vec::new()));

        let line = cmd
            .execute_with_transport(RecordingTransport {
                opens: Arc::clone(&opens),
            })
            .expect("open action should use reflect runtime open config");
        assert!(line.contains("REFLECTCTL OK"), "unexpected output: {line}");
        assert!(line.contains("capabilities=7"), "unexpected output: {line}");
        assert!(line.contains("protocol=1.0"), "unexpected output: {line}");
        assert!(line.contains("state=Open"), "unexpected output: {line}");

        let opens = opens.lock().expect("open config mutex should not be poisoned");
        assert_eq!(*opens, vec![RuntimeOpenConfig::reflect()]);
    }

    #[test]
    fn reflectctl_timeout_budget_fails_when_work_exceeds_budget() {
        let budget = TimeoutBudget::new(5).expect("timeout budget should be created");
        sleep(Duration::from_millis(15));

        let err = budget
            .run("probe", || Ok(()))
            .expect_err("expired budget should fail");
        assert_eq!(err.category(), "io_failure");
        assert_eq!(err.code(), 6);
        assert!(err.message().contains("exceeded timeout budget"));
    }

    #[test]
    fn reflectctl_timeout_budget_fails_after_work_completes_over_budget() {
        let budget = TimeoutBudget::new(20).expect("timeout budget should be created");

        let err = budget
            .run("open", || {
                sleep(Duration::from_millis(40));
                Ok(())
            })
            .expect_err("budget should fail on post-work timeout check");
        assert_eq!(err.category(), "io_failure");
        assert_eq!(err.code(), 6);
        assert!(err.message().contains("reflectctl open exceeded timeout budget"));
    }
}
