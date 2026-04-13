# WinDivert Rust 架构规范

## 概述

WinDivert Rust 是一个 Windows 网络驱动框架的 Rust 重写，采用分层架构，将内核态驱动逻辑与用户态运行时分离，通过 IOCTL 协议通信。

## Crate 依赖图

```
wd-cli
 ├── wd-user
 │    ├── wd-proto
 │    ├── wd-filter
 │    ├── wd-driver-shared
 │    └── windows-sys
 ├── wd-kmdf (测试依赖)
 └── clap

wd-kmdf
 ├── wd-kmdf-core
 ├── wd-proto
 ├── wd-filter
 └── wd-driver-shared

wd-kmdf-core (no_std)
 └── (无外部依赖)
```

## Crate 职责

### wd-proto

**职责**: 用户态/内核态共享的协议类型与编解码。

**核心类型**:
- `ProtocolVersion` - 协议版本 (当前 0.1)
- `Layer` - 操作层级 (Network, NetworkForward, Flow, Socket, Reflect)
- `CapabilityFlags` - 能力标志位
- `OpenRequest` / `OpenResponse` - 打开句柄请求/响应
- `RuntimeEventHeader` / `RuntimeEventFrame` - 运行时事件帧
- `NetworkEventPayload` - 网络事件载荷
- `RuntimeSendRequestHeader` / `RuntimeSendRequest` - 重注入请求
- `SocketEventPayload` / `FlowEventPayload` - Socket/Flow 事件载荷
- `SocketEventKind` / `FlowEventKind` - 事件类型枚举

**核心函数**:
- `encode_open_request` / `decode_open_request`
- `encode_open_response` / `decode_open_response`
- `encode_runtime_event` / `decode_runtime_event`
- `encode_network_event_payload` / `decode_network_event_payload`
- `encode_runtime_send_request` / `decode_runtime_send_request`
- `encode_socket_event_payload` / `decode_socket_event_payload`
- `encode_flow_event_payload` / `decode_flow_event_payload`

### wd-driver-shared

**职责**: 用户态/内核态共享的设备常量。

**常量**:
- `DEVICE_NAME` = `\\Device\\WdRust`
- `DOS_DEVICE_NAME` = `\\DosDevices\\WdRust`
- `IOCTL_OPEN` = `0x80002000`
- `IOCTL_RECV` = `0x80002004`
- `IOCTL_SEND` = `0x80002008`

### wd-filter

**职责**: 过滤器编译器，将文本表达式编译为二进制 IR。

**编译管线**: `lex` -> `parse` -> `analyze` -> `lower` -> `FilterIr`

**核心类型**:
- `FilterIr` - 编译后的过滤器 IR (required_layers, needs_payload, referenced_fields, program)
- `OpCode` - IR 操作码 (FieldTest, PacketLoad32/16/8, And, Or, Not)
- `LayerMask` - 层级位掩码
- `CompileError` - 编译错误

**核心函数**:
- `compile(input: &str) -> Result<FilterIr, CompileError>`
- `encode_ir(ir: &FilterIr) -> Vec<u8>`
- `decode_ir(bytes: &[u8]) -> Result<FilterIr, DecodeError>`

### wd-user

**职责**: 用户态运行时库，提供设备访问、会话管理和帧解码。

**核心 trait**:
- `RuntimeTransport` - 传输层抽象 (probe, open, open_session, close)
- `RuntimeSession` - 会话抽象 (recv_one, send_one, close)

**核心类型**:
- `WindowsTransport` / `WindowsSession` - Windows 平台实现
- `RuntimeOpenConfig` - 打开配置
- `RuntimeProbe` - 探测结果
- `RuntimeError` - 运行时错误
- `HandleConfig` / `DynamicHandle` - 句柄配置与动态句柄
- `RecvEvent` - 接收事件 (Network, Socket, Flow)
- `NetworkPacket` - 网络包 (含校验和修复)
- `DeviceAvailability` - 设备可用性

### wd-kmdf-core

**职责**: `no_std` 内核态核心数据结构。

**核心类型**:
- `HandleState` - 句柄状态机 (Opening -> Running -> RecvShutdown -> SendShutdown -> Closing -> Closed)
- `GlueIoStatus` / `GlueIoResult` - FFI IO 结果
- `ReinjectionToken` - 重注入令牌
- `FixedReinjectionTable<N>` - 固定大小重注入表
- `FixedPacket<N>` - 固定大小包存储
- `ByteRing<SLOTS, BYTES>` - 固定大小字节环形缓冲区

### wd-kmdf

**职责**: 内核态驱动运行时 (std 环境，用于测试和 FFI glue)。

**核心类型**:
- `RuntimeDevice` - 运行时设备 (状态机 + 过滤引擎 + 事件队列 + 重注入表)
- `RuntimeIoctlDispatcher` - IOCTL 分发器
- `NetworkRuntime` - 网络运行时 (事件编码/发送解码)
- `FilterEngine` - 过滤引擎 (IR 加载 + 事件匹配)
- `DriverEvent` - 驱动事件 (NetworkPacket, SocketConnect, ReflectOpen, ReflectClose, FlowEstablished)
- `RuntimeGlueApi` - FFI glue API

**FFI 导出**:
- `wd_runtime_glue_create(queue_capacity) -> *mut RuntimeGlueApi`
- `wd_runtime_glue_destroy(handle)`
- `wd_runtime_glue_device_control(handle, ioctl, input_ptr, input_len, output_ptr, output_len) -> GlueIoResult`
- `wd_runtime_glue_queue_network_event(handle, layer_wire, packet_id, packet_ptr, packet_len) -> GlueIoResult`

### wd-cli

**职责**: 命令行工具入口。

**子命令**:
- `netdump` - 网络包捕获
- `netfilter` - 网络过滤 (validate/observe/reinject)
- `flowtrack` - 流事件跟踪
- `socketdump` - Socket 事件捕获
- `reflectctl` - 反射控制 (probe/open/close/capabilities/state)

---

## 数据流

### 捕获流 (Capture)

```
内核 NDIS -> KMDF 回调
  -> queue_network_event(layer, packet_id, packet)
  -> FilterEngine.matches_network_packet() [过滤]
  -> ReinjectionTable.issue_for_network_packet() [签发 token]
  -> encode_runtime_event + encode_network_event_payload [编码]
  -> ByteRing.push() [入队]
  -> 用户态 IOCTL_RECV
  -> ByteRing.pop_into() [出队]
  -> 用户态 RecvEvent::decode() [解码]
```

### 重注入流 (Reinject)

```
用户态 encode_runtime_send_request(layer, token, packet)
  -> IOCTL_SEND
  -> decode_runtime_send_request() [解码]
  -> ReinjectionTable.consume_raw(token) [消费 token]
  -> AcceptedReinjection { layer, packet_id, packet } [返回给 KMDF 回调]
  -> NDIS 重新注入
```

### 事件流 (Socket/Flow)

```
内核 WFP 回调
  -> queue_socket_event(kind, process_id) / queue_flow_event(kind, flow_id, process_id)
  -> FilterEngine.matches() [过滤]
  -> encode_runtime_event + encode_socket/flow_event_payload [编码]
  -> ByteRing.push() [入队]
  -> 用户态 IOCTL_RECV
  -> 用户态 RecvEvent::decode() [解码]
```

---

## 关键常量

| 常量 | 值 | 来源 |
|------|-----|------|
| RUNTIME_EVENT_HEADER_LEN | 16 | wd-proto |
| RUNTIME_SEND_HEADER_LEN | 24 | wd-proto |
| OPEN_REQUEST_HEADER_LEN | 20 | wd-proto |
| OPEN_RESPONSE_LEN | 12 | wd-proto |
| NETWORK_EVENT_PAYLOAD_HEADER_LEN | 16 | wd-proto |
| SOCKET_EVENT_PAYLOAD_LEN | 16 | wd-proto |
| FLOW_EVENT_PAYLOAD_LEN | 24 | wd-proto |
| ACCEPTED_PACKET_BYTES | 2048 | wd-kmdf |
| RUNTIME_FRAME_SLOTS | 32 | wd-kmdf |
| RUNTIME_FRAME_BYTES | 2048 | wd-kmdf |
| MAX_REFERENCED_FIELDS | 256 | wd-filter |
| MAX_PROGRAM_LEN | 4096 | wd-filter |
| MAX_FIELD_BYTE_LEN | 32 | wd-filter |
