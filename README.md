# WinDivert Rust

WinDivert Rust 是一个 Rust-first 的 Windows 网络驱动框架，采用分层架构将内核态驱动逻辑与用户态运行时分离，通过 IOCTL 二进制协议通信。

## 项目状态

| 组件 | 状态 | 说明 |
|------|------|------|
| 协议编解码 (`wd-proto`) | 可用 | Open/Event/Send 帧完整编解码 |
| 过滤器编译 (`wd-filter`) | 可用 | 词法→语法→语义→IR→WDIR 二进制 |
| 用户态 API (`wd-user`) | 可用 | 传输/会话抽象 + Windows 实现 |
| CLI 工具 (`wd-cli`) | 可用 | 5 个子命令，runtime-first |
| 驱动核心逻辑 (`wd-kmdf`) | 可测试 | 纯 Rust 测试，无设备依赖 |
| 驱动 no_std 核心 (`wd-kmdf-core`) | 可用 | 状态机、环形缓冲、重注入表 |
| KMDF C 桥接 | 骨架 | C ABI glue + host smoke + KMDF skeleton |
| 驱动签名/安装 | 未实现 | 仅有占位 INF，无签名流程 |
| Demo 程序 | 可用 | DNS 重定向 + 连接监控 |

## 仓库结构

```
.
├── crates/
│   ├── wd-proto/            # 用户态/内核态共享协议类型与编解码
│   ├── wd-driver-shared/    # 共享常量 (设备名、IOCTL 码)
│   ├── wd-filter/           # 过滤器编译器 (文本 → WDIR 二进制)
│   ├── wd-user/             # 用户态运行时 API
│   ├── wd-cli/              # CLI 命令行工具
│   ├── demo-dns-redirect/   # Demo: DNS 请求拦截与重定向
│   └── demo-conn-monitor/   # Demo: 网络连接实时监控
├── driver/
│   ├── wd-kmdf-core/       # no_std 内核核心类型
│   ├── wd-kmdf/             # KMDF 运行时子集 (std, 可测试)
│   └── glue/                # INF、C 桥接、KMDF skeleton
├── tests/
│   └── windows/             # PowerShell 主机验证脚本
└── openspec/                # 开放规格文档
```

## 环境要求

| 要求 | 用途 |
|------|------|
| Rust stable | 编译所有 crate |
| PowerShell | 运行验证脚本 |
| Windows 10+ | 目标平台 |
| WDK + MSVC (可选) | 编译 KMDF skeleton |
| LLVM/clang-cl (可选) | host smoke C 桥接编译 |

## 快速开始

```powershell
# 构建 CLI
cargo build -p wd-cli

# 运行全部测试 (无需驱动)
cargo test --workspace --offline

# 查看命令帮助
cargo run -p wd-cli -- --help
```

---

## 架构

### Crate 依赖图

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
 ├── wd-kmdf-core (no_std)
 ├── wd-proto
 ├── wd-filter
 └── wd-driver-shared

wd-kmdf-core (no_std)
 └── (无外部依赖)
```

### 数据流

**捕获流**:
```
内核 NDIS/WFP → KMDF 回调
  → FilterEngine.matches()        [过滤]
  → ReinjectionTable.issue()      [签发 token]
  → encode_runtime_event()        [编码]
  → ByteRing.push()               [入队]
  → 用户态 IOCTL_RECV
  → RecvEvent::decode()           [解码]
```

**重注入流**:
```
用户态修改包 → encode_runtime_send_request()
  → IOCTL_SEND
  → ReinjectionTable.consume()    [消费 token]
  → AcceptedReinjection           [返回给 KMDF 回调]
  → NDIS 重新注入
```

### 五层模型

| 层级 | Wire | 能力 |
|------|------|------|
| Network | 1 | 入站包拦截、修改、重注入、校验和修复 |
| NetworkForward | 2 | 出站包拦截、修改、重注入、校验和修复 |
| Flow | 3 | 流事件观测 (ESTABLISHED)、过滤、队列 |
| Socket | 4 | Socket 事件观测 (CONNECT)、过滤、队列 |
| Reflect | 5 | 句柄生命周期可见性 (OPEN/CLOSE) |

---

## 用户态 API 接口

### 传输与会话

```rust
use wd_user::{
    RuntimeTransport, RuntimeSession, WindowsTransport,
    RuntimeOpenConfig, RuntimeProbe, RuntimeError,
    DeviceAvailability, default_device_path,
};

// 1. 创建传输
let transport = WindowsTransport::default();

// 2. 探测设备
let availability = transport.probe()?;
if availability == DeviceAvailability::Missing {
    return Err(RuntimeError::device_unavailable(default_device_path()));
}

// 3. 打开配置
let config = RuntimeOpenConfig::network(filter_ir);  // 或 .socket() / .flow() / .reflect()

// 4. 探测 + 打开会话
let probe = transport.open(&config)?;
let mut session = transport.open_session(&config)?;

// 5. 收发
let raw = session.recv_one(65535)?;
session.send_one(&send_bytes)?;

// 6. 关闭
session.close()?;
```

### 事件解码

```rust
use wd_user::RecvEvent;

let event = RecvEvent::decode(&raw)?;

match &event {
    RecvEvent::Network(packet) => {
        println!("layer={:?} bytes_len={}", packet.layer(), packet.bytes().len());
        if let Some(token) = packet.reinjection_token() {
            println!("reinjection_token={token}");
        }
    }
    RecvEvent::Socket(socket_event) => {
        println!("event={:?} process_id={}", socket_event.kind(), socket_event.process_id());
    }
    RecvEvent::Flow(flow_event) => {
        println!("event={:?} flow_id={} process_id={}",
            flow_event.kind(), flow_event.flow_id(), flow_event.process_id());
    }
}
```

### 包修改与重注入

```rust
use wd_proto::encode_runtime_send_request;
use wd_user::{HandleConfig, RecvEvent};

// 编译过滤器
let cfg = HandleConfig::network("tcp and inbound")?;
let config = RuntimeOpenConfig::network(cfg.filter_ir().to_vec());

// 打开会话并接收
let mut session = transport.open_session(&config)?;
let raw = session.recv_one(65535)?;
let mut event = RecvEvent::decode(&raw)?;

// 修改包
if let Some(packet) = event.packet_mut() {
    packet.set_ipv4_ttl(64);  // 标记校验和为 Dirty
    event.repair_checksums()?; // 重算 IPv4 头部校验和
}

// 重注入
if let Some(packet) = event.packet() {
    if let Some(token) = packet.reinjection_token() {
        let request = encode_runtime_send_request(packet.layer(), token, packet.bytes());
        session.send_one(&request)?;
    }
}
```

### 过滤器编译

```rust
use wd_filter::{compile, encode_ir, decode_ir, FilterIr, LayerMask};

// 编译
let ir = compile("tcp and inbound and localPort == 443")?;
assert!(ir.required_layers.contains(LayerMask::NETWORK));

// 编码为 WDIR 二进制
let bytes = encode_ir(&ir);

// 解码回 IR
let roundtrip = decode_ir(&bytes)?;
assert_eq!(roundtrip, ir);
```

**支持的谓词**:

| 类别 | 示例 |
|------|------|
| 协议 | `tcp`, `udp`, `ipv4`, `ipv6` |
| 方向 | `inbound`, `outbound` |
| 端口 | `localPort == 443`, `remotePort == 80` |
| 地址 | `localAddr == 192.168.1.1`, `remoteAddr == 10.0.0.0/8` |
| 事件 | `event == CONNECT`, `event == ESTABLISHED`, `event == OPEN` |
| 层级 | `layer == NETWORK`, `layer == FLOW` |
| 进程 | `processId == 1234` |
| 包偏移 | `packet[0] == 0x45`, `packet16[10] == 0xaabb`, `packet32[12] == 0x01020304` |
| 逻辑 | `and`, `or`, `not`, 括号 |

### 错误处理

```rust
use wd_user::RuntimeError;

// 错误码定义
// code=2: argument_error (CLI 层)
// code=3: device_unavailable
// code=4: open_failed
// code=5: protocol_mismatch
// code=6: io_failure

let err = RuntimeError::device_unavailable(r"\\.\WdRust");
assert_eq!(err.code(), 3);
assert_eq!(err.category(), "device_unavailable");
assert_eq!(err.suggestion(), "verify driver is installed and device link is present");
```

### 协议编解码

```rust
use wd_proto::{
    Layer, ProtocolVersion, OpenRequest, OpenResponse,
    encode_open_request, decode_open_request,
    encode_open_response, decode_open_response,
    encode_runtime_event, decode_runtime_event,
    encode_network_event_payload, decode_network_event_payload,
    encode_runtime_send_request, decode_runtime_send_request,
    encode_socket_event_payload, decode_socket_event_payload,
    encode_flow_event_payload, decode_flow_event_payload,
};

// Open 请求
let request = OpenRequest::new(Layer::Network, filter_ir, 0, 0);
let encoded = encode_open_request(&request);
let decoded = decode_open_request(&encoded)?;

// 运行时事件帧
let event_bytes = encode_runtime_event(Layer::Network, &payload);
let frame = decode_runtime_event(&event_bytes)?;

// 网络事件载荷 (含 reinjection token)
let net_payload = encode_network_event_payload(token, &packet_bytes);
let decoded = decode_network_event_payload(&net_payload)?;
```

**帧格式常量**:

| 帧类型 | Magic | 头部长度 |
|--------|-------|---------|
| RuntimeEvent | `WDRT` | 16 字节 |
| RuntimeSend | `WDSN` | 24 字节 |
| OpenRequest | - | 20 字节 |
| OpenResponse | - | 12 字节 |
| NetworkPayload | `WDNW` | 16 字节 |
| SocketPayload | - | 16 字节 |
| FlowPayload | - | 24 字节 |

---

## CLI 工具

### 通用选项

| 选项 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `--json` | flag | false | JSON 输出 |
| `--verbose` | flag | false | 详细输出 |
| `--timeout-ms` | u64 | 5000 | 超时 (毫秒) |

### 退出码

| 码 | 类别 | 说明 |
|----|------|------|
| 0 | success | 成功 |
| 2 | argument_error | 参数错误 |
| 3 | device_unavailable | 设备不可用 |
| 4 | open_failed | 打开失败 |
| 5 | protocol_mismatch | 协议不匹配 |
| 6 | io_failure | IO 失败 |

### reflectctl — 反射控制

```powershell
# 探测设备
cargo run -p wd-cli -- reflectctl --action probe

# 打开反射句柄
cargo run -p wd-cli -- reflectctl --action open

# 查询能力
cargo run -p wd-cli -- reflectctl --action capabilities --json

# 关闭
cargo run -p wd-cli -- reflectctl --action close
```

输出示例:
```
REFLECTCTL OK device=ready capabilities=31 protocol=0.1 state=Open
```

### netdump — 网络包捕获

```powershell
# 捕获一个包
cargo run -p wd-cli -- netdump

# JSON 输出
cargo run -p wd-cli -- netdump --json

# 持续捕获 10 个包
cargo run -p wd-cli -- netdump --follow --count 10
```

输出示例:
```
NETDUMP OK layer=NETWORK ttl=64 checksum=0x9c93 packet_len=20 timestamp=1744165948123
```

> 注意: `--filter` 当前未实现，传入非空值会返回 `argument_error`。

### netfilter — 网络过滤

```powershell
# 验证过滤器
cargo run -p wd-cli -- netfilter --filter "tcp and inbound" --mode validate

# 观测模式
cargo run -p wd-cli -- netfilter --filter "tcp and inbound" --mode observe

# 重注入模式
cargo run -p wd-cli -- netfilter --filter "tcp and inbound" --mode reinject
```

输出示例:
```
NETFILTER OK mode=validate layer=NETWORK filter="tcp and inbound" ir_bytes=24
```

**模式说明**:

| 模式 | 说明 | 约束 |
|------|------|------|
| validate | 编译过滤器并确认 runtime ready | count=1, 不接受 --follow |
| observe | 接收并显示匹配的网络包 | count>1 需要 --follow |
| reinject | 接收一个包后重注入 | count=1, 不接受 --follow |

### socketdump — Socket 事件

```powershell
cargo run -p wd-cli -- socketdump --filter "event == CONNECT and processId == 7"
```

输出示例:
```
SOCKETDUMP OK event=CONNECT process_id=7 matched=true timestamp=1744165948456
```

### flowtrack — 流事件

```powershell
cargo run -p wd-cli -- flowtrack --process-id 42
```

输出示例:
```
FLOWTRACK OK event=ESTABLISHED flow_id=65261 process_id=42 timestamp=1744165948799
```

---

## Demo 程序

### demo-dns-redirect — DNS 重定向

拦截出站 DNS 请求 (UDP 端口 53)，将目标 DNS 服务器修改为 8.8.8.8 后重注入。

```powershell
# 需要已安装驱动
cargo run -p demo-dns-redirect

# 限制处理数量
cargo run -p demo-dns-redirect -- --count=10
```

核心流程:
1. 编译过滤器 `"udp and outbound"`
2. 打开 `Layer::Network` 会话
3. 接收网络包，识别 DNS 请求 (UDP dst port 53)
4. 修改目标 IP 为 8.8.8.8，重算 IPv4 校验和
5. 通过 `encode_runtime_send_request` 重注入

### demo-conn-monitor — 连接监控

实时监控 Socket 连接和 Flow 建立事件。

```powershell
# 同时监控 Socket 和 Flow
cargo run -p demo-conn-monitor

# 仅监控 Socket
cargo run -p demo-conn-monitor -- --socket-only

# 仅监控 Flow
cargo run -p demo-conn-monitor -- --flow-only
```

输出示例:
```
[   1] [SOCKET] CONNECT pid=1234 @ 1744165948123ms
[   1] [FLOW]   ESTABLISHED flow_id=100 pid=1234 @ 1744165948456ms
```

---

## 驱动编译

### 当前状态

驱动编译链路分三层，由浅入深：

| 层级 | 内容 | 状态 |
|------|------|------|
| 1. Rust staticlib | `wd-kmdf` 编译为 `wd_kmdf.lib` | 可用 |
| 2. Host ABI smoke | C 代码调用 Rust 导出符号 | 可用 |
| 3. KMDF skeleton | MSBuild + WDK 头文件编译 | 需要本机 WDK |

### 1. 编译 Rust staticlib

```powershell
cargo build -p wd-kmdf
```

输出: `target/debug/libwd_kmdf.a` (MinGW) 或 `target/debug/wd_kmdf.lib` (MSVC)

`wd-kmdf` 的 `Cargo.toml` 配置了 `crate-type = ["rlib", "staticlib"]`，会同时产出 Rust 库和 C 静态库。

### 2. Host ABI Smoke

验证 Rust C ABI 能被外部 C 代码编译和链接：

```powershell
powershell -ExecutionPolicy Bypass -File driver/glue/build_host_smoke.ps1
```

流程:
1. `cargo build` 编译 `wd-kmdf` 为 `wd_kmdf.lib`
2. `clang-cl` 编译 `wd_runtime_host_smoke.c`
3. `llvm-lib` 链接为 `wd_glue_host_smoke.exe`
4. 运行冒烟测试

产出:
- `driver/glue/out/wd_kmdf.lib`
- `driver/glue/out/wd_glue_host_smoke.exe`

### 3. KMDF Skeleton

使用 MSBuild + WDK 头文件编译 KMDF C 驱动骨架：

```powershell
# 需要 WDK 和 Visual Studio
powershell -ExecutionPolicy Bypass -File driver/glue/kmdf-skeleton/build_kmdf_skeleton.ps1

# 或使用 MSBuild
powershell -ExecutionPolicy Bypass -File driver/glue/kmdf-skeleton/verify_kmdf_skeleton_build.ps1
```

流程:
1. `clang-cl` 编译 `Driver.c`, `Device.c`, `Queue.c`
2. 链接 WDF/KMDF 库
3. 产出 `wd_kmdf_skeleton.lib`

**前提条件**:
- Windows SDK 10 (自动检测最新版本)
- WDK (提供 KMDF 头文件)
- LLVM/clang-cl
- Visual Studio 2019+ (MSBuild)

### C ABI 桥接接口

Rust 侧导出的 C 函数，供 KMDF C 驱动调用：

```c
// 创建运行时 API 实例
wd_runtime_glue_api_handle* wd_runtime_glue_create(size_t queue_capacity);

// 销毁实例
void wd_runtime_glue_destroy(wd_runtime_glue_api_handle* handle);

// IOCTL 分发 (对应 IOCTL_OPEN / IOCTL_RECV / IOCTL_SEND)
WD_GLUE_IO_RESULT wd_runtime_glue_device_control(
    wd_runtime_glue_api_handle* handle,
    uint32_t ioctl,
    const uint8_t* input_ptr, size_t input_len,
    uint8_t* output_ptr, size_t output_len
);

// 从 KMDF 回调向运行时队列注入网络事件
WD_GLUE_IO_RESULT wd_runtime_glue_queue_network_event(
    wd_runtime_glue_api_handle* handle,
    uint8_t layer_wire,
    uint64_t packet_id,
    const uint8_t* packet_ptr, size_t packet_len
);
```

**Glue IO 状态码**:

| 状态 | 值 | NTSTATUS 映射 |
|------|-----|--------------|
| SUCCESS | 0 | STATUS_SUCCESS |
| UNSUPPORTED_IOCTL | 1 | STATUS_INVALID_DEVICE_REQUEST |
| DECODE_OPEN | 2 | STATUS_INVALID_PARAMETER |
| OUTPUT_TOO_SMALL | 3 | STATUS_BUFFER_TOO_SMALL |
| QUEUE_EMPTY | 4 | STATUS_NO_MORE_ENTRIES |
| RECV_DISABLED | 5 | STATUS_INVALID_DEVICE_STATE |
| SEND_DISABLED | 6 | STATUS_INVALID_DEVICE_STATE |
| INVALID_STATE | 7 | STATUS_INVALID_DEVICE_STATE |
| NETWORK_RUNTIME | 8 | STATUS_UNSUCCESSFUL |
| INVALID_POINTER | 9 | STATUS_ACCESS_VIOLATION |
| INVALID_HANDLE | 10 | STATUS_INVALID_HANDLE |
| INVALID_LAYER | 11 | STATUS_INVALID_PARAMETER |

### no_std 进度

| 组件 | 已移至 no_std | 仍在 std |
|------|:---:|------|
| HandleState 状态机 | `wd-kmdf-core` | - |
| GlueIoStatus / GlueIoResult | `wd-kmdf-core` | - |
| ReinjectionToken / ReinjectionError | `wd-kmdf-core` | - |
| FixedReinjectionTable | `wd-kmdf-core` | - |
| FixedPacket | `wd-kmdf-core` | - |
| ByteRing | `wd-kmdf-core` | - |
| FilterEngine | - | `wd-kmdf` (String 错误) |
| RuntimeDevice | - | `wd-kmdf` (Vec<u8>) |
| IoctlDispatcher | - | `wd-kmdf` (Vec<u8>) |
| RuntimeGlueApi | - | `wd-kmdf` (Box<dyn>) |

---

## 驱动签名与安装

### 当前状态

**驱动签名和安装流程尚未实现。** 以下是当前已有的资产和缺失的部分：

### 已有资产

| 文件 | 说明 |
|------|------|
| `driver/glue/wd-rust-x64.inf` | x64 占位 INF (未签名) |
| `driver/glue/wd-rust-x86.inf` | x86 占位 INF (未签名) |
| `driver/glue/wd_kmdf_bridge.h` | C ABI 桥接头文件 |
| `driver/glue/wd_kmdf_bridge.c` | C ABI 桥接实现 |
| `driver/glue/wd_ntstatus_mapping.h` | GlueIO → NTSTATUS 映射 |
| `driver/glue/kmdf-skeleton/` | KMDF C 驱动骨架项目 |

### 缺失部分 (需要补充)

| 项目 | 说明 |
|------|------|
| `.sys` 驱动文件 | 当前只产出 `.lib`，未链接为 `.sys` |
| 驱动签名 | 需要 EV 代码签名证书或自签名测试证书 |
| INF 签名 | INF 文件需要通过 `inf2cat` + `signtool` 签名 |
| WPP 追踪 | 未实现 |
| 服务安装脚本 | 未实现 |
| 测试签名模式 | 需要启用 `bcdedit /set testsigning on` |

### 测试签名安装步骤 (未来)

在驱动 `.sys` 文件可用后，开发环境安装流程如下：

```powershell
# 1. 创建自签名测试证书
New-SelfSignedCertificate -Type CodeSigningCert -Subject "WD-Rust-Test" -CertStoreLocation Cert:\CurrentUser\My

# 2. 签名驱动
signtool sign /s My /n "WD-Rust-Test" /t http://timestamp.digicert.com wd-rust.sys

# 3. 启用测试签名模式 (需重启)
bcdedit /set testsigning on

# 4. 创建设备服务
sc create WdRust type=kernel binPath="C:\path\to\wd-rust.sys"

# 5. 启动驱动
sc start WdRust

# 6. 验证设备节点
# 设备路径应为 \\.\WdRust
```

### 生产签名 (未来)

生产环境需要:
1. **EV 代码签名证书** — 从 DigiCert/Sectigo 等购买
2. **WHQL 认证** — 提交 Windows Hardware Developer Center 认证
3. **Windows Update 分发** — 通过 WHQL 后自动分发

---

## 验证命令

```powershell
# 单元测试 (无需驱动)
cargo test --workspace --offline

# 各 crate 单独测试
cargo test -p wd-proto
cargo test -p wd-filter
cargo test -p wd-user
cargo test -p wd-cli
cargo test -p wd-kmdf
cargo test -p wd-kmdf-core

# 编译检查
cargo check --workspace

# Windows 主机验证脚本
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1
powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1
powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1

# Driver glue 验证
powershell -ExecutionPolicy Bypass -File driver/glue/verify_staged_assets.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/verify_host_smoke_build.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/verify_kmdf_skeleton_assets.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/kmdf-skeleton/verify_kmdf_skeleton_build.ps1
```

---

## 协议规范

### IOCTL 命令

| 命令 | 值 | 说明 |
|------|-----|------|
| IOCTL_OPEN | `0x80002000` | 打开句柄 |
| IOCTL_RECV | `0x80002004` | 接收事件 |
| IOCTL_SEND | `0x80002008` | 发送/重注入 |

### 设备名称

| 名称 | 值 |
|------|-----|
| 设备名 | `\\Device\\WdRust` |
| DOS 设备名 | `\\DosDevices\\WdRust` |
| 用户态路径 | `\\.\WdRust` |

### 句柄状态机

```
Opening → Running → RecvShutdown → SendShutdown → Closing → Closed
```

### 能力标志

| 标志 | 值 | 说明 |
|------|-----|------|
| CHECKSUM_RECALC | 0x0001 | 校验和重计算 |
| NETWORK_REINJECT | 0x0002 | 网络包重注入 |
| FLOW_EVENTS | 0x0004 | 流事件支持 |
| SOCKET_EVENTS | 0x0008 | Socket 事件支持 |
| REFLECT_EVENTS | 0x0010 | 反射事件支持 |
