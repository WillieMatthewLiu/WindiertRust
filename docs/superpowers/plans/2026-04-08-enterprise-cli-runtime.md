# Enterprise CLI Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the deterministic phase-one `wd-cli` subcommands with real runtime commands that talk to the live device path when available and fail with stable diagnostics when the device path is unavailable.

**Architecture:** Build a shared runtime layer first, split between `wd-user` transport/probe code and `wd-cli` error/output orchestration. Convert `reflectctl` first to validate probe/open/close behavior, then wire the observation commands and `netfilter` onto the same runtime contract so output, exit codes, and diagnostics stay consistent.

**Tech Stack:** Rust stable, `clap`, `thiserror`, `wd-user`, `wd-proto`, `wd-driver-shared`, `wd-kmdf`, PowerShell validation scripts, Windows device I/O via Rust Windows bindings.

**Status (2026-04-10):** Complete and merged to `main`. Runtime probing/open/send/recv, CLI runtime error/output contracts, README updates, PowerShell validation, and driver-glue minimal build assets are all landed.

---

## Planned File Structure

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/wd-user/Cargo.toml`
- Modify: `crates/wd-user/src/lib.rs`
- Create: `crates/wd-user/src/device.rs`
- Create: `crates/wd-user/src/runtime.rs`
- Create: `crates/wd-user/src/windows.rs`
- Create: `crates/wd-user/tests/runtime.rs`
- Modify: `crates/wd-cli/Cargo.toml`
- Modify: `crates/wd-cli/src/lib.rs`
- Modify: `crates/wd-cli/src/cmd/common.rs`
- Create: `crates/wd-cli/src/runtime.rs`
- Create: `crates/wd-cli/src/output.rs`
- Create: `crates/wd-cli/src/error.rs`
- Modify: `crates/wd-cli/src/cmd/reflectctl.rs`
- Modify: `crates/wd-cli/src/cmd/netdump.rs`
- Modify: `crates/wd-cli/src/cmd/socketdump.rs`
- Modify: `crates/wd-cli/src/cmd/flowtrack.rs`
- Modify: `crates/wd-cli/src/cmd/netfilter.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Create: `crates/wd-cli/tests/runtime_errors.rs`
- Modify: `tests/windows/open_close.ps1`
- Modify: `tests/windows/network_reinject.ps1`
- Modify: `tests/windows/five_layer_observe.ps1`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-07-rust-windivert-rewrite.md`

## Task 1: Add Shared Runtime Probing and Transport to `wd-user`

**Files:**
- Modify: `crates/wd-user/Cargo.toml`
- Modify: `crates/wd-user/src/lib.rs`
- Create: `crates/wd-user/src/device.rs`
- Create: `crates/wd-user/src/runtime.rs`
- Create: `crates/wd-user/src/windows.rs`
- Create: `crates/wd-user/tests/runtime.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [x] **Step 1: Write failing transport/probe tests**

```rust
use wd_user::{DeviceAvailability, RuntimeError, RuntimeProbe, RuntimeTransport};

#[derive(Debug, Default)]
struct MissingDeviceTransport;

impl RuntimeTransport for MissingDeviceTransport {
    fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
        Ok(DeviceAvailability::Missing)
    }

    fn open(&self) -> Result<RuntimeProbe, RuntimeError> {
        Err(RuntimeError::device_unavailable(r"\\.\WdRust"))
    }
}

#[test]
fn probe_reports_missing_device() {
    let transport = MissingDeviceTransport::default();
    let availability = transport.probe().expect("probe should succeed");

    assert_eq!(availability, DeviceAvailability::Missing);
}

#[test]
fn open_maps_missing_device_to_runtime_error() {
    let transport = MissingDeviceTransport::default();
    let err = transport.open().expect_err("open should fail");

    assert_eq!(err.code(), 3);
    assert_eq!(err.category(), "device_unavailable");
}
```

- [x] **Step 2: Run the new runtime tests to verify they fail**

Run: `cargo test -p wd-user --test runtime`
Expected: FAIL because `RuntimeTransport`, `RuntimeError`, `DeviceAvailability`, and `RuntimeProbe` do not exist yet.

- [x] **Step 3: Add Windows dependency and runtime modules**

```toml
# crates/wd-user/Cargo.toml
[dependencies]
thiserror.workspace = true
wd-filter = { path = "../wd-filter" }
wd-proto = { path = "../wd-proto" }
wd-driver-shared = { path = "../wd-driver-shared" }
windows-sys = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_System_Ioctl",
] }
```

```rust
// crates/wd-user/src/lib.rs
mod checksum;
mod device;
mod error;
mod frame;
mod handle;
mod runtime;
mod windows;

pub use checksum::ChecksumUpdate;
pub use device::{default_device_path, DeviceAvailability};
pub use error::UserError;
pub use frame::RecvEvent;
pub use handle::{DynamicHandle, HandleConfig};
pub use runtime::{RuntimeError, RuntimeProbe, RuntimeTransport};
pub use windows::WindowsTransport;

pub mod test_support;
```

- [x] **Step 4: Implement the shared runtime types**

```rust
// crates/wd-user/src/device.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAvailability {
    Present,
    Missing,
}

pub fn default_device_path() -> &'static str {
    r"\\.\WdRust"
}
```

```rust
// crates/wd-user/src/runtime.rs
use thiserror::Error;

use crate::DeviceAvailability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbe {
    pub device_path: String,
    pub capabilities: u32,
    pub protocol_major: u16,
    pub protocol_minor: u16,
}

pub trait RuntimeTransport {
    fn probe(&self) -> Result<DeviceAvailability, RuntimeError>;
    fn open(&self) -> Result<RuntimeProbe, RuntimeError>;
    fn close(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct RuntimeError {
    code: u8,
    category: &'static str,
    message: String,
    suggestion: &'static str,
}

impl RuntimeError {
    pub fn device_unavailable(path: &str) -> Self {
        Self {
            code: 3,
            category: "device_unavailable",
            message: format!("WdRust device not found at {path}"),
            suggestion: "verify driver is installed and device link is present",
        }
    }

    pub fn open_failed(message: impl Into<String>) -> Self {
        Self {
            code: 4,
            category: "open_failed",
            message: message.into(),
            suggestion: "verify permissions and exclusive access settings",
        }
    }

    pub fn protocol_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: 5,
            category: "protocol_mismatch",
            message: message.into(),
            suggestion: "verify driver and user-mode binaries are from the same build",
        }
    }

    pub fn io_failure(message: impl Into<String>) -> Self {
        Self {
            code: 6,
            category: "io_failure",
            message: message.into(),
            suggestion: "retry the command and inspect verbose diagnostics",
        }
    }

    pub fn code(&self) -> u8 {
        self.code
    }

    pub fn category(&self) -> &'static str {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn suggestion(&self) -> &'static str {
        self.suggestion
    }
}
```

- [x] **Step 5: Implement the first Windows transport path**

```rust
// crates/wd-user/src/windows.rs
use std::ffi::CString;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileA, GetFileAttributesA, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::{default_device_path, DeviceAvailability, RuntimeError, RuntimeProbe, RuntimeTransport};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsTransport;

impl RuntimeTransport for WindowsTransport {
    fn probe(&self) -> Result<DeviceAvailability, RuntimeError> {
        let path = CString::new(default_device_path()).expect("device path should be valid");
        let attrs = unsafe { GetFileAttributesA(path.as_ptr() as *const u8) };
        if attrs == u32::MAX {
            Ok(DeviceAvailability::Missing)
        } else {
            Ok(DeviceAvailability::Present)
        }
    }

    fn open(&self) -> Result<RuntimeProbe, RuntimeError> {
        let path = CString::new(default_device_path()).expect("device path should be valid");
        let handle: HANDLE = unsafe {
            CreateFileA(
                path.as_ptr() as *const u8,
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(RuntimeError::device_unavailable(default_device_path()));
        }

        unsafe {
            CloseHandle(handle);
        }

        Ok(RuntimeProbe {
            device_path: default_device_path().to_string(),
            capabilities: 0,
            protocol_major: 0,
            protocol_minor: 1,
        })
    }
}
```

- [x] **Step 6: Run the runtime tests to verify they pass**

Run: `cargo test -p wd-user --test runtime`
Expected: PASS

- [x] **Step 7: Commit the runtime transport foundation**

```bash
git add Cargo.toml Cargo.lock crates/wd-user/Cargo.toml crates/wd-user/src/lib.rs crates/wd-user/src/device.rs crates/wd-user/src/runtime.rs crates/wd-user/src/windows.rs crates/wd-user/tests/runtime.rs
git commit -m "feat: add wd-user runtime probe foundation"
```

## Task 2: Add Shared CLI Error and Output Orchestration

**Files:**
- Modify: `crates/wd-cli/Cargo.toml`
- Modify: `crates/wd-cli/src/lib.rs`
- Modify: `crates/wd-cli/src/cmd/common.rs`
- Create: `crates/wd-cli/src/runtime.rs`
- Create: `crates/wd-cli/src/output.rs`
- Create: `crates/wd-cli/src/error.rs`
- Create: `crates/wd-cli/tests/runtime_errors.rs`

- [x] **Step 1: Write failing CLI error/output tests**

```rust
use wd_cli::error::CliError;
use wd_cli::output::{render_error_json, render_error_text};

#[test]
fn text_errors_include_code_category_and_message() {
    let err = CliError::device_unavailable("netdump", r"\\.\WdRust");
    let line = render_error_text(&err);

    assert!(line.contains("NETDUMP ERROR"));
    assert!(line.contains("code=3"));
    assert!(line.contains("category=device_unavailable"));
}

#[test]
fn json_errors_include_stable_fields() {
    let err = CliError::device_unavailable("netdump", r"\\.\WdRust");
    let line = render_error_json(&err);

    assert!(line.contains("\"command\":\"netdump\""));
    assert!(line.contains("\"status\":\"error\""));
    assert!(line.contains("\"category\":\"device_unavailable\""));
}
```

- [x] **Step 2: Run the CLI error tests to verify they fail**

Run: `cargo test -p wd-cli --test runtime_errors`
Expected: FAIL because `CliError` and output renderers do not exist yet.

- [x] **Step 3: Add CLI output mode and error types**

```rust
// crates/wd-cli/src/error.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub command: &'static str,
    pub code: u8,
    pub category: &'static str,
    pub message: String,
    pub suggestion: &'static str,
}

impl CliError {
    pub fn device_unavailable(command: &'static str, path: &str) -> Self {
        Self {
            command,
            code: 3,
            category: "device_unavailable",
            message: format!("WdRust device not found at {path}"),
            suggestion: "verify driver is installed and device link is present",
        }
    }
}
```

```rust
// crates/wd-cli/src/output.rs
use crate::error::CliError;

pub fn render_error_text(err: &CliError) -> String {
    format!(
        "{} ERROR code={} category={} message={} suggestion={}",
        err.command.to_ascii_uppercase(),
        err.code,
        err.category,
        err.message,
        err.suggestion
    )
}

pub fn render_error_json(err: &CliError) -> String {
    format!(
        "{{\"command\":\"{}\",\"status\":\"error\",\"code\":{},\"category\":\"{}\",\"message\":\"{}\",\"suggestion\":\"{}\"}}",
        err.command,
        err.code,
        err.category,
        err.message.replace('"', "\\\""),
        err.suggestion
    )
}
```

- [x] **Step 4: Add shared runtime helpers for command modules**

```rust
// crates/wd-cli/src/runtime.rs
use std::process::ExitCode;

use wd_user::{RuntimeError, RuntimeTransport, WindowsTransport};

use crate::error::CliError;

pub fn default_transport() -> impl RuntimeTransport {
    WindowsTransport
}

pub fn map_runtime_error(command: &'static str, err: RuntimeError) -> CliError {
    CliError {
        command,
        code: err.code(),
        category: err.category(),
        message: err.message().to_string(),
        suggestion: err.suggestion(),
    }
}

pub fn exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}
```

- [x] **Step 5: Run the CLI error tests to verify they pass**

Run: `cargo test -p wd-cli --test runtime_errors`
Expected: PASS

- [x] **Step 6: Commit the CLI runtime/error foundation**

```bash
git add crates/wd-cli/Cargo.toml crates/wd-cli/src/lib.rs crates/wd-cli/src/cmd/common.rs crates/wd-cli/src/runtime.rs crates/wd-cli/src/output.rs crates/wd-cli/src/error.rs crates/wd-cli/tests/runtime_errors.rs
git commit -m "feat: add shared CLI runtime error handling"
```

## Task 3: Convert `reflectctl` to the Real Probe/Open Path

**Files:**
- Modify: `crates/wd-cli/src/cmd/reflectctl.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/open_close.ps1`

- [x] **Step 1: Write the failing `reflectctl` runtime tests**

```rust
#[test]
fn reflectctl_probe_reports_device_missing_with_nonzero_exit() {
    let output = run_cli(&["reflectctl", "--action", "probe", "--json"]);

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("\"category\":\"device_unavailable\""));
}
```

- [x] **Step 2: Run the `reflectctl` tests to verify they fail**

Run: `cargo test -p wd-cli reflectctl_ -- --nocapture`
Expected: FAIL because `reflectctl` still uses deterministic scaffolding rather than the shared runtime error path.

- [x] **Step 3: Implement `reflectctl` actions on top of the runtime layer**

```rust
// crates/wd-cli/src/cmd/reflectctl.rs
use clap::{Args, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReflectAction {
    Probe,
    Open,
    Capabilities,
    State,
    Close,
}

#[derive(Debug, Args)]
pub struct ReflectctlCmd {
    #[arg(long, value_enum, default_value_t = ReflectAction::Probe)]
    action: ReflectAction,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
}
```

Implement `execute()` so it:

- probes the device first
- maps missing device to exit code `3`
- opens through `default_transport().open()`
- prints either:
  - `REFLECTCTL OK device=ready capabilities=<n> protocol=<major.minor> state=Open`
  - or a JSON success object

- [x] **Step 4: Upgrade the Windows control-path script**

```powershell
$output = & $CliPath reflectctl --action probe
if ($LASTEXITCODE -eq 0) {
    Assert-Contains -Text $output -Expected "REFLECTCTL OK"
}
else {
    if ($LASTEXITCODE -ne 3) {
        throw "reflectctl returned unexpected exit code $LASTEXITCODE"
    }
    Assert-Contains -Text $output -Expected "device_unavailable"
}
```

- [x] **Step 5: Run the `reflectctl` tests and script**

Run:
- `cargo test -p wd-cli reflectctl_ -- --nocapture`
- `powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1`

Expected: PASS

- [x] **Step 6: Commit the `reflectctl` runtime conversion**

```bash
git add crates/wd-cli/src/cmd/reflectctl.rs crates/wd-cli/tests/commands.rs tests/windows/open_close.ps1
git commit -m "feat: convert reflectctl to runtime probe path"
```

## Task 4: Convert `netdump` to Real Runtime Receive Semantics

**Files:**
- Modify: `crates/wd-cli/src/cmd/netdump.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/five_layer_observe.ps1`

- [x] **Step 1: Write failing `netdump` argument and error-path tests**

```rust
#[test]
fn netdump_json_errors_when_device_is_missing() {
    let output = run_cli(&["netdump", "--json"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("\"command\":\"netdump\""));
    assert!(stderr(&output).contains("\"category\":\"device_unavailable\""));
}
```

- [x] **Step 2: Run the `netdump` tests to verify they fail**

Run: `cargo test -p wd-cli netdump_ -- --nocapture`
Expected: FAIL because `netdump` still consumes deterministic fixtures and returns success.

- [x] **Step 3: Replace fixture-only behavior with runtime open/recv**

```rust
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
```

Implement `execute()` so it:

- validates `count > 0`
- probes/opens the device
- when the runtime path is unavailable, exits with code `3`
- when receive bytes are available, decodes through `RecvEvent::decode`
- emits:
  - `NETDUMP OK layer=NETWORK ttl=<n> checksum=<hex> packet_len=<n> timestamp=<ts>`
  - or JSON with the same stable fields

- [x] **Step 4: Update the Windows observe script for `netdump`**

```powershell
$netdump = & $CliPath netdump
if ($LASTEXITCODE -eq 0) {
    Assert-Contains -Text $netdump -Expected "NETDUMP OK"
    Assert-Contains -Text $netdump -Expected "layer=NETWORK"
}
else {
    if ($LASTEXITCODE -ne 3) {
        throw "netdump returned unexpected exit code $LASTEXITCODE"
    }
    Assert-Contains -Text $netdump -Expected "device_unavailable"
}
```

- [x] **Step 5: Run the `netdump` tests and script**

Run:
- `cargo test -p wd-cli netdump_ -- --nocapture`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: PASS for the `netdump` segment; later parts may still fail until their commands are converted.

- [x] **Step 6: Commit the `netdump` runtime conversion**

```bash
git add crates/wd-cli/src/cmd/netdump.rs crates/wd-cli/tests/commands.rs tests/windows/five_layer_observe.ps1
git commit -m "feat: convert netdump to runtime receive path"
```

## Task 5: Convert `socketdump` and `flowtrack` to Real Observation Commands

**Files:**
- Modify: `crates/wd-cli/src/cmd/socketdump.rs`
- Modify: `crates/wd-cli/src/cmd/flowtrack.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/five_layer_observe.ps1`

- [x] **Step 1: Write failing observation-command error tests**

```rust
#[test]
fn socketdump_json_errors_when_device_is_missing() {
    let output = run_cli(&["socketdump", "--filter", "event == CONNECT", "--json"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("\"category\":\"device_unavailable\""));
}

#[test]
fn flowtrack_json_errors_when_device_is_missing() {
    let output = run_cli(&["flowtrack", "--json"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("\"category\":\"device_unavailable\""));
}
```

- [x] **Step 2: Run the observation-command tests to verify they fail**

Run: `cargo test -p wd-cli 'socketdump_|flowtrack_' -- --nocapture`
Expected: FAIL because both commands still return deterministic success.

- [x] **Step 3: Implement `socketdump` runtime observation mode**

```rust
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
```

Implement it so it:

- probes and opens the device
- returns code `2` when `--filter` is invalid
- returns code `3` when the device is unavailable
- returns one-shot or streaming socket event output using the shared renderer

- [x] **Step 4: Implement `flowtrack` runtime observation mode**

```rust
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
```

Implement it so it:

- follows the same device/open/error contract
- emits one-shot or streaming flow events with stable fields

- [x] **Step 5: Update the Windows observe script for `socketdump` and `flowtrack`**

```powershell
$flowtrack = & $CliPath flowtrack
if ($LASTEXITCODE -eq 0) {
    Assert-Contains -Text $flowtrack -Expected "FLOWTRACK OK"
}
elseif ($LASTEXITCODE -eq 3) {
    Assert-Contains -Text $flowtrack -Expected "device_unavailable"
}
else {
    throw "flowtrack returned unexpected exit code $LASTEXITCODE"
}

$socketdump = & $CliPath socketdump --filter "event == CONNECT"
if ($LASTEXITCODE -eq 0) {
    Assert-Contains -Text $socketdump -Expected "SOCKETDUMP OK"
}
elseif ($LASTEXITCODE -eq 3) {
    Assert-Contains -Text $socketdump -Expected "device_unavailable"
}
else {
    throw "socketdump returned unexpected exit code $LASTEXITCODE"
}
```

- [x] **Step 6: Run the observation-command tests and script**

Run:
- `cargo test -p wd-cli 'socketdump_|flowtrack_' -- --nocapture`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: PASS

- [x] **Step 7: Commit the observation-command runtime conversion**

```bash
git add crates/wd-cli/src/cmd/socketdump.rs crates/wd-cli/src/cmd/flowtrack.rs crates/wd-cli/tests/commands.rs tests/windows/five_layer_observe.ps1
git commit -m "feat: convert socketdump and flowtrack to runtime commands"
```

## Task 6: Convert `netfilter` to Real Validate/Observe/Reinject Modes

**Files:**
- Modify: `crates/wd-cli/src/cmd/netfilter.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/network_reinject.ps1`

- [x] **Step 1: Write failing `netfilter` mode tests**

```rust
#[test]
fn netfilter_validate_errors_when_device_is_missing() {
    let output = run_cli(&["netfilter", "--filter", "tcp and inbound", "--mode", "validate", "--json"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("\"category\":\"device_unavailable\""));
}

#[test]
fn netfilter_rejects_unknown_mode_count_combination() {
    let output = run_cli(&["netfilter", "--filter", "tcp and inbound", "--count", "0"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("argument_error"));
}
```

- [x] **Step 2: Run the `netfilter` tests to verify they fail**

Run: `cargo test -p wd-cli netfilter_ -- --nocapture`
Expected: FAIL because `netfilter` still returns deterministic success and has no real mode contract.

- [x] **Step 3: Implement explicit `validate|observe|reinject` modes**

```rust
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
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
```

Implementation rules:

- `validate`
  - compile filter
  - probe/open the device
  - report readiness without implicit reinjection
- `observe`
  - probe/open the device
  - receive matching events one-shot or in streaming mode
- `reinject`
  - remain explicit and guarded
  - return stable runtime errors if reinjection path is unavailable

- [x] **Step 4: Update the network reinject script**

```powershell
$output = & $CliPath netfilter --filter "tcp and inbound" --mode validate
if ($LASTEXITCODE -eq 0) {
    Assert-Contains -Text $output -Expected "NETFILTER OK"
    Assert-Contains -Text $output -Expected "mode=validate"
}
elseif ($LASTEXITCODE -eq 3) {
    Assert-Contains -Text $output -Expected "device_unavailable"
}
else {
    throw "netfilter returned unexpected exit code $LASTEXITCODE"
}
```

- [x] **Step 5: Run the `netfilter` tests and script**

Run:
- `cargo test -p wd-cli netfilter_ -- --nocapture`
- `powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1`

Expected: PASS

- [x] **Step 6: Commit the `netfilter` runtime conversion**

```bash
git add crates/wd-cli/src/cmd/netfilter.rs crates/wd-cli/tests/commands.rs tests/windows/network_reinject.ps1
git commit -m "feat: convert netfilter to runtime modes"
```

## Task 7: Update Documentation and Close the Enterprise Runtime Transition

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-07-rust-windivert-rewrite.md`

- [x] **Step 1: Update README command docs**

Document for every command:

- real runtime purpose
- default behavior
- arguments
- text output shape
- JSON output availability
- exit-code contract
- device-unavailable behavior

Use examples like:

```text
NETDUMP ERROR code=3 category=device_unavailable message=WdRust device not found suggestion=verify driver is installed and device link is present
```

- [x] **Step 2: Update the main rewrite plan status**

Add a status line under Task 6 that states the deterministic phase has been replaced by runtime-first commands with uniform diagnostics and exit-code contracts.

- [x] **Step 3: Run the full enterprise CLI runtime verification**

Run:
- `cargo test -p wd-user --test runtime`
- `cargo test -p wd-cli`
- `cargo test -p wd-kmdf`
- `cargo check --workspace`
- `powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1`
- `powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: PASS

- [x] **Step 4: Commit the docs and final verification state**

```bash
git add README.md docs/superpowers/plans/2026-04-07-rust-windivert-rewrite.md
git commit -m "docs: describe enterprise CLI runtime behavior"
```

## Self-Review

### Spec Coverage

- Shared runtime/device probing: covered by Task 1 and Task 2.
- `reflectctl` real probe/open path: covered by Task 3.
- Observation commands with one-shot and `--follow`: covered by Task 4 and Task 5.
- `netfilter` explicit `validate|observe|reinject` modes: covered by Task 6.
- Stable text/JSON output and exit codes: covered by Task 2 through Task 6.
- Windows script assertions for both success and device-missing paths: covered by Task 3 through Task 7.

### Placeholder Scan

- No step uses `TODO`, `TBD`, or "implement later".
- All test steps include concrete assertions.
- All code steps include concrete file paths and concrete code blocks.
- All verification steps include exact commands and expected results.

### Type Consistency

- Shared runtime layer uses `RuntimeTransport`, `RuntimeProbe`, and `RuntimeError` consistently.
- CLI error/output layer uses `CliError` consistently across all command tasks.
- Exit code `3` is reserved for `device_unavailable` everywhere.
- Observation commands share `count`, `follow`, `json`, `verbose`, and `timeout_ms` naming consistently.
