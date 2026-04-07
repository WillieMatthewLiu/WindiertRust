# Rust WinDivert Rewrite Design

## Summary

Build a new Rust-based Windows packet interception system at
`/home/nextinfra/Desktop/Coder/AIResearch/Codex/00Explore/03PcapWinDivertRust`.
The new implementation keeps the original WinDivert-style split between
user mode and kernel mode, but does not attempt C ABI compatibility.

The first supported targets are:

- Windows 10 and newer
- `x64`
- `x86`

The design must keep room for future downward compatibility work, but that is
not a phase-one delivery requirement.

## Goals

- Rebuild the system as a Rust-first codebase across user mode and kernel mode.
- Preserve the original five-layer model:
  - `NETWORK`
  - `NETWORK_FORWARD`
  - `FLOW`
  - `SOCKET`
  - `REFLECT`
- Preserve packet interception, user-mode packet modification, and reinjection.
- Keep a string-based filter language for migration and operator ergonomics.
- Expose a Rust-native user API instead of cloning the old C surface.
- Allow a very small amount of `INF/rc/def/build glue` where Windows driver
  packaging or linking requires it.

## Non-Goals

- No drop-in replacement for the original `WinDivert.dll`.
- No C ABI compatibility promise.
- No behavior-by-behavior clone of every WinDivert edge case in phase one.
- No requirement to support Windows 7 or Windows 8 in the first delivery.
- No kernel-side packet rewrite fast path in phase one.

## Scope Definition

Phase one is considered successful only if all five layers exist, can be
opened from user mode, can produce events, and can participate in structured
receive/send flows. Packet rewrite and reinjection are guaranteed in phase one
for `NETWORK` and `NETWORK_FORWARD`.

For `FLOW`, `SOCKET`, and `REFLECT`, phase one focuses on observability,
structured events, filtering, queueing, and stable API semantics.

## High-Level Architecture

The new system is a Rust workspace with a clear split between shared ABI,
filter compilation, user-mode library logic, and kernel-mode driver logic.

Recommended repository structure:

- `crates/wd-proto`
  - Shared ABI
  - Layer and event enums
  - Frame definitions
  - IOCTL message layouts
  - Version negotiation
  - Stable serialized filter IR
- `crates/wd-filter`
  - Filter DSL lexer
  - Parser
  - Semantic analyzer
  - IR generation
- `crates/wd-user`
  - Rust-native user-mode API
  - Device open/close
  - Frame decode/encode
  - Packet parse and checksum helpers
  - Reinjection API
- `crates/wd-cli`
  - Example and operator tooling
  - Subcommands analogous to `netdump`, `netfilter`, `flowtrack`,
    `socketdump`, and `reflectctl`
- `crates/wd-driver-shared`
  - Shared constants needed by the driver boundary
  - Magic values
  - Layout assertions
  - Device naming constants
- `driver/wd-kmdf`
  - Rust kernel driver core
  - WDF lifecycle
  - WFP callout registration
  - Handle contexts
  - Queue management
  - Event delivery
  - Reinjection
- `driver/glue`
  - Minimal `INF`, resource, and linker glue
- `tests`
  - Unit tests
  - ABI tests
  - Integration tests
  - Host validation scripts

## Core Design Principles

### 1. Stable IR Between DSL and Driver

The user-mode API accepts string filters, but the driver never parses strings.
User mode compiles the filter into a stable `FilterIr`, then sends the encoded
IR to the driver. This keeps the DSL implementation and the kernel evaluator
decoupled.

### 2. Rust-Native API, Not C Compatibility

The user API is type-safe and Rust-native. The boundary format remains stable
for tooling and possible future bindings, but the public API uses typed enums,
structs, and error types.

### 3. Unified Event Model in Kernel Space

The driver normalizes incoming WFP and framework events into a small set of
internal event classes:

- `PacketEvent`
- `FlowEvent`
- `SocketEvent`
- `ReflectEvent`

Each event class exposes matchable fields through a shared evaluation model so
the filter engine can stay unified.

### 4. Explicit Reinjection Tokens

Packet reinjection must not rely on queue ordering. Every packet eligible for
user-mode rewrite and reinjection carries an explicit reinjection token so
multi-threaded readers and batched IO do not corrupt packet ownership.

## User-Mode Design

`wd-user` exposes typed and dynamic receive APIs.

Typed API examples:

- `Handle<NetworkLayer>`
- `Handle<SocketLayer>`
- `Handle<ReflectLayer>`

Dynamic API example:

- `Handle::recv() -> RecvEvent`

Representative receive variants:

- `RecvEvent::Network(NetworkPacket, AddressMeta)`
- `RecvEvent::NetworkForward(NetworkPacket, AddressMeta)`
- `RecvEvent::Flow(FlowEvent)`
- `RecvEvent::Socket(SocketEvent)`
- `RecvEvent::Reflect(ReflectEvent)`

Responsibilities of `wd-user`:

- compile filter strings through `wd-filter`
- open and configure device handles
- negotiate protocol version and capability bits
- decode event frames
- provide mutable packet views for `NETWORK` and `NETWORK_FORWARD`
- recalculate checksums after user edits
- send reinjection requests back to the driver

## Kernel-Mode Design

`wd-kmdf` is responsible for the real interception and event pipeline.

Responsibilities:

- KMDF driver lifecycle and device creation
- WFP callout and filter registration for the five-layer model
- per-handle context allocation and teardown
- filter evaluation using the encoded `FilterIr`
- queueing and backpressure
- delivery of matched events to user mode
- reinjection of modified packets for `NETWORK` and `NETWORK_FORWARD`
- reflect event generation for handle lifecycle visibility

Recommended explicit handle lifecycle states:

- `Opening`
- `Running`
- `RecvShutdown`
- `SendShutdown`
- `Closing`
- `Closed`

Avoid representing lifecycle with scattered booleans only.

## Data Flow

### Handle Open Path

1. User calls `open(filter, layer, priority, flags)`.
2. `wd-filter` compiles the string filter into `FilterIr`.
3. `wd-user` serializes configuration and IR into a startup frame.
4. Driver creates a per-handle context containing:
   - selected layer
   - priority
   - flags
   - queue limits
   - runtime stats
   - compiled filter state
5. Driver replies with negotiated version and supported capability bits.

### Intercept Path

1. WFP or driver framework code receives a packet or event.
2. Driver normalizes it into an internal event structure.
3. Driver evaluates the event against the handle filter IR.
4. Matching events enter the handle queue.
5. `NETWORK` and `NETWORK_FORWARD` events retain reinjection metadata.

### Receive Path

1. User reads an event frame from the device.
2. `wd-user` decodes it into typed Rust events.
3. Packet events expose mutable packet buffers.
4. Non-packet layers expose structured event data.

### Modify and Reinject Path

1. User modifies the packet buffer in Rust.
2. User calls `send()` or `reinject()`.
3. `wd-user` recalculates checksums and serializes the update.
4. Driver validates:
   - reinjection token
   - layer eligibility
   - packet shape and size
5. Driver reinjects the packet through the correct path.
6. Driver marks the packet as reinjected or impostor to prevent recursive
   interception loops.

### Close and Shutdown Path

1. `shutdown(recv/send/both)` changes the handle state.
2. New event intake stops as appropriate.
3. On `close()`, the driver drains and releases resources:
   - queues
   - pending reinjection state
   - callout references
   - filter state
   - reflect close event

## Filter DSL Design

The system keeps a string-based filter language but reimplements it in Rust.

### Filter Pipeline

- `Lexer`
  - tokenization
- `Parser`
  - AST construction
- `Semantic Analyzer`
  - field legality by layer
  - type checks
  - constant folding
  - IR generation

### IR Design Requirements

The IR is driver-oriented, not a raw AST dump. It should model:

- field comparisons
- logical composition
- negation
- ternary expressions
- literal values
- packet slice access

It must also record:

- required layers
- referenced fields
- whether payload access is needed
- whether random fields are referenced
- whether evaluation needs a complex path

### Phase-One DSL Compatibility

Must support:

- `and`, `or`, `not`
- parentheses
- ternary expressions
- boolean fields such as `inbound`, `outbound`, `loopback`, `impostor`,
  `fragment`
- protocol family and transport predicates such as `ip`, `ipv6`, `icmp`,
  `icmpv6`, `tcp`, `udp`
- structured fields such as `processId`, `localAddr`, `remoteAddr`,
  `localPort`, `remotePort`, `protocol`, `event`, `layer`, `priority`
- byte access forms:
  - `packet[...]`
  - `packet16[...]`
  - `packet32[...]`
- standard comparison operators

Can be deferred after phase one, while reserving IR space:

- full compatibility with all historical corner cases
- perfect format-to-string round-tripping
- every old helper behavior quirk

## Layer Capability Matrix

### NETWORK

- intercept packets
- deliver to user mode
- allow user packet mutation
- support reinjection
- support checksum recalculation helpers

### NETWORK_FORWARD

- same phase-one guarantee set as `NETWORK`

### FLOW

- observe flow lifecycle events
- deliver structured flow data
- allow filtering and queueing
- no flow mutation contract in phase one

### SOCKET

- observe bind/connect/listen/accept/close style events
- deliver structured socket event data
- allow filtering and queueing
- no reverse mutation contract in phase one

### REFLECT

- report handle open and close style events
- expose layer, flags, and priority metadata
- support observability and diagnostics

## Testing Strategy

The system must be tested at four levels.

### 1. Pure Rust Unit Tests

For:

- DSL lexer/parser/semantic analyzer
- IR serialization and deserialization
- packet parsing
- checksum recomputation
- user-mode frame handling

### 2. Kernel Logic Tests Without Device Dependency

Extract testable driver logic into ordinary Rust modules where possible:

- filter evaluator
- queue and backpressure rules
- reinjection token lifecycle
- handle state machine

### 3. User-Mode and Driver Integration Tests

Run on real Windows hosts:

- open and close handles
- receive five-layer events
- rewrite and reinject network packets
- shutdown semantics
- concurrent recv/send behavior

### 4. End-to-End Host Validation

Use CLI-driven traffic generation and capture to validate:

- real packet interception
- packet modification and reinjection
- flow/socket/reflect event delivery
- `x64` and `x86` installability

## Build and Toolchain Constraints

- Rust should remain the primary implementation language.
- User-mode crates should build on stable Rust by default.
- Driver logic should stay stable-compatible where practical.
- If Windows driver packaging requires toolchain-specific glue, keep it thin and
  isolated in `driver/glue`.
- Cargo handles Rust compilation, formatting, linting, and unit tests.
- Windows-specific packaging handles driver installation artifacts and signing.

## Delivery Milestones

### M1. Workspace and Protocol Foundation

- create the Rust workspace
- define ABI and version negotiation
- define device messages and basic communication
- implement filter DSL to IR pipeline

### M2. Network Interception and Reinjection

- implement `NETWORK`
- implement `NETWORK_FORWARD`
- user-mode receive path
- packet mutation helpers
- checksum repair
- reinjection

### M3. Five-Layer Observability

- implement `FLOW`
- implement `SOCKET`
- implement `REFLECT`
- structured event APIs
- filter evaluation across all layers
- CLI observability tools

### M4. Engineering Closure

- `x64` and `x86` packaging
- installation and validation flows
- real Windows end-to-end tests
- examples and operational documentation

## Risks and Mitigations

### Rust in Windows Kernel Space

Risk:

- ecosystem maturity is lower than common user-mode Rust development

Mitigation:

- move testable logic into plain Rust crates
- isolate WDK-facing boundary code

### Cross-Architecture ABI Drift

Risk:

- `x86` and `x64` layout mismatches

Mitigation:

- explicit layout assertions
- versioned message formats
- cross-architecture integration tests

### Reinjection Safety

Risk:

- recursive capture loops
- malformed packet reinjection
- checksum or length corruption

Mitigation:

- explicit reinjection tokens
- helper-enforced checksum repair
- strict validation before reinjection
- reinjected packet marking

### Scope Creep from DSL Compatibility

Risk:

- overcommitting to historical quirks slows core delivery

Mitigation:

- phase-one compatibility subset
- driver executes only stable IR
- defer edge-case parity until core system is stable

## Deliverables

Phase one must produce:

- a Rust workspace at
  `/home/nextinfra/Desktop/Coder/AIResearch/Codex/00Explore/03PcapWinDivertRust`
- a Rust user-mode library
- a Rust kernel driver core
- minimal driver packaging glue
- CLI tooling
- automated tests
- real Windows validation scripts
- installation and debugging documentation

## Decision Record

Chosen constraints and decisions:

- full Rust rewrite across user mode and kernel mode
- first supported platforms are Windows 10+ `x64` and `x86`
- preserve five-layer architecture
- user API is Rust-native
- keep string DSL filters
- guarantee packet mutation and reinjection for `NETWORK` and
  `NETWORK_FORWARD` in phase one
- accept minimal non-Rust packaging glue where necessary
