# WinDivert Rust 协议规范

## 概述

WinDivert Rust 实现了一个用户态与内核态之间的二进制协议，用于网络包捕获、过滤和重注入。

## 协议版本

当前协议版本: `0.1`

```rust
ProtocolVersion { major: 0, minor: 1 }
```

## 层级 (Layer)

系统支持 5 种操作层级：

| 层级 | Wire 值 | 描述 |
|------|---------|------|
| Network | 1 | 入站网络包 |
| NetworkForward | 2 | 出站网络包 |
| Flow | 3 | 流事件 |
| Socket | 4 | Socket 事件 |
| Reflect | 5 | 反射控制 |

## 能力标志 (CapabilityFlags)

```rust
bitflags! {
    pub struct CapabilityFlags: u32 {
        const CHECKSUM_RECALC = 0x0001;  // 校验和重计算
        const NETWORK_REINJECT = 0x0002; // 网络包重注入
        const FLOW_EVENTS = 0x0004;      // 流事件支持
        const SOCKET_EVENTS = 0x0008;    // Socket 事件支持
        const REFLECT_EVENTS = 0x0010;   // 反射事件支持
    }
}
```

## IOCTL 命令

| 命令 | 值 | 描述 |
|------|-----|------|
| IOCTL_OPEN | 0x80002000 | 打开句柄 |
| IOCTL_RECV | 0x80002004 | 接收事件 |
| IOCTL_SEND | 0x80002008 | 发送数据 |

## 设备名称

- 设备名: `\\Device\\WdRust`
- DOS 设备名: `\\DosDevices\\WdRust`
- 用户态访问路径: `\\.\WdRust`

---

## Open 请求/响应

### OpenRequest

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| major | 0 | 2 | 协议主版本 (u16 LE) |
| minor | 2 | 2 | 协议次版本 (u16 LE) |
| layer | 4 | 1 | 层级 (u8) |
| reserved | 5 | 1 | 保留 |
| priority | 6 | 2 | 优先级 (i16 LE) |
| flags | 8 | 8 | 标志位 (u64 LE) |
| filter_len | 16 | 4 | 过滤器长度 (u32 LE) |
| filter_ir | 20 | N | 过滤器 IR 字节 |

**总头部长度**: 20 字节

### OpenResponse

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| major | 0 | 2 | 协议主版本 (u16 LE) |
| minor | 2 | 2 | 协议次版本 (u16 LE) |
| capabilities | 4 | 4 | 能力标志 (u32 LE) |
| status | 8 | 4 | 状态码 (u32 LE) |

**总长度**: 12 字节

---

## Runtime 事件帧

### RuntimeEventHeader

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| magic | 0 | 4 | 魔数 "WDRT" |
| major | 4 | 2 | 协议主版本 (u16 LE) |
| minor | 6 | 2 | 协议次版本 (u16 LE) |
| layer | 8 | 1 | 层级 (u8) |
| reserved | 9 | 3 | 保留 |
| payload_len | 12 | 4 | 载荷长度 (u32 LE) |

**总头部长度**: 16 字节

### NetworkEventPayload

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| magic | 0 | 4 | 魔数 "WDNW" |
| reinjection_token | 4 | 8 | 重注入令牌 (u64 LE) |
| packet_len | 12 | 4 | 包长度 (u32 LE) |
| packet | 16 | N | 原始网络包数据 |

**总头部长度**: 16 字节

---

## Runtime Send 请求

### RuntimeSendRequestHeader

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| magic | 0 | 4 | 魔数 "WDSN" |
| major | 4 | 2 | 协议主版本 (u16 LE) |
| minor | 6 | 2 | 协议次版本 (u16 LE) |
| layer | 8 | 1 | 层级 (u8) |
| reserved | 9 | 3 | 保留 |
| reinjection_token | 12 | 8 | 重注入令牌 (u64 LE) |
| payload_len | 20 | 4 | 载荷长度 (u32 LE) |

**总头部长度**: 24 字节

---

## Socket 事件载荷

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| kind | 0 | 4 | 事件类型 (u32 LE) |
| reserved | 4 | 4 | 保留 |
| process_id | 8 | 8 | 进程 ID (u64 LE) |

**总长度**: 16 字节

### SocketEventKind

| 类型 | Code |
|------|------|
| Connect | 2 |

---

## Flow 事件载荷

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| kind | 0 | 4 | 事件类型 (u32 LE) |
| reserved | 4 | 4 | 保留 |
| flow_id | 8 | 8 | 流 ID (u64 LE) |
| process_id | 16 | 8 | 进程 ID (u64 LE) |

**总长度**: 24 字节

### FlowEventKind

| 类型 | Code |
|------|------|
| Established | 4 |

---

## 错误码

### GlueIoStatus

| 状态 | 值 | 描述 |
|------|-----|------|
| Success | 0 | 成功 |
| UnsupportedIoctl | 1 | 不支持的 IOCTL |
| DecodeOpen | 2 | Open 解码失败 |
| OutputTooSmall | 3 | 输出缓冲区过小 |
| QueueEmpty | 4 | 队列为空 |
| RecvDisabled | 5 | 接收已禁用 |
| SendDisabled | 6 | 发送已禁用 |
| InvalidState | 7 | 无效状态 |
| NetworkRuntime | 8 | 网络运行时错误 |
| InvalidPointer | 9 | 无效指针 |
| InvalidHandle | 10 | 无效句柄 |
| InvalidLayer | 11 | 无效层级 |

---

## 句柄状态机

```
Opening -> Running -> RecvShutdown -> SendShutdown -> Closing -> Closed
```

| 状态 | 描述 |
|------|------|
| Opening | 初始状态 |
| Running | 正常运行 |
| RecvShutdown | 接收已关闭 |
| SendShutdown | 发送已关闭 |
| Closing | 正在关闭 |
| Closed | 已关闭 |
