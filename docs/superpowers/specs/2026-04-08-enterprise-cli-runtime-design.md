# Enterprise CLI Runtime Design

## Summary

This design upgrades the five `wd-cli` subcommands from deterministic
phase-one commands into enterprise-oriented runtime commands.

The target is not "installer complete" or "service platform complete."
The target is a reliable operator CLI that:

- talks to the real device path when available
- fails with stable non-zero exit codes when the device path is unavailable
- supports both human-readable and structured JSON output
- supports one-shot and streaming observation modes
- emits uniform diagnostics across all five commands

This design explicitly excludes:

- driver installation
- code signing
- Windows service/agent deployment
- centralized configuration management

## Goals

- Make all five subcommands use real runtime paths rather than fixtures.
- Keep command behavior scriptable and automation-safe.
- Keep command failures diagnosable on machines where the device or driver is
  missing or misconfigured.
- Standardize output, error categories, and exit codes across commands.
- Support single-event mode by default and explicit streaming mode via
  `--follow`.

## Non-Goals

- No automatic driver installation or repair.
- No implicit fallback to fixture or placeholder behavior.
- No silent success when the device is unavailable.
- No requirement to build a long-running control-plane service in this phase.
- No expansion into unrelated packaging or deployment work.

## Scope Definition

This phase is successful only if:

- `reflectctl`, `netdump`, `socketdump`, `flowtrack`, and `netfilter`
  operate against the real runtime path
- every command can distinguish between:
  - invalid arguments
  - missing device
  - open failure
  - protocol/capability mismatch
  - runtime I/O failure
  - decode failure
- every command supports:
  - human-readable output by default
  - `--json`
  - `--verbose`
- observation commands support:
  - one-shot mode by default
  - `--follow` streaming mode

## High-Level Architecture

The CLI should be split into four layers.

### 1. CLI Front-End

Responsibilities:

- parse arguments with `clap`
- select output mode
- select one-shot or streaming mode
- pass structured requests to the runtime layer

This layer must remain thin.

### 2. Runtime Layer

Responsibilities:

- resolve and validate device paths
- probe device readiness
- open and close handles
- run protocol/version/capability negotiation
- normalize low-level OS or IOCTL errors into CLI error categories
- implement shared `--json`, `--verbose`, and exit-code behavior
- implement shared `--follow` loop semantics and graceful stop handling

This is the main shared layer that prevents the five commands from drifting
into inconsistent behavior.

### 3. Domain Layer

Responsibilities are command-specific:

- `reflectctl`
  - control-plane probing
  - open/close/state/capability actions
- `netdump`
  - real network receive path
  - packet decode and presentation
- `socketdump`
  - real socket event receive path
  - event filtering and presentation
- `flowtrack`
  - real flow event receive path
  - event filtering and presentation
- `netfilter`
  - filter validation
  - real open path
  - optional live observation and reinjection-oriented runtime actions

### 4. Diagnostics Layer

Responsibilities:

- stable error categories
- stable exit-code mapping
- consistent human-readable error lines
- consistent JSON error objects
- verbose diagnostic blocks for operator troubleshooting

## Shared Runtime Contract

The runtime layer should present a command-independent contract:

- `probe_device()`
  - verifies whether the expected device link exists
- `open_handle(layer, filter, flags, timeout)`
  - opens the runtime handle
- `recv_one()`
  - receives a single raw frame or event
- `recv_loop()`
  - receives repeatedly until count limit or user interrupt
- `send_or_control()`
  - used by commands that need control or reinjection operations
- `close_handle()`
  - closes and reports final state when relevant

The runtime layer should not know how to render a network packet or socket
event. It should know only enough to acquire bytes/events and map failures.

## Command Contracts

## `reflectctl`

### Purpose

Probe and operate the real control path for the runtime device.

### Default Behavior

Perform `probe + open + capability report` and exit.

### Supported Arguments

- `--action probe|open|close|capabilities|state`
- `--timeout-ms <u64>`
- `--json`
- `--verbose`

### Human-Readable Success Output

```text
REFLECTCTL OK device=ready capabilities=<n> protocol=<major.minor> state=<STATE>
```

### JSON Success Output

Fields:

- `command`
- `status`
- `device`
- `capabilities`
- `protocol`
- `state`

## `netdump`

### Purpose

Open the real network receive path, obtain network packet data, decode it, and
present packet metadata.

### Default Behavior

Read one event and exit.

### Supported Arguments

- `--filter <expr>`
- `--count <u64>`
- `--follow`
- `--timeout-ms <u64>`
- `--json`
- `--verbose`

### Human-Readable Success Output

One-shot mode:

```text
NETDUMP OK layer=NETWORK ttl=<n> checksum=<hex> packet_len=<n> timestamp=<ts>
```

Streaming mode:

- one line per event

### JSON Success Output

Fields:

- `command`
- `status`
- `layer`
- `direction`
- `ttl`
- `checksum`
- `packet_len`
- `timestamp`

## `socketdump`

### Purpose

Open the real socket event path, receive socket events, and render them through
one-shot or streaming output.

### Default Behavior

Read one event and exit.

### Supported Arguments

- `--filter <expr>`
- `--process-id <u64>`
- `--count <u64>`
- `--follow`
- `--timeout-ms <u64>`
- `--json`
- `--verbose`

### Human-Readable Success Output

```text
SOCKETDUMP OK event=<EVENT> process_id=<n> local=<addr:port> remote=<addr:port>
```

### JSON Success Output

Fields:

- `command`
- `status`
- `event`
- `process_id`
- `local_addr`
- `local_port`
- `remote_addr`
- `remote_port`

## `flowtrack`

### Purpose

Open the real flow event path and render flow events.

### Default Behavior

Read one event and exit.

### Supported Arguments

- `--process-id <u64>`
- `--count <u64>`
- `--follow`
- `--timeout-ms <u64>`
- `--json`
- `--verbose`

### Human-Readable Success Output

```text
FLOWTRACK OK event=<EVENT> flow_id=<n> process_id=<n> direction=<dir> timestamp=<ts>
```

### JSON Success Output

Fields:

- `command`
- `status`
- `event`
- `flow_id`
- `process_id`
- `direction`
- `timestamp`

## `netfilter`

### Purpose

Validate and install a real network filter, then operate in one of a small
set of explicit modes.

### Default Behavior

Default to `validate` mode.

This avoids unsafe implicit behavior such as automatically reinjecting traffic
just because a filter parsed successfully.

### Supported Arguments

- `--filter <expr>`
- `--mode validate|observe|reinject`
- `--count <u64>`
- `--follow`
- `--timeout-ms <u64>`
- `--json`
- `--verbose`

### Human-Readable Success Output

```text
NETFILTER OK mode=<MODE> layer=NETWORK filter=<expr> handle=<id> matched_count=<n>
```

### JSON Success Output

Fields:

- `command`
- `status`
- `mode`
- `layer`
- `filter`
- `handle_id`
- `matched_count`

## Observation-Mode Rules

The following commands are observation commands:

- `netdump`
- `socketdump`
- `flowtrack`
- `netfilter` when `--mode observe`

Shared rules:

- default mode is one-shot
- `--follow` enables continuous streaming
- `--count` limits the number of events
- `--count` without `--follow` means "read up to N events, then exit"
- `--follow` with `--count` means "stream until N events or user interrupt"
- Ctrl+C returns a graceful stop result rather than an internal error

## Output Model

### Default Output

Human-readable single-line summary for success and single-line summary for
failure.

### `--json`

Structured machine-parseable output.

For one-shot commands:

- emit one JSON object

For streaming commands:

- emit one JSON object per line

### `--verbose`

Adds a diagnostics block with:

- device path
- selected layer
- selected filter
- command mode
- low-level error string when present
- next-step suggestion

`--verbose` should enrich output, not change exit-code semantics.

## Error Model

## Exit Codes

- `0`
  - success
- `2`
  - argument error
- `3`
  - device unavailable
- `4`
  - permission or open failure
- `5`
  - protocol or capability mismatch
- `6`
  - runtime I/O failure
- `7`
  - decode failure
- `8`
  - graceful user interrupt

## Error Categories

- `argument_error`
- `device_unavailable`
- `open_failed`
- `protocol_mismatch`
- `capability_mismatch`
- `io_failure`
- `decode_failure`
- `interrupted`

### Human-Readable Error Output

Example:

```text
NETDUMP ERROR code=3 category=device_unavailable message=WdRust device not found suggestion=verify driver is installed and device link is present
```

### JSON Error Output

Example:

```json
{
  "command": "netdump",
  "status": "error",
  "code": 3,
  "category": "device_unavailable",
  "message": "WdRust device not found",
  "details": {
    "device_path": "\\\\.\\WdRust",
    "layer": "NETWORK"
  },
  "suggestion": "verify driver is installed and device link is present"
}
```

### Diagnostic Sequence for Missing/Unavailable Device

All commands should follow the same diagnostic order:

1. resolve expected device path
2. probe whether the device link exists
3. attempt open when probing suggests the path should exist
4. distinguish:
   - device not found
   - device found but open failed
   - device opened but negotiation failed
5. emit one stable category and one stable exit code

## Safety and Runtime Behavior

- No command should silently downgrade to fixture mode.
- No command should claim success when the device path is missing.
- `netfilter` must not perform implicit reinjection in default mode.
- `--follow` must terminate cleanly on Ctrl+C.
- runtime handles must be explicitly closed on success, failure, and interrupt.

## Testing Strategy

### Unit Tests

- argument parsing
- output rendering
- JSON schema stability
- exit-code mapping
- error-category mapping

### Integration Tests

- device-missing path
- open-failure path
- protocol/capability mismatch path
- one-shot success path through transport abstraction or controlled test target
- streaming stop semantics

### Windows Script Validation

Windows host scripts should validate both:

- success-path command behavior when the runtime is available
- error-path contract when the runtime is unavailable

The scripts should assert:

- exit code
- error category
- key fields in default or JSON output

## Implementation Notes

The current repository already contains phase-one deterministic command logic.
That work should be treated as temporary scaffolding rather than the final
runtime implementation.

The recommended implementation order is:

1. build the shared runtime/error/output layer
2. convert `reflectctl` first to validate probe/open/close behavior
3. convert `netdump` next to validate the real receive/decode path
4. convert `socketdump` and `flowtrack`
5. convert `netfilter` last because it has the most operational risk

## Risks

- Real device I/O may reveal protocol gaps that the deterministic commands
  never exercised.
- The current `wd-user` crate does not yet expose a real device-handle I/O API,
  so this design requires new runtime work rather than shallow CLI edits.
- Streaming commands can easily diverge in formatting or shutdown behavior if
  the shared runtime layer is skipped.
- `netfilter` can become unsafe if validate/observe/reinject modes are not kept
  explicit and separate.

## Open Decisions Already Resolved

The following decisions are already fixed by design discussion:

- all five commands should be real runtime commands
- the scope is runtime capability only, not installation/signing/service
- missing device or driver is expected in the field and must be diagnosed
- commands need both automation-safe and human-friendly outputs
- observation commands default to one-shot and support `--follow`
- default output is human-readable with explicit `--json`

