use wd_user::{
    DeviceAvailability, RuntimeError, RuntimeOpenConfig, RuntimeProbe, RuntimeSession,
    RuntimeTransport,
};

#[derive(Debug, Default)]
struct MissingDeviceTransport;

#[derive(Debug, Default)]
struct BufferedSession {
    next: Option<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}

impl RuntimeSession for BufferedSession {
    fn recv_one(&mut self, _max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        self.next
            .take()
            .ok_or_else(|| RuntimeError::io_failure("buffer exhausted"))
    }

    fn send_one(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.sent.push(bytes.to_vec());
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
        Err(RuntimeError::device_unavailable(r"\\.\WdRust"))
    }

    fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
        Err(RuntimeError::device_unavailable(r"\\.\WdRust"))
    }
}

#[test]
fn probe_reports_missing_device() {
    let transport = MissingDeviceTransport;
    let availability = transport.probe().expect("probe should succeed");

    assert_eq!(availability, DeviceAvailability::Missing);
}

#[test]
fn open_maps_missing_device_to_runtime_error() {
    let transport = MissingDeviceTransport;
    let err = transport
        .open(&RuntimeOpenConfig::network(Vec::new()))
        .expect_err("open should fail");

    assert_eq!(err.code(), 3);
    assert_eq!(err.category(), "device_unavailable");
}

#[derive(Debug, Default)]
struct BufferedTransport;

impl RuntimeTransport for BufferedTransport {
    type Session = BufferedSession;

    fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
        Ok(DeviceAvailability::Present)
    }

    fn open(&self, _config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
        Ok(RuntimeProbe {
            device_path: r"\\.\WdRust".to_string(),
            capabilities: None,
            protocol_major: None,
            protocol_minor: None,
        })
    }

    fn open_session(&self, _config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
        Ok(BufferedSession {
            next: Some(vec![1, 2, 3, 4]),
            sent: Vec::new(),
        })
    }
}

#[test]
fn open_session_returns_recvable_bytes() {
    let transport = BufferedTransport;
    let mut session = transport
        .open_session(&RuntimeOpenConfig::network(Vec::new()))
        .expect("session open should succeed");

    let bytes = session.recv_one(8).expect("recv should succeed");
    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[test]
fn open_session_supports_runtime_send() {
    let transport = BufferedTransport;
    let mut session = transport
        .open_session(&RuntimeOpenConfig::network(Vec::new()))
        .expect("session open should succeed");

    session
        .send_one(&[9, 8, 7, 6])
        .expect("send should succeed");

    assert_eq!(session.sent, vec![vec![9, 8, 7, 6]]);
}
