# Real CLI Subcommands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the five `wd-cli` subcommands from placeholder entrypoints into phase-one real commands with deterministic arguments, domain-backed behavior, machine-checkable output, and upgraded Windows validation scripts.

**Architecture:** Keep the CLI thin and push domain behavior into existing crates wherever possible. Treat "real command" in phase one as "parses meaningful arguments, exercises repository logic instead of printing placeholders, and emits stable output that tests and PowerShell scripts can assert" rather than "talks to a live installed driver."

**Tech Stack:** Rust stable, `clap`, existing workspace crates (`wd-filter`, `wd-user`, `wd-kmdf`, `wd-proto`), PowerShell validation scripts.

---

## Scope and Assumptions

- Phase-one CLI commands should stop being pure placeholders.
- The commands do not need live kernel/device access in this plan.
- The commands should be testable in CI and on a developer machine without driver installation.
- Existing library support is uneven:
  - `netfilter` and `netdump` can already lean on `wd-user`.
  - `reflectctl` and `socketdump` can already lean on `wd-kmdf` and `wd-user`.
  - `flowtrack` needs the largest supporting change because current workspace code has no real flow event surface.

## Definition of "Real Command"

Each subcommand is considered real only when all of the following are true:

- It accepts at least one meaningful argument beyond the subcommand name.
- It executes repository logic instead of only printing a placeholder string.
- It prints stable, machine-checkable output that tests and `tests/windows/*.ps1` can assert.
- It has a focused Rust test covering argument handling and behavior.
- Its corresponding Windows script validates command output, not just exit code.

## Planned File Structure

- Modify: `crates/wd-cli/src/lib.rs`
- Modify: `crates/wd-cli/src/main.rs`
- Modify: `crates/wd-cli/src/cmd/netfilter.rs`
- Modify: `crates/wd-cli/src/cmd/netdump.rs`
- Modify: `crates/wd-cli/src/cmd/flowtrack.rs`
- Modify: `crates/wd-cli/src/cmd/socketdump.rs`
- Modify: `crates/wd-cli/src/cmd/reflectctl.rs`
- Create: `crates/wd-cli/src/cmd/common.rs`
- Create: `crates/wd-cli/src/fixtures.rs`
- Modify: `crates/wd-cli/tests/cli.rs`
- Create: `crates/wd-cli/tests/commands.rs`
- Modify: `crates/wd-user/src/handle.rs`
- Modify: `driver/wd-kmdf/src/filter_eval.rs`
- Modify: `driver/wd-kmdf/tests/filter_eval.rs`
- Modify: `tests/windows/open_close.ps1`
- Modify: `tests/windows/network_reinject.ps1`
- Modify: `tests/windows/five_layer_observe.ps1`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-07-rust-windivert-rewrite.md`

## Rollout Strategy

- Milestone 1: shared CLI contract and test harness
- Milestone 2: make `netfilter` and `netdump` real
- Milestone 3: make `reflectctl` and `socketdump` real
- Milestone 4: add minimal flow support and make `flowtrack` real
- Milestone 5: upgrade scripts and docs to assert behavior

### Task 1: Establish a Shared CLI Contract

**Files:**
- Modify: `crates/wd-cli/src/lib.rs`
- Create: `crates/wd-cli/src/cmd/common.rs`
- Create: `crates/wd-cli/src/fixtures.rs`
- Modify: `crates/wd-cli/tests/cli.rs`
- Create: `crates/wd-cli/tests/commands.rs`

- [ ] **Step 1: Define the output contract per command**

Lock in one stable summary line per command so tests and scripts can parse it:

- `netfilter`: `NETFILTER OK layer=NETWORK filter=<...> ir_bytes=<n> token=<n>`
- `netdump`: `NETDUMP OK layer=NETWORK ttl=<n> checksum=<hex>`
- `socketdump`: `SOCKETDUMP OK event=CONNECT process_id=<n> matched=<true|false>`
- `reflectctl`: `REFLECTCTL OK capabilities=<n> state=<STATE>`
- `flowtrack`: `FLOWTRACK OK event=<name> flow_id=<n> process_id=<n>`

- [ ] **Step 2: Add failing CLI behavior tests for the output contract**

Write Rust tests in `crates/wd-cli/tests/commands.rs` that invoke command entrypoints directly and assert:

- `netfilter` rejects missing `--filter`
- `netfilter` accepts a valid network filter and prints `NETFILTER OK`
- `netdump` prints decoded network metadata
- `socketdump` prints `event=CONNECT`
- `reflectctl` prints `state=Closed`
- `flowtrack` prints `FLOWTRACK OK`

- [ ] **Step 3: Run the focused CLI behavior test and confirm RED**

Run: `cargo test -p wd-cli --test commands`
Expected: FAIL because the command modules still only print placeholder strings and take no meaningful arguments.

- [ ] **Step 4: Add shared helpers for formatting and deterministic fixtures**

Create:

- `crates/wd-cli/src/cmd/common.rs`
  - helpers for rendering `KEY=value` pairs
  - shared `ExitCode` helpers for success/failure
- `crates/wd-cli/src/fixtures.rs`
  - one deterministic IPv4 frame fixture
  - one deterministic socket event fixture
  - one deterministic reflect open-response fixture
  - one deterministic flow event fixture

- [ ] **Step 5: Re-run the focused CLI behavior test**

Run: `cargo test -p wd-cli --test commands`
Expected: still FAIL, but now the command modules have the shared pieces needed for the later tasks.

### Task 2: Turn `netfilter` Into a Real Network Filter/Reinject Command

**Files:**
- Modify: `crates/wd-cli/src/cmd/netfilter.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/network_reinject.ps1`

- [ ] **Step 1: Add a failing `netfilter` test around real arguments**

Test behavior:

- `wd-cli netfilter --filter "tcp and inbound"`
  - compiles the filter via `wd_user::HandleConfig::network`
  - issues a reinjection token via `wd_kmdf::ReinjectionTable`
  - prints `NETFILTER OK`
- `wd-cli netfilter --filter "event == OPEN"`
  - fails because the filter is incompatible with `Layer::Network`

- [ ] **Step 2: Run the exact `netfilter` test and verify RED**

Run: `cargo test -p wd-cli netfilter_ -- --nocapture`
Expected: FAIL because `NetfilterCmd` currently has no fields and only prints a placeholder string.

- [ ] **Step 3: Implement the real `netfilter` command**

Add arguments:

- `--filter <EXPR>` required
- `--packet-id <u64>` optional, default fixture value
- `--show-ir-len` optional flag

Implementation path:

- compile the filter through `HandleConfig::network`
- issue a token through `ReinjectionTable::issue_for_network_packet`
- print the stable `NETFILTER OK ...` summary line
- return non-zero with a real error message on filter incompatibility or compile failure

- [ ] **Step 4: Upgrade `tests/windows/network_reinject.ps1`**

Make the script assert output contains:

- `NETFILTER OK`
- `layer=NETWORK`

and invoke:

`wd-cli.exe netfilter --filter "tcp and inbound"`

- [ ] **Step 5: Re-run the `netfilter` test and script**

Run:

- `cargo test -p wd-cli --test commands`
- `powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1`

Expected: PASS

### Task 3: Turn `netdump` Into a Real Packet Decode Command

**Files:**
- Modify: `crates/wd-cli/src/cmd/netdump.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/five_layer_observe.ps1`

- [ ] **Step 1: Add a failing `netdump` test around decoded packet output**

Test behavior:

- `wd-cli netdump`
  - decodes a deterministic IPv4 frame fixture using `wd_user::RecvEvent::decode`
  - prints TTL and header checksum fields
  - prints `NETDUMP OK`

- [ ] **Step 2: Run the `netdump` test and verify RED**

Run: `cargo test -p wd-cli netdump_ -- --nocapture`
Expected: FAIL because `NetdumpCmd` still only prints a placeholder string.

- [ ] **Step 3: Implement the real `netdump` command**

Implementation path:

- load fixture bytes from `crates/wd-cli/src/fixtures.rs`
- decode them through `RecvEvent::decode`
- read the packet metadata from the parsed packet
- print a stable summary line:
  - `NETDUMP OK`
  - `layer=NETWORK`
  - `ttl=<n>`
  - `checksum=<hex>`

- [ ] **Step 4: Upgrade `tests/windows/five_layer_observe.ps1` for `netdump`**

Capture command output and assert:

- `NETDUMP OK`
- `layer=NETWORK`

- [ ] **Step 5: Re-run the `netdump` test and script**

Run:

- `cargo test -p wd-cli --test commands`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: `netdump` assertions PASS; the script may still fail later until `flowtrack` and `socketdump` are upgraded.

### Task 4: Turn `reflectctl` Into a Real Handle Lifecycle Command

**Files:**
- Modify: `crates/wd-cli/src/cmd/reflectctl.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/open_close.ps1`

- [ ] **Step 1: Add a failing `reflectctl` test around open/close lifecycle output**

Test behavior:

- `wd-cli reflectctl`
  - builds a handle from `OpenResponse::success(0x1f)`
  - walks `HandleState` through `opening -> running -> recv shutdown -> send shutdown -> closed`
  - prints `REFLECTCTL OK capabilities=31 state=Closed`

- [ ] **Step 2: Run the `reflectctl` test and verify RED**

Run: `cargo test -p wd-cli reflectctl_ -- --nocapture`
Expected: FAIL because `ReflectctlCmd` is still placeholder-only.

- [ ] **Step 3: Implement the real `reflectctl` command**

Implementation path:

- construct `DynamicHandle` from `OpenResponse::success`
- construct and advance `HandleState`
- print negotiated capabilities and final state
- return non-zero if any lifecycle transition fails

- [ ] **Step 4: Upgrade `tests/windows/open_close.ps1`**

Make the script assert output contains:

- `REFLECTCTL OK`
- `state=Closed`

- [ ] **Step 5: Re-run the `reflectctl` test and script**

Run:

- `cargo test -p wd-cli --test commands`
- `powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1`

Expected: PASS

### Task 5: Turn `socketdump` Into a Real Socket Event Command

**Files:**
- Modify: `crates/wd-cli/src/cmd/socketdump.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/five_layer_observe.ps1`

- [ ] **Step 1: Add a failing `socketdump` test around filter matching output**

Test behavior:

- `wd-cli socketdump --filter "event == CONNECT and processId == 7"`
  - compiles and validates the filter through `wd_kmdf::FilterEngine::compile(Layer::Socket, ...)`
  - evaluates a deterministic socket-connect fixture
  - prints `SOCKETDUMP OK event=CONNECT process_id=7 matched=true`

- [ ] **Step 2: Run the `socketdump` test and verify RED**

Run: `cargo test -p wd-cli socketdump_ -- --nocapture`
Expected: FAIL because `SocketdumpCmd` takes no arguments and does not use the filter engine.

- [ ] **Step 3: Implement the real `socketdump` command**

Add arguments:

- `--filter <EXPR>` required
- `--process-id <u64>` optional, default `7`

Implementation path:

- compile filter through `FilterEngine::compile(Layer::Socket, ...)`
- build a deterministic `DriverEvent::socket_connect(process_id)`
- evaluate and print the stable summary line

- [ ] **Step 4: Upgrade `tests/windows/five_layer_observe.ps1` for `socketdump`**

Capture command output and assert:

- `SOCKETDUMP OK`
- `matched=true`

- [ ] **Step 5: Re-run the `socketdump` test and script**

Run:

- `cargo test -p wd-cli --test commands`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: `socketdump` assertions PASS; the script may still fail later until `flowtrack` is upgraded.

### Task 6: Add Minimal Flow Support and Turn `flowtrack` Into a Real Command

**Files:**
- Modify: `driver/wd-kmdf/src/filter_eval.rs`
- Modify: `driver/wd-kmdf/tests/filter_eval.rs`
- Modify: `crates/wd-cli/src/cmd/flowtrack.rs`
- Modify: `crates/wd-cli/tests/commands.rs`
- Modify: `tests/windows/five_layer_observe.ps1`

- [ ] **Step 1: Add failing driver-core tests for a minimal flow event subset**

Target subset:

- one flow event kind such as `ESTABLISHED`
- one deterministic `flow_id`
- one `process_id`
- ability to validate `layer == FLOW`
- optional support for `processId == <n>`

- [ ] **Step 2: Run the focused `wd-kmdf` flow test and verify RED**

Run: `cargo test -p wd-kmdf flow_ -- --nocapture`
Expected: FAIL because the current driver-event/filter subset has no flow event model.

- [ ] **Step 3: Implement the minimal flow subset in `wd-kmdf`**

Implementation path:

- extend `DriverEvent` with one flow event variant
- extend layer mapping helpers for `Layer::Flow`
- extend field validation/evaluation only as far as required by the `flowtrack` command
- do not overbuild a general flow engine beyond the single deterministic subset required here

- [ ] **Step 4: Add a failing `flowtrack` CLI test**

Test behavior:

- `wd-cli flowtrack --process-id 42`
  - evaluates a deterministic flow event
  - prints `FLOWTRACK OK event=ESTABLISHED flow_id=<n> process_id=42`

- [ ] **Step 5: Run the `flowtrack` CLI test and verify RED**

Run: `cargo test -p wd-cli flowtrack_ -- --nocapture`
Expected: FAIL because `FlowtrackCmd` still only prints a placeholder string.

- [ ] **Step 6: Implement the real `flowtrack` command**

Add arguments:

- `--process-id <u64>` optional, default fixture value

Implementation path:

- build the deterministic flow event fixture
- optionally validate it through the new minimal flow filter/evaluation subset
- print the stable `FLOWTRACK OK ...` summary line

- [ ] **Step 7: Upgrade `tests/windows/five_layer_observe.ps1` for `flowtrack`**

Capture command output and assert:

- `FLOWTRACK OK`
- `process_id=<n>`

- [ ] **Step 8: Re-run the flow tests and the observe script**

Run:

- `cargo test -p wd-kmdf`
- `cargo test -p wd-cli --test commands`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: PASS

### Task 7: Finish Documentation and Close Task 6

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-07-rust-windivert-rewrite.md`

- [ ] **Step 1: Update README command examples**

Document for each subcommand:

- supported arguments
- exact sample invocation
- exact summary line shape
- what is real versus still simulated in phase one

- [ ] **Step 2: Update the main rewrite plan**

Revise Task 6 status from "partial placeholder flow" to "implemented deterministic real CLI flow" once the code and scripts are actually green.

- [ ] **Step 3: Run full Task 6 verification**

Run:

- `cargo test -p wd-cli`
- `cargo test -p wd-kmdf`
- `cargo test -p wd-user --test user_api`
- `cargo test -p wd-filter --test compile`
- `cargo check --workspace`
- `powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1`
- `powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1`
- `powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1`

Expected: PASS

## Self-Review

### Spec Coverage

- CLI tooling: covered by Tasks 1 through 6.
- Windows host validation scripts: covered by Tasks 2 through 6 and finalized in Task 7.
- Installation/debugging docs for current CLI scope: covered by Task 7.
- Network and reinjection-focused phase-one flow: prioritized first in Task 2 and Task 3.

### Placeholder Scan

- No task asks to "improve later" or "fill in later."
- `flowtrack` explicitly calls out the supporting `wd-kmdf` changes it needs instead of hand-waving them.
- The plan does not assume live driver/device integration.

### Type Consistency

- `HandleConfig::network` and `RecvEvent::decode` are used only for network-oriented commands.
- `FilterEngine::compile(Layer::Socket, ...)` is used only for `socketdump`.
- `HandleState` remains the basis for `reflectctl`.
- Flow support is introduced in `wd-kmdf` before the `flowtrack` CLI task relies on it.
