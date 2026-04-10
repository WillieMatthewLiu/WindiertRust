# Rust WinDivert Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Track and complete a Rust-first WinDivert rewrite with a stable protocol crate, a filter compiler, a user-mode API, a testable driver-core crate, minimal Windows packaging glue, CLI tooling, and host-validation scripts aligned to the approved phase-one design.

**Architecture:** Build a Cargo workspace that separates transport-stable protocol types (`wd-proto`), string filter compilation (`wd-filter`), user-mode API and frame codecs (`wd-user`), CLI tooling (`wd-cli`), driver-only shared constants (`wd-driver-shared`), and a KMDF-oriented driver core (`driver/wd-kmdf`). Keep kernel-testable logic in plain Rust modules so filter evaluation, queueing, handle state transitions, and reinjection bookkeeping can be verified without a live Windows device.

**Tech Stack:** Rust stable, Cargo workspace, `thiserror`, `bitflags`, `zerocopy`, `bincode`, `clap`, handwritten lexer/parser for the filter DSL, PowerShell validation scripts for Windows host checks.

---

## Current Workspace Notes

- The current directory is the main repository worktree on branch `main`.
- The workspace is no longer empty. The Cargo workspace, crates, driver skeleton, packaging glue, host-validation scripts, and README described below already exist in the repository.
- Task 1 through Task 6 now have concrete implementations and merged verification state as of 2026-04-10.
- The runtime-first CLI/device transport path, README command docs, PowerShell validation scripts, and minimal driver-glue build assets have been landed on `main`.
- The adjacent reference implementation lives at `../03PcapWinDivert` and should be used only as a semantic reference for layer/event behavior, not as a line-by-line port target.

## Current Status Snapshot (2026-04-10)

- Task 1 `Bootstrap the Cargo Workspace and Protocol Skeleton`: complete.
- Task 2 `Define Stable ABI Frames and Driver-Shared Constants`: complete.
- Task 3 `Implement the Filter DSL Compiler to Stable IR`: complete.
- Task 4 `Implement User-Mode Frames, Typed Handles, and Checksum Helpers`: complete.
- Task 5 `Implement Kernel-Testable Driver Core for State, Queueing, and Reinjection`: complete for the current pure-Rust testable subset.
- Task 6 `Wire CLI Tooling and Windows Host Validation Scripts`: complete as runtime-first CLI flow, including stable text/JSON diagnostics, live-device open/probe behavior when present, and upgraded Windows validation scripts.

Fresh verification run on 2026-04-10:

- `cargo test --offline -p wd-cli`
- `cargo test --offline -p wd-user`
- `cargo test --offline -p wd-filter`
- `cargo test --offline -p wd-proto`
- `cargo test --offline -p wd-kmdf-core`
- `cargo test --offline -p wd-kmdf`
- `cargo check --offline --workspace`
- `powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1 -CliPath target\debug\wd-cli.exe`
- `powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1 -CliPath target\debug\wd-cli.exe`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1 -CliPath target\debug\wd-cli.exe`
- `powershell -ExecutionPolicy Bypass -File driver/glue/verify_staged_assets.ps1`
- `powershell -ExecutionPolicy Bypass -File driver/glue/verify_kmdf_skeleton_assets.ps1`
- `powershell -ExecutionPolicy Bypass -File driver/glue/verify_host_smoke_build.ps1`

## Planned File Structure

- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `crates/wd-proto/Cargo.toml`
- Create: `crates/wd-proto/src/lib.rs`
- Create: `crates/wd-proto/tests/protocol.rs`
- Create: `crates/wd-driver-shared/Cargo.toml`
- Create: `crates/wd-driver-shared/src/lib.rs`
- Create: `crates/wd-driver-shared/tests/layout.rs`
- Create: `crates/wd-filter/Cargo.toml`
- Create: `crates/wd-filter/src/lib.rs`
- Create: `crates/wd-filter/src/lexer.rs`
- Create: `crates/wd-filter/src/parser.rs`
- Create: `crates/wd-filter/src/semantics.rs`
- Create: `crates/wd-filter/src/ir.rs`
- Create: `crates/wd-filter/tests/compile.rs`
- Create: `crates/wd-user/Cargo.toml`
- Create: `crates/wd-user/src/lib.rs`
- Create: `crates/wd-user/src/error.rs`
- Create: `crates/wd-user/src/frame.rs`
- Create: `crates/wd-user/src/handle.rs`
- Create: `crates/wd-user/src/checksum.rs`
- Create: `crates/wd-user/src/test_support.rs`
- Create: `crates/wd-user/tests/user_api.rs`
- Create: `crates/wd-cli/Cargo.toml`
- Create: `crates/wd-cli/src/lib.rs`
- Create: `crates/wd-cli/src/main.rs`
- Create: `crates/wd-cli/src/cmd/netdump.rs`
- Create: `crates/wd-cli/src/cmd/netfilter.rs`
- Create: `crates/wd-cli/src/cmd/flowtrack.rs`
- Create: `crates/wd-cli/src/cmd/socketdump.rs`
- Create: `crates/wd-cli/src/cmd/reflectctl.rs`
- Create: `crates/wd-cli/tests/cli.rs`
- Create: `driver/wd-kmdf/Cargo.toml`
- Create: `driver/wd-kmdf/src/lib.rs`
- Create: `driver/wd-kmdf/src/state.rs`
- Create: `driver/wd-kmdf/src/queue.rs`
- Create: `driver/wd-kmdf/src/filter_eval.rs`
- Create: `driver/wd-kmdf/src/reinject.rs`
- Create: `driver/wd-kmdf/tests/state_machine.rs`
- Create: `driver/wd-kmdf/tests/filter_eval.rs`
- Create: `driver/wd-kmdf/tests/reinject.rs`
- Create: `driver/wd-kmdf/tests/queue.rs`
- Create: `driver/glue/README.md`
- Create: `driver/glue/wd-rust-x64.inf`
- Create: `driver/glue/wd-rust-x86.inf`
- Create: `driver/glue/build.ps1`
- Create: `tests/windows/open_close.ps1`
- Create: `tests/windows/network_reinject.ps1`
- Create: `tests/windows/five_layer_observe.ps1`
- Create: `README.md`

### Task 1: Bootstrap the Cargo Workspace and Protocol Skeleton

**Current status (2026-04-08):** Complete.

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `crates/wd-proto/Cargo.toml`
- Create: `crates/wd-proto/src/lib.rs`
- Test: `crates/wd-proto/tests/protocol.rs`

- [x] **Step 1: Write the failing protocol smoke test**

```rust
use wd_proto::{CapabilityFlags, Layer, ProtocolVersion};

#[test]
fn protocol_version_and_layers_match_phase_one_contract() {
    assert_eq!(ProtocolVersion::CURRENT.major, 0);
    assert_eq!(ProtocolVersion::CURRENT.minor, 1);
    assert_eq!(Layer::all(), [
        Layer::Network,
        Layer::NetworkForward,
        Layer::Flow,
        Layer::Socket,
        Layer::Reflect,
    ]);
    assert!(CapabilityFlags::CHECKSUM_RECALC.bits() != 0);
}
```

- [x] **Step 2: Run the targeted test to confirm the workspace is not implemented yet**

Run: `cargo test -p wd-proto protocol_version_and_layers_match_phase_one_contract -- --exact`
Expected: FAIL with a workspace or unresolved import error because `wd-proto` does not exist yet.

- [x] **Step 3: Create the minimal workspace manifests and proto crate**

```toml
# Cargo.toml
[workspace]
members = [
    "crates/wd-proto",
    "crates/wd-driver-shared",
    "crates/wd-filter",
    "crates/wd-user",
    "crates/wd-cli",
    "driver/wd-kmdf",
]
resolver = "2"

[workspace.package]
edition = "2024"
license = "LGPL-3.0-or-later"
version = "0.1.0"

[workspace.dependencies]
bitflags = "2.6"
bincode = "1.3"
clap = { version = "4.5", features = ["derive"] }
thiserror = "2.0"
zerocopy = { version = "0.8", features = ["derive"] }
```

```rust
// crates/wd-proto/src/lib.rs
use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self { major: 0, minor: 1 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Network,
    NetworkForward,
    Flow,
    Socket,
    Reflect,
}

impl Layer {
    pub const fn all() -> [Layer; 5] {
        [
            Layer::Network,
            Layer::NetworkForward,
            Layer::Flow,
            Layer::Socket,
            Layer::Reflect,
        ]
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilityFlags: u32 {
        const CHECKSUM_RECALC = 0x0001;
        const NETWORK_REINJECT = 0x0002;
        const FLOW_EVENTS = 0x0004;
        const SOCKET_EVENTS = 0x0008;
        const REFLECT_EVENTS = 0x0010;
    }
}
```

- [x] **Step 4: Re-run the targeted test and the workspace check**

Run: `cargo test -p wd-proto protocol_version_and_layers_match_phase_one_contract -- --exact`
Expected: PASS

Run: `cargo check --workspace`
Expected: FAIL because the remaining workspace members are declared but not implemented yet.

- [x] **Step 5: Create stub manifests for the remaining members so the workspace resolves**

```toml
# crates/wd-filter/Cargo.toml
[package]
name = "wd-filter"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
thiserror.workspace = true
wd-proto = { path = "../wd-proto" }
```

```rust
// crates/wd-filter/src/lib.rs
pub fn placeholder() {}
```

```rust
// crates/wd-user/src/lib.rs
pub fn placeholder() {}
```

```rust
// crates/wd-driver-shared/src/lib.rs
pub fn placeholder() {}
```

```rust
// driver/wd-kmdf/src/lib.rs
pub fn placeholder() {}
```

```rust
// crates/wd-cli/src/main.rs
fn main() {}
```

- [x] **Step 6: Verify the workspace bootstrap checkpoint**

Run: `cargo check --workspace`
Expected: PASS with placeholder crates compiling.

### Task 2: Define Stable ABI Frames and Driver-Shared Constants

**Current status (2026-04-08):** Complete.

**Files:**
- Modify: `crates/wd-proto/src/lib.rs`
- Create: `crates/wd-driver-shared/Cargo.toml`
- Create: `crates/wd-driver-shared/src/lib.rs`
- Create: `crates/wd-driver-shared/tests/layout.rs`
- Test: `crates/wd-proto/tests/protocol.rs`

- [x] **Step 1: Add failing tests for frame layout and version negotiation**

```rust
use wd_driver_shared::{DEVICE_NAME, IOCTL_OPEN};
use wd_proto::{Layer, OpenRequest, OpenResponse, ProtocolVersion};

#[test]
fn open_request_has_stable_header_and_filter_bytes() {
    let request = OpenRequest::new(Layer::Network, "tcp and inbound".into(), 0, 0);
    assert_eq!(request.version, ProtocolVersion::CURRENT);
    assert_eq!(request.filter_len as usize, request.filter_ir.len());
    assert!(IOCTL_OPEN != 0);
    assert!(DEVICE_NAME.starts_with(r"\\Device\\"));
}

#[test]
fn open_response_exposes_capabilities() {
    let response = OpenResponse::success(0x1f);
    assert_eq!(response.version, ProtocolVersion::CURRENT);
    assert_eq!(response.capabilities, 0x1f);
}
```

- [x] **Step 2: Run the ABI tests**

Run: `cargo test -p wd-proto --test protocol`
Expected: FAIL with missing `OpenRequest`, `OpenResponse`, or `wd-driver-shared` items.

- [x] **Step 3: Implement the ABI structs and constants**

```rust
// crates/wd-proto/src/lib.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub version: ProtocolVersion,
    pub layer: Layer,
    pub priority: i16,
    pub flags: u64,
    pub filter_len: u32,
    pub filter_ir: Vec<u8>,
}

impl OpenRequest {
    pub fn new(layer: Layer, filter_ir: Vec<u8>, priority: i16, flags: u64) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            layer,
            priority,
            flags,
            filter_len: filter_ir.len() as u32,
            filter_ir,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenResponse {
    pub version: ProtocolVersion,
    pub capabilities: u32,
    pub status: u32,
}

impl OpenResponse {
    pub const fn success(capabilities: u32) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            capabilities,
            status: 0,
        }
    }
}
```

```rust
// crates/wd-driver-shared/src/lib.rs
pub const DEVICE_NAME: &str = r"\\Device\\WdRust";
pub const DOS_DEVICE_NAME: &str = r"\\DosDevices\\WdRust";
pub const IOCTL_OPEN: u32 = 0x8000_2000;
pub const IOCTL_RECV: u32 = 0x8000_2004;
pub const IOCTL_SEND: u32 = 0x8000_2008;
```

- [x] **Step 4: Verify ABI and layout tests**

Run: `cargo test -p wd-driver-shared`
Expected: PASS

Run: `cargo test -p wd-proto --test protocol`
Expected: PASS

- [x] **Step 5: Lock in a no-placeholder workspace checkpoint**

Run: `cargo check --workspace`
Expected: PASS

### Task 3: Implement the Filter DSL Compiler to Stable IR

**Current status (2026-04-08):** Complete.

**Files:**
- Modify: `crates/wd-filter/src/lib.rs`
- Create: `crates/wd-filter/src/lexer.rs`
- Create: `crates/wd-filter/src/parser.rs`
- Create: `crates/wd-filter/src/semantics.rs`
- Create: `crates/wd-filter/src/ir.rs`
- Test: `crates/wd-filter/tests/compile.rs`

- [x] **Step 1: Write failing compile tests for boolean logic, symbolic fields, and packet access**

```rust
use wd_filter::{compile, FilterIr, LayerMask, OpCode};

#[test]
fn compile_network_filter_tracks_payload_access() {
    let ir = compile("tcp and inbound and packet32[0] == 0x12345678").unwrap();
    assert!(ir.required_layers.contains(LayerMask::NETWORK));
    assert!(ir.needs_payload);
    assert!(matches!(ir.program[0], OpCode::FieldTest { .. }));
}

#[test]
fn compile_reflect_filter_tracks_symbolic_event_and_layer() {
    let ir = compile("event == OPEN and layer == NETWORK").unwrap();
    assert!(ir.required_layers.contains(LayerMask::REFLECT));
    assert!(!ir.needs_payload);
}

#[test]
fn reject_flow_packet_access() {
    let err = compile("layer == FLOW and packet[0] == 1").unwrap_err();
    assert!(err.to_string().contains("packet access is not valid for FLOW"));
}
```

- [x] **Step 2: Run the filter tests to observe the initial failures**

Run: `cargo test -p wd-filter --test compile`
Expected: FAIL because `compile`, `FilterIr`, and the IR model do not exist yet.

- [x] **Step 3: Implement lexer, parser, semantic analysis, and IR lowering**

```rust
// crates/wd-filter/src/lib.rs
mod ir;
mod lexer;
mod parser;
mod semantics;

pub use ir::{FilterIr, LayerMask, OpCode};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("lex error: {0}")]
    Lex(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("semantic error: {0}")]
    Semantic(String),
}

pub fn compile(input: &str) -> Result<FilterIr, CompileError> {
    let tokens = lexer::lex(input).map_err(CompileError::Lex)?;
    let ast = parser::parse(&tokens).map_err(CompileError::Parse)?;
    semantics::lower(ast).map_err(CompileError::Semantic)
}
```

```rust
// crates/wd-filter/src/ir.rs
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LayerMask: u8 {
        const NETWORK = 0b00001;
        const NETWORK_FORWARD = 0b00010;
        const FLOW = 0b00100;
        const SOCKET = 0b01000;
        const REFLECT = 0b10000;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpCode {
    FieldTest { field: &'static str, value: u64 },
    PacketLoad32 { offset: u16, value: u32 },
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterIr {
    pub required_layers: LayerMask,
    pub needs_payload: bool,
    pub referenced_fields: Vec<&'static str>,
    pub program: Vec<OpCode>,
}
```

- [x] **Step 4: Verify the compiler behavior**

Run: `cargo test -p wd-filter --test compile`
Expected: PASS

Run: `cargo test -p wd-filter`
Expected: PASS

- [x] **Step 5: Verify workspace integration after adding real compiler code**

Run: `cargo check --workspace`
Expected: PASS

### Task 4: Implement User-Mode Frames, Typed Handles, and Checksum Helpers

**Current status (2026-04-08):** Complete.

**Files:**
- Create: `crates/wd-user/src/error.rs`
- Create: `crates/wd-user/src/frame.rs`
- Create: `crates/wd-user/src/handle.rs`
- Create: `crates/wd-user/src/checksum.rs`
- Create: `crates/wd-user/src/test_support.rs`
- Modify: `crates/wd-user/src/lib.rs`
- Test: `crates/wd-user/tests/user_api.rs`

- [x] **Step 1: Write failing tests for open configuration, event decode, and checksum repair**

```rust
use wd_proto::{Layer, OpenResponse};
use wd_user::{ChecksumUpdate, HandleConfig, RecvEvent};

#[test]
fn handle_config_compiles_filter_before_open() {
    let cfg = HandleConfig::network("tcp and inbound").unwrap();
    assert_eq!(cfg.layer(), Layer::Network);
    assert!(!cfg.filter_ir().is_empty());
}

#[test]
fn decode_network_event_and_apply_checksum_fix() {
    let raw = wd_user::test_support::network_frame_bytes();
    let mut event = RecvEvent::decode(&raw).unwrap();
    let change = event.packet_mut().unwrap().set_ipv4_ttl(31);
    assert_eq!(change, ChecksumUpdate::Dirty);
    event.repair_checksums().unwrap();
}

#[test]
fn negotiated_capabilities_are_exposed_to_callers() {
    let handle = wd_user::test_support::opened_handle(OpenResponse::success(0x1f));
    assert_eq!(handle.capabilities_bits(), 0x1f);
}
```

- [x] **Step 2: Run the user-mode tests**

Run: `cargo test -p wd-user --test user_api`
Expected: FAIL because `HandleConfig`, `RecvEvent`, and checksum helpers do not exist yet.

- [x] **Step 3: Implement the user-mode API around compiled filter IR and frame codecs**

```rust
// crates/wd-user/src/lib.rs
mod checksum;
mod error;
mod frame;
mod handle;

pub use checksum::ChecksumUpdate;
pub use error::UserError;
pub use frame::RecvEvent;
pub use handle::{DynamicHandle, HandleConfig};

pub mod test_support;
```

```rust
// crates/wd-user/src/handle.rs
use wd_filter::compile;
use wd_proto::Layer;

use crate::UserError;

pub struct HandleConfig {
    layer: Layer,
    filter_ir: Vec<u8>,
}

impl HandleConfig {
    pub fn network(filter: &str) -> Result<Self, UserError> {
        let ir = compile(filter)?;
        Ok(Self {
            layer: Layer::Network,
            filter_ir: bincode::serialize(&ir).map_err(UserError::encode)?,
        })
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn filter_ir(&self) -> &[u8] {
        &self.filter_ir
    }
}
```

- [x] **Step 4: Verify typed-handle and frame-codec behavior**

Run: `cargo test -p wd-user --test user_api`
Expected: PASS

Run: `cargo test -p wd-user`
Expected: PASS

- [x] **Step 5: Verify cross-crate integration**

Run: `cargo check --workspace`
Expected: PASS

### Task 5: Implement Kernel-Testable Driver Core for State, Queueing, and Reinjection

**Current status (2026-04-08):** Complete for the current pure-Rust testable subset.

**Files:**
- Create: `driver/wd-kmdf/src/state.rs`
- Create: `driver/wd-kmdf/src/queue.rs`
- Create: `driver/wd-kmdf/src/filter_eval.rs`
- Create: `driver/wd-kmdf/src/reinject.rs`
- Modify: `driver/wd-kmdf/src/lib.rs`
- Test: `driver/wd-kmdf/tests/state_machine.rs`
- Test: `driver/wd-kmdf/tests/filter_eval.rs`
- Test: `driver/wd-kmdf/tests/reinject.rs`
- Test: `driver/wd-kmdf/tests/queue.rs`

- [x] **Step 1: Write failing driver-core tests for lifecycle, filter matching, and one-shot reinjection tokens**

```rust
use wd_kmdf::{DriverEvent, FilterEngine, HandleState, ReinjectionTable};
use wd_proto::Layer;

#[test]
fn handle_state_machine_enforces_shutdown_order() {
    let mut state = HandleState::opening();
    state.mark_running().unwrap();
    state.shutdown_recv().unwrap();
    state.shutdown_send().unwrap();
    state.close().unwrap();
    assert!(state.is_closed());
}

#[test]
fn filter_engine_matches_socket_event_symbolically() {
    let engine = FilterEngine::compile(Layer::Socket, "event == CONNECT and processId == 7").unwrap();
    let event = DriverEvent::socket_connect(7);
    assert!(engine.matches(&event));
}

#[test]
fn reinjection_tokens_are_single_use() {
    let mut table = ReinjectionTable::default();
    let token = table.issue_for_network_packet(7);
    assert!(table.consume(token).is_ok());
    assert!(table.consume(token).is_err());
}
```

- [x] **Step 2: Run the driver-core tests**

Run: `cargo test -p wd-kmdf`
Expected: FAIL because the driver-core modules are placeholders.

- [x] **Step 3: Implement the pure-Rust driver logic**

```rust
// driver/wd-kmdf/src/lib.rs
pub mod filter_eval;
pub mod queue;
pub mod reinject;
pub mod state;

pub use filter_eval::{DriverEvent, FilterEngine};
pub use reinject::ReinjectionTable;
pub use state::HandleState;
```

```rust
// driver/wd-kmdf/src/state.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleState {
    Opening,
    Running,
    RecvShutdown,
    SendShutdown,
    Closing,
    Closed,
}

impl HandleState {
    pub const fn opening() -> Self { Self::Opening }
    pub fn mark_running(&mut self) -> Result<(), &'static str> { *self = Self::Running; Ok(()) }
    pub fn shutdown_recv(&mut self) -> Result<(), &'static str> { *self = Self::RecvShutdown; Ok(()) }
    pub fn shutdown_send(&mut self) -> Result<(), &'static str> { *self = Self::SendShutdown; Ok(()) }
    pub fn close(&mut self) -> Result<(), &'static str> { *self = Self::Closed; Ok(()) }
    pub fn is_closed(self) -> bool { matches!(self, Self::Closed) }
}
```

- [x] **Step 4: Verify kernel-testable logic without a live device**

Run: `cargo test -p wd-kmdf`
Expected: PASS

Run: `cargo check --workspace`
Expected: PASS

- [x] **Step 5: Add a queue pressure regression test**

```rust
#[test]
fn queue_drops_oldest_when_capacity_is_hit() {
    let mut queue = wd_kmdf::queue::EventQueue::new(2);
    queue.push(DriverEvent::reflect_open());
    queue.push(DriverEvent::reflect_close());
    queue.push(DriverEvent::reflect_open());
    assert_eq!(queue.len(), 2);
}
```

Run: `cargo test -p wd-kmdf queue_drops_oldest_when_capacity_is_hit -- --exact`
Expected: PASS

### Task 6: Wire CLI Tooling and Windows Host Validation Scripts

**Status (2026-04-09):** Completed as runtime-first CLI flow. The five `wd-cli` subcommands now probe/open the real device path when available, emit stable machine-checkable text/JSON diagnostics, and the Windows host scripts assert both success-path and `device_unavailable` behavior instead of only checking exit codes.

**Files:**
- Create: `crates/wd-cli/src/lib.rs`
- Create: `crates/wd-cli/src/main.rs`
- Create: `crates/wd-cli/src/cmd/netdump.rs`
- Create: `crates/wd-cli/src/cmd/netfilter.rs`
- Create: `crates/wd-cli/src/cmd/flowtrack.rs`
- Create: `crates/wd-cli/src/cmd/socketdump.rs`
- Create: `crates/wd-cli/src/cmd/reflectctl.rs`
- Create: `crates/wd-cli/tests/cli.rs`
- Create: `tests/windows/open_close.ps1`
- Create: `tests/windows/network_reinject.ps1`
- Create: `tests/windows/five_layer_observe.ps1`
- Create: `driver/glue/README.md`
- Create: `driver/glue/wd-rust-x64.inf`
- Create: `driver/glue/wd-rust-x86.inf`
- Create: `driver/glue/build.ps1`
- Create: `README.md`

- [x] **Step 1: Write failing smoke tests for CLI command registration**

```rust
use clap::CommandFactory;

#[test]
fn cli_exposes_phase_one_commands() {
    let mut cmd = wd_cli::Cli::command();
    let help = cmd.render_long_help().to_string();
    assert!(help.contains("netdump"));
    assert!(help.contains("netfilter"));
    assert!(help.contains("flowtrack"));
    assert!(help.contains("socketdump"));
    assert!(help.contains("reflectctl"));
}
```

- [x] **Step 2: Run the CLI smoke test**

Run: `cargo test -p wd-cli cli_exposes_phase_one_commands -- --exact`
Expected: FAIL because the CLI crate does not expose the command surface yet.

- [x] **Step 3: Implement CLI dispatch and host-validation scripts**

```rust
// crates/wd-cli/src/lib.rs
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "wd-cli")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Netdump,
    Netfilter,
    Flowtrack,
    Socketdump,
    Reflectctl,
}

// crates/wd-cli/src/main.rs
use clap::Parser;
use wd_cli::Cli;

fn main() {
    let _ = Cli::parse();
}
```

```powershell
# tests/windows/open_close.ps1
param([string]$Binary = ".\\target\\release\\wd-cli.exe")

& $Binary reflectctl | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "reflectctl open/close smoke test failed"
}
```

- [x] **Step 4: Verify CLI registration and workspace docs**

Run: `cargo test -p wd-cli`
Expected: PASS

Run: `cargo check --workspace`
Expected: PASS

- [x] **Step 5: Document the Windows packaging and host validation flow**

Run: `find tests/windows -maxdepth 1 -type f | sort`
Expected: PASS with `tests/windows/open_close.ps1`, `tests/windows/network_reinject.ps1`, and `tests/windows/five_layer_observe.ps1` listed.

## Self-Review

### Spec Coverage

- Workspace and protocol foundation: covered by Task 1 and Task 2.
- Stable DSL-to-driver IR pipeline: covered by Task 3.
- User-mode receive, mutation, checksum repair, and reinjection API surface: covered by Task 4.
- Five-layer observability, handle lifecycle, queueing, and reinjection token safety: covered by Task 5.
- CLI tooling, packaging glue, installation validation, and Windows host scripts: covered by Task 6.

### Placeholder Scan

- No `TODO`, `TBD`, or cross-task shorthand remains in the plan body.
- The only intentionally deferred item is real KMDF/WFP binding code generation, which stays outside phase-one pure-Rust verification and is represented here as glue and host-validation work, matching the approved design.

### Type Consistency

- `Layer`, `ProtocolVersion`, `OpenRequest`, and `OpenResponse` are introduced in Task 1 and Task 2 before later tasks consume them.
- `FilterIr` and `LayerMask` are defined in Task 3 before `wd-user` and `wd-kmdf` rely on compiled IR.
- `HandleState`, `FilterEngine`, and `ReinjectionTable` are introduced in Task 5 before validation scripts describe runtime behavior.
