# WinDivert DevBench

`WinDivert DevBench` 是一个面向 WinDivert Rust 重写实验的 workspace。
当前仓库的 phase-one 目标不是直接提供可安装的生产驱动，而是把协议、过滤器、用户态 API、KMDF 运行时子集、CLI 子命令和 Windows 主机验证脚本连成一条可测试、可断言、可演进的开发链路。

## 当前状态

`wd-cli` 的 5 个子命令已经不是纯 placeholder，而是带有真实参数、真实仓库逻辑和稳定输出合同的 phase-one 命令：

- `netfilter`
- `netdump`
- `reflectctl`
- `socketdump`
- `flowtrack`

这里的“真实”指：

- 命令至少接受一个有意义的参数，或者执行明确的 deterministic flow
- 命令会调用现有 crate，而不是只打印固定占位文案
- 命令输出稳定 summary line，便于 Rust 测试和 PowerShell 脚本断言
- 行为不依赖已安装驱动或 live kernel device，适合本地开发机和 CI

## 仓库结构

| 路径 | 作用 |
| --- | --- |
| `crates/wd-proto` | phase-one 协议对象，例如 `OpenRequest`、`OpenResponse`、`Layer`、能力位定义。 |
| `crates/wd-driver-shared` | 驱动与用户态共享常量，例如设备名和 IOCTL 常量。 |
| `crates/wd-filter` | 过滤表达式编译器与 WDIR 编解码。 |
| `crates/wd-user` | 用户态 API 骨架，负责 filter 编译、打开参数构造、事件解码与校验和修复。 |
| `crates/wd-cli` | 当前命令行入口。 |
| `driver/wd-kmdf-core` | `no_std` 内核桥接核心类型层，当前承载 `HandleState`、`GlueIoStatus`、`GlueIoResult`、`ReinjectionToken`、固定容量 reinjection/byte-ring 容器。 |
| `driver/wd-kmdf` | KMDF 侧 phase-one 运行时子集，例如 filter runtime、reinjection token、handle state。 |
| `tests/windows` | Windows 主机验证脚本。 |
| `driver/glue` | INF 与打包脚手架。 |

## 环境要求

- Rust 工具链，可执行 `cargo build` / `cargo test` / `cargo check`
- PowerShell，用于运行 `tests/windows/*.ps1` 和 `driver/glue/build.ps1`
- Windows 开发环境

当前阶段不要求：

- 已安装驱动
- 驱动签名
- live packet capture
- live reinjection

## 快速开始

### 1. 构建 CLI

```powershell
cargo build -p wd-cli
```

默认可执行文件位置：

```text
.\target\debug\wd-cli.exe
```

### 2. 查看命令面

```powershell
cargo run -p wd-cli -- --help
```

### 3. 运行 Windows 主机验证脚本

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1
powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1
powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1
```

这些脚本不再只看退出码，而是会断言各命令输出是否满足稳定合同。

## CLI 使用方法（Runtime-First）

以下内容以当前 runtime-first 行为为准；后面的旧 phase-one 说明仅保留作历史对照，不再代表当前命令行为。

所有 `wd-cli` 子命令现在都遵循统一 runtime 合同：

- 默认输出人类可读的 summary line
- `--json` 输出稳定字段，便于自动化采集
- 缺设备或驱动时返回稳定错误，而不是静默退回 fixture
- 观测类命令默认 one-shot；要拿多条结果必须显式加 `--follow`

统一错误文本形态：

```text
NETDUMP ERROR code=3 category=device_unavailable message=WdRust device not found at \\.\WdRust suggestion=verify driver is installed and device link is present
```

统一 JSON 错误形态：

```json
{"command":"netdump","status":"error","code":3,"category":"device_unavailable","message":"WdRust device not found at \\\\.\\WdRust","suggestion":"verify driver is installed and device link is present"}
```

常见退出码：

- `0`: 成功
- `2`: 参数错误或当前模式不接受该参数组合
- `3`: `device_unavailable`
- `4`: `open_failed`
- `5`: `protocol_mismatch`
- `6`: `io_failure`

### `reflectctl`

用途：

- 探测、打开、关闭并查询反射控制面 runtime 状态

主要参数：

- `--action probe|open|close|capabilities|state`
- `--json`
- `--verbose`
- `--timeout-ms <u64>`

默认行为：

- 默认 `--action open`
- 先 `probe`，再进入对应 action
- `close` 只报告 `CloseAttempted`，不会虚构 `Closed`

示例：

```powershell
cargo run -p wd-cli -- reflectctl --action probe
```

成功输出示例：

```text
REFLECTCTL OK device=ready capabilities=unknown protocol=unknown state=Probed
```

### `netdump`

用途：

- 通过真实 runtime session 接收一条 network 事件并解码 IPv4 摘要

主要参数：

- `--filter <expr>`
- `--count <u64>`
- `--follow`
- `--json`
- `--verbose`
- `--timeout-ms <u64>`

默认行为：

- 默认 one-shot
- `--count > 1` 必须配合 `--follow`
- 当前 runtime `netdump` 不接受非空 `--filter`，会明确报 `argument_error`

示例：

```powershell
cargo run -p wd-cli -- netdump
```

成功输出示例：

```text
NETDUMP OK layer=NETWORK ttl=64 checksum=0x9c93 packet_len=20 timestamp=1744165948123
```

### `socketdump`

用途：

- 通过真实 runtime session 接收 socket 事件，并用当前 runtime subset filter 做匹配

主要参数：

- `--filter <expr>`
- `--process-id <u64>`
- `--count <u64>`
- `--follow`
- `--json`
- `--verbose`
- `--timeout-ms <u64>`

默认行为：

- 默认 one-shot
- `--count > 1` 必须配合 `--follow`
- filter 走 `wd_kmdf::FilterEngine` 的 socket runtime subset；不支持超出该子集的谓词

示例：

```powershell
cargo run -p wd-cli -- socketdump --filter "event == CONNECT and processId == 7"
```

成功输出示例：

```text
SOCKETDUMP OK event=CONNECT process_id=7 matched=true timestamp=1744165948456
```

### `flowtrack`

用途：

- 通过真实 runtime session 接收 flow 事件并输出 flow 摘要

主要参数：

- `--process-id <u64>`
- `--count <u64>`
- `--follow`
- `--json`
- `--verbose`
- `--timeout-ms <u64>`

默认行为：

- 默认 one-shot
- `--count > 1` 必须配合 `--follow`

示例：

```powershell
cargo run -p wd-cli -- flowtrack --process-id 42
```

成功输出示例：

```text
FLOWTRACK OK event=ESTABLISHED flow_id=9001 process_id=42 timestamp=1744165948799
```

### `netfilter`

用途：

- 编译 network filter，并以显式 `validate|observe|reinject` 模式运行

主要参数：

- `--filter <expr>`
- `--mode validate|observe|reinject`
- `--count <u64>`
- `--follow`
- `--json`
- `--verbose`
- `--timeout-ms <u64>`

默认行为：

- 默认 `--mode validate`
- `validate` 只验证 filter 并确认 runtime ready，不做隐式 reinjection
- `observe` 走真实 runtime receive/decode，输出 network 摘要
- `reinject` 现在会走真实 runtime send API：先接收一条带 reinjection token 的 network event，再按 runtime send 协议回发
- 如果驱动返回的是旧式 raw IPv4 event、没有携带 reinjection token，`reinject` 会明确返回 `io_failure`

参数约束：

- `validate` / `reinject` 不接受 streaming 语义，必须保持默认 `--count 1` 且不带 `--follow`
- `observe` 下 `--count > 1` 必须配合 `--follow`

示例：

```powershell
cargo run -p wd-cli -- netfilter --filter "tcp and inbound" --mode validate
```

成功输出示例：

```text
NETFILTER OK mode=validate layer=NETWORK filter="tcp and inbound" ir_bytes=24
```

Runtime network filter subset:
- Protocol and direction: `tcp`, `udp`, `ipv4`, `ipv6`, `inbound`, `outbound`
- Address fields: `localAddr`, `remoteAddr`
- Transport fields: `localPort`, `remotePort`
- Layer field: `layer`
- Raw packet access: `packet[...]`, `packet16[...]`, `packet32[...]`

Current address semantics:
- `localAddr` and `remoteAddr` currently support IPv4 exact match and IPv4 CIDR, for example `localAddr == 2.2.2.2` and `remoteAddr == 1.1.1.0/24`
- `Layer::Network` is treated as inbound: `local* = dst`, `remote* = src`
- `Layer::NetworkForward` is treated as outbound: `local* = src`, `remote* = dst`
- IPv6 address fields are not implemented yet; use `ipv6` together with protocol and port predicates for now

Examples:
```powershell
cargo run -p wd-cli -- netfilter --filter "tcp and localPort == 443 and remoteAddr == 1.1.1.0/24" --mode validate
cargo run -p wd-cli -- netfilter --filter "ipv6 and tcp and outbound" --mode observe --count 1
```

### Windows 主机验证脚本

3 个 PowerShell 脚本都支持：

```powershell
-CliPath <path-to-wd-cli.exe>
```

默认值：

```text
.\target\debug\wd-cli.exe
```

这些脚本现在都合并捕获 `stdout/stderr`，因此 success 和 `device_unavailable` 两条路径都能断言。

- `tests/windows/open_close.ps1`
  调 `reflectctl --action open` 和 `reflectctl --action close`，接受 success 或 `device_unavailable`
- `tests/windows/network_reinject.ps1`
  调 `wd-cli.exe netfilter --filter "tcp and inbound" --mode reinject`，接受 success、`device_unavailable` 或稳定的 `io_failure`
- `tests/windows/five_layer_observe.ps1`
  依次验证 `netdump`、`flowtrack --process-id 42`、`socketdump --filter "event == CONNECT and processId == 7"`，接受 success 或 `device_unavailable`

## 历史说明（已过期）

以下内容保留为旧 deterministic phase-one 说明，不再代表当前 runtime-first 行为。

### `netfilter`

用途：编译 network filter，生成 deterministic reinjection token，并打印稳定 summary line。

参数：

- `--filter <EXPR>` 必填
- `--packet-id <u64>` 可选，默认使用内置 fixture 值
- `--show-ir-len` 可选，当前 phase-one 输出固定包含 `ir_bytes`

示例：

```powershell
cargo run -p wd-cli -- netfilter --filter "tcp and inbound"
```

示例输出：

```text
NETFILTER OK layer=NETWORK filter=tcp and inbound ir_bytes=24 token=1
```

当前真实行为：

- 通过 `wd_user::HandleConfig::network` 编译并校验过滤器
- 通过 `wd_kmdf::ReinjectionTable` 生成 token

当前仍是模拟：

- 不连接真实驱动
- 不执行真实 packet reinjection

### `netdump`

用途：解码 deterministic IPv4 frame fixture，并打印关键网络元数据。

参数：无

示例：

```powershell
cargo run -p wd-cli -- netdump
```

示例输出：

```text
NETDUMP OK layer=NETWORK ttl=64 checksum=0x9c93
```

当前真实行为：

- 通过 `wd_user::RecvEvent::decode` 解码固定 IPv4 帧
- 读取 IPv4 TTL 与 header checksum

当前仍是模拟：

- 不从 live network path 读取数据

### `reflectctl`

用途：模拟 handle open/close 生命周期，并打印协商能力位和最终状态。

参数：无

示例：

```powershell
cargo run -p wd-cli -- reflectctl
```

示例输出：

```text
REFLECTCTL OK capabilities=31 state=Closed
```

当前真实行为：

- 通过 `wd_proto::OpenResponse::success` 构造 open response
- 通过 `wd_user::DynamicHandle` 解析能力位
- 通过 `wd_kmdf::HandleState` 推进生命周期状态机直到 `Closed`

当前仍是模拟：

- 不访问真实内核设备

### `socketdump`

用途：编译 socket 层过滤器，评估 deterministic socket connect 事件，并打印匹配结果。

参数：

- `--filter <EXPR>` 必填
- `--process-id <u64>` 可选，默认 `7`

示例：

```powershell
cargo run -p wd-cli -- socketdump --filter "event == CONNECT and processId == 7"
```

示例输出：

```text
SOCKETDUMP OK event=CONNECT process_id=7 matched=true
```

当前真实行为：

- 通过 `wd_kmdf::FilterEngine::compile(Layer::Socket, ...)` 编译过滤器
- 通过 deterministic socket-connect fixture 评估匹配结果

当前仍是模拟：

- 不监听真实 socket event stream

### `flowtrack`

用途：构建 deterministic flow 事件，经过最小 flow runtime 子集校验，并打印 flow 摘要。

参数：

- `--process-id <u64>` 可选，默认 `42`

示例：

```powershell
cargo run -p wd-cli -- flowtrack --process-id 42
```

示例输出：

```text
FLOWTRACK OK event=ESTABLISHED flow_id=65261 process_id=42
```

当前真实行为：

- 通过 `wd_kmdf::DriverEvent::flow_established` 构建 deterministic flow fixture
- 通过 `wd_kmdf::FilterEngine::compile(Layer::Flow, ...)` 校验最小 flow 子集

当前仍是模拟：

- 不连接真实 flow tracking 来源

### Windows 主机验证脚本

3 个 PowerShell 脚本都支持：

```powershell
-CliPath <path-to-wd-cli.exe>
```

默认值是：

```text
.\target\debug\wd-cli.exe
```

### `tests/windows/open_close.ps1`

执行：

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1
```

断言内容：

- 输出包含 `REFLECTCTL OK`
- 输出包含 `state=Closed`

### `tests/windows/network_reinject.ps1`

执行：

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1
```

脚本内部调用：

```powershell
wd-cli.exe netfilter --filter "tcp and inbound"
```

断言内容：

- 输出包含 `NETFILTER OK`
- 输出包含 `layer=NETWORK`

### `tests/windows/five_layer_observe.ps1`

执行：

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1
```

脚本会依次验证：

1. `wd-cli.exe netdump`
2. `wd-cli.exe flowtrack --process-id 42`
3. `wd-cli.exe socketdump --filter "event == CONNECT and processId == 7"`

断言内容包括：

- `NETDUMP OK`
- `FLOWTRACK OK`
- `SOCKETDUMP OK`
- `matched=true`

## Driver Glue 打包脚手架

`driver/glue` 当前已经分成三层：

- KMDF handoff 模板层：给未来真实 WDF 工程接线
- host-compilable smoke 层：在没有 WDK/WDF 头文件的当前仓库里，先验证 Rust C ABI 能被外部 C 代码编译并链接
- compile-only KMDF skeleton 层：在本机 WDK 头文件可用时，验证真实 `.sln/.vcxproj` 骨架和 KMDF C 源文件可以被编译

执行：

```powershell
powershell -ExecutionPolicy Bypass -File driver/glue/build.ps1
```

当前行为：

1. 创建 `driver/glue/out`
2. 拷贝 `wd-rust-x64.inf`
3. 拷贝 `wd-rust-x86.inf`
4. 拷贝 KMDF glue 模板与说明文档
5. 拷贝 host smoke 相关源码与脚本

它不负责：

- 驱动编译
- 驱动签名
- 驱动安装
- 生产安装包生成

### 最小可编译实现

当前仓库里“可编译”的最小落地形态现在有两条：

1. host ABI smoke
   - `driver/wd-kmdf` 导出 `staticlib` 形式的 Rust C ABI
   - `driver/glue/wd_runtime_host_smoke.c` 作为纯 C smoke harness
   - `driver/glue/build_host_smoke.ps1` 负责把两者编译并链接成一个最小宿主可执行文件
2. compile-only KMDF skeleton
   - `driver/glue/kmdf-skeleton/wd_kmdf_skeleton.sln`
   - `driver/glue/kmdf-skeleton/wd_kmdf_skeleton.vcxproj`
   - `driver/glue/kmdf-skeleton/build_kmdf_skeleton.ps1`
   - `driver/glue/kmdf-skeleton/verify_kmdf_skeleton_build.ps1`

执行：

```powershell
powershell -ExecutionPolicy Bypass -File driver/glue/build_host_smoke.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/kmdf-skeleton/verify_kmdf_skeleton_build.ps1
```

这两条链路分别验证：

- host smoke：
  - `wd_kmdf_bridge.h` 可以被外部 C 源文件直接包含
  - `wd_runtime_glue_*` 导出符号可以被 Rust `staticlib` 提供
  - C 调用者可以跨 ABI 调用最小 glue API 并拿到稳定状态码
- KMDF skeleton：
  - `MSBuild` 可以加载 solution / project
  - KMDF C 侧源文件能对本机 WDK 头文件完成 compile-only 构建
  - 当前会产出 `wd_kmdf_skeleton.lib`

这条链路仍然不代表：

- 已能生成最终 `.sys`
- 已能加载到真实设备栈
- 已具备签名、安装、部署链路

当前硬边界：

- 已抽出 `driver/wd-kmdf-core` 作为 `no_std` 类型层，但 `driver/wd-kmdf` 主体仍然依赖 `std`。
- 已从 `std` 侧移出的部分：状态机、glue ABI 状态、reinjection token/error、固定容量 reinjection table、固定容量 runtime byte ring。
- 当前剩余 `std` 阻塞点主要在 runtime frame 对外返回仍使用 `Vec<u8>`、`ioctl_dispatch` 的 `Vec<u8>` 返回面、以及 `filter_eval` 的 `String` 错误消息。

## crate 级接入示例

### `wd-filter`

```rust
use wd_filter::{compile, decode_ir, encode_ir};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ir = compile("tcp and inbound")?;
    let bytes = encode_ir(&ir);
    let roundtrip = decode_ir(&bytes)?;
    assert_eq!(roundtrip, ir);
    Ok(())
}
```

Additional examples currently covered by the runtime subset:
```rust
use wd_filter::compile;

fn examples() -> Result<(), Box<dyn std::error::Error>> {
    compile("udp and outbound")?;
    compile("ipv6 and tcp")?;
    compile("localPort == 443 and remotePort == 12345")?;
    compile("localAddr == 2.2.2.2 and remoteAddr == 1.1.1.0/24")?;
    compile("packet16[10] == 0xaabb and packet32[12] == 0x01010101")?;
    Ok(())
}
```

### `wd-user`

```rust
use wd_user::HandleConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = HandleConfig::network("tcp and inbound")?;
    assert!(cfg.filter_ir().starts_with(b"WDIR"));
    Ok(())
}
```

### `wd-kmdf`

```rust
use wd_kmdf::{DriverEvent, FilterEngine};
use wd_proto::Layer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = FilterEngine::compile(Layer::Socket, "event == CONNECT and processId == 7")?;
    assert!(engine.matches(&DriverEvent::socket_connect(7)));
    Ok(())
}
```

## 常用验证命令

```powershell
cargo test -p wd-cli
cargo test -p wd-kmdf
cargo test -p wd-user --test user_api
cargo test -p wd-filter --test compile
cargo check --workspace
powershell -ExecutionPolicy Bypass -File driver/glue/verify_staged_assets.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/verify_host_smoke_build.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/verify_kmdf_skeleton_assets.ps1
powershell -ExecutionPolicy Bypass -File driver/glue/kmdf-skeleton/verify_kmdf_skeleton_build.ps1
```

## 当前边界

当前 Task 6 已经从“命令注册 + placeholder 脚本”升级为“deterministic real CLI flow”，但仍然不是完整产品化实现：

- 真实的是参数解析、过滤器编译、事件/状态机评估、稳定输出合同
- 仍然模拟的是 live driver access、真实 packet stream、真实 reinjection 和真实安装链路

如果下一步继续推进，比较合理的顺序通常是：

1. 让某个子命令从 deterministic fixture 切到真实设备输入
2. 让对应的 PowerShell 脚本从 summary line 断言升级到端到端行为断言
3. 最后补齐签名、安装、部署和问题排查文档
