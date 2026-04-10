use std::ffi::CString;
use std::ptr::{null, null_mut};
use std::sync::Arc;

use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
use wd_proto::{OpenRequest, decode_open_response, encode_open_request};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND,
    ERROR_SHARING_VIOLATION, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileAttributesA, INVALID_FILE_ATTRIBUTES, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::{
    DeviceAvailability, RuntimeError, RuntimeOpenConfig, RuntimeProbe, RuntimeSession,
    RuntimeTransport, default_device_path,
};

trait WindowsBackend: Send + Sync {
    fn get_file_attributes(&self, device_path: *const u8) -> Result<u32, u32>;
    fn open_device_handle(&self, device_path: *const u8) -> Result<HANDLE, u32>;
    fn device_io_control(
        &self,
        handle: HANDLE,
        ioctl: u32,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, u32>;
    fn close_handle(&self, handle: HANDLE) -> Result<(), u32>;
}

#[derive(Debug, Default, Clone, Copy)]
struct RealWindowsBackend;

impl WindowsBackend for RealWindowsBackend {
    fn get_file_attributes(&self, device_path: *const u8) -> Result<u32, u32> {
        let attrs = unsafe { GetFileAttributesA(device_path) };
        if attrs == INVALID_FILE_ATTRIBUTES {
            return Err(unsafe { GetLastError() });
        }

        Ok(attrs)
    }

    fn open_device_handle(&self, device_path: *const u8) -> Result<HANDLE, u32> {
        let handle: HANDLE = unsafe {
            CreateFileA(
                device_path,
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(unsafe { GetLastError() });
        }

        Ok(handle)
    }

    fn device_io_control(
        &self,
        handle: HANDLE,
        ioctl: u32,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, u32> {
        let input_ptr = if input.is_empty() {
            null()
        } else {
            input.as_ptr().cast()
        };
        let output_ptr = if output.is_empty() {
            null_mut()
        } else {
            output.as_mut_ptr().cast()
        };
        let input_len = u32::try_from(input.len()).map_err(|_| ERROR_ACCESS_DENIED)?;
        let output_len = u32::try_from(output.len()).map_err(|_| ERROR_ACCESS_DENIED)?;
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                ioctl,
                input_ptr,
                input_len,
                output_ptr,
                output_len,
                &mut returned,
                null_mut(),
            )
        };

        if ok == 0 {
            return Err(unsafe { GetLastError() });
        }

        Ok(returned as usize)
    }

    fn close_handle(&self, handle: HANDLE) -> Result<(), u32> {
        let ok = unsafe { CloseHandle(handle) };
        if ok == 0 {
            return Err(unsafe { GetLastError() });
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct WindowsTransport {
    backend: Arc<dyn WindowsBackend>,
}

pub struct WindowsSession {
    handle: HANDLE,
    device_path: String,
    backend: Arc<dyn WindowsBackend>,
}

impl std::fmt::Debug for WindowsTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WindowsTransport")
    }
}

impl Default for WindowsTransport {
    fn default() -> Self {
        Self {
            backend: Arc::new(RealWindowsBackend),
        }
    }
}

impl std::fmt::Debug for WindowsSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsSession")
            .field("handle", &self.handle)
            .field("device_path", &self.device_path)
            .finish_non_exhaustive()
    }
}

impl WindowsTransport {
    #[cfg(test)]
    fn with_backend_for_test(backend: Box<dyn WindowsBackend>) -> Self {
        Self {
            backend: Arc::from(backend),
        }
    }
}

impl RuntimeTransport for WindowsTransport {
    type Session = WindowsSession;

    fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
        let path = CString::new(default_device_path()).expect("device path should be valid");
        let _attrs = match self.backend.get_file_attributes(path.as_ptr() as *const u8) {
            Ok(attrs) => attrs,
            Err(last_error) => return classify_probe_last_error(last_error),
        };
        Ok(DeviceAvailability::Present)
    }

    fn open(&self, config: &RuntimeOpenConfig) -> Result<RuntimeProbe, RuntimeError> {
        let path = CString::new(default_device_path()).expect("device path should be valid");
        let handle = self
            .backend
            .open_device_handle(path.as_ptr() as *const u8)
            .map_err(|last_error| classify_open_error(last_error, default_device_path()))?;
        let probe = match negotiate_open(&*self.backend, handle, default_device_path(), config) {
            Ok(probe) => probe,
            Err(err) => {
                close_handle_after_negotiate(&*self.backend, handle, default_device_path())?;
                return Err(err);
            }
        };
        close_handle_after_negotiate(&*self.backend, handle, default_device_path())?;
        Ok(probe)
    }

    fn open_session(&self, config: &RuntimeOpenConfig) -> Result<Self::Session, RuntimeError> {
        let path = CString::new(default_device_path()).expect("device path should be valid");
        let handle = self
            .backend
            .open_device_handle(path.as_ptr() as *const u8)
            .map_err(|last_error| classify_open_error(last_error, default_device_path()))?;
        if let Err(err) = negotiate_open(&*self.backend, handle, default_device_path(), config) {
            close_handle_after_negotiate(&*self.backend, handle, default_device_path())?;
            return Err(err);
        }

        Ok(WindowsSession {
            handle,
            device_path: default_device_path().to_string(),
            backend: Arc::clone(&self.backend),
        })
    }
}

impl RuntimeSession for WindowsSession {
    fn recv_one(&mut self, max_bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        let mut bytes = vec![0u8; max_bytes];
        let returned = self
            .backend
            .device_io_control(self.handle, IOCTL_RECV, &[], &mut bytes)
            .map_err(|last_error| {
                RuntimeError::io_failure(format!(
                    "DeviceIoControl(IOCTL_RECV) failed for {} with win32 error {last_error}",
                    self.device_path
                ))
            })?;
        bytes.truncate(returned);
        Ok(bytes)
    }

    fn send_one(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.backend
            .device_io_control(self.handle, IOCTL_SEND, bytes, &mut [])
            .map_err(|last_error| {
                RuntimeError::io_failure(format!(
                    "DeviceIoControl(IOCTL_SEND) failed for {} with win32 error {last_error}",
                    self.device_path
                ))
            })?;

        Ok(())
    }

    fn close(mut self) -> Result<(), RuntimeError> {
        self.close_handle()
    }
}

impl Drop for WindowsSession {
    fn drop(&mut self) {
        let _ = self.close_handle();
    }
}

impl WindowsSession {
    fn close_handle(&mut self) -> Result<(), RuntimeError> {
        if self.handle == INVALID_HANDLE_VALUE {
            return Ok(());
        }

        let handle = self.handle;
        self.handle = INVALID_HANDLE_VALUE;
        self.backend.close_handle(handle).map_err(|last_error| {
            RuntimeError::io_failure(format!(
                "CloseHandle failed for {} with win32 error {last_error}",
                self.device_path
            ))
        })?;
        Ok(())
    }
}

fn classify_probe_last_error(last_error: u32) -> Result<DeviceAvailability, RuntimeError> {
    if is_not_found_error(last_error) {
        return Ok(DeviceAvailability::Missing);
    }

    Err(RuntimeError::io_failure(format!(
        "GetFileAttributesA failed for {} with win32 error {last_error}",
        default_device_path()
    )))
}

fn classify_open_error(last_error: u32, device_path: &str) -> RuntimeError {
    if is_not_found_error(last_error) {
        return RuntimeError::device_unavailable(device_path);
    }

    if is_open_failure_error(last_error) {
        return RuntimeError::open_failed(format!(
            "CreateFileA failed for {device_path} with win32 error {last_error}"
        ));
    }

    RuntimeError::io_failure(format!(
        "CreateFileA failed for {device_path} with win32 error {last_error}"
    ))
}

fn negotiate_open(
    backend: &dyn WindowsBackend,
    handle: HANDLE,
    device_path: &str,
    config: &RuntimeOpenConfig,
) -> Result<RuntimeProbe, RuntimeError> {
    let request = encode_open_request(&OpenRequest::new(
        config.layer(),
        config.filter_ir().to_vec(),
        config.priority(),
        config.flags(),
    ));
    let mut response = [0u8; 12];
    let written = backend
        .device_io_control(handle, IOCTL_OPEN, &request, &mut response)
        .map_err(|last_error| {
            RuntimeError::open_failed(format!(
                "DeviceIoControl(IOCTL_OPEN) failed for {device_path} with win32 error {last_error}"
            ))
        })?;
    let response = decode_open_response(&response[..written]).map_err(|err| {
        RuntimeError::protocol_mismatch(format!(
            "open response decode failed for {device_path}: {err}"
        ))
    })?;
    if response.status != 0 {
        return Err(RuntimeError::open_failed(format!(
            "open response reported status {} for {device_path}",
            response.status
        )));
    }

    Ok(RuntimeProbe {
        device_path: device_path.to_string(),
        capabilities: Some(response.capabilities),
        protocol_major: Some(response.version.major),
        protocol_minor: Some(response.version.minor),
    })
}

fn close_handle_after_negotiate(
    backend: &dyn WindowsBackend,
    handle: HANDLE,
    device_path: &str,
) -> Result<(), RuntimeError> {
    backend.close_handle(handle).map_err(|last_error| {
        RuntimeError::io_failure(format!(
            "CloseHandle failed for {device_path} with win32 error {last_error}"
        ))
    })?;
    Ok(())
}

fn is_not_found_error(last_error: u32) -> bool {
    matches!(last_error, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
}

fn is_open_failure_error(last_error: u32) -> bool {
    matches!(last_error, ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
}

#[cfg(test)]
mod tests {
    use super::{WindowsBackend, WindowsTransport, classify_open_error, classify_probe_last_error};
    use crate::{DeviceAvailability, HandleConfig, RuntimeOpenConfig, RuntimeSession, RuntimeTransport};
    use std::sync::{Arc, Mutex};
    use wd_driver_shared::{IOCTL_OPEN, IOCTL_RECV, IOCTL_SEND};
    use wd_proto::{Layer, OpenResponse, ProtocolVersion, decode_open_request, encode_open_response};
    use windows_sys::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_INVALID_DATA, ERROR_PATH_NOT_FOUND, HANDLE,
        INVALID_HANDLE_VALUE, ERROR_SHARING_VIOLATION,
    };

    #[derive(Debug)]
    struct FakeBackendState {
        attrs: Result<u32, u32>,
        open_handle: Result<isize, u32>,
        open_response: Vec<u8>,
        recv_payload: Vec<u8>,
        recv_error: Option<u32>,
        send_error: Option<u32>,
        close_error: Option<u32>,
        sent_payloads: Vec<Vec<u8>>,
        closed_handles: Vec<isize>,
        open_requests: Vec<Vec<u8>>,
    }

    #[derive(Debug)]
    struct FakeBackend {
        state: Arc<Mutex<FakeBackendState>>,
    }

    impl FakeBackend {
        fn new(state: FakeBackendState) -> (Self, Arc<Mutex<FakeBackendState>>) {
            let state = Arc::new(Mutex::new(state));
            (
                Self {
                    state: Arc::clone(&state),
                },
                state,
            )
        }
    }

    impl WindowsBackend for FakeBackend {
        fn get_file_attributes(&self, _device_path: *const u8) -> Result<u32, u32> {
            self.state
                .lock()
                .expect("fake backend mutex should not be poisoned")
                .attrs
        }

        fn open_device_handle(&self, _device_path: *const u8) -> Result<HANDLE, u32> {
            self.state
                .lock()
                .expect("fake backend mutex should not be poisoned")
                .open_handle
                .map(|handle| handle as HANDLE)
        }

        fn device_io_control(
            &self,
            handle: HANDLE,
            ioctl: u32,
            input: &[u8],
            output: &mut [u8],
        ) -> Result<usize, u32> {
            assert_eq!(handle, 0x1234isize as HANDLE, "unexpected handle");
            let mut state = self
                .state
                .lock()
                .expect("fake backend mutex should not be poisoned");
            match ioctl {
                IOCTL_OPEN => {
                    state.open_requests.push(input.to_vec());
                    output[..state.open_response.len()].copy_from_slice(&state.open_response);
                    Ok(state.open_response.len())
                }
                IOCTL_RECV => {
                    if let Some(err) = state.recv_error {
                        return Err(err);
                    }
                    output[..state.recv_payload.len()].copy_from_slice(&state.recv_payload);
                    Ok(state.recv_payload.len())
                }
                IOCTL_SEND => {
                    state.sent_payloads.push(input.to_vec());
                    if let Some(err) = state.send_error {
                        return Err(err);
                    }
                    Ok(0)
                }
                _ => panic!("unexpected ioctl {ioctl:#x}"),
            }
        }

        fn close_handle(&self, handle: HANDLE) -> Result<(), u32> {
            let mut state = self
                .state
                .lock()
                .expect("fake backend mutex should not be poisoned");
            state.closed_handles.push(handle as isize);
            if let Some(err) = state.close_error {
                return Err(err);
            }
            Ok(())
        }
    }

    fn open_success_state() -> FakeBackendState {
        FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: encode_open_response(OpenResponse::success(0x1f)),
            recv_payload: vec![1, 2, 3, 4],
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        }
    }

    fn assert_open_request_encoding(
        config: RuntimeOpenConfig,
        expected_layer: Layer,
        expected_filter_ir: Vec<u8>,
        expected_priority: i16,
        expected_flags: u64,
    ) {
        let (backend, state) = FakeBackend::new(open_success_state());
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));

        let probe = transport.open(&config).expect("open should succeed");
        assert_eq!(probe.device_path, r"\\.\WdRust");

        let session = transport
            .open_session(&config)
            .expect("open_session should succeed");
        session.close().expect("close should succeed");

        let state = state
            .lock()
            .expect("fake backend mutex should not be poisoned");
        assert_eq!(
            state.open_requests.len(),
            2,
            "open and open_session should each issue IOCTL_OPEN"
        );

        for request_bytes in &state.open_requests {
            let request = decode_open_request(request_bytes).expect("open request should decode");
            assert_eq!(request.version, ProtocolVersion::CURRENT);
            assert_eq!(request.layer, expected_layer);
            assert_eq!(request.priority, expected_priority);
            assert_eq!(request.flags, expected_flags);
            assert_eq!(request.filter_ir, expected_filter_ir);
            assert_eq!(request.filter_len as usize, request.filter_ir.len());
        }
    }

    #[test]
    fn probe_maps_not_found_to_missing() {
        let result = classify_probe_last_error(ERROR_FILE_NOT_FOUND);
        assert_eq!(result, Ok(DeviceAvailability::Missing));

        let result = classify_probe_last_error(ERROR_PATH_NOT_FOUND);
        assert_eq!(result, Ok(DeviceAvailability::Missing));
    }

    #[test]
    fn probe_maps_other_errors_to_io_failure() {
        let err = classify_probe_last_error(ERROR_INVALID_DATA)
            .expect_err("non not-found probe error should map to RuntimeError");
        assert_eq!(err.category(), "io_failure");
        assert_eq!(err.code(), 6);
    }

    #[test]
    fn open_maps_not_found_to_device_unavailable() {
        let err = classify_open_error(ERROR_FILE_NOT_FOUND, r"\\.\WdRust");
        assert_eq!(err.category(), "device_unavailable");
        assert_eq!(err.code(), 3);
    }

    #[test]
    fn open_maps_access_issues_to_open_failed() {
        let denied = classify_open_error(ERROR_ACCESS_DENIED, r"\\.\WdRust");
        assert_eq!(denied.category(), "open_failed");
        assert_eq!(denied.code(), 4);

        let sharing = classify_open_error(ERROR_SHARING_VIOLATION, r"\\.\WdRust");
        assert_eq!(sharing.category(), "open_failed");
        assert_eq!(sharing.code(), 4);
    }

    #[test]
    fn open_maps_other_errors_to_io_failure() {
        let err = classify_open_error(ERROR_INVALID_DATA, r"\\.\WdRust");
        assert_eq!(err.category(), "io_failure");
        assert_eq!(err.code(), 6);
    }

    #[test]
    fn open_session_round_trips_recv_send_and_close_through_backend() {
        let (backend, state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: encode_open_response(OpenResponse::success(0x1f)),
            recv_payload: vec![1, 2, 3, 4],
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));
        let config = RuntimeOpenConfig::socket();

        let probe = transport.open(&config).expect("open should succeed");
        assert_eq!(probe.device_path, r"\\.\WdRust");
        assert_eq!(probe.capabilities, Some(0x1f));
        assert_eq!(probe.protocol_major, Some(ProtocolVersion::CURRENT.major));
        assert_eq!(probe.protocol_minor, Some(ProtocolVersion::CURRENT.minor));

        let mut session = transport
            .open_session(&config)
            .expect("open_session should succeed");
        let recv = session.recv_one(32).expect("recv should succeed");
        assert_eq!(recv, vec![1, 2, 3, 4]);

        session
            .send_one(&[9, 8, 7, 6])
            .expect("send should succeed");
        session.close().expect("close should succeed");

        let state = state
            .lock()
            .expect("fake backend mutex should not be poisoned");
        assert_eq!(state.sent_payloads, vec![vec![9, 8, 7, 6]]);
        assert_eq!(state.closed_handles, vec![0x1234isize, 0x1234isize]);
        assert_eq!(state.open_requests.len(), 2, "open and open_session should each negotiate once");
        let open_request = decode_open_request(&state.open_requests[0])
            .expect("open request should decode");
        assert_eq!(open_request.layer, Layer::Socket);
        assert!(open_request.filter_ir.is_empty());
    }

    #[test]
    fn open_encodes_socket_open_request() {
        assert_open_request_encoding(RuntimeOpenConfig::socket(), Layer::Socket, Vec::new(), 0, 0);
    }

    #[test]
    fn open_encodes_flow_open_request() {
        assert_open_request_encoding(RuntimeOpenConfig::flow(), Layer::Flow, Vec::new(), 0, 0);
    }

    #[test]
    fn open_encodes_reflect_open_request() {
        assert_open_request_encoding(RuntimeOpenConfig::reflect(), Layer::Reflect, Vec::new(), 0, 0);
    }

    #[test]
    fn open_encodes_network_open_request_with_filter_ir() {
        let filter_ir = HandleConfig::network("tcp and inbound")
            .expect("filter should compile")
            .filter_ir()
            .to_vec();
        assert!(!filter_ir.is_empty(), "compiled filter ir should not be empty");
        assert_open_request_encoding(
            RuntimeOpenConfig::network(filter_ir.clone()),
            Layer::Network,
            filter_ir,
            0,
            0,
        );
    }

    #[test]
    fn open_encodes_full_open_request_header_from_custom_runtime_config() {
        let filter_ir = vec![0x57, 0x44, 0x49, 0x52, 0x01, 0xaa, 0xbb];
        assert_open_request_encoding(
            RuntimeOpenConfig::new(Layer::NetworkForward, filter_ir.clone(), -12, 0x0102_0304_0506_0708),
            Layer::NetworkForward,
            filter_ir,
            -12,
            0x0102_0304_0506_0708,
        );
    }

    #[test]
    fn open_session_maps_recv_and_send_backend_failures_to_io_error() {
        let (backend, _state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: encode_open_response(OpenResponse::success(0x1f)),
            recv_payload: Vec::new(),
            recv_error: Some(ERROR_INVALID_DATA),
            send_error: Some(ERROR_INVALID_DATA),
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));
        let mut session = transport
            .open_session(&RuntimeOpenConfig::network(vec![1, 2, 3]))
            .expect("open_session should succeed");

        let recv = session.recv_one(32).expect_err("recv should fail");
        assert_eq!(recv.category(), "io_failure");

        let send = session.send_one(&[1]).expect_err("send should fail");
        assert_eq!(send.category(), "io_failure");
    }

    #[test]
    fn probe_uses_backend_result_for_present_device() {
        let (backend, _state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(INVALID_HANDLE_VALUE as isize),
            open_response: encode_open_response(OpenResponse::success(0x1f)),
            recv_payload: Vec::new(),
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));

        let availability = transport.probe().expect("probe should succeed");
        assert_eq!(availability, DeviceAvailability::Present);
    }

    #[test]
    fn open_session_rejects_invalid_open_response() {
        let (backend, _state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: vec![0u8; 4],
            recv_payload: Vec::new(),
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));

        let err = transport
            .open_session(&RuntimeOpenConfig::flow())
            .expect_err("invalid open response should reject session");
        assert_eq!(err.category(), "protocol_mismatch");
    }

    #[test]
    fn open_closes_handle_when_open_response_reports_failure() {
        let (backend, state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: encode_open_response(OpenResponse {
                version: ProtocolVersion::CURRENT,
                capabilities: 0x1f,
                status: 9,
            }),
            recv_payload: Vec::new(),
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));

        let err = transport
            .open(&RuntimeOpenConfig::reflect())
            .expect_err("non-zero open response status should fail");
        assert_eq!(err.category(), "open_failed");

        let state = state
            .lock()
            .expect("fake backend mutex should not be poisoned");
        assert_eq!(
            state.closed_handles,
            vec![0x1234isize],
            "failed open negotiation should close the temporary handle"
        );
    }

    #[test]
    fn open_session_closes_handle_when_open_response_is_invalid() {
        let (backend, state) = FakeBackend::new(FakeBackendState {
            attrs: Ok(0),
            open_handle: Ok(0x1234isize),
            open_response: vec![0u8; 4],
            recv_payload: Vec::new(),
            recv_error: None,
            send_error: None,
            close_error: None,
            sent_payloads: Vec::new(),
            closed_handles: Vec::new(),
            open_requests: Vec::new(),
        });
        let transport = WindowsTransport::with_backend_for_test(Box::new(backend));

        let err = transport
            .open_session(&RuntimeOpenConfig::flow())
            .expect_err("invalid open response should reject session");
        assert_eq!(err.category(), "protocol_mismatch");

        let state = state
            .lock()
            .expect("fake backend mutex should not be poisoned");
        assert_eq!(
            state.closed_handles,
            vec![0x1234isize],
            "failed session negotiation should close the session handle"
        );
    }
}
